#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#[macro_use]
extern crate log;
extern crate uefi;
extern crate uefi_services;
extern crate alloc;

extern crate efi_pcm;

mod connect;
mod data;

use uefi::prelude::*;
use efi_pcm::SimpleAudioOut;

fn test_tone(audio_out: &mut SimpleAudioOut) -> uefi::Result {

    const I: u16 = 250;

    const C3: u16 = 130;
    const D3: u16 = 146;
    const D3SHARP: u16 = 155;
    const F3: u16 = 174;
    const G3: u16 = 196;
    const A3SHARP: u16 = 233;
    const C4: u16 = 261;
    const D4: u16 = 293;
    const D4SHARP: u16 = 311;
    const F4: u16 = 349;
    const G4: u16 = 392;
    const G4SHARP: u16 = 415;
    const A4SHARP: u16 = 466;
    const C5: u16 = 523;
    const D5: u16 = 587;
    const D5SHARP: u16 = 622;

    const SAMPLES: &[(u16, u16)] = &[
        (C4, I),
        (D4, I),
        (D4SHARP, 4*I),
        (D4SHARP, I),
        (F4, I),
        (G4, 4*I),
        (G4, I),
        (A4SHARP, I),
        (F4, 4*I),
        (G4, I/2),
        (F4, I/2),
        (D4SHARP, I),
        (D4, I),
        (C4, 4*I),
        (C4, I),
        (D4, I),
        (D4SHARP, 4*I),
        (D4SHARP, I),
        (F4, I),
        (G4, 4*I),
        (G4, I),
        (A4SHARP, I),
        (C5, 4*I),
        (A4SHARP, 3*I),
        (D5, I/2),
        (C5, 4*I),
        (C5, 3*I),
        (D5, I/2),
        (D5SHARP, 2*I),
        (D5, 2*I),
        (C5, 2*I),
        (A4SHARP, I*8/10),
        (C5, I/10),
        (A4SHARP, I/10),
        (G4SHARP, 2*I),
        (G4, 2*I),
        (F4, 4*I),
        (D4SHARP, I),
        (G4, I),
        (F4, 4*I),
        (D4SHARP, I),
        (D4, I),
        (C4, 4*I),
    ];

    for &(freq, duration) in SAMPLES {
        audio_out.tone(freq, duration)
            .log_warning()?;
    }

    Ok(().into())
}

fn test_cracks(audio_out: &mut SimpleAudioOut) -> uefi::Result {
    let mut freq = 1;
    loop {
        audio_out.tone(freq, 250);
        freq += 10;
    }

    Ok(().into())
}

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
    let data = unsafe { core::mem::transmute(data::TEST_DATA) };
    audio_out.reset()
        .warning_as_error()?;
    audio_out.write(efi_pcm::AUDIO_RATE_22050, 2, efi_pcm::AUDIO_FORMAT_S16LE, data)
        .map_err(|error| {
            error!("pcm write failed: {:?}", error.status());
            error
        })
        .warning_as_error()?;
    // test_tone(audio_out).warning_as_error()?;
    // test_cracks(audio_out).warning_as_error()?;
    uefi::Status::SUCCESS
}
