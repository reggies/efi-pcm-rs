use uefi::proto::console::serial::Serial;
use uefi::prelude::*;

use core::fmt::{self, Write};
use core::ptr::NonNull;

struct MySerial<'boot>(Serial<'boot>);

pub struct Logger {
    writer: Option<NonNull<MySerial<'static>>>,
}

impl Logger {
    pub unsafe fn new(proto: &mut Serial) -> Self {
        Logger {
            writer: NonNull::new(proto as *const _ as *mut _),
        }
    }
}

impl<'boot> log::Log for Logger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        self.writer.is_some()
    }

    fn log(&self, record: &log::Record) {
        if let Some(mut ptr) = self.writer {
            let writer = unsafe { ptr.as_mut() };
            let _ = writeln!(writer, "{}: {}", record.level(), record.args());
        }
    }

    fn flush(&self) {
        // This simple logger does not buffer output.
    }
}

unsafe impl Sync for Logger {}
unsafe impl Send for Logger {}

impl<'boot> fmt::Write for MySerial<'boot> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        self.0.write(bytes)
            .warning_as_error()
            .map_err(|_| fmt::Error)
    }
}
