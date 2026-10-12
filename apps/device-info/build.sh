#!/bin/sh
set -eu
mode="${1:-device}"
stage=sdk
case "$mode" in
    device)
        platform=linux/arm64; output=target-device/device-info
        if [ "$(uname -m)" = x86_64 ]; then
            platform=linux/amd64; stage=cross-device
        fi
        ;;
    host) platform="linux/$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/')"; output=target/desktop/device-info ;;
    *) echo 'Usage: sh apps/device-info/build.sh [device|host]' >&2; exit 2 ;;
esac
repo="$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)"
cd "$repo"
docker build --platform "$platform" --target "$stage" -t "slot-device-info-$mode" -f apps/device-info/Dockerfile apps/device-info
mkdir -p "$output" target/device-info-go-cache target/device-info-go-mod
docker run --rm --platform "$platform" \
    -v "$repo:/src" \
    -v "$repo/target/device-info-go-cache:/root/.cache/go-build" \
    -v "$repo/target/device-info-go-mod:/go/pkg/mod" \
    -e "APP_OUTPUT=/src/$output" -e "APP_MODE=$mode" \
    "slot-device-info-$mode"
