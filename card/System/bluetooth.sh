#!/bin/sh
# BaseOS owns drivers, D-Bus and BlueZ; Slot starts audio support on demand.
set -eu
command -v bluetoothctl >/dev/null || { echo 'Bluetooth tools unavailable'; exit 1; }
command -v rfkill >/dev/null && rfkill unblock bluetooth || true
if [ ! -d /sys/class/bluetooth/hci0 ]; then
    [ -x /mnt/vendor/ctrl/setBluetooth.sh ] || { echo 'Bluetooth radio unavailable'; exit 1; }
    /mnt/vendor/ctrl/setBluetooth.sh all </dev/null >/tmp/slot-bluetooth-radio.log 2>&1
fi
systemctl start bluetooth </dev/null >/tmp/slot-bluetooth-service.log 2>&1
# BaseOS currently ships the older bluealsa name; newer firmware uses bluealsad.
if ! pidof bluealsa bluealsad >/dev/null 2>&1; then
    if command -v bluealsad >/dev/null; then
        bluealsad -p a2dp-source --initial-volume=100 </dev/null >/tmp/slot-bluealsa.log 2>&1 &
    elif command -v bluealsa >/dev/null; then
        bluealsa -p a2dp-source --initial-volume=100 </dev/null >/tmp/slot-bluealsa.log 2>&1 &
    else
        echo 'Bluetooth audio service unavailable'
        exit 1
    fi
fi
# Catch missing firmware libraries or an unsupported daemon option before pairing.
sleep 1
pidof bluealsa bluealsad >/dev/null 2>&1 || { echo 'Bluetooth audio failed to start'; exit 1; }
# The Rust worker verifies the adapter and connection after startup.
echo 'Bluetooth services started'
