[Defines]
  PLATFORM_NAME                  = RustTestPkg
  PLATFORM_GUID                  = 61c385f1-eaea-4a26-ade0-bb552168c34d
  PLATFORM_VERSION               = 1.00
  DSC_SPECIFICATION              = 0x00010005
  OUTPUT_DIRECTORY               = Build/RustTestPkg
  SUPPORTED_ARCHITECTURES        = X64
  BUILD_TARGETS                  = DEBUG|RELEASE

[PcdsFixedAtBuild]
  gEfiMdePkgTokenSpaceGuid.PcdDebugPropertyMask|0x0f
  gEfiMdePkgTokenSpaceGuid.PcdDebugPrintErrorLevel|0xFFFFFFFF
  gEfiMdePkgTokenSpaceGuid.PcdFixedDebugPrintErrorLevel|0xFFFFFFFF

[LibraryClasses]
  BaseLib|MdePkg/Library/BaseLib/BaseLib.inf
  BaseMemoryLib|MdePkg/Library/BaseMemoryLib/BaseMemoryLib.inf
  DebugPrintErrorLevelLib|MdePkg/Library/BaseDebugPrintErrorLevelLib/BaseDebugPrintErrorLevelLib.inf
  DevicePathLib|MdePkg/Library/UefiDevicePathLib/UefiDevicePathLib.inf
  MemoryAllocationLib|MdePkg/Library/UefiMemoryAllocationLib/UefiMemoryAllocationLib.inf
  PcdLib|MdePkg/Library/BasePcdLibNull/BasePcdLibNull.inf
  PrintLib|MdePkg/Library/BasePrintLib/BasePrintLib.inf
  SynchronizationLib|MdePkg/Library/BaseSynchronizationLib/BaseSynchronizationLib.inf
  UefiApplicationEntryPoint|MdePkg/Library/UefiApplicationEntryPoint/UefiApplicationEntryPoint.inf
  UefiBootServicesTableLib|MdePkg/Library/UefiBootServicesTableLib/UefiBootServicesTableLib.inf
  UefiLib|MdePkg/Library/UefiLib/UefiLib.inf
  UefiRuntimeServicesTableLib|MdePkg/Library/UefiRuntimeServicesTableLib/UefiRuntimeServicesTableLib.inf
  # IoLib|MdePkg/Library/BaseIoLibIntrinsic/BaseIoLibIntrinsic.inf
  # PciCf8Lib|MdePkg/Library/BasePciCf8Lib/BasePciCf8Lib.inf
  # PciLib|MdePkg/Library/BasePciLibCf8/BasePciLibCf8.inf
  # PlatformHookLib|MdeModulePkg/Library/PlatformHookLibSerialPortPpi/PlatformHookLibSerialPortPpi.inf
  # SerialPortLib|MdeModulePkg/Library/BaseSerialPortLib16550/BaseSerialPortLib16550.inf
  # DebugLib|MdePkg/Library/BaseDebugLibSerialPort/BaseDebugLibSerialPort.inf
  DebugLib|MdePkg/Library/UefiDebugLibConOut/UefiDebugLibConOut.inf

[Components]
  RustTestPkg/Application/Launcher/Launcher.inf