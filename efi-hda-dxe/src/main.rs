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

/// PCI Configuration Space
/// Section 1.1, Intel I/O Controller Hub 7 Family External Design Specification, April 2005
const PCI_VID: u32      = 0x0;                      // ro, u16
const PCI_DID: u32      = 0x2;                      // ro, u16
const PCI_COMMAND: u32  = 0x4;                      // rw, u16
const PCI_STATUS: u32   = 0x6;                      // rwc, u16
const PCI_RID: u32      = 0x8;                      // ro, u8
const PCI_PI: u32       = 0x9;                      // ro, u8
const PCI_SCC: u32      = 0xA;                      // ro, u8
const PCI_BCC: u32      = 0xB;                      // ro, u8
const PCI_NAMBBAR: u32  = 0x10;                     // rw, u32
const PCI_NBMBBAR: u32  = 0x14;                     // rw, u32
const PCI_MMBAR: u32    = 0x18;                     // rw, u32
const PCI_MBBAR: u32    = 0x1C;                     // rw, u32
const PCI_INT_LN: u32   = 0x3C;                     // rw, u8
const PCI_INT_PN: u32   = 0x3D;                     // ro, u8

const BIT0: u32 = 0b1;
const BIT1: u32 = 0b10;
const BIT2: u32 = 0b100;
const BIT3: u32 = 0b1000;
const BIT4: u32 = 0b10000;
const BIT5: u32 = 0b100000;
const BIT6: u32 = 0b1000000;
const BIT7: u32 = 0b10000000;
const BIT8: u32 = 0b100000000;
const BIT9: u32 = 0b1000000000;
const BIT10: u32 = 0b10000000000;
const BIT11: u32 = 0b100000000000;
const BIT12: u32 = 0b1000000000000;
const BIT13: u32 = 0b10000000000000;
const BIT14: u32 = 0b100000000000000;
const BIT15: u32 = 0b1000000000000000;
const BIT16: u32 = 0b10000000000000000;
const BIT17: u32 = 0b100000000000000000;
const BIT18: u32 = 0b1000000000000000000;
const BIT19: u32 = 0b10000000000000000000;
const BIT20: u32 = 0b100000000000000000000;
const BIT21: u32 = 0b1000000000000000000000;
const BIT22: u32 = 0b10000000000000000000000;
const BIT23: u32 = 0b100000000000000000000000;
const BIT24: u32 = 0b1000000000000000000000000;
const BIT25: u32 = 0b10000000000000000000000000;
const BIT26: u32 = 0b100000000000000000000000000;
const BIT27: u32 = 0b1000000000000000000000000000;
const BIT28: u32 = 0b10000000000000000000000000000;
const BIT29: u32 = 0b100000000000000000000000000000;
const BIT30: u32 = 0b1000000000000000000000000000000;
const BIT31: u32 = 0b10000000000000000000000000000000;

/// Memory Mapped configuration
/// Section 1.2, Intel I/O Controller Hub 7 Family External Design Specification, April 2005
const PCI_GCAP: u32         = 0x0; // ro, u16
const PCI_VMIN: u32         = 0x2; // ro, u8
const PCI_VMAJ: u32         = 0x3; // ro, u8
const PCI_OUTPAY: u32       = 0x4; // ro, u16
const PCI_INPAY: u32        = 0x6; // ro, u16
const PCI_GCTL: u32         = 0x8; // rw, u32
const PCI_WAKEEN: u32       = 0xB; // rw, u16
const PCI_STATESTS: u32     = 0xE; // rwc, u16
const PCI_GSTS: u32         = 0x10; // rwc, u16
const PCI_OUTSTRMPAY: u32   = 0x18; // ro, u16
const PCI_INSTRMPAY: u32    = 0x1A; // ro, u16
const PCI_INTCTL: u32       = 0x20; // rw, u32
const PCI_INTSTS: u32       = 0x24; // ro, u32
const PCI_WALCLK: u32       = 0x30; // ro, u32
const PCI_SSYNC: u32        = 0x34; // rw, u32
const PCI_CORBLBASE: u32    = 0x40; // rw, u32
const PCI_CORBUBASE: u32    = 0x44; // rw, u32
const PCI_CORBWP: u32       = 0x48; // rw, u16
const PCI_CORBRP: u32       = 0x4A; // rw, u16
const PCI_CORBCTL: u32      = 0x4C; // rw, u8
const PCI_CORBST: u32       = 0x4D; // rwc, u8
const PCI_CORBSIZE: u32     = 0x4E; // ro, u8
const PCI_RIRBLBASE: u32    = 0x50; // rw, u32
const PCI_RIRBUBASE: u32    = 0x54; // rw, u32
const PCI_RIRBWP: u32       = 0x58; // rw, u16
const PCI_RINTCNT: u32      = 0x5A; // rw, u16
const PCI_RIRBCTL: u32      = 0x5C; // rw, u8
const PCI_RIRBSTS: u32      = 0x5D; // rwc, u8
const PCI_RIRBSIZE: u32     = 0x5E; // ro, u8
const PCI_IC: u32           = 0x60; // rw, u32
const PCI_IR: u32           = 0x64; // ro, u32
const PCI_IRS: u32          = 0x68; // rwc, u16

/// CORBCTL, Section 1.2.20
const PCI_CORBCTL_CMEIE_BIT: u32 = BIT0;
const PCI_CORBCTL_RUN_BIT: u32 = BIT1;

/// CORBRP, Section 1.2.19
const PCI_CORBRP_RST_BIT: u32 = BIT15;

struct DeviceContext {
    handle: Handle,
    driver_handle: Handle,
    audio_interface: SimpleAudioOut,
}

// SAFETY: we only have a single thread so global mutable statics are safe
unsafe impl Sync for DeviceContext {}
unsafe impl Send for DeviceContext {}

static mut DEVICE_CONTEXTS: Option<alloc::vec::Vec<Box<DeviceContext>>> = None;

impl DeviceContext {
    // BootServices reference is only needed to inherit its lifetime
    fn from_protocol<'a>(_bs: &'a uefi::table::boot::BootServices, raw: *const SimpleAudioOut) -> Option<&'a DeviceContext> {
        unsafe {
            DEVICE_CONTEXTS
                .as_ref()
                .and_then(|contexts| {
                    contexts.iter().find(|&context| {
                        &context.audio_interface as *const SimpleAudioOut == raw
                    })
                })
                .map(|context| {
                    context.as_ref()
                })
        }
    }

    // BootServices reference is only needed to inhert its lifetime
    fn from_protocol_mut<'a>(_bs: &'a uefi::table::boot::BootServices, raw: *mut SimpleAudioOut) -> Option<&'a mut DeviceContext> {
        unsafe {
            DEVICE_CONTEXTS
                .as_deref_mut()
                .and_then(|contexts| {
                    contexts.iter_mut().find(|context| {
                        &context.audio_interface as *const SimpleAudioOut == raw as *const SimpleAudioOut
                    })
                })
                .map(|context| {
                    context.as_mut()
                })
        }
    }
}

#[repr(C, packed)]
struct Ring {
    va: usize,
    pa: usize,
    rp: u16,
    wp: u16,
    cmd: u32,
    res: u32
}

extern "efiapi" fn hda_tone(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> Status {
    info!("hda_tone");
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
    info!("hda_tone -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_write(this: &mut SimpleAudioOut, sampling_rate: u32, channel_count: u8, format: u32, samples: *const i16, sample_count: usize) -> Status {
    info!("hda_write");
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
    info!("hda_write -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_reset(this: &mut SimpleAudioOut) -> Status {
    info!("hda_reset");
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
    info!("hda_reset -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_query_mode(this: &mut SimpleAudioOut, index: usize, mode: &mut SimpleAudioMode) -> Status {
    info!("hda_query_mode");
    let device = DeviceContext::from_protocol_mut(boot_services(), this)
        .ok_or(uefi::Status::INVALID_PARAMETER.into())?;
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
    info!("hda_query_mode -- ok");
    uefi::Status::SUCCESS
}

fn init_context(driver_handle: Handle, handle: Handle, pci: &PciIO) -> uefi::Result<Box<DeviceContext>> {
    let device = Box::new(DeviceContext {
        handle,
        driver_handle,
        audio_interface: SimpleAudioOut {
            reset: hda_reset,
            write: hda_write,
            tone: hda_tone,
            query_mode: hda_query_mode,
            max_mode: 1,
            capabilities: AUDIO_CAP_RESET | AUDIO_CAP_WRITE | AUDIO_CAP_TONE | AUDIO_CAP_MODE
        }
    });
    Ok (device.into())
}

fn deinit_context(pci: &PciIO) -> uefi::Result {
    Ok(().into())
}

//
// DriverBinding routines
//

//
// PCI Vendor ID
//
const VID_INTEL: u16 = 0x8086;

const HDA_ICH9: u16 = 0x293e;
const HDA_ICH6: u16 = 0x2668;
const HDA_ICH7: u16 = 0x27d8;
const HDA_C230: u16 = 0xa170;

fn read_config_word(pci: &PciIO, offset: u32) -> uefi::Result<u16> {
    let buffer = &mut [0];
    pci.read_config::<u16>(offset, buffer)
        .map_err(|error| {
            error!("read_config at {:} returned {:?}", offset, error.status());
            error
        })
        .warning_as_error()?;
    Ok((buffer[0].into()))
}

extern "efiapi" fn hda_supported(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    // Opening the protocol BY_DRIVER results in
    // UNSUPPORTED, SUCCESS or ACCESS_DENIED. All must be
    // passed to boot manager.
    let pci = boot_services()
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .warning_as_error()?;
    info!("hda_supported -- got PCI");
    pci.with_proto(|pci| {
        let vendor_id = read_config_word(pci, PCI_VID)
            .warning_as_error()?;
        let device_id = read_config_word(pci, PCI_DID)
            .warning_as_error()?;
        info!("vendor: {:#x}, device: {:#x}", vendor_id, device_id);
        // TBD: check class (4h) and subclass (3h)
        let supported = {
            [
                (VID_INTEL, HDA_ICH6), // ICH6
                (VID_INTEL, HDA_ICH7), // Intel ICH7 integrated HDA Controller
                (VID_INTEL, HDA_ICH9), // ICH9
                (VID_INTEL, HDA_C230), // 100 Series/C230 Series Chipset Family HD Audio Controller
            ].iter().any(|&(vid, did)| {
                vendor_id == vid && device_id == did
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
    info!("hda_supported -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hda_start(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    info!("hda_start");
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
        .with_proto(|pci| init_context(this.driver_handle(), handle, pci))
        .warning_as_error()?;
    let audio_out = &device.audio_interface;
    boot_services()
        .install_interface::<SimpleAudioOut>(handle, audio_out)
        .map_err(|error| {
            error!("failed to install audio protocol: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // consume PCI I/O
    pci.dont_close();
    // produce audio protocol and let it live in database as
    // long as the driver's image stay resident or until the
    // DisconnectController() will be invoked
    unsafe {
        DEVICE_CONTEXTS
            .as_mut()
            .unwrap()
            .push(device)
            ;
    }
    info!("hda_start -- ok");
    uefi::Status::SUCCESS
}

fn hda_stop(this: &DriverBinding, controller: Handle) -> uefi::Result {
    info!("hda_stop");
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
    pci.with_proto(|pci| deinit_context(pci))
        .log_warning()
        .map_err(|error| {
            warn!("Failed to deinitialize audio codec: {:?}", error.status());
            error
        })
        .or::<()>(Ok(().into()))
        .unwrap();                                       // ignore error
    unsafe {
        DEVICE_CONTEXTS
            .as_mut()
            .unwrap()
            .retain(|context| {
                &**context as *const DeviceContext == device as *const DeviceContext
            })
    }
    info!("hda_stop -- ok");
    Ok(().into())                                // drop audio
}

extern "efiapi" fn hda_stop_entry(this: &DriverBinding, controller: Handle, _num_child_controller: usize, _child_controller: *mut Handle) -> Status {
    hda_stop(this, controller)
        .status()
}

//
// Image entry points
//

extern "efiapi" fn hda_unload(image_handle: Handle) -> Status {
    info!("hda_unload");
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
    let driver_binding_ref = unsafe { driver_binding.as_proto().get().as_ref().unwrap() };
    let handles = boot_services()
        .find_handles::<SimpleAudioOut>()
        .log_warning()
        .map_err(|error| {
            warn!("failed to get hda protocol handles: {:?}", error.status());
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
        hda_stop(driver_binding_ref, controller)
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
    info!("hda_unload -- ok");
    // Cleanup allocator and logging facilities
    efi_dxe::unload(image_handle);
    uefi::Status::SUCCESS
}

#[entry]
fn efi_main(handle: uefi::Handle, system_table: SystemTable<Boot>) -> uefi::Status {
    efi_dxe::init(handle, &system_table)
        .warning_as_error()?;
    info!("hda_main");
    unsafe {
        DEVICE_CONTEXTS = Some(alloc::vec::Vec::new());
    }
    // TBD: allocate in runtime pool or .bss
    let driver_binding = Box::new(DriverBinding::new(
        hda_start,
        hda_supported,
        hda_stop_entry,
        0x0,
        handle,
        handle)
    );
    let loaded_image = boot_services()
        .handle_protocol::<LoadedImage>(handle)
        .warning_as_error()?;
    // SAFETY: TBD
    let loaded_image = unsafe { &mut *loaded_image.get() };
    loaded_image.set_unload_routine(Some(hda_unload));
    let driver_binding = Box::into_raw(driver_binding);
    // SAFETY: TBD
    let driver_binding_ref = unsafe { driver_binding.as_ref().unwrap() };
    boot_services()
        .install_interface::<DriverBinding>(handle, driver_binding_ref)
        .map_err(|error| {
            error!("failed to install driver binding: {:?}", error.status());
            // SAFETY: TBD
            unsafe { Box::from_raw(driver_binding) };
            error.status().into()
        })?;
    info!("initialization complete");
    boot_services()
        .handle_protocol::<DriverBinding>(handle)
        .warning_as_error()?;
    info!("hda_main -- ok");
    uefi::Status::SUCCESS
}
