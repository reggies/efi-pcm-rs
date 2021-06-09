#!/usr/bin/env bash

# rustup install nightly
# rustup component add build-std
# rustup default nightly
cargo build -Z build-std --target x86_64-unknown-uefi
