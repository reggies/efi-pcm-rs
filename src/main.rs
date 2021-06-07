#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![allow(unused_imports)]

// TBD: necessary for derive(Protocol) in our crate
#![feature(negative_impls)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate uefi;
#[macro_use]
extern crate uefi_services;
#[macro_use]
extern crate alloc;

use uefi::prelude::*;
use uefi::proto::device_path::DevicePath;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::driver_binding::DriverBinding;
use uefi::proto::pci::PciIO;
use uefi::table::boot::OpenAttribute;

use core::fmt::{self, Write};
use core::str;
use core::fmt::*;
use core::mem;

use alloc::boxed::*;

mod connect;

// TBD: these must be located on the root crate so that unsafe_guid macro will work
// for our proto module. Would be better to modify unsafe_guid macro to
// use module local definitions for Guid and Identify
use uefi::Guid;
use uefi::Identify;
mod proto;
use proto::*;

//
// 1. DriverBinding [efi_main, forever)
// 2. ComponentName [efi_main, forever)
// 3. SimpleAudioOut [start, stop]
// 4. SimpleAudioOut [start, unload]
//

//
// SimpleAudioOut
//

extern "efiapi" fn my_reset(this: &SimpleAudioOut) -> Status {
    uefi::Status::UNSUPPORTED
}

extern "efiapi" fn my_feed(this: &SimpleAudioOut, sample: *const u8, sample_count: usize, delay_usec: u64) -> Status {
    uefi::Status::UNSUPPORTED
}

//
// DriverBinding

/// PCI Device Configuration Space
/// Section 6.1, PCI Local Bus Specification, 2.2
#[repr(C, packed)]
struct PciType00 {
    vendor_id: u16,
    device_id: u16,
    command: u16,
    status: u16,
    revision_id: u8,
    class_code: [u8; 3],
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
    base_address_registers: [u32; 6],
    cardbus_cis_pointer: u32,
    subsystem_vendor_id: u16,
    subsystem_id: u16,
    expansion_rom_base_address: u32,
    capability_ptr: u8,
    reserved1: [u8; 3],
    reserved2: u32,
    interrupt_line: u8,
    interrupt_pin: u8,
    min_gnt: u8,
    max_lat: u8,
}

// TBD: this is supposed to be packed even though all registers are naturally aligned.
// Rustc complains that _creating_ unaligned references is UB. What should we do?
#[repr(C, packed)]
#[derive(Debug, Default)]
struct BaseRegisterSet {
    reset: u16,
    master_volume: u16,
    aux_out_volume: u16,
    mono_volume: u16,
    master_tone: u16,
    pc_beep_volume: u16,
    phone_volume: u16,
    mic_volume: u16,
    line_in_volume: u16,
    cd_volume: u16,
    video_volume: u16,
    aux_in_volume: u16,
    pcm_out_volume: u16,
    record_select: u16,
    record_gain: u16,
    record_gain_mic: u16,
    general_purpose: u16,
    _3d_control: u16,
    audio_int_and_paging: u16,
    powerdown_ctrl_stat: u16,
    extended_audio: [u16; 9],
    extended_modem: [u16; 14],
    vendor_reserved: [u8; 5],
    page_registers: [u16; 8],
    vendor_reserved2: [u8; 10],
    vendor_id1: u8,
    vendor_id2: u8,
}

fn dump_registers(pci: &PciIO) -> uefi::Result {
    let mut registers = mem::MaybeUninit::<BaseRegisterSet>::uninit();
    let buffer = unsafe {
        core::slice::from_raw_parts_mut(mem::transmute(registers.as_mut_ptr()), mem::size_of::<BaseRegisterSet>())
    };

    pci.read_io::<u16>(uefi::proto::pci::IoRegister::R0, 0, buffer)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("read registers: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("reading registers failed: {:?}", error.status());
            error
        })?;

    let registers = unsafe { registers.assume_init() };
    info!("registers: {:#?}", registers);

    Ok(().into())
}

extern "efiapi" fn my_supported(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let pci = bt
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .log_warning()?;

    info!("my_supported -- got PCI");

    pci.with_proto(|pci| {
        let mut type00 = mem::MaybeUninit::<PciType00>::uninit();
        let buffer = unsafe {
            core::slice::from_raw_parts_mut(mem::transmute(type00.as_mut_ptr()), mem::size_of::<PciType00>())
        };
        pci.read_config::<u8>(0, buffer)
            .map(|completion| {
                let (status, result) = completion.split();
                info!("read_config type00: {:?}", status);
                result
            })
            .map_err(|error| {
                error!("read_config type00: {:?}", error.status());
                error
            })?;
        let type00 = unsafe { type00.assume_init() };
        info!("vendor: {:x}, device: {:x}", type00.vendor_id, type00.device_id);
        if type00.vendor_id != 0x8086 || type00.device_id != 0x2415 {
            return uefi::Status::UNSUPPORTED.into();
        }
        Ok(().into())
    }).log_warning()?;

    info!("my_supported -- ok");

    uefi::Status::SUCCESS
}

extern "efiapi" fn my_start(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    info!("my_start");

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let _tpl = unsafe { bt.raise_tpl(uefi::table::boot::Tpl::CALLBACK) };

    let pci = bt
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open PCI I/O protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })?;

    let audio_out = Box::new(SimpleAudioOut {
        reset: my_reset,
        feed: my_feed
    });

    bt.install_interface::<SimpleAudioOut>(handle, audio_out)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open audio protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to install audio protocol: {:?}", error.status());
            error.status().into()                        // drop audio protocol
        })?;

    pci.with_proto(dump_registers)
        .log_warning()
        .expect("no problem");

    // consume PCI I/O
    uefi::table::boot::leak(pci);

    info!("my_start -- ok");

    uefi::Status::SUCCESS
}

extern "efiapi" fn my_stop(this: &DriverBinding, controller: Handle, num_child_controller: usize, child_controller: *mut Handle) -> Status {
    info!("my_stop");

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let _tpl = unsafe { bt.raise_tpl(uefi::table::boot::Tpl::CALLBACK) };

    let audio_out = bt
        .open_protocol::<SimpleAudioOut>(controller, this.driver_handle(), controller, OpenAttribute::GET_PROTOCOL)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open audio protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to open audio protocol: {:?}", error.status());
            error
        })?;

    let pci = bt
        .open_protocol::<PciIO>(controller, this.driver_handle(), controller, OpenAttribute::GET_PROTOCOL)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open PCI I/O: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })?;

    let audio_out = unsafe { Box::from_raw(audio_out.as_proto().get()) };

    bt.uninstall_interface::<SimpleAudioOut>(controller, audio_out)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("uninstall audio protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed uninstall audio protocol: {:?}", error.status());
            error.status().into()                        // drop audio protocol
        })?;

    info!("my_stop -- ok");

    uefi::Status::SUCCESS
}

//
// Entry point
//

extern "efiapi" fn my_unload(image_handle: Handle) -> Status {
    info!("my_unload");
    uefi::Status::UNSUPPORTED
}

#[entry]
fn efi_main(handle: uefi::Handle, system_table: SystemTable<Boot>) -> uefi::Status {

    uefi_services::init(&system_table)
        .expect_success("Failed to initialized utilities");

    info!("Entry point");

    let driver_binding = Box::new(DriverBinding::new(
        my_start,
        my_supported,
        my_stop,
        0xa,
        handle,
        handle)
    );

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let loaded_image = bt
        .handle_protocol::<LoadedImage>(handle)
        .expect_success("failed to get loaded image protocol");

    let loaded_image = unsafe { &mut *loaded_image.get() };

    loaded_image.set_unload_routine(Some(my_unload));

    bt.install_interface::<DriverBinding>(handle, driver_binding)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("install driver binding: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to install driver binding: {:?}", error.status());
            error.status().into()
        })?;

    info!("initialization complete");

    bt.handle_protocol::<DriverBinding>(handle)
        .expect_success("DriverBinding not found on my handle");

    info!("test complete");

    // connect::connect_pci_recursively();

    // connect::enum_simple_audio_out();

    uefi::Status::SUCCESS
}
