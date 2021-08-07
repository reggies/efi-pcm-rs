use uefi::proto::console::text::Output;

use core::fmt::{self, Write};
use core::ptr::NonNull;
use uefi::{CStr16, ResultExt};

use alloc::vec::Vec;

struct MyOutput<'boot>(Output<'boot>);

pub struct Logger<'boot> {
    system_table: &'boot uefi::table::SystemTable<uefi::table::Boot>,
    enabled: bool
}

impl<'boot> Logger<'boot> {
    pub fn new(system_table: &'boot uefi::table::SystemTable<uefi::table::Boot>) -> Self {
        Logger {
            system_table,
            enabled: true
        }
    }

    pub fn disable(&mut self) {
        self.enabled = false
    }
}

impl<'boot> log::Log for Logger<'boot> {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        self.enabled
    }

    fn log(&self, record: &log::Record) {
        if self.enabled {
            let writer = self.system_table.stdout();
            let _ = writeln!(writer, "{}: {}", record.level(), record.args());
        }
    }

    fn flush(&self) {
        // This simple logger does not buffer output.
    }
}

unsafe impl<'boot> Sync for Logger<'boot> {}
unsafe impl<'boot> Send for Logger<'boot> {}

impl<'boot> fmt::Write for MyOutput<'boot> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s)
    }
}
