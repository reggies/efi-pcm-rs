use uefi::proto::Protocol;

use uefi::unsafe_guid;

type ResetFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut) -> uefi::Status;

type FeedFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut, sample_rate: u32, samples: *const u16, sample_count: usize) -> uefi::Status;

type ToneFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> uefi::Status;

#[repr(C)]
#[unsafe_guid("e4ed3d66-6402-4f8d-902d-5c67d5d49882")]
#[derive(Protocol)]
pub struct SimpleAudioOut {
    pub reset: ResetFn,
    pub feed: FeedFn,
    pub tone: ToneFn,
}

impl SimpleAudioOut {
    pub fn tone(&mut self, freq: u16, duration: u16) -> uefi::Result {
        (self.tone)(self, freq, duration)
            .into()
    }
    pub fn feed(&mut self, sample_rate: u32, samples: &[u16]) -> uefi::Result {
        (self.feed)(self, sample_rate, samples.as_ptr(), samples.len())
            .into()
    }
}
