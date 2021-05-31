#![no_std]
#![no_main]
#![feature(abi_efiapi)]

extern crate uefi;
extern crate uefi_services;

use uefi::prelude::*;

#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> uefi::Status {
    use uefi::table::runtime::ResetType;
    let rt = unsafe { system_table.runtime_services() };
    rt.reset(ResetType::Shutdown, Status::SUCCESS, None)
}
