#!/bin/sh
cargo ndk -t arm64-v8a -o ./RustAndroid/app/src/main/jniLibs/ build --no-default-features --features="android,vulkan" --lib
