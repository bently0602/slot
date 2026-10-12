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
rm "$root/System/device-info/device-info"
sh "$repo/card/System/device-info.sh" "$tmp/frontend"
[ "$(cat "$TEST_CAPTURE")" = '--return-device-info' ]
[ "${SDL_VIDEODRIVER:-}" != mali ]
echo 'Device Info handoff: OK (success, failure, crash, missing app, paths with spaces)'
