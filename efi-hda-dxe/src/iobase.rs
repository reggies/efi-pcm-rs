use core::marker::PhantomData;
use core::ops::{BitAnd, BitOr};
use uefi::proto::pci::PciIO;
use uefi::prelude::*;
use uefi::table::boot::BootServices;
use core::fmt::LowerHex;

use efi_dxe::*;

pub struct IoBase<T> {
    offset: u64,
    width: PhantomData<T>
}

impl<T: Copy + Clone + Default + uefi::proto::pci::ToIoWidth> IoBase<T> {
    pub const fn new(offset: u32) -> IoBase<T> {
        IoBase {offset: offset as u64, width: PhantomData}
    }

    pub fn read(&self, pci: &PciIO) -> uefi::Result<T, ()> {
        let value = &mut [Default::default()];
        pci.read_mem::<T>(uefi::proto::pci::IoRegister::R0, self.offset, value)
            .ignore_warning()?;
        Ok(value[0].into())
    }

    pub fn write(&self, pci: &PciIO, value: T) -> uefi::Result {
        pci.write_mem::<T>(uefi::proto::pci::IoRegister::R0, self.offset, &[value])
            .ignore_warning()?;
        Ok(().into())
    }
}

impl<T> IoBase<T> where
    T: Copy + Clone + uefi::proto::pci::ToIoWidth + BitAnd<Output=T> + Default + PartialEq<T> + LowerHex {
    pub fn wait(&self, pci: &PciIO, timeout: u64, mask: T, value: T) -> uefi::Result {
        let mut time = 0;
        loop {
            let register = self.read(pci).ignore_warning()?;
            if (register & mask) == value {
                return Ok(().into());
            }
            boot_services().stall(100);
            time += 100;
            if time >= timeout {
                return Err (uefi::Status::TIMEOUT.into());
            }
        }
    }
}

impl<T> IoBase<T> where
    T: Copy + Clone + BitAnd<Output=T> + uefi::proto::pci::ToIoWidth + Default {
    pub fn and(&self, pci: &PciIO, mask: T) -> uefi::Result {
        let value = self.read(pci).ignore_warning()?;
        self.write(pci, value & mask)
    }
}

impl<T> IoBase<T> where
    T: Copy + Clone + BitOr<Output=T> + uefi::proto::pci::ToIoWidth + Default {
    pub fn or(&self, pci: &PciIO, mask: T) -> uefi::Result {
        let value = self.read(pci).ignore_warning()?;
        self.write(pci, value | mask)
    }
}
