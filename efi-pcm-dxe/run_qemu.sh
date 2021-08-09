#!/usr/bin/env bash

set -e

    # -device ioh3420,bus=pci.0,addr=1c.0,multifunction=on,port=1,chassis=1,id=root.1
    # -device vfio-pci,host=01:00.0,bus=root.1,addr=00.0,multifunction=on,x-vga=on

./build.sh

pushd ../efi-pcm-test
./build.sh
popd

# pushd ../edk2-test
# ./build.sh
# popd

cp ./target/x86_64-unknown-uefi/debug/efi-pcm-dxe.efi hda
cp ~/edk2/Build/RustTestPkg/DEBUG_GCC5/X64/Launcher.efi hda
cp ./../efi-pcm-test/target/x86_64-unknown-uefi/debug/efi-pcm-test.efi hda

qemu-system-x86_64 \
    -machine q35 \
    -m 1024 \
    -vga std \
    -hda fat:rw:hda \
    -bios ovmf/OVMF.fd \
    -global e1000.romfile="" \
    -debugcon file:debug.log \
    -global isa-debugcon.iobase=0x402 \
    -s \
    -serial file:serial.txt \
    -serial stdio \
    -soundhw ac97
