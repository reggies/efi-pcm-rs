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

#if 0
EFI_STATUS
PlayTest(EFI_SIMPLE_AUDIO_OUT_PROTOCOL *SimpleAudioOut)
{
  EFI_STATUS Status;
  UINTN Index;

  DEBUG((EFI_D_VERBOSE, "PlayTest Protocol: %p\n", SimpleAudioOut));

  for (Index = 0; Index < ARRAY_SIZE(kTestTone); ++Index)
  {
    Status = SimpleAudioOut->Feed(SimpleAudioOut, 14000, 50);
    if (EFI_ERROR(Status)) {
      DEBUG((EFI_D_ERROR, "Feed (%d) returned %p\n", Index, Status));
      return Status;
    }
  }

  DEBUG((EFI_D_VERBOSE, "ToneTest done\n"));

  return EFI_SUCCESS;
}
#endif

EFI_STATUS
ToneTest(EFI_SIMPLE_AUDIO_OUT_PROTOCOL *SimpleAudioOut)
{
  EFI_STATUS Status;
  UINTN Index;

  DEBUG((EFI_D_VERBOSE, "ToneTest Protocol: %p\n", SimpleAudioOut));

  for (Index = 0; Index < ARRAY_SIZE(kTestTone); ++Index)
  {
    Status = SimpleAudioOut->Tone(SimpleAudioOut, 14000, 50);
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

  // for(Index = 0; Index < HandleCount; ++Index) {
  //   EFI_SIMPLE_AUDIO_OUT_PROTOCOL *SimpleAudioOut;
  //   Status = gBS->HandleProtocol (
  //       HandleBuffer[Index],
  //       &gEfiSimpleAudioOutProtocolGuid,
  //       (VOID **) &SimpleAudioOut);
  //   if (EFI_ERROR(Status)) {
  //     DEBUG((EFI_D_WARN, "HandleProtocol returned %r\n", Status));
  //     continue;
  //   }
  //   PlayTest(SimpleAudioOut);
  // }

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
