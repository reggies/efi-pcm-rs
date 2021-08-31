#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// We are accessing packed structures. Make sure that we
// don't produce undefined behavior
#![deny(unaligned_references)]

// TBD: needed to allocate BDL on the heap without creating
//      it first on the stack and then allocate a heap
//      structure. Can we do better?
#![feature(new_uninit)]

// TBD: move into another crate (for static assert)
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

use core::str;
use core::fmt::*;
use core::mem;
use alloc::boxed::*;

use efi_dxe::*;
use efi_pcm::*;

// TBD: move into another crate
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

/// PCI Device Configuration Space
/// Section 6.1, PCI Local Bus Specification, 2.2
///
/// Note that this struct is supposed to be packed but rustc
/// complains about unaligned field access and unaligned
/// references, but with repr(C) this structure is naturally
/// aligned so that we will only ensure the proper size
/// with static_assert().
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
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

static_assert!(mem::size_of::<PciType00>() == 64);

/// AC'97 baseline audio register set.
/// Section 5.7, AC'97 specification, r2.3, April 2002.
///
/// Note that this struct is supposed to be packed but rustc
/// complains about unaligned field access and unaligned
/// references, but with repr(C) this structure is naturally
/// aligned so that we will only ensure the proper size
/// with static_assert().
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
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

static_assert!(mem::size_of::<BaseRegisterSet>() == 0x80);

//
// PCI Vendor ID
//
const VID_INTEL: u16 = 0x8086;

//
// Reset bit selfclear timeout in microseconds
//
const RESET_TIMEOUT: u64 = 1000;


//
// Buffer Descriptor control flags
//
const BDBAR_IOC_BIT: u16 = 0x8000;  // interrupt fired when data from this entry is transferred
const BDBAR_LAST_BIT: u16 = 0x4000; // last entry of buffer, stop playing

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct Descriptor {
    address: u32,               // physical address to sound data
    length: u16,                // sample count
    control: u16
}

const BUFFER_SIZE: usize = 1 << 16;
const BUFFER_COUNT: usize = 32;

// For ICH5 max number of buffers is 32. The data should be
// aligned on 8-byte boundaries. Each buffer descriptor is 8
// bytes long and the list can contain a maximum of 32
// entries. Empty buffers are allowed but not first and not
// last. Number of samples must be multiple of channel
// count. Left channel is always assumed to be first and
// right channel is assumed to be the last.
#[repr(C, align(8))]
#[derive(Copy, Clone)]
struct BufferDescriptorListWithBuffers {
    descriptors: [Descriptor; BUFFER_COUNT],
    buffers: [[i16; BUFFER_SIZE]; BUFFER_COUNT]
}

//
// Mixer registers
//
const MIXER_RESET: u64        = 0x00; // reset
const MIXER_MASTER: u64       = 0x02; // master volume
const MIXER_PCM_OUT: u64      = 0x18; // PCM OUT volume
const PCM_RATE_FRONT: u64     = 0x2C; // PCM front channel DAC sample rate
const PCM_RATE_SURROUND: u64  = 0x2E; // PCM surround channel DAC sample rate
const PCM_RATE_LFE: u64       = 0x30; // PCM LFE channel DAC sample rate

//
// Bus Master PCM OUT NAMB
//
const BDBAR_PCM_OUT: u64      = 0x10; // Buffer Descriptor List Base Address
const CIV_PCM_OUT: u64        = 0x14; // Current index value, number of actual processed buffers
const LVI_PCM_OUT: u64        = 0x15; // Last valid index
const STATUS_PCM_OUT: u64     = 0x16; // Transfer status
const PICB_PCM_OUT: u64       = 0x18; // Position in current buffer
const PIV_PCM_OUT: u64        = 0x1A; // Prefetched index value
const CONTROL_PCM_OUT: u64    = 0x1B; // Transfer Control
const GLOBAL_CONTROL: u64     = 0x2C; // Global Control
const GLOBAL_STATUS: u64      = 0x30; // Global Status
const CAS: u64                = 0x34; // Codec Access Semaphore register (ICH7)

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
const CONTROL_DMA_BIT: u8   = 0x1;        // 0 to pause transferring, 1 to start transferring
const CONTROL_RESET_BIT: u8 = 0x2;        // 1 reset this NABM, cleared by DMA engine
const CONTROL_LVBCI_BIT: u8 = 0x4;        // 0 to disable this interrupt, 1 to enable
const CONTROL_BCIS_BIT: u8  = 0x8;        // 0 to disable this interrupt, 1 to enable
const CONTROL_FIFOE_BIT: u8 = 0x10;       // 0 to disable this interrupt, 1 to enable

//
// PCM OUT Transfer status register bits
//
const STATUS_DCH_BIT: u16   = 0x1;        // dma controller halted
const STATUS_CELV_BIT: u16  = 0x2;        // current equals last valid
const STATUS_LVBCI_BIT: u16 = 0x4;        // last valid buffer completion interrupt
const STATUS_BCIS_BIT: u16  = 0x8;        // buffer completion interrupt
const STATUS_FIFOE_BIT: u16 = 0x10;       // fifo error

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


struct PciMappingGuard<'a> {
    pci: &'a PciIO,
    mapping: Option<uefi::proto::pci::Mapping>
}

impl<'a> PciMappingGuard<'a> {
    fn wrap(pci: &'a PciIO, mapping: uefi::proto::pci::Mapping) -> PciMappingGuard<'a> {
        PciMappingGuard {
            pci,
            mapping: Some(mapping)
        }
    }

    fn unwrap(&self) -> &uefi::proto::pci::Mapping {
        self.mapping.as_ref().unwrap()
    }
}

impl<'a> Drop for PciMappingGuard<'a> {
    fn drop(&mut self) {
        let mapping = self.mapping.take().unwrap();
        self.pci
            .unmap(mapping)
            .discard_errdata()                           // discard mapping even if failed to unmap
            .map_err(|error| {
                error!("unmap operation failed: {:?}", error.status());
                error
            })
            .warning_as_error()
            .or::<()>(Ok(().into()))                     // never fail
            .unwrap();
    }
}

const DEVICE_CONTEXT_SIGNATURE: u64 = 0x74_75_6f_6f_69_64_75_61; // "audioout"

struct DeviceContext {
    signature: u64,
    handle: Handle,
    driver_handle: Handle,                               // TBD: -- get rid of this
    audio_interface: SimpleAudioOut,
    picb_event: EventGuard,
    playback_event: EventGuard,
    bdl: Box<BufferDescriptorListWithBuffers>,
}

impl DeviceContext {
    // BootServices reference is only needed to inherit its lifetime
    fn from_protocol<'a>(_bs: &'a uefi::table::boot::BootServices, raw: *const SimpleAudioOut) -> Option<&'a DeviceContext> {
        use memoffset::offset_of;
        let offset_bytes = memoffset::offset_of!(DeviceContext, audio_interface);
        // TBD: reinterpret cast to DeviceContext creates
        //      unaligned pointers. Should I peek an 8 byte
        //      string and compare it with the signature?
        // SAFETY: TBD
        let context: *const DeviceContext = unsafe {
            (raw as *const u8)
                .sub(offset_bytes).cast()
        };
        if (context as *const u8 as usize) % mem::align_of::<DeviceContext>() == 0 {
            // SAFETY: TBD
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
        let offset_bytes = memoffset::offset_of!(DeviceContext, audio_interface);
        // TBD: reinterpret cast to DeviceContext creates
        //      unaligned pointers. Should I peek an 8 byte
        //      string and compare it with the signature?
        // SAFETY: TBD
        let context: *mut DeviceContext = unsafe {
            (raw as *mut u8)
                .sub(offset_bytes).cast()
        };
        if (context as *mut u8 as usize) % mem::align_of::<DeviceContext>() == 0 {
            // SAFETY: TBD
            let context = unsafe { &mut *context };
            if context.signature == DEVICE_CONTEXT_SIGNATURE {
                return Some(context);
            }
        }
        None
    }
}

fn init_bdl(mapping: &uefi::proto::pci::Mapping, bdl: &mut BufferDescriptorListWithBuffers) {
    let bdl_base = bdl as *mut BufferDescriptorListWithBuffers as *mut u8;
    for (descriptor, buffer) in bdl.descriptors.iter_mut().zip(bdl.buffers.iter()) {
        // TBD: -- I presume that this is UB because pointers are not derived from the same type
        // SAFETY: see dma-buffer miri test #1
        let buffer_offset = unsafe {
            (buffer.as_ptr() as *const u8)
                .offset_from(bdl_base)
        };
        // TBD: UB if mapping address or bdl_base is not a valid pointer
        // SAFETY: TBD
        let descriptor_address = unsafe {
            (mapping.device_address() as *const u8)
                .offset(buffer_offset)
        };
        // TBD: descriptor is packed and we are accessing
        //      unaligned fields via reference -- this is UB
        descriptor.address = descriptor_address as u32;
        descriptor.length = 0;
        descriptor.control = 0;
    }
}

fn read_register_byte(pci: &PciIO, offset: u64) -> uefi::Result<u8, ()> {
    let value = &mut [0];
    pci.read_io::<u8>(uefi::proto::pci::IoRegister::R1, offset, value)?;
    Ok(value[0].into())
}

fn read_register_word(pci: &PciIO, offset: u64) -> uefi::Result<u16, ()> {
    let value = &mut [0];
    pci.read_io::<u16>(uefi::proto::pci::IoRegister::R1, offset, value)?;
    Ok(value[0].into())
}

fn read_register_dword(pci: &PciIO, offset: u64) -> uefi::Result<u32, ()> {
    let value = &mut [0];
    pci.read_io::<u32>(uefi::proto::pci::IoRegister::R1, offset, value)?;
    Ok(value[0].into())
}

fn write_register_byte(pci: &PciIO, offset: u64, value: u8) -> uefi::Result {
    pci.write_io::<u8>(uefi::proto::pci::IoRegister::R1, offset, &[value])?;
    Ok(().into())
}

fn write_register_word(pci: &PciIO, offset: u64, value: u16) -> uefi::Result {
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R1, offset, &[value])?;
    Ok(().into())
}

fn write_register_dword(pci: &PciIO, offset: u64, value: u32) -> uefi::Result {
    pci.write_io::<u32>(uefi::proto::pci::IoRegister::R1, offset, &[value])?;
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
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, MIXER_MASTER, &[value])?;
    Ok(().into())
}

fn read_mixer_master_register(pci: &PciIO) -> uefi::Result<u16, ()> {
    let value = &mut [0];
    pci.read_io::<u16>(uefi::proto::pci::IoRegister::R0, MIXER_MASTER, value)?;
    Ok(value[0].into())
}

fn write_mixer_pcm_out_register(pci: &PciIO, value: u16) -> uefi::Result {
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, MIXER_PCM_OUT, &[value])?;
    Ok(().into())
}

fn probe_master_volume(pci: &PciIO) -> uefi::Result<u16, ()> {
    let probe_value = stereo_volume(0x20, 0x20, true);
    write_mixer_master_register(pci, probe_value)?;
    // If AC ‘97 only supports 5 bits of resolution in its
    // mixer and the driver writes a 1xxxxx, then AC ‘97
    // must interpret that as x11111. It will also respond
    // when read with x11111 rather then 1xxxxx (the original value).
    let probe_result = read_mixer_master_register(pci)
        .warning_as_error()?;
    if probe_value == probe_result {
        Ok(0b11111.into())
    } else {
        Ok(0b1111.into())
    }
}

fn set_sampling_rate(pci: &PciIO, sampling_rate: u32) -> uefi::Result {
    if sampling_rate != AUDIO_RATE_8000 &&
        sampling_rate != AUDIO_RATE_11025 &&
        sampling_rate != AUDIO_RATE_16000 &&
        sampling_rate != AUDIO_RATE_22050 &&
        sampling_rate != AUDIO_RATE_32000 &&
        sampling_rate != AUDIO_RATE_44100 &&
        sampling_rate != AUDIO_RATE_48000
    {
        return uefi::Status::INVALID_PARAMETER.into()
    }
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, PCM_RATE_FRONT, &[sampling_rate as u16])?;
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, PCM_RATE_SURROUND, &[sampling_rate as u16])?;
    pci.write_io::<u16>(uefi::proto::pci::IoRegister::R0, PCM_RATE_LFE, &[sampling_rate as u16])?;
    Ok(().into())
}

fn set_master_volume(pci: &PciIO, volume: u16) -> uefi::Result {
    write_mixer_master_register(pci, volume)?;
    write_mixer_pcm_out_register(pci, volume)?;
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
    let bdbar = read_register_dword(pci, BDBAR_PCM_OUT)
        .warning_as_error()?;
    let civ = read_register_byte(pci, CIV_PCM_OUT)
        .warning_as_error()?;
    let lvi = read_register_byte(pci, LVI_PCM_OUT)
        .warning_as_error()?;
    let status = read_register_word(pci, STATUS_PCM_OUT)
        .warning_as_error()?;
    let picb = read_register_word(pci, PICB_PCM_OUT)
        .warning_as_error()?;
    let piv = read_register_byte(pci, PIV_PCM_OUT)
        .warning_as_error()?;
    let control = read_register_byte(pci, CONTROL_PCM_OUT)
        .warning_as_error()?;
    info!("bdbar 0x{:x}, civ {}, lvi {}, sts 0x{:x}, picb {}, piv {}, control 0x{:x}",
          bdbar, civ, lvi, status, picb, piv, control);
    Ok(().into())
}

fn dump_global_bar1_registers(pci: &PciIO) -> uefi::Result {
    let global_sts = read_register_dword(pci, GLOBAL_STATUS)
        .map_err(|error| {
            error!("read_register_dword GLOBAL_STATUS failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    let global_ctl = read_register_dword(pci, GLOBAL_CONTROL)
        .map_err(|error| {
            error!("read_register_dword GLOBAL_CONTROL failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    info!("GLOBAL_CONTROL = 0x{:x}, GLOBAL_STATUS = 0x{:x}", global_ctl, global_sts);
    Ok(().into())
}

fn wait_word(pci: &PciIO, offset: u64, mask: u16, value: u16) -> uefi::Result {
    info!("wait word");
    loop {
        dump_pcm_out_registers(pci);
        let register = read_register_word(pci, offset)
            .map_err(|error| {
                error!("read_register_word {:x} failed: {:?}", offset, error.status());
                error
            })
            .warning_as_error()?;
        if (register & mask) == value {
            info!("wait word -- success");
            return Ok(().into());
        }
        boot_services().stall(20);
    }
}

fn wait_byte(pci: &PciIO, timeout: u64, offset: u64, mask: u8, value: u8) -> uefi::Result {
    info!("wait byte");
    let mut time = 0;
    loop {
        let register = read_register_byte(pci, offset)
            .map_err(|error| {
                error!("read_register_byte {:x} failed: {:?}", offset, error.status());
                error
            })
            .warning_as_error()?;
        if (register & mask) == value {
            info!("wait byte -- success");
            return Ok(().into());
        }
        boot_services().stall(20);
        time += 20;
        if time >= timeout {
            return Err (uefi::Status::TIMEOUT.into());
        }
    }
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

fn init_device_context(driver_handle: Handle, handle: Handle, pci: &PciIO) -> uefi::Result<Box<DeviceContext>> {
    //
    // Careful now, we might be doing a bad thing creating
    // unaligned pointers inside the struct like this.
    // Also, allocating big structure must be made entirely
    // on the heap.
    let mut bdl = Box::<BufferDescriptorListWithBuffers>::new_uninit();
    let picb_event = boot_services()
        .create_timer_event()
        .warning_as_error()?;
    //
    // Wrap the handle so that it won't get leaked
    //
    let picb_event = EventGuard::wrap(picb_event);
    let playback_event = boot_services()
        .create_timer_event()
        .warning_as_error()?;
    let playback_event = EventGuard::wrap(playback_event);
    // TBD: isn't it possible for this pointer to BDL to change
    //      after the further down Box::into_raw invocation?
    // SAFETY: see dma-buffer miri test #1
    let mut bdl = unsafe { bdl.assume_init() };
    let device = Box::new(DeviceContext {
        signature: DEVICE_CONTEXT_SIGNATURE,
        handle,
        driver_handle,
        picb_event,
        playback_event,
        bdl,
        audio_interface: SimpleAudioOut {
            reset: pcm_reset,
            write: pcm_write,
            tone: pcm_tone,
            query_mode: pcm_query_mode,
            max_mode: 1,
            capabilities: AUDIO_CAP_RESET | AUDIO_CAP_WRITE | AUDIO_CAP_TONE | AUDIO_CAP_MODE
        }
    });
    Ok (device.into())
}

fn init_playback(pci: &PciIO, sampling_rate: u32, device: &mut DeviceContext) -> uefi::Result {
    let max_master_volume = probe_master_volume(pci)
        .warning_as_error()?;
    info!("max master volume: {:#?}", max_master_volume);
    set_sampling_rate(pci, sampling_rate)?;
    set_channel_count(pci, CHANNELS_2)?;
    set_master_volume(pci, stereo_volume(0, 0, false))?;
    pci.flush()?;
    Ok (().into())
}

fn stop_playback(pci: &PciIO) -> uefi::Result {
    write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_RESET_BIT)?;
    wait_byte(pci, RESET_TIMEOUT, CONTROL_PCM_OUT, CONTROL_RESET_BIT, 0)?;
    Ok (().into())
}

fn loop_samples(pci: &PciIO, samples: &[i16], channel_count: u8, sampling_rate: u32, duration: u64, device: &mut DeviceContext) -> uefi::Result {
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
    // 1. Reset control register byte
    // 2. Set descriptor buffer address
    // 3. set LVI to max number of valid buffers
    // 4. for each valid buffer
    // 4.1. set 0x1 in control register byte
    // 4.2. poll reset bit =0 in control register byte
    // 4.3. wait for playback to finish
    //
    let mapping = pci
        .map(
            uefi::proto::pci::IoOperation::BusMasterWrite,
            &mut *device.bdl as *mut BufferDescriptorListWithBuffers as *mut _,
            mem::size_of::<BufferDescriptorListWithBuffers>())
        .map_err(|error| {
            error!("map operation failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // Drop will unmap the memory buffer for us
    let mapping = PciMappingGuard::wrap(pci, mapping);
    // TBD: creating reference to uninitialized object to
    //      initialize it is UB, but see miri dma-buffer test #3
    init_bdl(&mapping.unwrap(), &mut device.bdl);
    let bdl = &mut device.bdl;
    copy_samples_to_buffer(bdl, 0, 0, samples);
    copy_samples_to_buffer(bdl, 1, 0, samples);
    // Reset PCM OUT register box and wait for the chip to clear the bit
    write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_RESET_BIT)?;
    // TBD: add timeout and check return status
    wait_byte(pci, RESET_TIMEOUT, CONTROL_PCM_OUT, CONTROL_RESET_BIT, 0)?;
    // Write descriptor list address and its size to respective registers
    write_register_dword(pci, BDBAR_PCM_OUT, mapping.unwrap().device_address() as u32)?;
    write_register_byte(pci, LVI_PCM_OUT, 1)?;
    // We use a timer event to stop the playback
    let playback_time = milliseconds_to_timer_period(duration);
    boot_services()
        .set_timer(
            device.playback_event.unwrap(),
            uefi::table::boot::TimerTrigger::Relative(playback_time))?;
    let delay = milliseconds_to_timer_period(1000 * samples.len() as u64 / channel_count as u64 / sampling_rate as u64);
    boot_services()
        .set_timer(
            device.picb_event.unwrap(),
            uefi::table::boot::TimerTrigger::Periodic(delay))?;
    loop {
        dump_pcm_out_registers(pci);
        let picb = read_register_word(pci, PICB_PCM_OUT).warning_as_error()?;
        let civ = read_register_byte(pci, CIV_PCM_OUT).warning_as_error()?;
        if picb < channel_count as u16 {
            if civ >= 1 {
                write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_RESET_BIT)?;
                wait_byte(pci, RESET_TIMEOUT, CONTROL_PCM_OUT, CONTROL_RESET_BIT, 0)?;
                write_register_dword(pci, BDBAR_PCM_OUT, mapping.unwrap().device_address() as u32)?;
                write_register_byte(pci, LVI_PCM_OUT, 1)?;
            }
            write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_DMA_BIT);
        }
        // Playback event must be placed first so that it
        // would be checked first
        let index = boot_services()
            .wait_for_event (&mut [device.playback_event.unwrap(), device.picb_event.unwrap()])
            .discard_errdata()?;
        if index.unwrap() == 0 {
            break;
        }
    }
    info!("loop_samples -- done");
    Ok (().into())
}

fn play_samples(pci: &PciIO, samples: &[i16], channel_count: u8, sampling_rate: u32, device: &mut DeviceContext) -> uefi::Result {
    let mut total_offset = 0;
    let mut total_sample_count = samples.len();
    let mapping = pci
        .map(
            uefi::proto::pci::IoOperation::BusMasterWrite,
            &mut *device.bdl as *mut BufferDescriptorListWithBuffers as *mut _,
            mem::size_of::<BufferDescriptorListWithBuffers>())
        .map_err(|error| {
            error!("map operation failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // Drop will unmap the memory buffer for us
    let mapping = PciMappingGuard::wrap(pci, mapping);
    // TBD: creating reference to uninitialized object to
    //      initialize it is UB, but see miri dma-buffer test #3
    init_bdl(&mapping.unwrap(), &mut device.bdl);
    // Reset CIV and PICB. If necessary, prefill must
    // happend before BDL is set and reset condition is
    // cleared
    let bdl = &mut device.bdl;
    write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_RESET_BIT)?;
    wait_byte(pci, RESET_TIMEOUT, CONTROL_PCM_OUT, CONTROL_RESET_BIT, 0)?;
    write_register_dword(pci, BDBAR_PCM_OUT, mapping.unwrap().device_address() as u32)?;
    let playback_time = milliseconds_to_timer_period(1000 * samples.len() as u64 / channel_count as u64 / sampling_rate as u64);
    boot_services()
        .set_timer(
            device.playback_event.unwrap(),
            uefi::table::boot::TimerTrigger::Relative(playback_time))?;
    // Set periodic timer to wait until BUFFER_SIZE samples
    // are transferred. For each chunk of BUFFER_SIZE
    // samples there is a DMA transfer going on. The
    // duration is choosen based on assumestion that all but
    // last buffer must be fill completely. The last buffer
    // transfer will be interrupted by playback event.
    // TBD: Relative timer would be better because it does not
    //      suffer from biasing.
    let delay = milliseconds_to_timer_period(1000 * BUFFER_SIZE as u64 / channel_count as u64 / sampling_rate as u64);
    boot_services()
        .set_timer(
            device.picb_event.unwrap(),
            uefi::table::boot::TimerTrigger::Periodic(delay))?;
    // Basically, this is a cached value of (LVI+1)%32.
    // Anything besides initial CIV=0.
    let mut queue_head = 1;
    loop {
        let civ = read_register_byte(pci, CIV_PCM_OUT).warning_as_error()?;
        dump_pcm_out_registers(pci);
        // Queue up some samples. Please note that no
        // transfer took place yet if this is the first
        // iteration thus we prefer to copy buffers faster.
        if queue_head != civ {
            let (bc, sc) = copy_samples_to_buffer(bdl, queue_head as usize, total_offset, samples);
            if bc != 0 {
                total_offset += sc;
                total_sample_count -= sc;
                write_register_byte(pci, LVI_PCM_OUT, queue_head as u8);
                queue_head += 1;
                if queue_head >= BUFFER_COUNT as u8 {
                    queue_head = 0;
                }
            }
        }
        let picb = read_register_word(pci, PICB_PCM_OUT)
            .warning_as_error()?;
        if picb < channel_count as u16 {
            // Maybe there is no other buffer queued up and
            // thus we must end the playback instead of
            // transferring a new chunk of data. Very unlikely
            // but might cause a piece of junk being played otherwise.
            let playback_done = boot_services()
                .check_event (device.playback_event.unwrap())
                .warning_as_error();
            if playback_done.is_err() {
                write_register_byte(pci, CONTROL_PCM_OUT, CONTROL_DMA_BIT)?;
            } else {
                break;
            }
            // Check for underrun condition. This is not a
            // proper way to handle underrun but will still
            // do it because it is simple.
            let lvi = read_register_byte(pci, LVI_PCM_OUT).warning_as_error()?;
            if lvi == civ {
                break;
            }
        }
        // Playback event must be placed first so that it
        // would be checked first
        let index = boot_services()
            .wait_for_event (&mut [device.playback_event.unwrap(), device.picb_event.unwrap()])
            .discard_errdata()?;
        if index.unwrap() == 0 {
            break;
        }
    }
    info!("play_samples -- done");
    Ok (().into())
}

fn dump_registers(pci: &PciIO) -> uefi::Result {
    let mut registers = BaseRegisterSet::default();
    // SAFETY: TBD
    let buffer = unsafe {
        core::slice::from_raw_parts_mut(
            &mut registers as *mut BaseRegisterSet as *mut u16,
            mem::size_of::<BaseRegisterSet>())
    };
    pci.read_io::<u16>(uefi::proto::pci::IoRegister::R0, 0, buffer)
        .map_err(|error| {
            error!("reading registers failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    info!("registers: {:#x?}", registers);
    Ok(().into())
}

fn copy_samples_to_buffer(bdl: &mut BufferDescriptorListWithBuffers, index: usize, offset: usize, samples: &[i16]) -> (usize, usize) {
    let mut buffer_offset = offset;
    let mut buffer_count = 0;
    for (descriptor, buffer) in bdl.descriptors.iter_mut().zip(bdl.buffers.iter_mut()).skip(index).take(1) {
        let count = (samples.len() - buffer_offset).min(buffer.len());
        &mut buffer[0..count]
            .copy_from_slice(&samples[buffer_offset..buffer_offset+count]);
        info!("copy_samples_to_buffer: schedule {} samples starting at {}", count, buffer_offset);
        if count > 1 {
            buffer_offset += count as usize;
            descriptor.length = (count - 1) as u16;
            descriptor.control = 0;
            buffer_count += 1;
        } else {
            descriptor.length = 0;
            // TBD: ignored on qemu
            descriptor.control = BDBAR_LAST_BIT;
        }
    }
    (buffer_count, buffer_offset - offset)
}

fn div_round_up(a: usize, b: usize) -> usize {
    (a + b - 1) / b
}

fn square(phase: usize, period: usize) -> i16 {
    if (phase % period) * 2 < period {
        i16::MAX
    } else {
        i16::MIN
    }
}

fn wave(buffer: &mut [i16], channels: u8, sampling_rate: u32, freq: u16) -> usize {
    if freq == 0 || channels == 0 || sampling_rate < freq as u32 {
        // TBD: other checks
        return 0;
    }
    // Get the number of full periods as a number of frames
    let samples_per_period      = channels as usize * sampling_rate as usize / freq as usize;
    let periods                 = buffer.len() / samples_per_period;
    let frames_per_period       = samples_per_period / channels as usize;
    let mut count               = 0;
    for period in buffer.chunks_mut(samples_per_period).take(periods) {
        for (index, v) in period.iter_mut().enumerate() {
            let frame_index = index / channels as usize;
            *v = square(frame_index, frames_per_period);
        }
        count += period.len();
    }
    count
}

extern "efiapi" fn pcm_tone(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> Status {
    info!("pcm_tone");
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
    // Opening protocol with GET_PROTOCOL does not require
    // use to close protocol but if we do we will remove all
    // open protocol information from handle database (even
    // with different attributes, even with BY_DRIVER).
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            device.handle,
            device.driver_handle,
            device.handle,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    pci.dont_close();
    let channel_count = 2;
    let sampling_rate = AUDIO_RATE_44100;
    let mut tone_samples = alloc::vec::Vec::new();
    tone_samples.resize(BUFFER_SIZE, 0);
    let sample_count = wave(tone_samples.as_mut_slice(), channel_count, sampling_rate, freq);
    tone_samples.truncate(sample_count);
    pci.with_proto(|pci| init_playback(pci, sampling_rate, &mut *device))?;
    pci.with_proto(|pci| loop_samples(pci, tone_samples.as_slice(), channel_count, sampling_rate, duration as u64, &mut *device))?;
    pci.with_proto(|pci| stop_playback(pci))?;
    info!("scheduled {} samples", sample_count);
    uefi::Status::SUCCESS
}

fn milliseconds_to_timer_period(msec: u64) -> u64 {
    // Number of 100 ns units
    msec * 10000
}

extern "efiapi" fn pcm_write(this: &mut SimpleAudioOut, sampling_rate: u32, channel_count: u8, format: u32, samples: *const i16, sample_count: usize) -> Status {
    info!("pcm_write");
    // TBD: this is a pointer and we should check it for alignment and null
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
    // Opening protocol with GET_PROTOCOL does not require
    // use to close protocol but if we do we will remove all
    // open protocol information from handle database (even
    // with different attributes, even with BY_DRIVER).
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            device.handle,
            device.driver_handle,
            device.handle,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    pci.dont_close();
    if channel_count != 2 {
        warn!("The channel count {} is not supported!", channel_count);
        return uefi::Status::INVALID_PARAMETER;
    }
    if format != AUDIO_FORMAT_S16LE {
        warn!("The format {:x} is not supported!", format);
        return uefi::Status::INVALID_PARAMETER;
    }
    if samples.is_null() || sample_count >= isize::MAX as usize {
        return uefi::Status::INVALID_PARAMETER;
    }
    // We check the alignment of the pointer as well because this is generally enforced by EDK2
    if (samples as *mut u8 as usize) % mem::align_of::<i16>() != 0 {
        return uefi::Status::INVALID_PARAMETER;
    }
    // TBD: samples must be readable in range [0, sample_count)
    // TBD: samples must not be mutated
    // TBD: each element of samples must be properly initialized
    // SAFETY: this is safe because samples are checked for null, alignment and size
    let samples = unsafe { core::slice::from_raw_parts(samples, sample_count) };
    info!("about to schedule a total of {} samples", sample_count);
    pci.with_proto(|pci| init_playback(pci, sampling_rate, &mut *device))?;
    pci.with_proto(|pci| play_samples(pci, samples, channel_count, sampling_rate, &mut *device))?;
    pci.with_proto(|pci| stop_playback(pci))?;
    info!("scheduling done {}", sample_count);
    uefi::Status::SUCCESS
}

extern "efiapi" fn pcm_reset(this: &mut SimpleAudioOut) -> Status {
    info!("pcm_reset");
    info!("pcm_reset -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn pcm_query_mode(this: &mut SimpleAudioOut, index: usize, mode: &mut SimpleAudioMode) -> Status {
    info!("pcm_query_mode");
    if index > 0 {
        // We only support a single mode -
        // Stereo|S16_LE|22050hz. Other modes can be
        // specified too in write() but they are not
        // guaranteed to work.
        warn!("Requested mode with index {} does not exist", index);
        return uefi::Status::INVALID_PARAMETER;
    }
    mode.sampling_rate = AUDIO_RATE_22050;
    mode.channel_count = 2;
    mode.sample_format = AUDIO_FORMAT_S16LE;
    info!("pcm_query_mode -- ok");
    uefi::Status::SUCCESS
}

//
// DriverBinding routines
//

extern "efiapi" fn pcm_supported(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    // Opening the protocol BY_DRIVER results in
    // UNSUPPORTED, SUCCESS or ACCESS_DENIED. All must be
    // passed to boot manager.
    let pci = boot_services()
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .log_warning()?;
    info!("pcm_supported -- got PCI");
    pci.with_proto(|pci| {
        let mut type00: PciType00 = Default::default();
        // SAFETY: TBD
        let buffer = unsafe {
            core::slice::from_raw_parts_mut(
                &mut type00 as *mut PciType00 as *mut u8,
                mem::size_of::<PciType00>())
        };
        pci.read_config::<u8>(0, buffer)
            .map_err(|error| {
                error!("read_config type00: {:?}", error.status());
                error
            })
            .warning_as_error()?;
        info!("vendor: {:#x}, device: {:#x}", type00.vendor_id, type00.device_id);
        let supported = {
            [
                (VID_INTEL, 0x2415), // Intel 82801AA (ICH) integrated AC'97 Controller
                (VID_INTEL, 0x2425), // Intel 82801AB (ICH0) integrated AC'97 Controller
                (VID_INTEL, 0x2445), // Intel 82801BA (ICH2) integrated AC'97 Controller
                (VID_INTEL, 0x2485), // Intel 82801CA (ICH3) integrated AC'97 Controller
                (VID_INTEL, 0x24c5), // Intel 82801DB (ICH4) integrated AC'97 Controller
                (VID_INTEL, 0x24d5), // Intel 82801EB/ER (ICH5/ICH5R) integrated AC'97 Controller
                (VID_INTEL, 0x25a6), // Intel 6300ESB integrated AC'97 Controller
                (VID_INTEL, 0x266e), // Intel 82801FB (ICH6) integrated AC'97 Controller
                (VID_INTEL, 0x27de), // Intel 82801GB (ICH7) integrated AC'97 Controller
                (VID_INTEL, 0x7195), // Intel 82443MX integrated AC'97 Controller
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
        .map_err(|error| {
            error!("close protocol PCI I/O failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    info!("pcm_supported -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn pcm_start(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    info!("pcm_start");
    // Sync with stop
    // SAFETY: when called by firmware we will be at notify or callback; for other cases we may
    //         as well check current TPL
    let _tpl = unsafe { boot_services().raise_tpl(uefi::table::boot::Tpl::NOTIFY) };
    let mut pci = boot_services()
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    let device = pci
        .with_proto(|pci| init_device_context(this.driver_handle(), handle, pci))
        .log_warning()?;
    let audio_out = &device.audio_interface;
    boot_services()
        .install_interface::<SimpleAudioOut>(handle, audio_out)
        .map_err(|error| {
            error!("failed to install audio protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    pci.with_proto(dump_registers)?;
    // consume PCI I/O
    pci.dont_close();
    // produce audio protocol and let it live in database as
    // long as the driver's image stay resident or until the
    // DisconnectController() will be invoked
    mem::forget(device);
    info!("pcm_start -- ok");
    uefi::Status::SUCCESS
}

fn pcm_stop(this: &DriverBinding, controller: Handle) -> uefi::Result {
    info!("pcm_stop");
    // Sync with start
    // SAFETY: when called by firmware we will be at notify or callback; for other cases we may
    //         as well check current TPL
    let _tpl = unsafe { boot_services().raise_tpl(uefi::table::boot::Tpl::NOTIFY) };
    let audio_out = boot_services()
        .open_protocol::<SimpleAudioOut>(
            controller,
            this.driver_handle(),
            controller,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open audio protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // Opening protocol with GET_PROTOCOL does not require
    // use to close protocol but if we do we will remove all
    // open protocol information from handle database (even
    // with different attributes, even with BY_DRIVER).
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            controller,
            this.driver_handle(),
            controller,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    pci.dont_close();
    let audio_out = audio_out.as_proto().get();
    // Note that this operation does not consume anything
    let device = if let Some(device) = DeviceContext::from_protocol_mut(boot_services(), audio_out) {
        device
    } else {
        return uefi::Status::INVALID_PARAMETER.into();
    };
    // SAFETY: TBD
    let audio_out_ref = unsafe { audio_out.as_ref().unwrap() };
    boot_services()
        .uninstall_interface::<SimpleAudioOut>(controller, audio_out_ref)
        .map_err(|error| {
            error!("failed uninstall audio protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // SAFETY: safe as long as DeviceContext is only created inside the Box
    let device = unsafe {
        Box::from_raw(device as *mut DeviceContext)
    };
    info!("pcm_stop -- ok");
    Ok(().into())                     // drop audio
}

extern "efiapi" fn pcm_stop_entry(this: &DriverBinding, controller: Handle, _num_child_controller: usize, _child_controller: *mut Handle) -> Status {
    pcm_stop(this, controller)
        .status()
}

//
// Image entry points
//

extern "efiapi" fn pcm_unload(image_handle: Handle) -> Status {
    info!("pcm_unload");
    let driver_binding = boot_services()
        .open_protocol::<DriverBinding>(
            image_handle,
            image_handle,
            image_handle,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open driver binding: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // SAFETY: TBD
    let driver_binding_ref = unsafe { &*driver_binding.as_proto().get() };
    let handles = boot_services()
        .find_handles::<SimpleAudioOut>()
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
        // Checking error is crucial because Unload() must
        // fail if something went wrong
        pcm_stop(driver_binding_ref, controller)
            .or_else(|error| {
                // If handle is controlled by our driver just skip it
                if error.status() == uefi::Status::INVALID_PARAMETER {
                    Ok(().into())
                } else {
                    Err(error)
                }
            })?;
    }
    boot_services()
        .uninstall_interface::<DriverBinding>(image_handle, driver_binding_ref)
        .map_err(|error| {
            error!("failed to uninstall driver binding: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // Driver binding is already disconnected and it is
    // about to be destroyed so no need to close it.
    let driver_binding = uefi::table::boot::leak(driver_binding);
    // SAFETY: TBD
    unsafe { Box::from_raw(driver_binding.get()) };
    info!("pcm_unload -- ok");
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
        pcm_start,
        pcm_supported,
        pcm_stop_entry,
        0x0,
        handle,
        handle)
    );
    let loaded_image = boot_services()
        .handle_protocol::<LoadedImage>(handle)
        .warning_as_error()?;
    // SAFETY: TBD
    let loaded_image = unsafe { &mut *loaded_image.get() };
    loaded_image.set_unload_routine(Some(pcm_unload));
    let driver_binding = Box::into_raw(driver_binding);
    // SAFETY: TBD
    let driver_binding_ref = unsafe { driver_binding.as_ref().unwrap() };
    boot_services()
        .install_interface::<DriverBinding>(handle, driver_binding_ref)
        .map_err(|error| {
            error!("failed to install driver binding: {:?}", error.status());
            // SAFETY: TBD
            unsafe { Box::from_raw(driver_binding) };
            error
        })
        .warning_as_error()?;
    info!("initialization complete");
    boot_services()
        .handle_protocol::<DriverBinding>(handle)
        .warning_as_error()?;
    info!("efi_main -- ok");
    uefi::Status::SUCCESS
}
