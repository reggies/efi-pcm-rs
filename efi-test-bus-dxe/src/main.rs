#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![allow(unused_imports)]
#![allow(unused_variables)]

// Because there are too many constants that we won't gonna use
#![allow(dead_code)]

// Because extra parens lead to better readability
#![allow(unused_parens)]

// For IoBase
#![feature(const_fn_trait_bound)]

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

struct DeviceContext {
    controller_handle: Handle,
    child_handle: Option<Handle>,
    driver_handle: Handle,
    index: u32,
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

fn register_device_context(device: Box<DeviceContext>) {
    unsafe {
        DEVICE_CONTEXTS
            .as_mut()
            .unwrap()
            .push(device)
            ;
    }
}

fn unregister_device_context(device: &DeviceContext) {
    unsafe {
        DEVICE_CONTEXTS
            .as_mut()
            .unwrap()
            .retain(|context| {
                &**context as *const DeviceContext != device as *const DeviceContext
            })
    }
}

extern "efiapi" fn hdbus_tone(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> Status {
    info!("hdbus_tone");
    info!("hdbus_tone -- ok");
    uefi::Status::UNSUPPORTED
}

extern "efiapi" fn hdbus_write(this: &mut SimpleAudioOut, sampling_rate: u32, channel_count: u8, format: u32, samples: *const i16, sample_count: usize) -> Status {
    info!("hdbus_write");
    info!("hdbus_write -- ok");
    uefi::Status::UNSUPPORTED
}

extern "efiapi" fn hdbus_reset(this: &mut SimpleAudioOut) -> Status {
    info!("hdbus_reset");
    info!("hdbus_reset -- ok");
    uefi::Status::UNSUPPORTED
}

extern "efiapi" fn hdbus_query_mode(this: &mut SimpleAudioOut, index: usize, mode: &mut SimpleAudioMode) -> Status {
    info!("hdbus_query_mode");
    info!("hdbus_query_mode -- ok");
    uefi::Status::UNSUPPORTED
}

fn init_context(driver_handle: Handle, controller_handle: Handle, index: u32) -> uefi::Result<Box<DeviceContext>> {
    let device = Box::new(DeviceContext {
        child_handle: None,
        controller_handle,
        driver_handle,
        index,
        audio_interface: SimpleAudioOut {
            reset: hdbus_reset,
            write: hdbus_write,
            tone: hdbus_tone,
            query_mode: hdbus_query_mode,
            max_mode: 1,
            capabilities: 0
        }
    });
    Ok (device.into())
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

/// PCI Configuration Space
/// Section 1.1, Intel I/O Controller Hub 7 Family External Design Specification, April 2005
const PCI_VID: u32      = 0x0;                      // ro, u16
const PCI_DID: u32      = 0x2;                      // ro, u16

extern "efiapi" fn hdbus_supported(this: &DriverBinding, handle: Handle, remaining_path: *mut DevicePath) -> Status {
    // Opening the protocol BY_DRIVER results in
    // UNSUPPORTED, SUCCESS or ACCESS_DENIED. All must be
    // passed to boot manager.
    let pci = boot_services()
        .open_protocol::<PciIO>(handle, this.driver_handle(), handle, OpenAttribute::BY_DRIVER)
        .ignore_warning()?;
    info!("hdbus_supported -- got PCI on handle {:?}", handle);
    pci.with_proto(|pci| {
        let vendor_id = pci.read_config_single::<u16>(PCI_VID)
            .ignore_warning()?;
        let device_id = pci.read_config_single::<u16>(PCI_DID)
            .ignore_warning()?;
        info!("vendor: {:#x}, device: {:#x}", vendor_id, device_id);
        // TBD: check class (4h) and subclass (3h)
        let supported = {
            [
                (VID_INTEL, HDA_ICH6), // ICH6
                (VID_INTEL, HDA_ICH7), // Intel ICH7 integrated HDA Controller
                (VID_INTEL, HDA_ICH9), // ICH9
            ].iter().any(|&(vid, did)| {
                vendor_id == vid && device_id == did
            })
        };
        if !supported {
            return uefi::Status::UNSUPPORTED.into();
        }
        Ok(().into())
    }).ignore_warning()?;
    info!("hdbus_supported -- ok");
    uefi::Status::SUCCESS
}

extern "efiapi" fn hdbus_start(this: &DriverBinding, controller: Handle, remaining_path: *mut DevicePath) -> Status {
    info!("hdbus_start");
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            controller,
            this.driver_handle(),
            controller,
            OpenAttribute::BY_DRIVER)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    // TBD: what to do when some child controllers failed to be created?
    pci.dont_close();
    for n in 0..5 {
        let mut device = pci
            .with_proto(|pci| init_context(this.driver_handle(), controller, n))
            .ignore_warning()?;
        let audio_out = &device.audio_interface;
        device.child_handle = boot_services()
            .create_child::<SimpleAudioOut>(audio_out)
            .map_err(|error| {
                error!("failed to install audio protocol: {:?}", error.status());
                error
            })
            .ignore_warning()
            .map(|result| Some(result))?;
        let result = boot_services()
            .open_protocol::<PciIO>(
                device.controller_handle,
                device.driver_handle,
                device.child_handle.unwrap(),
                OpenAttribute::BY_CHILD
            )
            .map_err(|error| {
                error!("failed to open PCI I/O by child: {:?}", error.status());
                error
            })
            .ignore_warning();
        if let Err(error) = result {
            boot_services()
                .uninstall_interface::<SimpleAudioOut>(
                    device.child_handle.unwrap(),
                    audio_out);
            return error.status();
        }
        let mut pci = result.ok().unwrap();
        pci.dont_close();
        info!("registering device context for {:?} for child {:?}: {:?}",
              controller,
              device.child_handle.unwrap(),
              (&device.audio_interface as *const SimpleAudioOut as *const u8)
        );
        register_device_context(device);
    }
    info!("hdbus_start -- ok");
    uefi::Status::SUCCESS
}

fn hdbus_stop_bus(this: &DriverBinding, controller: Handle) -> uefi::Result {
    info!("hdbus_stop_bus");
    // Note that this does not destroy child handles and if
    // called directly could leave system in undefined
    // state.
    boot_services()
        .close_protocol::<PciIO>(
            controller,
            this.driver_handle(),
            controller
        )
        .map_err(|error| {
            error!("failed to close PCI I/O protocol: {:?}", error.status());
            error
        })?;
    info!("hdbus_stop_bus -- ok");
    uefi::Status::SUCCESS.into()
}

fn hdbus_stop_child(this: &DriverBinding, controller: Handle, child: Handle) -> uefi::Result {
    info!("hdbus_stop_child: controller {:?}, child {:?}", controller, child);
    let audio_out = boot_services()
        .open_protocol::<SimpleAudioOut>(
            child,
            this.driver_handle(),
            controller,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open audio protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    let audio_out = audio_out.as_proto().get();
    // Note that this operation does not consume anything
    let device = if let Some(device) = DeviceContext::from_protocol_mut(boot_services(), audio_out) {
        device
    } else {
        error!("invalid device context");
        return uefi::Status::INVALID_PARAMETER.into();
    };
    let audio_out_ref = unsafe { audio_out.as_ref().unwrap() };
    let mut pci = boot_services()
        .open_protocol::<PciIO>(
            device.controller_handle,
            this.driver_handle(),
            controller,
            OpenAttribute::GET_PROTOCOL)
        .map_err(|error| {
            error!("failed to open PCI I/O protocol: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    pci.dont_close();
    // Note, that we need to close PCI I/O on behalf of the
    // child controller before we do uninstall_interface()
    // because the handle gets closed and invalidated
    if let Err(status) = pci.close() {
        warn!("failed to close PCI I/O: {:?}", status);
    }
    // Note that UninstallMultipleProtocolInterfaces() can
    // returned EFI_ACCESS_DENIED if any agents hold the
    // protocol opened and refuse to close.
    boot_services()
        .uninstall_interface::<SimpleAudioOut>(child, audio_out_ref)
        .map_err(|error| {
            error!("failed uninstall audio protocol: {:?}", error.status());
            error
        })?;
    unregister_device_context(device);
    info!("hdbus_stop_child -- ok");
    Ok(().into())
}

fn hdbus_stop(this: &DriverBinding, controller: Handle, child_controllers: &[Handle]) -> uefi::Result {
    info!("hdbus_stop");
    for &child in child_controllers {
        info!("stop child");
        if let Err(status) = hdbus_stop_child(this, controller, child) {
            error!("failed to stop child device: {:?}", status);
            return uefi::Status::DEVICE_ERROR.into();
        }
    }
    info!("hdbus_stop -- ok");
    uefi::Status::SUCCESS.into()
}

extern "efiapi" fn hdbus_stop_entry(this: &DriverBinding, controller: Handle, num_child_controller: usize, child_controller: *mut Handle) -> Status {
    if num_child_controller != 0 {
        if child_controller.is_null() {
            return uefi::Status::INVALID_PARAMETER;
        }
        let child_controllers = unsafe {
            core::slice::from_raw_parts(child_controller, num_child_controller)
        };
        hdbus_stop(this, controller, child_controllers)
            .status()
    } else {
        hdbus_stop_bus(this, controller)
            .status()
    }
}

//
// Image entry points
//

extern "efiapi" fn hdbus_unload(image_handle: Handle) -> Status {
    info!("hdbus_unload");
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
        .ignore_warning()?;
    let driver_binding_ref = unsafe { driver_binding.as_proto().get().as_ref().unwrap() };
    let handles = boot_services()
        .find_handles::<PciIO>()
        .ignore_warning()
        .map_err(|error| {
            warn!("failed to get PCI I/O handles: {:?}", error.status());
            error
        }).or_else(|error| {
            if error.status() == uefi::Status::NOT_FOUND {
                Ok(alloc::vec::Vec::new())
            } else {
                Err(error)
            }
        })?;
    for controller in handles {
        // If our drivers does not manage the specified
        // controller (i.e. does not hold it open BY_DRIVER)
        // then the disconnect will succeed
        info!("disconnecting child controller {:?}", controller);
        info!("driver handle: {:?}", driver_binding_ref.driver_handle());
        info!("image handle: {:?}", image_handle);
        boot_services()
            .disconnect(
                controller,
                Some(driver_binding_ref.driver_handle()),
                None)
            .map_err(|error| {
                warn!("failed to disconnect PCI I/O controller {:?}: {:?}", controller, error.status());
                error
            });
    }
    if unsafe { !DEVICE_CONTEXTS.as_ref().unwrap().is_empty() } {
        error!("failed to disconnect some devices");
        return uefi::Status::DEVICE_ERROR.into();
    }
    boot_services()
        .uninstall_interface::<DriverBinding>(image_handle, driver_binding_ref)
        .map_err(|error| {
            error!("failed to uninstall driver binding: {:?}", error.status());
            error
        })
        .ignore_warning()?;
    // Driver binding is already disconnected and it is
    // about to be destroyed so no need to close it.
    let driver_binding = uefi::table::boot::leak(driver_binding);
    unsafe { Box::from_raw(driver_binding.get()) };
    info!("hdbus_unload -- ok");
    // Cleanup allocator and logging facilities
    efi_dxe::unload(image_handle);
    uefi::Status::SUCCESS
}

#[entry]
fn efi_main(handle: uefi::Handle, system_table: SystemTable<Boot>) -> uefi::Status {
    efi_dxe::init(handle, &system_table)
        .ignore_warning()?;
    info!("hdbus_main");
    unsafe {
        DEVICE_CONTEXTS = Some(alloc::vec::Vec::new());
    }
    // TBD: allocate in runtime pool or .bss
    let driver_binding = Box::new(DriverBinding::new(
        hdbus_start,
        hdbus_supported,
        hdbus_stop_entry,
        0x0,
        handle,
        handle)
    );
    let loaded_image = boot_services()
        .handle_protocol::<LoadedImage>(handle)
        .ignore_warning()?;
    let loaded_image = unsafe { &mut *loaded_image.get() };
    loaded_image.set_unload_routine(Some(hdbus_unload));
    let driver_binding = Box::into_raw(driver_binding);
    let driver_binding_ref = unsafe { driver_binding.as_ref().unwrap() };
    boot_services()
        .install_interface::<DriverBinding>(handle, driver_binding_ref)
        .map_err(|error| {
            error!("failed to install driver binding: {:?}", error.status());
            unsafe { Box::from_raw(driver_binding) };
            error.status().into()
        })?;
    info!("initialization complete");
    boot_services()
        .handle_protocol::<DriverBinding>(handle)
        .ignore_warning()?;
    info!("hdbus_main -- ok");
    uefi::Status::SUCCESS
}
