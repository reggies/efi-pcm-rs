use uefi::prelude::*;
use uefi::proto::driver_binding::DriverBinding;
use uefi::proto::component_name::{ComponentName2,ComponentName};
use uefi::data_types::{CStr8, Char8, CStr16, Char16};
use core::pin::Pin;
use alloc::boxed::*;
use alloc::string::String;
use core::ptr::NonNull;

use crate::{hda_start, hda_supported, hda_stop_entry};

#[repr(C)]
struct DriverContext {
    driver_binding: DriverBinding,
    component_name: ComponentName,
    component_name2: ComponentName2,
    driver_name: Pin<Box<[Char16]>>,
    component_name_supported_languages: Pin<Box<[Char8]>>,
    component_name2_supported_languages: Pin<Box<[Char8]>>
}

static mut DRIVER_CONTEXT: Option<DriverContext> = None;

impl DriverContext {
    fn driver_name(&self, language: &CStr8) -> uefi::Result<*const Char16> {
        let language = String::from_utf8(language.to_bytes().to_vec())
            .map_err(|_| uefi::Status::INVALID_PARAMETER)?;
        if language != "eng" {
            return Err(uefi::Status::UNSUPPORTED.into());
        }
        Ok(self.driver_name.as_ptr().into())
    }

    fn driver_name2(&self, language: &CStr8) -> uefi::Result<*const Char16> {
        let language = String::from_utf8(language.to_bytes().to_vec())
            .map_err(|_| uefi::Status::INVALID_PARAMETER)?;
        if language != "en" {
            return Err(uefi::Status::UNSUPPORTED.into());
        }
        Ok(self.driver_name.as_ptr().into())
    }
}

extern "efiapi" fn get_driver_name(this: &ComponentName, language: *const Char8, driver_name: *mut *const Char16) -> uefi::Status {
    if language.is_null() || driver_name.is_null() {
        return uefi::Status::INVALID_PARAMETER;
    }
    let language = unsafe {
        CStr8::from_ptr(language)
    };
    let driver_context = unsafe {
        &DRIVER_CONTEXT
    };
    let name = driver_context
        .as_ref()
        .unwrap()
        .driver_name(language)
        .ignore_warning()?;
    unsafe {
        (*driver_name) = &*name;
    }
    uefi::Status::SUCCESS
}

extern "efiapi" fn get_driver_name2(this: &ComponentName2, language: *const Char8, driver_name: *mut *const Char16) -> uefi::Status {
    if language.is_null() || driver_name.is_null() {
        return uefi::Status::INVALID_PARAMETER;
    }
    let language = unsafe {
        CStr8::from_ptr(language)
    };
    let driver_context = unsafe {
        &DRIVER_CONTEXT
    };
    let name = driver_context
        .as_ref()
        .unwrap()
        .driver_name2(language)
        .ignore_warning()?;
    unsafe {
        (*driver_name) = &*name;
    }
    uefi::Status::SUCCESS
}

extern "efiapi" fn get_controller_name(this: &ComponentName, controller: Handle, child: Option<NonNull<Handle>>, language: *const Char8, controller_name: *mut *const Char16) -> uefi::Status {
    uefi::Status::UNSUPPORTED
}

extern "efiapi" fn get_controller_name2(this: &ComponentName2, controller: Handle, child: Option<NonNull<Handle>>, language: *const Char8, controller_name: *mut *const Char16) -> uefi::Status {
    uefi::Status::UNSUPPORTED
}

pub fn init_driver_binding(image_handle: Handle) {
    // TBD: better way to deal with null terminated strings?
    let component_name_supported_languages = Pin::new(
        b"eng\0"
            .to_vec()
            .into_iter()
            .map(Char8::from)
            .collect::<alloc::vec::Vec<_>>()
            .into_boxed_slice());
    // TBD: better way to deal with null terminated strings?
    let component_name2_supported_languages = Pin::new(
        b"en\0"
            .to_vec()
            .into_iter()
            .map(Char8::from)
            .collect::<alloc::vec::Vec<_>>()
            .into_boxed_slice());
    // TBD: better way to deal with null terminated strings?
    let driver_name = Pin::new(
        b"HDA Bus Driver\0"
            .to_vec()
            .into_iter()
            .map(Char16::from)
            .collect::<alloc::vec::Vec<_>>()
            .into_boxed_slice());
    let driver_context = DriverContext {
        driver_binding: DriverBinding::new(
            hda_start,
            hda_supported,
            hda_stop_entry,
            0x0,
            image_handle,
            image_handle),
        component_name: ComponentName::new(
            get_driver_name,
            get_controller_name,
            (&*component_name_supported_languages).as_ptr()
        ),
        component_name2: ComponentName2::new(
            get_driver_name2,
            get_controller_name2,
            (&*component_name2_supported_languages).as_ptr()
        ),
        component_name_supported_languages,
        component_name2_supported_languages,
        driver_name,
    };
    unsafe {
        if DRIVER_CONTEXT.is_none() {
            DRIVER_CONTEXT = Some(driver_context);
        }
    }
}

pub fn driver_binding<'a>() -> &'a DriverBinding {
    unsafe {
        &DRIVER_CONTEXT.as_ref().unwrap().driver_binding
    }
}

pub fn component_name<'a>() -> &'a ComponentName {
    unsafe {
        &DRIVER_CONTEXT.as_ref().unwrap().component_name
    }
}

pub fn component_name2<'a>() -> &'a ComponentName2 {
    unsafe {
        &DRIVER_CONTEXT.as_ref().unwrap().component_name2
    }
}
