# Bluetooth headphone validation

Slot's shelf menu uses BaseOS's vendor UART initializer and systemctl shim, BlueZ
`bluetoothctl`, and BlueALSA's A2DP source service. The device bundle includes
`System/bluetooth.sh`; it does not install firmware drivers or replace ALSA's
system configuration. All commands run on a background worker with deadlines.
Slot's Linux ALSA worker opens `bluealsa:DEV=<address>,PROFILE=a2dp` only for Auto
output while the selected connected headphones are active.

References: [BaseOS runtime documentation](https://github.com/pvaibhav/BaseOS/blob/main/docs/05-runtime-power-network.md),
[BlueZ bluetoothctl](https://github.com/bluez/bluez/blob/master/doc/bluetoothctl.rst),
[BlueALSA PCM documentation](https://github.com/arkq/bluez-alsa/blob/master/doc/bluealsa-plugins.7.rst).

Automated checks cover BlueZ output parsing and filtering, validated addresses,
selection without changing the active route, successful-connect persistence,
explicit disconnect, command errors/timeouts, disconnect monitoring, headphone
volume/mute, menu navigation, routing precedence and music pause on Bluetooth
loss with HDMI available. Desktop builds never start Bluetooth services.

Turn Bluetooth Off blocks the Bluetooth radio with rfkill, stops BlueALSA and
BlueZ, terminates the UART attachment, and clears the vendor attachment lock
so Scan/Connect can initialize it again. Pairing keys and the remembered headset
stay intact. `Config/bluetooth-off.txt` suppresses startup reconnect and reapplies
shutdown on device boot. Off requests wait behind pending startup operations.
Automated shutdown-helper checks use fake firmware commands and never access
the host radio: `sh tests/test-bluetooth-off.sh`.

## Hardware acceptance (pending)

- Rebuild with `bash setup-device.sh` and merge the generated System directory
  onto SD2. Boot with no saved headset and verify the radio stays idle until Scan.
- Scan and pair an unpaired A2DP headset. Check names, discovery, already-paired
  reconnect, unavailable headset, denied pairing and busy-operation navigation.
  Controllers should not appear in the headphone selector. Devices requiring PIN
  entry are outside the NoInputNoOutput pairing flow; verify a useful failure.
- Play GBA, GB/GBC, PS1, SNES, MP3 and FLAC. Confirm stereo output, resampling,
  volume/mute and absence of unintended speaker playback on transport failures.
- Test Auto with HDMI attached, then explicit Device and HDMI. Browse a second
  headset without connecting it; audio must remain on the current one.
- During music playback turn the headset off, both handheld and docked. Music
  should pause and stay paused after reconnect. Test with a wired jack inserted.
- Disconnect, reboot and confirm no automatic reconnect; Connect, reboot and
  confirm the saved headset reconnects. Forget and confirm BlueZ removes its bond.
- Choose Turn Bluetooth Off while connected, disconnected and scanning. Confirm
  the radio is blocked, Bluetooth daemons/UART attachment stop, music on Bluetooth
  pauses, and Wi-Fi still works. Reboot to check it stays off, then use Scan or
  Connect to bring the radio and audio back without losing pairings. Measure
  idle power on the handheld before and after shutdown.
- Check lid playback, power-button suspend/resume, HDMI frontend restart and
  recovery when the firmware radio or audio daemon is unavailable. Measure game
  audio latency on the actual headset; A2DP latency depends on the transport.
- Inspect `slot.log`, `/tmp/slot-bluetooth-radio.log`,
  `/tmp/slot-bluetooth-service.log` and `/tmp/slot-bluealsa.log` on failures.

LE Audio, headset microphones, PIN/passkey entry and Bluetooth controller pairing
are not part of this feature. Firmware owns pairing-key persistence and radio
power across suspend. Slot retries opening audio while BlueZ reports connection;
after connection loss, reconnect through the menu.
