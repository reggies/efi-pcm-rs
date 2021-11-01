#ifndef __SIMPLEAUDIOOUT_H__
#define __SIMPLEAUDIOOUT_H__

#include <Uefi.h>

#define EFI_SIMPLE_AUDIO_OUT_PROTOCOL_GUID \
  { 0xe4ed3d66, 0x6402, 0x4f8d, { 0x90, 0x2d, 0x5c, 0x67, 0xd5, 0xd4, 0x98, 0x82 }}

typedef struct _EFI_SIMPLE_AUDIO_OUT_PROTOCOL EFI_SIMPLE_AUDIO_OUT_PROTOCOL;

typedef struct _EFI_SIMPLE_AUDIO_OUT_MODE EFI_SIMPLE_AUDIO_OUT_MODE;

//
// Device Capabilities
//
#define EFI_AUDIO_CAP_RESET         (0x1)
#define EFI_AUDIO_CAP_WRITE         (0x2)
#define EFI_AUDIO_CAP_TONE          (0x4)
#define EFI_AUDIO_CAP_MODE          (0x8)

//
// Sampling Rate
//
#define EFI_AUDIO_RATE_8000         (8000)
#define EFI_AUDIO_RATE_11025        (11025)
#define EFI_AUDIO_RATE_16000        (16000)
#define EFI_AUDIO_RATE_22050        (22050)
#define EFI_AUDIO_RATE_32000        (32000)
#define EFI_AUDIO_RATE_44100        (44100)
#define EFI_AUDIO_RATE_48000        (48000)

//
// Sample Formats
//
#define EFI_AUDIO_FORMAT_S16LE      (0x0)

typedef
EFI_STATUS
(EFIAPI * EFI_SIMPLE_AUDIO_OUT_QUERY_MODE) (
  IN EFI_SIMPLE_AUDIO_OUT_PROTOCOL *This,
  IN UINTN Index,
  OUT EFI_SIMPLE_AUDIO_OUT_MODE *Mode
  );

typedef
EFI_STATUS
(EFIAPI * EFI_SIMPLE_AUDIO_OUT_TONE) (
  IN EFI_SIMPLE_AUDIO_OUT_PROTOCOL *This,
  IN INT16 Frequency,
  IN UINT16 Duration
  );

typedef
EFI_STATUS
(EFIAPI * EFI_SIMPLE_AUDIO_OUT_RESET) (
  IN EFI_SIMPLE_AUDIO_OUT_PROTOCOL *This
  );

typedef
EFI_STATUS
(EFIAPI * EFI_SIMPLE_AUDIO_OUT_WRITE) (
  IN EFI_SIMPLE_AUDIO_OUT_PROTOCOL *This,
  IN UINT32 SamplingRate,
  IN UINT8 ChannelCount,
  IN UINT32 SampleFormat,
  IN INT16 *Samples,
  IN UINTN SampleCount
  );

struct _EFI_SIMPLE_AUDIO_OUT_PROTOCOL {
  EFI_SIMPLE_AUDIO_OUT_RESET Reset;
  EFI_SIMPLE_AUDIO_OUT_WRITE Write;
  EFI_SIMPLE_AUDIO_OUT_TONE Tone;
  EFI_SIMPLE_AUDIO_OUT_QUERY_MODE QueryMode;
  UINTN MaxMode;
  UINT32 Capabilities;
};

struct _EFI_SIMPLE_AUDIO_OUT_MODE {
  UINT32 SamplingRate;
  UINT8 ChannelCount;
  UINT32 SampleFormat;
};

extern EFI_GUID gEfiSimpleAudioOutProtocolGuid;

#endif
