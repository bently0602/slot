#!/bin/sh
# Slot has exited and released the framebuffer before this script runs.
SD="${SLOT_ROOT:-/mnt/sdcard}"
APP="$SD/System/device-info"
(
    cd "$APP" || exit 1
    export LD_LIBRARY_PATH="$APP/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export SDL_AUDIODRIVER=dummy
    if [ -e /dev/fb0 ]; then
        export PLATFORM=h700 SDL_VIDEODRIVER=mali
    else
        export ENVIRONMENT=DEV WINDOW_WIDTH=720 WINDOW_HEIGHT=480
    fi
    ./device-info
) >> "$SD/device-info.log" 2>&1
status=$?
if [ "$status" -eq 64 ]; then
    exec "$@" --return-device-info --device-info-sleep
fi
if [ "$status" -ne 0 ]; then
    echo "slot: Device Info exited with status $status; see $SD/device-info.log" >&2
fi
# The remaining arguments are the original executable (or loader + binary).
exec "$@" --return-device-info
