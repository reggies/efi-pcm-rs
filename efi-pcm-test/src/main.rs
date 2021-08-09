#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#[macro_use]
extern crate log;
extern crate uefi;
extern crate uefi_services;
extern crate alloc;

extern crate efi_pcm;

// use uefi::prelude::*;

mod connect;
mod data;

// use data::*;

use uefi::prelude::*;
// use uefi::proto::pci::PciIO;
use efi_pcm::SimpleAudioOut;

// fn load_samples() -> uefi::Result {
//     let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };
//     let sfs = bt.locate_protocol::<SimpleFileSystem>()
//         .warning_as_error()?;
//     // let sfs = sfs.expect("Cannot open `SimpleFileSystem` protocol");
//     let sfs = unsafe { &mut *sfs.get() };
//     let mut directory = sfs.open_volume().unwrap().unwrap();
//     let mut buffer = vec![0; 128];
//     loop {
//         let file_info = match directory.read_entry(&mut buffer) {
//             Ok(completion) => {
//                 if let Some(info) = completion.unwrap() {
//                     info
//                 } else {
//                     // We've reached the end of the directory
//                     break;
//                 }
//             }
//             Err(error) => {
//                 // Buffer is not big enough, allocate a bigger one and try again.
//                 let min_size = error.data().unwrap();
//                 buffer.resize(min_size, 0);
//                 continue;
//             }
//         };
//         info!("Root directory entry: {:?}", file_info);
//     }
//     directory.reset_entry_readout().unwrap().unwrap();
// }

fn test_tone(audio_out: &mut SimpleAudioOut) -> uefi::Result {
    
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
        (C4, 500),
        (D4, 500),
        (D4SHARP, 1000),
        (10, 1000),
        (D4SHARP, 500),
        (F4, 500),
        (G4, 1000),
        (10, 1000),
        (G4, 500),
        (A4SHARP, 500),
        (F4, 1000),
        (10, 500),
        (G4, 250),
        (F4, 250),
        (D4SHARP, 500),
        (D4, 500),
        (C4, 1000),

        (C4, 500),
        (D4, 500),
        (D4SHARP, 1000),
        (10, 1000),
        (D4SHARP, 500),
        (F4, 500),
        (G4, 1000),
        (10, 1000),
        (G4, 500),
        (A4SHARP, 500),
        (C5, 1000),
        (A4SHARP, 1000),
        (D5, 500),
        (C5, 500),
        (10, 1000),
        
        (C5, 1000),
        (D5, 250),
        (D5SHARP, 1000),
        (D5, 1000),
        (C5, 500),
        (A4SHARP, 500),
        (G4SHARP, 1000),
        (G4, 1000),
        (F4, 1000),
        (10, 1000),
        (D4SHARP, 500),
        (G4, 250),
        (F4, 1000),

        (D4SHARP, 1000),
        (D4, 500),
        (C4, 500),        
    ];

    for &(freq, duration) in SAMPLES {
        audio_out.tone(freq, duration)
            .log_warning()?;
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

    // let my_data = load_samples()?;
    audio_out.feed(22100, data::TEST_DATA).log_warning()?;

    // test_tone(audio_out).warning_as_error()?;

    uefi::Status::SUCCESS
}
