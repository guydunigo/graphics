#!/bin/sh
RUSTFLAGS='-Clink-arg=-Wl,-zcommon-page-size=16384 -Clink-arg=-Wl,-zmax-page-size=16384' \
    cargo ndk -t arm64-v8a -o ./RustAndroid/app/src/main/jniLibs/ build --no-default-features --features="android,vulkan" --lib
