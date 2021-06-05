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
//

extern "efiapi" fn my_supported(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    info!("my_supported");

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let pci = bt
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .log_warning()?;

    let device_path = bt
        .open_protocol::<DevicePath>(handle, this.driver_handle(), handle, OpenAttribute::GET_PROTOCOL)
        .log_warning()?;

    info!("my_supported -- ok");

    uefi::Status::SUCCESS
}

extern "efiapi" fn my_start(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    info!("my_start");

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let tpl = unsafe { bt.raise_tpl(uefi::table::boot::Tpl::CALLBACK) };

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

    let device_path = bt
        .open_protocol::<DevicePath>(handle, this.driver_handle(), handle, OpenAttribute::GET_PROTOCOL)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open DevicePath protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to get DevicePath protocol: {:?}", error.status());
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
            error.status().into()
        })?;

    let audio_out1 = Box::new(SimpleAudioOut {
        reset: my_reset,
        feed: my_feed
    });

    bt.install_interface::<SimpleAudioOut>(handle, audio_out1)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("install audio (2) protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to install (2) audio protocol: {:?}", error.status());
            error.status().into()
        })?;

    // consume PCI I/O
    uefi::table::boot::leak(pci);

    info!("my_start -- ok");

    uefi::Status::SUCCESS
}

extern "efiapi" fn my_stop(this: &DriverBinding, controller: Handle, num_child_controller: usize, child_controller: *mut Handle) -> Status {
    info!("my_stop");

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let tpl = unsafe { bt.raise_tpl(uefi::table::boot::Tpl::CALLBACK) };

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

    let audio_out = &audio_out.as_proto().get();

    bt.uninstall_interface::<SimpleAudioOut>(controller, unsafe { &**audio_out })
        .map(|completion| {
            let (status, result) = completion.split();
            info!("uninstall audio protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed uninstall audio protocol: {:?}", error.status());
            error
        })?;

    let audio_out = unsafe { Box::from_raw(*audio_out) };

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
