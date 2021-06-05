use uefi::prelude::*;
use uefi::Event;
use uefi::proto::Protocol;
use uefi::Status;

use uefi::unsafe_guid;
use uefi::Guid;
use uefi::Identify;

type ResetFn =
    unsafe extern "efiapi" fn(this: &SimpleAudioOut) -> Status;

type FeedFn =
    unsafe extern "efiapi" fn(this: &SimpleAudioOut, sample: *const u8, sample_count: usize, delay_usec: u64) -> Status;

// type StartFn =
//     unsafe extern "efiapi" fn(this: &SimpleAudioOut) -> Status;

// type StopFn =
//     unsafe extern "efiapi" fn(this: &SimpleAudioOut) -> Status;

// type SetModeFn =
//     unsafe extern "efiapi" fn(this: &SimpleAudioOut, mode: PlaybackMode) -> Status;

// #[repr(C)]
// pub struct PlaybackMode {
//     sample_rate: usize,
//     sample_freq: usize,
//     volume_level: usize
// }

// #[repr(C)]
// pub struct PlaybackState {
//     buffer_items: usize,
//     buffer_usec: usize
// }

#[repr(C)]
#[unsafe_guid("e4ed3d66-6402-4f8d-902d-5c67d5d49882")]
#[derive(Protocol)]
pub struct SimpleAudioOut {
    pub reset: ResetFn,
    pub feed: FeedFn,

    // done: Event,
    // mode: *const PlaybackMode,
    // queue: usize,
    
    // start: StartFn,
    // stop: StopFn,
    // set_mode: SetModeFn,
    
    // status: *const PlaybackState,
}

impl Drop for SimpleAudioOut {
    fn drop(&mut self) {
        info!("my audio is dropped");
    }
}
