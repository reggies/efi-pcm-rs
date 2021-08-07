use uefi::prelude::*;
use uefi::table::boot::BootServices;

use core::fmt::{self, Write};
use core::str;
use core::mem;

mod serial;
mod stderr;

static mut SYSTEM_TABLE: Option<uefi::table::SystemTable<uefi::table::Boot>> = None;
static mut LOGGER: Option<stderr::Logger> = None;

// unsafe fn init_serial_logger(st: &uefi::table::SystemTable<uefi::table::Boot>) -> () {
//     st.boot_services()
//         .locate_protocol::<uefi::proto::console::serial::Serial>()
//         .warning_as_error()
//         .ok()
//         .map(|serial| {
//             let logger = {
//                 LOGGER = Some(serial::Logger::new(&mut *serial.get()));
//                 LOGGER.as_ref().unwrap()
//             };
//             log::set_logger(logger).unwrap();
//             log::set_max_level(log::LevelFilter::Info);
//         });
// }

unsafe fn init_logger(st: &'static uefi::table::SystemTable<uefi::table::Boot>) -> () {
    let logger = {
        LOGGER = Some(stderr::Logger::new(st));
        LOGGER.as_ref().unwrap()
    };

    log::set_logger(logger).unwrap();
    log::set_max_level(log::LevelFilter::Info);
}

pub fn boot_services() -> &'static uefi::table::boot::BootServices {
    unsafe { SYSTEM_TABLE.as_ref().unwrap().boot_services() }
}

pub fn init(handle: uefi::Handle, system_table: &SystemTable<Boot>) -> uefi::Result {
    unsafe {
        SYSTEM_TABLE = Some(system_table.unsafe_clone());
        init_logger(SYSTEM_TABLE.as_ref().unwrap());
        uefi::alloc::init(system_table.boot_services());
    }
    Ok (().into())
}

pub fn unload(handle: uefi::Handle) {
    // Nothing in here for the moment
}

#[lang = "eh_personality"]
fn eh_personality() {}

#[cfg(not(feature = "no_panic_handler"))]
#[panic_handler]
fn panic_handler(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!(
            "Panic in {} at ({}, {}):",
            location.file(),
            location.line(),
            location.column()
        );
        if let Some(message) = info.message() {
            error!("{}", message);
        }
    }

    // Give the user some time to read the message
    if let Some(st) = unsafe { SYSTEM_TABLE.as_ref() } {
        st.boot_services().stall(10_000_000);
    } else {
        let mut dummy = 0u64;
        // FIXME: May need different counter values in debug & release builds
        for i in 0..300_000_000 {
            unsafe {
                core::ptr::write_volatile(&mut dummy, i);
            }
        }
    }

    // If the system table is available, use UEFI's standard shutdown mechanism
    if let Some(st) = unsafe { SYSTEM_TABLE.as_ref() } {
        use uefi::table::runtime::ResetType;
        st.runtime_services()
            .reset(ResetType::Shutdown, uefi::Status::ABORTED, None);
    }

    // If we don't have any shutdown mechanism handy, the best we can do is loop
    error!("Could not shut down, please power off the system manually...");

    loop {
        // just run forever dammit how do you return never anyway
    }
}

#[alloc_error_handler]
fn out_of_memory(layout: ::core::alloc::Layout) -> ! {
    panic!(
        "Ran out of free memory while trying to allocate {:#?}",
        layout
    );
}
