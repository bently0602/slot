#!/bin/sh
# Slot has exited and released the framebuffer before this script runs.
SD="${SLOT_ROOT:-/mnt/sdcard}"
APP="$SD/System/device-info"
(
    cd "$APP" || exit 1
    export LD_LIBRARY_PATH="$APP/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export SDL_AUDIODRIVER=dummy
    loader="${SLOT_DEVICE_INFO_LOADER:-}"
    if [ -e /dev/fb0 ]; then
        export PLATFORM=h700 SDL_VIDEODRIVER=mali
        # Match Slot's BaseOS launch: FAT/exFAT files need no executable bits.
        loader="${loader:-/lib/ld-linux-aarch64.so.1}"
    else
        export ENVIRONMENT=DEV WINDOW_WIDTH=720 WINDOW_HEIGHT=480
    fi
    printf 'Device Info: launching %s (video=%s, loader=%s)\n' \
        "$APP/device-info" "${SDL_VIDEODRIVER:-auto}" "${loader:-direct}"
    if [ -n "$loader" ]; then
        "$loader" ./device-info
    else
        ./device-info
    fi
) >> "$SD/device-info.log" 2>&1
status=$?
printf 'Device Info: exited with status %s\n' "$status" >> "$SD/device-info.log"
if [ "$status" -eq 64 ]; then
    exec "$@" --return-device-info --device-info-sleep
fi
if [ "$status" -ne 0 ]; then
    echo "slot: Device Info exited with status $status; see $SD/device-info.log" >&2
fi
# The remaining arguments are the original executable (or loader + binary).
exec "$@" --return-device-info
