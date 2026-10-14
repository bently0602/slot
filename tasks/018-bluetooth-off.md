# Completely turn Bluetooth off

Add Turn Bluetooth Off to the shelf's Bluetooth menu. Block the Bluetooth radio,
stop Bluetooth audio and BlueZ services, and release the UART attachment. Keep
pairings and headphone selection, remember the off setting across restarts,
and allow Scan or Connect to turn Bluetooth on again. Queue shutdown behind
any in-flight operation so a pending connection cannot re-enable the radio.
Pause music using Bluetooth before shutdown and leave desktop Bluetooth under
the computer's own settings.

Status: complete. Requested by the user.

The Bluetooth page includes Turn Bluetooth Off above Status. Shutdown uses
Bluetooth-only rfkill blocking, stops BlueALSA/BlueZ and the UART attachment,
and clears the vendor initialization lock so Scan/Connect can restart cleanly.
An atomic `Config/bluetooth-off.txt` preference suppresses reconnect and reapplies
shutdown on device boot; pairings and the saved headset address remain intact.
Music using Bluetooth pauses before shutdown. Off requests queue behind busy
workers and discard their results to prevent a late connection from re-enabling
the radio. Desktop builds direct users to computer settings.

Validation: all 77 focused Rust tests passed across Bluetooth unit/integration,
music, menu behavior and menu artwork suites. The device-feature build check,
formatting, whitespace, shell syntax and fake-firmware shutdown-helper checks
passed. Generated and inspected `target/bluetooth-off-menu-preview.png`, then
removed the temporary preview harness. Physical radio/power savings and restart
acceptance remain pending; the checklist is in BLUETOOTH.md.
