#!/bin/sh
# Exercise the real shutdown helper with fake firmware commands, never host radios.
set -eu
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
test_root=$(mktemp -d)
trap 'rm -rf "$test_root"' EXIT HUP INT TERM
mkdir "$test_root/bin"
export BT_TEST_LOG="$test_root/log" BT_TEST_STOPPED="$test_root/stopped"
cat > "$test_root/bin/mock" <<'MOCK'
#!/bin/sh
name=${0##*/}
printf '%s %s\n' "$name" "$*" >> "$BT_TEST_LOG"
case "$name" in
    rfkill)
        [ "$*" = 'block bluetooth' ] || exit 1
        [ "${BT_TEST_FAIL:-0}" = 0 ]
        ;;
    systemctl) [ "$*" = 'stop bluetooth' ] ;;
    killall) : > "$BT_TEST_STOPPED" ;;
    pidof)
        [ ! -f "$BT_TEST_STOPPED" ] || exit 1
        echo 12345
        ;;
    rm) [ "$*" = '-f /tmp/.init_bt' ] ;;
    *) echo 'Unexpected firmware command'; exit 1 ;;
esac
MOCK
chmod +x "$test_root/bin/mock"
for command in rfkill systemctl killall pidof rm bluetoothctl; do
    ln -s mock "$test_root/bin/$command"
done
PATH="$test_root/bin:$PATH" sh "$repo/card/System/bluetooth.sh" off > "$test_root/output"
grep -qx 'Bluetooth off' "$test_root/output"
grep -qx 'rfkill block bluetooth' "$BT_TEST_LOG"
grep -qx 'systemctl stop bluetooth' "$BT_TEST_LOG"
grep -qx 'killall bluealsa bluealsad bluetoothd rtk_hciattach' "$BT_TEST_LOG"
grep -qx 'rm -f /tmp/.init_bt' "$BT_TEST_LOG"
! grep -Eq 'unblock|start bluetooth|bluetoothctl' "$BT_TEST_LOG"

# Already-off radio and absent daemons remain an idempotent success.
PATH="$test_root/bin:$PATH" sh "$repo/card/System/bluetooth.sh" off > "$test_root/output"
grep -qx 'Bluetooth off' "$test_root/output"

# A failed radio block cannot be reported as a completed battery-saving shutdown.
: > "$BT_TEST_LOG"
if PATH="$test_root/bin:$PATH" BT_TEST_FAIL=1 sh "$repo/card/System/bluetooth.sh" off > "$test_root/output" 2>&1; then
    echo 'Radio-block failure incorrectly succeeded'; exit 1
fi
grep -qx 'Could not turn Bluetooth radio off' "$test_root/output"
[ "$(wc -l < "$BT_TEST_LOG")" -eq 1 ]
echo 'Bluetooth shutdown helper checks passed'
