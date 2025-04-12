#!/bin/sh

SCRIPT_DIR="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"

# for arm64-v8a
export AARCH64_UNKNOWN_LINUX_OHOS_OPENSSL_DIR="$SCRIPT_DIR/../ohos-openssl/prelude/arm64-v8a/"
# for armeabi-v7a
export ARMV7_UNKNOWN_LINUX_OHOS_OPENSSL_DIR="$SCRIPT_DIR/../ohos-openssl/prelude/armeabi-v7a/"
# for x86_64
export X86_64_UNKNOWN_LINUX_OHOS_OPENSSL_DIR="$SCRIPT_DIR/../ohos-openssl/prelude/x86_64/"

ohrs build --release