use uefi::proto::Protocol;

use uefi::unsafe_guid;

type WriteFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut, sampling_rate: u32, samples: *const i16, sample_count: usize) -> uefi::Status;

type ToneFn =
    extern "efiapi" fn(this: &mut SimpleAudioOut, freq: u16, duration: u16) -> uefi::Status;

//
// device capabilities
//
const AUDIO_CAPABILITY_RESET: u32 = 0x1;
const AUDIO_CAPABILITY_WRITE: u32 = 0x2;
const AUDIO_CAPABILITY_TONE: u32 = 0x4;

//
// sample formats
//
const AUDIO_FORMAT_S16LE: u32 = 0x0;

#[repr(C)]
pub struct SimpleAudioMode {
    sampling_rate: u32,
    channel_count: u8,
    sample_format: u32,
}

// TBD: all fields must be private
#[repr(C)]
#[unsafe_guid("e4ed3d66-6402-4f8d-902d-5c67d5d49882")]
#[derive(Protocol)]
pub struct SimpleAudioOut {
    pub reset: usize,
    pub write: WriteFn,
    pub tone: ToneFn,
    pub set_mode: usize,
    pub query_mode: usize,
    pub mode: usize,
    pub max_mode: usize,
    pub capabilities: u32,
}

impl SimpleAudioOut {
    pub fn tone(&mut self, freq: u16, duration: u16) -> uefi::Result {
        (self.tone)(self, freq, duration)
            .into()
    }
    pub fn write(&mut self, sampling_rate: u32, samples: &[i16]) -> uefi::Result {
        (self.write)(self, sampling_rate, samples.as_ptr(), samples.len())
            .into()
    }
}
