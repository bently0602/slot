#!/bin/sh
set -eu
repo="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
root="$tmp/card with spaces"
mkdir -p "$root/System/device-info/lib"
export SLOT_ROOT="$root"
export TEST_CAPTURE="$tmp/capture"
cat > "$tmp/frontend" <<'EOF'
#!/bin/sh
printf '%s\n' "$@" > "$TEST_CAPTURE"
EOF
chmod +x "$tmp/frontend"
for status in 0 1 139; do
    printf '#!/bin/sh\nexit %s\n' "$status" > "$root/System/device-info/device-info"
    chmod +x "$root/System/device-info/device-info"
    sh "$repo/card/System/device-info.sh" "$tmp/frontend" 'original argument'
    [ "$(cat "$TEST_CAPTURE")" = "$(printf 'original argument\n--return-device-info')" ]
done
printf '#!/bin/sh\nexit 64\n' > "$root/System/device-info/device-info"
sh "$repo/card/System/device-info.sh" "$tmp/frontend"
[ "$(cat "$TEST_CAPTURE")" = "$(printf '%s\n' --return-device-info --device-info-sleep)" ]
# An explicit loader must start a readable app even without executable bits.
cat > "$tmp/loader" <<'EOF'
#!/bin/sh
printf '%s\n' "$@" > "$TEST_LOADER_CAPTURE"
exec sh "$@"
EOF
chmod +x "$tmp/loader"
export TEST_LOADER_CAPTURE="$tmp/loader-capture"
printf '#!/bin/sh\nexit 0\n' > "$root/System/device-info/device-info"
chmod 644 "$root/System/device-info/device-info"
SLOT_DEVICE_INFO_LOADER="$tmp/loader" sh "$repo/card/System/device-info.sh" "$tmp/frontend"
[ "$(cat "$TEST_LOADER_CAPTURE")" = './device-info' ]
[ "$(cat "$TEST_CAPTURE")" = '--return-device-info' ]
grep -q 'Device Info: launching' "$root/device-info.log"
grep -q 'Device Info: exited with status 0' "$root/device-info.log"
rm "$root/System/device-info/device-info"
sh "$repo/card/System/device-info.sh" "$tmp/frontend"
[ "$(cat "$TEST_CAPTURE")" = '--return-device-info' ]
[ "${SDL_VIDEODRIVER:-}" != mali ]
echo 'Device Info handoff: OK (success, failure, crash, missing app, paths with spaces, loader without executable bits, startup logging)'
