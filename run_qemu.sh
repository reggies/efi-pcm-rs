#!/usr/bin/env bash

    # -device ioh3420,bus=pci.0,addr=1c.0,multifunction=on,port=1,chassis=1,id=root.1
    # -device vfio-pci,host=01:00.0,bus=root.1,addr=00.0,multifunction=on,x-vga=on


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
    -cpu SandyBridge,+rdrand \
    -soundhw ac97
