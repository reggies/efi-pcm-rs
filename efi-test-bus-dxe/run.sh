#!/usr/bin/env bash

set -e

./build.sh

cp ./../efi-test-bus-dxe/target/x86_64-unknown-uefi/debug/efi-test-bus-dxe.efi hda

rm -f hda/NvVars

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
    -device ich9-intel-hda,debug=255 \
    -device hda-micro,debug=255
