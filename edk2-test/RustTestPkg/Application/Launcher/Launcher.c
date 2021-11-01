#include <Uefi.h>

#include <Library/BaseMemoryLib.h>
#include <Library/DebugLib.h>
#include <Library/MemoryAllocationLib.h>
#include <Library/PcdLib.h>
#include <Library/PrintLib.h>
#include <Library/UefiBootServicesTableLib.h>
#include <Library/UefiLib.h>
#include <Library/UefiRuntimeServicesTableLib.h>

#include <Protocol/LoadedImage.h>
#include <Protocol/SimpleFileSystem.h>

#include <Protocol/SimpleAudioOut.h>

CONST STATIC struct {
  INT16 Frequency;
  UINT16 DurationMilliseconds;
} kTestTone[] = {
  { 14000, 100 },
  { 2000, 100 },
  { 14000, 100 },
  { 2000, 100 },
  { 14000, 100 },
  { 2000, 100 },
  { 8000, 100 },
  { 2000, 100 },
  { 8000, 100 },
};

INT16 Lerpi(INT16 X0, INT16 X1, UINTN Numer, UINTN Denom)
{
  return (INT16)((INTN) X0 * ((INTN) Denom - (INTN) Numer) / (INTN) Denom +
      (INT16)((INTN) X1 * (INTN) Numer / (INTN) Denom));
  return 0;
}

EFI_STATUS
PcmTest(EFI_SIMPLE_AUDIO_OUT_PROTOCOL *SimpleAudioOut)
{
  EFI_STATUS Status;
  UINTN Index;
  INT16 *Samples;
  UINTN SampleCount;
  CONST UINT32 kFrequencies[] = {
    2*260,
    2*480,
    2*170,
    4*260,
    4*480,
    4*170,
    1*260,
    1*480,
    1*170,
  };
  UINT8 ChannelCount = 2;
  UINT32 SamplingRate = EFI_AUDIO_RATE_22050;
  UINTN Period;
  UINTN WritePointer;
  UINTN ChannelIndex;

  DEBUG((EFI_D_VERBOSE, "PcmTest Protocol: %p\n", SimpleAudioOut));

  SampleCount = ChannelCount * SamplingRate / 3;
  Samples = AllocateZeroPool(SampleCount * sizeof(INT16));
  if (Samples == NULL) {
    return RETURN_OUT_OF_RESOURCES;
  }

  for (Index = 0; Index < ARRAY_SIZE(kFrequencies); ++Index)
  {
    /* Generate some mess */
    for(WritePointer = 0; WritePointer * ChannelCount < SampleCount; ++WritePointer) {
      for(ChannelIndex = 0; ChannelIndex < ChannelCount; ++ChannelIndex) {
        Period = (SamplingRate + kFrequencies[Index] - 1) / kFrequencies[Index];
        Samples[ChannelCount*WritePointer+ChannelIndex] = Lerpi(
          MIN_INT16,
          0,
          WritePointer % Period,
          Period);
      }
    }

    /* Play some mess */
    Status = SimpleAudioOut->Write(
        SimpleAudioOut,
        SamplingRate,
        ChannelCount,
        EFI_AUDIO_FORMAT_S16LE,
        Samples,
        SampleCount);
    if (EFI_ERROR(Status)) {
      DEBUG((EFI_D_ERROR, "Write (%d) returned %p\n", Index, Status));
      goto Cleanup;
    }
  }

  DEBUG((EFI_D_VERBOSE, "PcmTest done\n"));
  Status = RETURN_SUCCESS;

Cleanup:
  if (Samples) {
    FreePool(Samples);
  }
  return Status;
}

EFI_STATUS
ToneTest(EFI_SIMPLE_AUDIO_OUT_PROTOCOL *SimpleAudioOut)
{
  EFI_STATUS Status;
  UINTN Index;

  DEBUG((EFI_D_VERBOSE, "ToneTest Protocol: %p\n", SimpleAudioOut));

  for (Index = 0; Index < ARRAY_SIZE(kTestTone); ++Index)
  {
    Status = SimpleAudioOut->Tone(SimpleAudioOut, 260, 1000);
    if (EFI_ERROR(Status)) {
      DEBUG((EFI_D_ERROR, "Tone (%d) returned %p\n", Index, Status));
      return Status;
    }
    break;
  }

  DEBUG((EFI_D_VERBOSE, "ToneTest done\n"));

  return EFI_SUCCESS;
}

EFI_STATUS
EFIAPI
UefiMain(IN EFI_HANDLE ImageHandle, IN EFI_SYSTEM_TABLE *SystemTable)
{
  EFI_STATUS Status;
  EFI_HANDLE *HandleBuffer = NULL;
  UINTN HandleCount = 0;
  UINTN Index;

  DEBUG((EFI_D_VERBOSE, "UefiMain\n"));

  Status = gBS->LocateHandleBuffer (
      ByProtocol,
      &gEfiSimpleAudioOutProtocolGuid,
      NULL,
      &HandleCount,
      &HandleBuffer);
  if (EFI_ERROR(Status)) {
    DEBUG((EFI_D_WARN, "LocateHandleBuffer returned %r\n", Status));
    goto ErrorExit;
  }

  DEBUG((EFI_D_VERBOSE, "Got %d handles\n", HandleCount));

  for(Index = 0; Index < HandleCount; ++Index) {
    EFI_SIMPLE_AUDIO_OUT_PROTOCOL *SimpleAudioOut;
    Status = gBS->HandleProtocol (
        HandleBuffer[Index],
        &gEfiSimpleAudioOutProtocolGuid,
        (VOID **) &SimpleAudioOut);
    if (EFI_ERROR(Status)) {
      DEBUG((EFI_D_WARN, "HandleProtocol returned %r\n", Status));
      continue;
    }
    DEBUG((EFI_D_INFO, "Testing PCM samples\n"));
    PcmTest(SimpleAudioOut);
    DEBUG((EFI_D_INFO, "Testing Beep generator\n"));
    ToneTest(SimpleAudioOut);
  }

  gBS->FreePool(HandleBuffer);

  return EFI_SUCCESS;

ErrorExit:

  if (HandleBuffer) {
    gBS->FreePool(HandleBuffer);
  }

  return Status;
}
