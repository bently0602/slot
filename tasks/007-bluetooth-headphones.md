# Bluetooth headphones

Requested by the user. Status: implementation complete; hardware acceptance pending.

Add a shelf-menu Bluetooth page to scan, select, pair/connect, disconnect and
forget A2DP headphones on BaseOS. Run bounded Bluetooth operations off the UI
thread, show status/errors, remember the selected headset and reconnect it on
device startup. Use firmware radio initialization and BlueZ/BlueALSA rather than
bundling drivers. Desktop previews explain that pairing uses the computer's OS.

Auto audio prefers the connected selected headset over HDMI/internal audio.
Explicit Device/HDMI routing retains its existing behavior. Headphones use the
existing headphone volume/mute; music pauses on connection loss. Preserve pairing
keys in BlueZ and store only the selected device address in Slot's Config folder.

Validate menu navigation, parsing, command failures/timeouts, persistence, audio
selection and connection-loss behavior. Document required firmware services and
hardware acceptance checks; physical radio/pairing/playback verification requires
an RG SP and headphones.

Implemented the shelf Bluetooth page, asynchronous bounded BlueZ commands,
BaseOS service startup helper, successful-connect persistence and startup
reconnect, A2DP ALSA routing and headphone volume/mute. Explicit disconnect
disables startup reconnect. Music pauses on transport loss even if HDMI is
available. Desktop previews direct pairing to the computer's settings.

Validation in Ubuntu/WSL: device-feature cargo check, 142 targeted Rust tests
(Slot unit, Bluetooth, menu, controller settings, docked audio, music and Slot UI
menu tests), and shell syntax check passed. Rendered and visually inspected the
main and Bluetooth menus at 720×480; previews remain under target/desktop.
The firmware dependency and physical acceptance checklist is in BLUETOOTH.md.
