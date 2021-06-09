use uefi::prelude::*;
use uefi::proto::pci::PciIO;

use efi_pcm::SimpleAudioOut;

pub fn connect_pci_recursively() {

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let handles = bt.find_handles::<PciIO>()
        .expect_success("failed to find any handles");

    for &handle in handles.iter() {
        if let Err(e) = bt.connect_all(handle, None, true) {
            use core::mem::transmute;
            warn!("handle failed to connect, handle: {:?}, status: {:?}",
                  unsafe { transmute::<_, *const ()>(handle) }, e);
        }
    }

    info!("{} handles connected", handles.len());
}

pub fn enum_simple_audio_out() {

    let bt = unsafe { uefi_services::system_table().as_ref().boot_services() };

    let handles = bt.find_handles::<SimpleAudioOut>()
        .expect_success("failed to find any audio handles");

    info!("{} handles enumerated", handles.len());
}
