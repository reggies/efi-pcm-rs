#!/usr/bin/env bash

cp ./target/x86_64-unknown-uefi/debug/my-test.efi hda

qemu-system-x86_64 \
    -machine q35 \
    -m 5120 \
    -vga std \
    -hda fat:rw:hda \
    -smp 4 \
    -bios ovmf/OVMF.fd \
    -global e1000.romfile="" \
    -debugcon file:debug.log \
    -global isa-debugcon.iobase=0x402 \
    -s \
    -serial file:serial.txt \
    -serial stdio \
    -cpu SandyBridge,+rdrand
