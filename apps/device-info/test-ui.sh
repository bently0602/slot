#!/bin/sh
# Run inside the host build image with xvfb-run; all windows use a virtual display.
set -eu
output=/src/target/desktop/device-info
export LD_LIBRARY_PATH="$output/lib"
export ENVIRONMENT=DEV WINDOW_WIDTH=720 WINDOW_HEIGHT=480
export SDL_VIDEODRIVER=x11 SDL_AUDIODRIVER=dummy
export SLOT_ROOT=/src/sdcard SLOT_VERSION=test SLOT_BUILD=preview
"$output/device-info" > /src/target/device-info-ui-test.log 2>&1 &
app_pid=$!
trap 'kill "$app_pid" 2>/dev/null || true' EXIT
window="$(timeout 20 xdotool search --sync --name '^Device Info$')"
window="$(printf '%s\n' "$window" | head -n1)"
test -n "$window"
sleep 2
import -window "$window" /src/target/device-info-preview.png
# Confirm refresh keeps the app open, then scroll down to expose storage info.
xdotool key --window "$window" z
sleep 1
kill -0 "$app_pid"
xdotool key --window "$window" --repeat 11 --delay 180 Down
sleep 1
import -window "$window" /src/target/device-info-storage-preview.png
sleep 1
import -window "$window" /src/target/device-info-storage-preview-2.png
xdotool key --window "$window" x
wait "$app_pid"
echo 'Device Info UI: OK (render, refresh, scroll, back)'
