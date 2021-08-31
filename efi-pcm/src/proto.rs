use uefi::proto::Protocol;

use uefi::unsafe_guid;

type ResetFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut) -> uefi::Status;

type WriteFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut, sampling_rate: u32, channel_count: u8, format: u32, samples: *const i16, sample_count: usize) -> uefi::Status;

type ToneFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> uefi::Status;

type QueryModeFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut, index: usize, mode: &mut SimpleAudioMode) -> uefi::Status;

//
// device capabilities
//
pub const AUDIO_CAP_RESET: u32 = 0x1;
pub const AUDIO_CAP_WRITE: u32 = 0x2;
pub const AUDIO_CAP_TONE: u32 = 0x4;
pub const AUDIO_CAP_MODE: u32 = 0x8;

//
// sampling rate
//
pub const AUDIO_RATE_8000: u32 = 8000;
pub const AUDIO_RATE_11025: u32 = 11025;
pub const AUDIO_RATE_16000: u32 = 16000;
pub const AUDIO_RATE_22050: u32 = 22050;
pub const AUDIO_RATE_32000: u32 = 32000;
pub const AUDIO_RATE_44100: u32 = 44100;
pub const AUDIO_RATE_48000: u32 = 48000;

//
// sample formats
//
pub const AUDIO_FORMAT_S16LE: u32 = 0x0;

#[repr(C)]
pub struct SimpleAudioMode {
    pub sampling_rate: u32,
    pub channel_count: u8,
    pub sample_format: u32,
}

// TBD: all fields must be private
#[repr(C)]
#[unsafe_guid("e4ed3d66-6402-4f8d-902d-5c67d5d49882")]
#[derive(Protocol)]
pub struct SimpleAudioOut {
    pub reset: ResetFn,
    pub write: WriteFn,
    pub tone: ToneFn,
    pub query_mode: QueryModeFn,
    pub max_mode: usize,
    pub capabilities: u32,
}

impl SimpleAudioOut {
    pub fn reset(&mut self) -> uefi::Result {
        (self.reset)(self)
            .into()
    }
    pub fn tone(&mut self, freq: u16, duration: u16) -> uefi::Result {
        (self.tone)(self, freq, duration)
            .into()
    }
    pub fn write(&mut self, sampling_rate: u32, channel_count: u8, format: u32, samples: &[i16]) -> uefi::Result {
        (self.write)(self, sampling_rate, channel_count, format, samples.as_ptr(), samples.len())
            .into()
    }
}
