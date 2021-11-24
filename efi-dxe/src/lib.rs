#![no_std]
#![feature(abi_efiapi)]

// For DXE stuff, from uefi-services
#![feature(lang_items)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate log;
extern crate uefi;
extern crate alloc;

use uefi::prelude::*;

mod serial;
mod stderr;

static mut SYSTEM_TABLE: Option<uefi::table::SystemTable<uefi::table::Boot>> = None;

#[cfg(all(feature = "log_serial", feature = "log_stderr"))]
compile_error!("Features log_serial and log_stderr are mutually exclusive");

#[cfg(all(feature = "log_serial", not(feature = "log_stderr")))]
mod details {
    use super::*;

    static mut LOGGER: Option<serial::Logger> = None;

    pub unsafe fn init_logger(st: &uefi::table::SystemTable<uefi::table::Boot>) {
        st.boot_services()
            .locate_protocol::<uefi::proto::console::serial::Serial>()
            .warning_as_error()
            .ok()
            .map(|serial| {
                let logger = {
                    LOGGER = Some(serial::Logger::new(&mut *serial.get()));
                    LOGGER.as_ref().unwrap()
                };
                log::set_logger(logger).unwrap();
                log::set_max_level(log::LevelFilter::Info);
            });
    }
}

#[cfg(all(feature = "log_stderr", not(feature = "log_serial")))]
mod details {
    use super::*;

    static mut LOGGER: Option<stderr::Logger> = None;

    pub unsafe fn init_logger(st: &'static uefi::table::SystemTable<uefi::table::Boot>) {
        let logger = {
            LOGGER = Some(stderr::Logger::new(st));
            LOGGER.as_ref().unwrap()
        };

        log::set_logger(logger).unwrap();
        log::set_max_level(log::LevelFilter::Info);
    }
}

#[cfg(all(not(feature = "log_serial"), not(feature = "log_stderr")))]
mod details {
    pub unsafe fn init_logger(st: &'static uefi::table::SystemTable<uefi::table::Boot>) {}
}

// fn exit_boot_services(_e: uefi::Event) {
//     uefi::alloc::exit_boot_services();
// }

pub fn boot_services() -> &'static uefi::table::boot::BootServices {
    unsafe { SYSTEM_TABLE.as_ref().unwrap().boot_services() }
}

pub fn init(_handle: uefi::Handle, system_table: &SystemTable<Boot>) -> uefi::Result {
    unsafe {
        SYSTEM_TABLE = Some(system_table.unsafe_clone());
        details::init_logger(SYSTEM_TABLE.as_ref().unwrap());
        uefi::alloc::init(system_table.boot_services());
        // TBD: the event handle must be closed when
        //  - DriverEntry() returns an error
        //  - Unload() routine has been called
        // So far we do not posses proper code structure to handle these cases.
        // boot_services()
        //     .create_event(
        //         uefi::table::boot::EventType::SIGNAL_EXIT_BOOT_SERVICES,
        //         uefi::table::boot::Tpl::NOTIFY,
        //         Some(exit_boot_services))?;
    }
    Ok (().into())
}

pub fn unload(_handle: uefi::Handle) {
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
