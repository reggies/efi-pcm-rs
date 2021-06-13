#!/usr/bin/env bash

set -e

export WORKSPACE=~/edk2
export EDK_TOOLS_PATH=~/edk2/BaseTools
export PACKAGES_PATH=~/edk2:$(pwd)

source ~/edk2/edksetup.sh

build \
    -t GCC5 \
    -b DEBUG \
    -p RustTestPkg/RustTestPkg.dsc \
    -m RustTestPkg/Application/Launcher/Launcher.inf \
    -a X64 \
    -n 8
