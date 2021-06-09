#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#[macro_use]
extern crate log;
extern crate uefi;
extern crate uefi_services;
extern crate alloc;

extern crate efi_pcm;

use uefi::prelude::*;

mod connect;
mod data;

use data::*;

use uefi::prelude::*;
use uefi::proto::pci::PciIO;
use efi_pcm::SimpleAudioOut;

#[entry]
fn efi_main(_handle: uefi::Handle, system_table: SystemTable<Boot>) -> uefi::Status {

    uefi_services::init(&system_table)
        .expect_success("this is only the beginning");

    info!("efi_main");

    connect::connect_pci_recursively();

    connect::enum_simple_audio_out();

    info!("efi_main -- ok");

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let audio_out = bt
        .locate_protocol::<SimpleAudioOut>()
        .log_warning()?;

    let audio_out = unsafe { &mut *audio_out.get() };

    audio_out.feed(22100, TEST_DATA)?;

    audio_out.tone(8000, (1 << 15))?;
    audio_out.tone(12000, (1 << 15))?;
    audio_out.tone(6000, (1 << 15))?;
    audio_out.tone(100, (1 << 14))?;
    audio_out.tone(20000, (1 << 14))?;
    audio_out.tone(100, (1 << 14))?;
    audio_out.tone(16000, (1 << 14))?;

    uefi::Status::SUCCESS
}
