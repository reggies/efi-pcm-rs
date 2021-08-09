#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#![deny(unaligned_references)]

#![feature(new_uninit)]

// TBD: for static assert
#![feature(const_panic)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate uefi;
#[macro_use]
extern crate alloc;
#[macro_use]
extern crate memoffset;
#[macro_use]
extern crate bitflags;
extern crate efi_pcm;
extern crate efi_dxe;

use bitflags::bitflags;
use uefi::prelude::*;
use uefi::proto::device_path::DevicePath;
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::driver_binding::DriverBinding;
use uefi::proto::pci::PciIO;
use uefi::table::boot::OpenAttribute;
use uefi::table::boot::BootServices;

use core::fmt::{self, Write};
use core::str;
use core::fmt::*;
use core::mem;
use alloc::boxed::*;

use efi_dxe::*;
use efi_pcm::*;

// TBD: this is supposed to be packed even though all registers are naturally aligned in C.
// Rustc complains that _creating_ unaligned references is UB. What should we do?
/// PCI Device Configuration Space
/// Section 6.1, PCI Local Bus Specification, 2.2
// #[repr(C, packed)]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
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

// TBD: see comments above
// #[repr(C, packed)]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
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
    powerdown_ctrl_stat: u16,                            // 26h-28h
    extended_audio: [u16; 10],
    extended_modem: [u16; 15],
    vendor_reserved: [u16; 3],
    page_registers: [u16; 8],
    vendor_reserved2: [u16; 6],
    vendor_id1: u16,
    vendor_id2: u16,
}

macro_rules! static_assert {
    ($cond:expr) => {
        static_assert!($cond, concat!("assertion failed: ", stringify!($cond)));
    };
    ($cond:expr, $($t:tt)+) => {
        #[forbid(const_err)]
        const _: () = {
            if !$cond {
                core::panic!($($t)+)
            }
        };
    };
}

static_assert!(mem::size_of::<BaseRegisterSet>() == 0x80);

const DESCRIPTOR_IOC_BIT: u16 = 0x8000;
const DESCRIPTOR_LAST_BIT: u16 = 0x4000;

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct Descriptor {
    address: u32,               // physical address to sound data
    length: u16,                // length of sound data -1
    control: u16,               // bit 15: interrupt fired when data from this entry is transferred
                                // bit 14: last entry of buffer, stop playing
                                // other bits: reserved
}

const BUFFER_SIZE: usize = 1 << 15;
const BUFFER_COUNT: usize = 32;

// For ICH7 max number of buffers is 32. The data should be
// aligned on 8-byte boundaries. Each buffer descriptor is 8
// bytes long and the list can contain a maximum of 32
// entries.
// TBD: -- supposed to be packed but pointers
// might become unaligned as produced by Box::new
// #[repr(C, packed)]
#[repr(C, align(8))]
#[derive(Copy, Clone)]
struct Buffers {
    descriptors: [Descriptor; BUFFER_COUNT],
    pointers: [*const i16; BUFFER_COUNT],
    buffers: [[i16; BUFFER_SIZE]; BUFFER_COUNT]
}

// Mixer registers
const MIXER_RESET: u64        = 0x00; // reset
const MIXER_MASTER: u64       = 0x02; // master volume
const MIXER_PCM_OUT: u64      = 0x18; // PCM OUT volume
const PCM_RATE_FRONT: u64     = 0x2C; // PCM front channel DAC sample rate
const PCM_RATE_SURROUND: u64  = 0x2E; // PCM surround channel DAC sample rate
const PCM_RATE_LFE: u64       = 0x30; // PCM LFE channel DAC sample rate

//
// Bus Master registers
//
const DESCRIPTOR_PCM_OUT: u64 = 0x10; // PCM OUT descriptor base address
const CIV_PCM_OUT: u64        = 0x14; // PCM OUT current index value
const LVI_PCM_OUT: u64        = 0x15; // PCM OUT last valid index
const STATUS_PCM_OUT: u64     = 0x16; // PCM OUT status
const SAMPLES_PCM_OUT: u64    = 0x18; // PCM OUT number of transferred samples
const PROCESSED_PCM_OUT: u64  = 0x1A; // PCM OUT processed entry
const CONTROL_PCM_OUT: u64    = 0x1B; // PCM OUT control
const GLOBAL_CONTROL: u64     = 0x2C; // global control
const GLOBAL_STATUS: u64      = 0x30; // global status

#[repr(C, packed)]
struct NativeAudioMixerBaseRegisterBox {
    buffer_dsc_addr: u32,       // physical address of buffer descriptor list
    cur_entry_val: u8,          // number of actual processed buffer descriptor entry
    last_valid_entry: u8,       // number of all descriptor entries
    transfer_sts: u16,          // status of transferring data
    cur_idx_proc_samples: u16,  // number of transferred samples in actual processed entry
    prcsd_entry: u8,            // number of actual processed buffer entry
    buffer_cnt: u8,             // most important register for controlling transfers
}

//
// PCM OUT capability bits
//
const CHANNELS_2: u8 = 0b00;
const CHANNELS_4: u8 = 0b01;
const CHANNELS_6: u8 = 0b10;
const CHANNELS_R: u8 = 0b11;                             // reserved

//
// PCM OUT Buffer control register bits
//
const CONTROL_DMA_BIT: u8   = 0x1;                                     // 0=Pause transferring 1=Transfer sound data
const CONTROL_RESET_BIT: u8 = 0x2;                                     // 0=Remove reset condition 1=Reset this NABM register box, this bit is selfcleared
const CONTROL_LVBCI_BIT: u8 = 0x4;                                     // 0=Disable interrupt 1=Enable interrupt
const CONTROL_BCIS_BIT: u8  = 0x8;                                     // 0=Disable interrupt 1=Enable interrupt
const CONTROL_FIFOE_BIT: u8 = 0x10;                                    // 0=Disable interrupt 1=Enable interrupt

//
// PCM OUT Transfer status register bits
//
const STATUS_DCH_BIT: u16   = 0x1;                                     // dma controller halted
const STATUS_CELV_BIT: u16  = 0x2;                                     // current equals last valid
const STATUS_LVBCI_BIT: u16 = 0x4;                                     // last valid buffer completion interrupt
const STATUS_BCIS_BIT: u16  = 0x8;                                     // buffer completion interrupt
const STATUS_FIFOE_BIT: u16 = 0x10;                                    // fifo error

struct EventGuard (uefi::Event);

impl EventGuard {
    fn wrap(event: uefi::Event) -> EventGuard {
        EventGuard (event)
    }

    fn unwrap(&self) -> uefi::Event {
        self.0
    }
}

impl Drop for EventGuard {
    fn drop(&mut self) {
        boot_services()
            .close_event(self.0)
            .expect_success("no good");
    }
}

const DEVICE_CONTEXT_SIGNATURE: u64 = 0x74_75_6f_6f_69_64_75_61; // "audioout"

struct DeviceContext {
    signature: u64,
    handle: Handle,
    driver_handle: Handle,                               // TBD: -- get rid of this
    mapping: uefi::proto::pci::Mapping,
    for_plebs: SimpleAudioOut,
    timer_event: EventGuard,
    tone_event: EventGuard,
    buffers: *mut Buffers,
}

impl DeviceContext {
    // BootServices reference is only needed to inherit its lifetime
    fn from_protocol<'a>(_bs: &'a uefi::table::boot::BootServices, raw: *const SimpleAudioOut) -> Option<&'a DeviceContext> {
        use memoffset::offset_of;
        let offset_bytes = memoffset::offset_of!(DeviceContext, for_plebs);
        let context: *const DeviceContext = unsafe {
            (raw as *const u8)
                .sub(offset_bytes).cast()
        };
        if (context as *const u8 as usize) % mem::align_of::<DeviceContext>() == 0 {
            let context = unsafe { &*context };
            if context.signature == DEVICE_CONTEXT_SIGNATURE {
                return Some(context);
            }
        }
        None
    }

    // BootServices reference is only needed to inhert its lifetime
    fn from_protocol_mut<'a>(_bs: &'a uefi::table::boot::BootServices, raw: *mut SimpleAudioOut) -> Option<&'a mut DeviceContext> {
        use memoffset::offset_of;
        let offset_bytes = memoffset::offset_of!(DeviceContext, for_plebs);
        let context: *mut DeviceContext = unsafe {
            (raw as *mut u8)
                .sub(offset_bytes).cast()
        };
        if (context as *mut u8 as usize) % mem::align_of::<DeviceContext>() == 0 {
            let context = unsafe { &mut *context };
            if context.signature == DEVICE_CONTEXT_SIGNATURE {
                return Some(context);
            }
        }
        None
    }
}

fn init_buffers(mapping: &uefi::proto::pci::Mapping, buffers: &mut Buffers) {
    let buffers_base = buffers as *mut Buffers as *mut u8;
    for ((descriptor, ptr), buffer) in buffers.descriptors.iter_mut().zip(buffers.pointers.iter_mut()).zip(buffers.buffers.iter()) {
        // TBD: -- I presume that this is UB because:
        //  1) Pointers are not derived from the same type
        //  2) not the same allocated(?) object
        let buffer_offset = unsafe {
            (buffer.as_ptr() as *const u8)
                .offset_from(buffers_base)
        };
        *ptr = buffer.as_ptr();
        // TBD: -- same as above
        let descriptor_address = unsafe {
            (mapping.device_address() as *const u8)
                .offset(buffer_offset)
        };
        descriptor.address = descriptor_address as u32;
        descriptor.length = 0;
        descriptor.control = 0;
    }
}

fn read_register_byte(pci: &PciIO, offset: u64) -> uefi::Result<u8, ()> {
    let value = &mut [0];
    pci.read_io::<u8>(uefi::proto::pci::IoRegister::R1, offset, value)
        .warning_as_error()?;
    Ok(value[0].into())
}

fn read_register_word(pci: &PciIO, offset: u64) -> uefi::Result<u16, ()> {
    let value = &mut [0];
    pci.read_io::<u16>(uefi::proto::pci::IoRegister::R1, offset, value)
        .warning_as_error()?;
    Ok(value[0].into())
}

fn read_register_dword(pci: &PciIO, offset: u64) -> uefi::Result<u32, ()> {
    let value = &mut [0];
    pci.read_io::<u32>(uefi::proto::pci::IoRegister::R1, offset, value)
        .warning_as_error()?;
    Ok(value[0].into())
}

fn write_register_byte(pci: &PciIO, offset: u64, value: u8) -> uefi::Result {
    pci.write_io::<u8>(uefi::proto::pci::IoRegister::R1, offset, &[value])
        .warning_as_error()?;
    Ok(().into())
}

fn write_register_word(pci: &PciIO, offset: u64, value: u16) -> uefi::Result {
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R1, offset, &[value])
        .warning_as_error()?;
    Ok(().into())
}

fn write_register_dword(pci: &PciIO, offset: u64, value: u32) -> uefi::Result {
    pci.write_io::<u32>(uefi::proto::pci::IoRegister::R1, offset, &[value])
        .warning_as_error()?;
    Ok(().into())
}

fn stereo_volume(left: u16, right: u16, mute: bool) -> u16 {
    let mut result = 0;
    if mute {
        result |= 0x8000;
    }
    result | ((left & 0x3f) << 8) | (right & 0x3f)
}

fn write_mixer_master_register(pci: &PciIO, value: u16) -> uefi::Result {
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, MIXER_MASTER, &[value])
        .warning_as_error()?;
    Ok(().into())
}

fn read_mixer_master_register(pci: &PciIO) -> uefi::Result<u16, ()> {
    // Basically uninit.
    let value = &mut [0];
    pci.read_io::<u16>(uefi::proto::pci::IoRegister::R0, MIXER_MASTER, value)
        .warning_as_error()?;
    Ok(value[0].into())
}

fn write_mixer_pcm_out_register(pci: &PciIO, value: u16) -> uefi::Result {
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, MIXER_PCM_OUT, &[value])
        .warning_as_error()?;
    Ok(().into())
}

fn probe_master_volume(pci: &PciIO) -> uefi::Result<u16, ()> {
    let probe_value = stereo_volume(0x20, 0x20, true);
    write_mixer_master_register(pci, probe_value)
        .warning_as_error()?;
    // If AC ‘97 only supports 5 bits of resolution in its
    // mixer and the driver writes a 1xxxxx, then AC ‘97
    // must interpret that as x11111. It will also respond
    // when read with x11111 rather then 1xxxxx (the original value).
    let probe_result = read_mixer_master_register(pci)
        .warning_as_error()?;
    if probe_value == probe_result {
        Ok(0b11111.into())
    } else {
        Ok(0b01111.into())
    }
}

fn set_sampling_rate(pci: &PciIO, sampling_rate: u16) -> uefi::Result {
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, PCM_RATE_FRONT, &[sampling_rate])
        .warning_as_error()?;
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, PCM_RATE_SURROUND, &[sampling_rate])
        .warning_as_error()?;
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, PCM_RATE_LFE, &[sampling_rate])
        .warning_as_error()?;
    Ok(().into())
}

fn set_master_volume(pci: &PciIO, volume: u16) -> uefi::Result {
    write_mixer_master_register(pci, volume)
        .warning_as_error()?;
    write_mixer_pcm_out_register(pci, volume)
        .warning_as_error()?;
    Ok(().into())
}

fn get_channel_count(pci: &PciIO) -> uefi::Result<u8, ()> {
    // Read GLOBAL_CONTROL register DWORD and check current channel count
    let global_control = read_register_dword(pci, 0x2C)
        .warning_as_error()?;
    Ok((((global_control >> 21) & 0b11) as u8).into())
}

// @retval 00=2 channels 01=4 channels 10=6 channels 11=Reserved
fn get_supported_channel_count(pci: &PciIO) -> uefi::Result<u8, ()> {
    // Read GLOBAL_STATUS register DWORD and check current channel count
    let channel_capabilities = read_register_dword(pci, 0x30)
        .warning_as_error()?;
    Ok((((channel_capabilities >> 21) & 0b11) as u8).into())
}

fn set_channel_count(pci: &PciIO, channels: u8) -> uefi::Result {
    // Write GLOBAL_CONTROL
    let mut global_control = read_register_dword(pci, 0x2C)
        .warning_as_error()?;
    global_control &= !((CHANNELS_R as u32) << 21);
    global_control |= (channels as u32) << 21;
    write_register_dword(pci, 0x2C, global_control)
}

fn dump_pcm_out_registers(pci: &PciIO) -> uefi::Result {
    let buffer_dsc_addr = read_register_dword(pci, DESCRIPTOR_PCM_OUT)
        .warning_as_error()?;
    let civ = read_register_byte(pci, CIV_PCM_OUT)
        .warning_as_error()?;
    let lvi = read_register_byte(pci, LVI_PCM_OUT)
        .warning_as_error()?;
    let transfer_sts = read_register_word(pci, STATUS_PCM_OUT)
        .warning_as_error()?;
    let cur_idx_proc_samples = read_register_byte(pci, SAMPLES_PCM_OUT)
        .warning_as_error()?;
    let prcsd_entry = read_register_byte(pci, PROCESSED_PCM_OUT)
        .warning_as_error()?;
    let buffer_cnt = read_register_byte(pci, CONTROL_PCM_OUT)
        .warning_as_error()?;
    info!("addr {:x}, civ {}, lvi {}, sts {:x}, samples {}, prcsd {}, cnt {:x}",
          buffer_dsc_addr, civ, lvi, transfer_sts, cur_idx_proc_samples, prcsd_entry, buffer_cnt);
    Ok(().into())
}

fn dump_global_bar1_registers(pci: &PciIO) -> uefi::Result {
    let global_sts = read_register_dword(pci, GLOBAL_STATUS)
        .map_err(|error| {
            error!("read_register_dword GLOBAL_STS failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    let global_ctl = read_register_dword(pci, GLOBAL_CONTROL)
        .map_err(|error| {
            error!("read_register_dword GLOBAL_CTL failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    info!("GLOBAL_CONTROL = 0x{:x}, GLOBAL_STATUS = 0x{:x}", global_ctl, global_sts);
    Ok(().into())
}

fn wait_end_of_transfer(pci: &PciIO) -> uefi::Result {
    info!("wait end of transfer");
    loop {
        let buffer_cnt = read_register_byte(pci, CONTROL_PCM_OUT)
            .map_err(|error| {
                error!("read_register_byte CONTROL_PCM_OUT failed: {:?}", error.status());
                error
            })
            .warning_as_error()?;
        if (buffer_cnt & CONTROL_RESET_BIT) == 0 {
            return Ok(().into());
        }
        boot_services().stall(20);
    }
}

fn init_audio_codec(driver_handle: Handle, handle: Handle, pci: &PciIO) -> uefi::Result<Box<DeviceContext>> {
    // Nice yet unstable feature
    // let mut buffer = Box::<Buffers>::new_zeroed();

    // Careful now, we might be doing a bad thing creating
    // unaligned pointers inside the struct like this.
    // Also, allocating big structure must be made entirely
    // on the heap.

    let mut buffers = Box::<Buffers>::new_uninit();
    let buffers_ptr = buffers.as_mut_ptr().cast();

    let bt = boot_services();

    let timer_event = unsafe {
        bt
            .create_event(
                uefi::table::boot::EventType::TIMER,
                uefi::table::boot::Tpl::NOTIFY,
                None
            )
            .map(|completion| {
                let (status, result) = completion.split();
                info!("create timer event returned {:?}", status);
                result
            })
            .map_err(|error| {
                error!("failed to create event: {:?}", error.status());
                error
            })?
    };

    //
    // Wrap the handle so that it won't get leaked
    //
    let timer_event = EventGuard::wrap(timer_event);

    let tone_event = unsafe {
        bt
            .create_event(
                uefi::table::boot::EventType::TIMER,
                uefi::table::boot::Tpl::NOTIFY,
                None
            )
            .map(|completion| {
                let (status, result) = completion.split();
                info!("create tone event returned {:?}", status);
                result
            })
            .map_err(|error| {
                error!("failed to create tone event: {:?}", error.status());
                error
            })?
    };

    let tone_event = EventGuard::wrap(tone_event);

    let mapping = pci
        .map(uefi::proto::pci::IoOperation::BusMasterWrite, buffers_ptr, mem::size_of::<Buffers>())
        .map(|completion| {
            let (status, result) = completion.split();
            info!("map operation: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("map operation failed: {:?}", error.status());
            error
        })?;

    let mut buffers = Box::into_raw(unsafe { buffers.assume_init() });

    init_buffers(&mapping, unsafe { &mut *buffers });

    let device = Box::new(DeviceContext {
        signature: DEVICE_CONTEXT_SIGNATURE,
        handle,
        driver_handle,
        mapping,
        timer_event,
        tone_event,
        buffers,
        for_plebs: SimpleAudioOut {
            reset: my_reset,
            feed: my_feed,
            tone: my_tone,
        }
    });

    Ok (device.into())
}

fn init_playback(pci: &PciIO, sampling_rate: u32, device: &mut DeviceContext) -> uefi::Result {

    let max_master_volume = probe_master_volume(pci)
        .warning_as_error()?;

    info!("max master volume: {:#?}", max_master_volume);

    // TBD: what should we do with unsupported values?
    set_sampling_rate(pci, sampling_rate as u16)
        .warning_as_error()?;

    // Commented because even mono (reported by aplay) seems to work
    // set_channel_count(pci, CHANNELS_2)
    //     .expect_success("set channel count failed");

    set_master_volume(pci, stereo_volume(0, 0, false))
        .warning_as_error()?;

    pci.flush()
        .warning_as_error()?;

    Ok (().into())
}

fn play_samples(pci: &PciIO, channel_count: u16, sampling_rate: u32, buffer_count: usize, sample_count: usize, device: &mut DeviceContext) -> uefi::Result {

    // Disable interrupts in PCM OUT transfer control register and
    // set Reset Registers (RR) bit
    //
    // ICH7: Contents of all Bus master related registers to be
    // reset, except the interrupt enable bits (bit 4,3,2 of
    // this register). Software needs to set this bit but
    // need not clear it since the bit is self
    // clearing. This bit must be set only when the
    // Run/Pause bit (D30:F2:2Bh, bit 0) is cleared. Setting
    // it when the Run bit is set will cause undefined
    // consequences
    //
    // 0b00010
    write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_RESET_BIT)
        .warning_as_error()?;

    // Write pointer to buffer descriptor list
    write_register_dword(pci, DESCRIPTOR_PCM_OUT, device.mapping.device_address() as u32)
        .warning_as_error()?;

    // Set Last Valid Index register with number of valid buffers
    write_register_byte(pci, LVI_PCM_OUT, (buffer_count - 1) as u8)
        .warning_as_error()?;

    for lvi in 0..buffer_count as u8 {
        // Calculate the delay between buffers in 100ns intervals
        let buffers = unsafe { &mut *device.buffers };
        let buffer_size = buffers.descriptors[lvi as usize].length;
        let delay = milliseconds_to_timer_period(1000 * buffer_size as u64 / channel_count as u64 / sampling_rate as u64);

        boot_services()
            .set_timer(device.timer_event.unwrap(), uefi::table::boot::TimerTrigger::Relative(delay))
            .warning_as_error()?;

        // Set bit for transferring data in transfer control register (bit 0) - 0x15 (0b10101)
        write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_DMA_BIT | CONTROL_LVBCI_BIT | CONTROL_FIFOE_BIT)
            .warning_as_error()?;

        // Clear status register by writing 0x1c (0b11100)
        write_register_word(pci, STATUS_PCM_OUT, STATUS_LVBCI_BIT | STATUS_BCIS_BIT | STATUS_FIFOE_BIT)
            .warning_as_error()?;

        // Flush WC writes in status register
        pci.flush()
            .warning_as_error()?;

        wait_end_of_transfer(pci)
            .warning_as_error()?;

        let _ = boot_services()
            .wait_for_event (&mut [device.timer_event.unwrap()])
            .discard_errdata()?;
    }

    Ok (().into())
}

fn deinit_audio_codec(pci: &PciIO, mapping: uefi::proto::pci::Mapping) -> uefi::Result {
    pci.unmap(mapping)
        .discard_errdata()                               // discard mapping even if failed to unmap
        .map(|completion| {
            let (status, result) = completion.split();
            info!("unmap operation: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("unmap operation failed: {:?}", error.status());
            error
        })?;

    Ok(().into())
}


fn dump_registers(pci: &PciIO) -> uefi::Result {
    // TBD: why uninit?
    let mut registers = mem::MaybeUninit::<BaseRegisterSet>::uninit();
    let buffer = unsafe {
        core::slice::from_raw_parts_mut(registers.as_mut_ptr().cast(), mem::size_of::<BaseRegisterSet>())
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

    info!("registers: {:#x?}", unsafe { registers.assume_init() });

    Ok(().into())
}


fn copy_samples(buffers: &mut Buffers, offset: usize, feed: &[u16]) -> (usize, usize) {
    let mut buffer_offset = offset;
    let mut buffer_count = 0;
    for (descriptor, buffer) in buffers.descriptors.iter_mut().zip(buffers.buffers.iter_mut()) {
        let mut count : usize = 0;
        for (index, v) in buffer.iter_mut().enumerate() {
            if buffer_offset + index < feed.len() {
                *v = feed[buffer_offset + index].to_be() as i16;
                count += 1;
            }
        }
        descriptor.length = 0;
        descriptor.control = DESCRIPTOR_LAST_BIT;

        info!("schedule {} samples starting at {}", count, buffer_offset);
        if count > 0 {
            buffer_offset += count as usize;
            descriptor.length = count as u16 - 1;
            descriptor.control = 0;
        }
        buffer_count += 1;
    }
    (buffer_count, buffer_offset - offset)
}

fn div_round_up(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

fn isine(phase: usize, period: usize) -> i16 {
    if (phase % period) * 2 < period {
        i16::MIN
    } else {
        i16::MAX
    }
}

fn wave(buffers: &mut Buffers, channels: u16, sampling_rate: u32, freq: u16, samples: usize) -> (usize, usize) {
    let mut buffer_count = 0;
    let mut sample_count = 0;

    const HALFPERIODS_PER_PERIOD: usize = 2;

    if freq == 0 || channels == 0 || sampling_rate < freq as u32 {
        // TBD: other checks
        return (0, 0);
    }

    // Get the number of full periods as a number of frames
    let frames_per_halfperiod   = sampling_rate as usize / freq as usize;
    let samples_per_halfperiod  = channels as usize * frames_per_halfperiod;
    let frames_per_buffer       = BUFFER_SIZE / channels as usize;
    let full_halfperiods        = div_round_up(samples as usize, samples_per_halfperiod);
    let full_frames             = full_halfperiods * frames_per_halfperiod;

    info!("  samples {}", samples);
    info!("  frames_per_halfperiod {}", frames_per_halfperiod);
    info!("  frames_per_buffer {}", frames_per_buffer);
    info!("  full_frames {}", full_frames);

    let samples_per_period = channels as usize * frames_per_halfperiod * HALFPERIODS_PER_PERIOD;

    for (descriptor, buffer) in buffers.descriptors.iter_mut().zip(buffers.buffers.iter_mut()) {
        let mut count = 0;
        let period_remained = if full_frames <= buffer_count * frames_per_buffer {
            0
        } else {
            (full_frames - buffer_count * frames_per_buffer) / frames_per_halfperiod / HALFPERIODS_PER_PERIOD
        };
        for period in buffer.chunks_mut(samples_per_period).take(period_remained) {
            for (index, v) in period.iter_mut().enumerate() {
                let frame_index = index / channels as usize;
                *v = isine(frame_index, frames_per_halfperiod * HALFPERIODS_PER_PERIOD);
            }
            count += period.len();
        }
        sample_count += count;
        descriptor.length = 0;
        descriptor.control = DESCRIPTOR_LAST_BIT;
        if count > 0 {
            info!("packed {} samples", count);
            descriptor.length = (count - 1) as u16;
            descriptor.control = 0;
            buffer_count += 1;
        }
    }
    (buffer_count, sample_count)
}

extern "efiapi" fn my_tone(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> Status {

    info!("my_tone");

    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;

    let pci = boot_services()
        .open_protocol::<PciIO>(device.handle, device.driver_handle, device.handle, OpenAttribute::GET_PROTOCOL)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open PCI I/O: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })?;

    let channel_count = 2;
    // let sampling_rate = 22100;
    let sampling_rate = 44100;
    let mut samples = div_round_up(duration as usize * channel_count as usize * sampling_rate as usize, 1000);
    info!("about to schedule a total of {} samples", samples);

    pci.with_proto(|pci| init_playback(pci, sampling_rate, &mut *device))
        .log_warning()?;

    loop {
        let buffers = unsafe { &mut *device.buffers };
        let (buffer_count, sample_count) = wave(buffers, channel_count, sampling_rate, freq, samples);

        info!("filled {} buffers, {} samples total", buffer_count, sample_count);

        if buffer_count == 0 {
            break;
        }

        if samples >= sample_count {
            samples -= sample_count;
        } else {
            samples = 0;
        }

        info!("scheduled {} buffers {} samples", buffer_count, sample_count);

        pci.with_proto(|pci| play_samples(pci, channel_count, sampling_rate, buffer_count, sample_count, &mut *device))
            .warning_as_error()?;
    }

    info!("scheduling done {}", samples);

    uefi::Status::SUCCESS
}

fn milliseconds_to_timer_period(msec: u64) -> u64 {
    // Number of 100 ns units
    msec * 10000
}

extern "efiapi" fn my_feed(this: &mut SimpleAudioOut, sampling_rate: u32, feed: *const u16, feed_count: usize) -> Status {

    info!("my_feed");

    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;

    let pci = boot_services()
        .open_protocol::<PciIO>(device.handle, device.driver_handle, device.handle, OpenAttribute::GET_PROTOCOL)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open PCI I/O: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })?;

    // TBD:
    // feed must be not null
    // feed must be aligned
    // feed_count must be not bigger than isize::MAX
    // feed must be readable in range [0, feed_count)
    // feed must not be mutated
    // each element of feed must be properly initialized

    // SAFETY: feed align is checked and read access guarantee is on the caller
    let feed = unsafe { core::slice::from_raw_parts(feed, feed_count) };
    let mut samples = feed_count;
    let mut offset = 0;
    info!("about to schedule a total of {} samples", samples);

    pci.with_proto(|pci| init_playback(pci, sampling_rate, &mut *device))
            .warning_as_error()?;

    while samples > 0 {
        let (buffer_count, sample_count) = copy_samples( unsafe { &mut *device.buffers }, offset, feed);

        offset += sample_count;
        samples -= sample_count;

        info!("scheduled {} buffers {} samples", buffer_count, sample_count);

        pci.with_proto(|pci| play_samples(pci, 2, sampling_rate, buffer_count, sample_count, &mut *device))
            .warning_as_error()?;
    }

    info!("scheduling done {}", samples);

    uefi::Status::SUCCESS
}


extern "efiapi" fn my_reset(this: &mut SimpleAudioOut) -> Status {
    info!("my_reset");

    uefi::Status::UNSUPPORTED
}

//
// DriverBinding routines
//

//
// PCI Vendor ID
//
const INTEL: u16 = 0x8086;

extern "efiapi" fn my_supported(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {

    let bt = boot_services();

    let pci = bt
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .log_warning()?;

    info!("my_supported -- got PCI");

    pci.with_proto(|pci| {
        let mut type00 = mem::MaybeUninit::<PciType00>::uninit();
        let buffer = unsafe {
            core::slice::from_raw_parts_mut(type00.as_mut_ptr().cast(), mem::size_of::<PciType00>())
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
        info!("vendor: {:#x}, device: {:#x}", type00.vendor_id, type00.device_id);
        let supported = {
            [
                (INTEL, 0x2415), // Intel 82801AA (ICH) integrated AC'97 Controller
                (INTEL, 0x2425), // Intel 82801AB (ICH0) integrated AC'97 Controller
                (INTEL, 0x2445), // Intel 82801BA (ICH2) integrated AC'97 Controller
                (INTEL, 0x2485), // Intel 82801CA (ICH3) integrated AC'97 Controller
                (INTEL, 0x24c5), // Intel 82801DB (ICH4) integrated AC'97 Controller
                (INTEL, 0x24d5), // Intel 82801EB/ER (ICH5/ICH5R) integrated AC'97 Controller
                (INTEL, 0x25a6), // Intel 6300ESB integrated AC'97 Controller
                (INTEL, 0x266e), // Intel 82801FB (ICH6) integrated AC'97 Controller
                (INTEL, 0x27de), // Intel 82801GB (ICH7) integrated AC'97 Controller
                (INTEL, 0x7195), // Intel 82443MX integrated AC'97 Controller
            ].iter().any(|&(vid, did)| {
                type00.vendor_id == vid && type00.device_id == did
            })
        };
        if !supported {
            return uefi::Status::UNSUPPORTED.into();
        }
        Ok(().into())
    }).log_warning()?;

    pci.close()
        .map(|completion| {
            let (status, result) = completion.split();
            info!("close protocol PCI I/O: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("close protocol PCI I/O failed: {:?}", error.status());
            error
        })?;

    info!("my_supported -- ok");

    uefi::Status::SUCCESS
}


extern "efiapi" fn my_start(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {

    info!("my_start");

    let bt = boot_services();

    // Sync with stop
    let _tpl = unsafe { bt.raise_tpl(uefi::table::boot::Tpl::NOTIFY) };

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

    let device = pci
        .with_proto(|pci| init_audio_codec(this.driver_handle(), handle, pci))
        .log_warning()?;

    let audio_out = &device.for_plebs;

    bt.install_interface::<SimpleAudioOut>(handle, audio_out)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open audio protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to install audio protocol: {:?}", error.status());
            error
        })?;

    pci.with_proto(dump_registers)
        .warning_as_error()?;

    // pci.with_proto(|pci| test_audio_codec(pci, &mut *device))
    //     .log_warning()
    //     .expect("not problem init");

    // consume PCI I/O
    uefi::table::boot::leak(pci);

    // produce audio protocol and let it live in database as
    // long as the driver's image stay resident or until the
    // DisconnectController() will be invoked
    mem::forget(device);

    info!("my_start -- ok");

    uefi::Status::SUCCESS
}

extern "efiapi" fn my_stop(this: &DriverBinding, controller: Handle, _num_child_controller: usize, _child_controller: *mut Handle) -> Status {

    info!("my_stop");

    let bt = boot_services();

    // Sync with start
    let _tpl = unsafe { bt.raise_tpl(uefi::table::boot::Tpl::NOTIFY) };

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

    // This is safe assuming that audio_out was created by us
    // let audio_out = unsafe { Box::from_raw(audio_out.as_proto().get()) };
    let audio_out = audio_out.as_proto().get();

    // Note that this operation does not consume anything
    let device = DeviceContext::from_protocol_mut(boot_services(), audio_out)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;

    let audio_out_ref = unsafe { audio_out.as_ref().unwrap() };

    bt.uninstall_interface::<SimpleAudioOut>(controller, audio_out_ref)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("uninstall audio protocol: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed uninstall audio protocol: {:?}", error.status());
            error
        })?;

    let device = unsafe {
        Box::from_raw(device as *mut DeviceContext)
    };

    pci.with_proto(|pci| deinit_audio_codec(pci, device.mapping))
        .log_warning()
        .map_err(|error| {
            warn!("Failed to deinitialize audio codec: {:?}", error.status());
            error
        })
        .or::<()>(Ok(().into()))
        .unwrap();                                       // ignore error

    info!("my_stop -- ok");

    uefi::Status::SUCCESS                                // drop audio here if everything allright
}

//
// Image entry points
//

extern "efiapi" fn my_unload(image_handle: Handle) -> Status {

    info!("my_unload");

    let bt = boot_services();

    let handles = bt.find_handles::<SimpleAudioOut>()
        .log_warning()
        .map_err(|error| {
            warn!("failed to get simple audio protocol handles: {:?}", error.status());
            error
        }).or_else(|error| {
            if error.status() == uefi::Status::NOT_FOUND {
                Ok(alloc::vec::Vec::new())
            } else {
                Err(error)
            }
        })?;

    for controller in handles {
        bt.disconnect(controller, Some(image_handle), None)
            .warning_as_error()
            .map_err(|error| {
                error!("failed to disconnect audio handle: {:?}", error.status());
                error
            })?;
    }

    let driver_binding = bt
        .open_protocol::<DriverBinding>(image_handle, image_handle, image_handle, OpenAttribute::GET_PROTOCOL)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("open driver binding: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to open driver binding: {:?}", error.status());
            error
        })?;

    let driver_binding_ref = unsafe { driver_binding.as_proto().get().as_ref().unwrap() };

    bt.uninstall_interface::<DriverBinding>(image_handle, driver_binding_ref)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("uninstall driver: {:?}", status);
        })
        .map_err(|error| {
            error!("failed to uninstall driver binding: {:?}", error.status());
            error
        })?;

    // Driver binding is already disconnected and it is
    // about to be destroyed so no need to close it.
    let driver_binding = uefi::table::boot::leak(driver_binding);
    unsafe { Box::from_raw(driver_binding.get()) };

    info!("my_unload -- ok");

    // Cleanup allocator and logging facilities
    efi_dxe::unload(image_handle);

    uefi::Status::SUCCESS
}

#[entry]
fn efi_main(handle: uefi::Handle, system_table: SystemTable<Boot>) -> uefi::Status {

    efi_dxe::init(handle, &system_table)
        .warning_as_error()?;

    info!("efi_main");

    // TBD: allocate in runtime pool or .bss
    let driver_binding = Box::new(DriverBinding::new(
        my_start,
        my_supported,
        my_stop,
        0x0,
        handle,
        handle)
    );

    let bt = boot_services();

    let loaded_image = bt
        .handle_protocol::<LoadedImage>(handle)
        .warning_as_error()?;

    let loaded_image = unsafe { &mut *loaded_image.get() };

    loaded_image.set_unload_routine(Some(my_unload));

    let driver_binding = Box::into_raw(driver_binding);

    let driver_binding_ref = unsafe { driver_binding.as_ref().unwrap() };

    bt.install_interface::<DriverBinding>(handle, driver_binding_ref)
        .map(|completion| {
            let (status, result) = completion.split();
            info!("install driver binding: {:?}", status);
            result
        })
        .map_err(|error| {
            error!("failed to install driver binding: {:?}", error.status());
            unsafe { Box::from_raw(driver_binding) };
            error.status().into()
        })?;

    info!("initialization complete");

    bt.handle_protocol::<DriverBinding>(handle)
        .warning_as_error()?;

    info!("efi_main -- ok");

    uefi::Status::SUCCESS
}
