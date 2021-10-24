#!/usr/bin/env bash

cargo build -Z patch-in-config -Z build-std --target x86_64-unknown-uefi
