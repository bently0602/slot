# Handheld-to-TV play

Target setup: RG SP with BaseOS, HDMI, a USB hub, and an 8BitDo Ultimate in
X-input mode using a USB cable or its 2.4 GHz receiver.

## Implemented

- USB controller detection, hot-plugging, and release of held inputs on removal.
- One selected external pad alongside the built-in controls. Auto keeps focus on
  the original pad through disconnect/reconnect; explicit selection persists.
- Per-controller A/B and X/Y swaps, left-stick dead zone, and reset from the
  Controllers menu. Profiles persist independently of game/display settings.
- Pause when the selected controller is missing during docked play, with an
  on-screen notice, automatic reconnect resume, and handheld Start override.
- HDMI Auto saves and reloads Slot on cable changes to resume the current game.
- HDMI resolution, fit/integer/stretch scaling, and safe-area settings.
- Keep Awake Docked defaults to On: a closed lid darkens the internal backlight
  while active HDMI play continues. Handheld lid sleep and manual power sleep
  remain available. Closed-lid undocking sleeps and waits for the lid to open
  before finishing the display restart.
- Auto / Device / HDMI audio output, applied without a game restart. ALSA
  playback endpoints are discovered from card and PCM descriptions, with the
  internal codec after HDMI as a fallback. Output errors reopen the PCM instead
  of permanently stopping the audio worker.
- TV output uses speaker volume/mute and leaves headphone preferences intact.

Automated checks cover selection, profile persistence, hot-plugging, held-button
cleanup, emulator pause/resume, closed-lid frame production and undock sleep,
audio discovery/fallback, interruptible PCM writes, and headphone preferences.
Physical RG SP verification is still pending.

## Next additions

1. **Handheld and docked preferences.** Remember separate volumes and display
   preferences and apply them on docking/undocking. Keep controller profiles
   associated with the controller rather than the display mode.
2. **Connection status.** Expand the current controller name and pause notice
   into a menu page showing controller, actual display mode, and audio output.

## Hardware acceptance checks

- Confirm the Ultimate appears in `/dev/input` in the chosen USB/receiver mode.
- Verify all four face buttons, shoulders/triggers, Start/Select, Guide or
  left-stick click, D-pad, and left-stick directions in the menu and a game.
- Change swaps/dead zone, release controls, reconnect, and restart Slot; verify
  the profile persists and handheld buttons keep their normal mapping.
- Play handheld, connect the hub/controller and HDMI, and verify the same game
  resumes on TV. Repeat with HDMI connected before the controller.
- While docked, disconnect the active pad with a direction held. Verify a pause
  notice, no stuck inputs, and resume after reconnecting. Verify an inactive pad
  does not take over. Test Start on the handheld and Pause if Lost set to Off.
- Disconnect HDMI while paused and verify handheld play resumes from the saved
  game. Repeat docking/undocking with a PS1 multi-disc playlist and check the
  selected disc and memory-card progress.
- Test a save failure and verify the display restart is deferred without ending
  the running game. Inspect `slot.log` and, when enabled, `input-trace.log`.
- With HDMI active and the Ultimate connected, close the lid and keep playing.
  Confirm the internal backlight is off and the TV picture continues. Open the
  lid, undock, and verify normal handheld play and lid sleep. Repeat with Keep
  Awake Docked set to Off and with manual power-button sleep while docked.
- Undock with the lid closed. Confirm sleep, then open the lid and verify the
  saved game resumes on the handheld. Boot with a saved game and the lid closed,
  then open it; verify the pending game load completes.
- Test Auto TV audio, Device audio through speaker/headphones while on HDMI,
  and HDMI preference before and after unplugging. Change the route on the
  shelf, start a game, and check audio during the next dock/undock cycle.
- Test TV volume/mute with headphones inserted and check headphone preferences
  when returning to Device. Verify audio recovers after cable loss and silence.

## Firmware evidence and audio diagnosis

BaseOS documents that the frontend initiates suspend by writing to
`/sys/power/state`; Slot's current doze policy darkens the backlight, pauses the
game and eventually powers off. This addition changes Slot's lid exception and
does not modify BaseOS's Super Standby or kernel suspend configuration.
See [BaseOS power documentation](https://github.com/pvaibhav/BaseOS/blob/main/docs/05-runtime-power-network.md).

Audio discovery uses the kernel's documented
[`/proc/asound/cards` and `/proc/asound/pcm` inventory](https://www.kernel.org/doc/html/latest/sound/designs/procfile.html).
The PCM opens in [ALSA nonblocking mode](https://www.alsa-project.org/alsa-doc/alsa-lib/pcm.html),
so a busy or disconnected output does not block the frontend or display restart.
The `plughw` adapter permits sample-rate and format conversion for TV endpoints.
Handheld audio first uses the firmware's configured default PCM, preserving its
codec mixer hooks. Before accepting that default, Slot queries the underlying
ALSA card and verifies it matches the discovered onboard codec; a default aimed
at HDMI or USB audio is rejected. Explicit codec PCMs remain fallbacks.
BaseOS's [startup sound helper](https://github.com/pvaibhav/BaseOS/blob/main/overlay/usr/sbin/baseos-startup-sound)
also uses the configured default speaker route. Opening a raw codec PCM can
bypass these controls and leave audio silent even when ALSA reports success.

If TV audio is silent on your firmware, capture these from an on-device shell
along with `slot.log`, both with HDMI attached and disconnected:

```sh
cat /proc/asound/cards
cat /proc/asound/pcm
cat /etc/asound.conf
```

The first hardware pass should confirm that the firmware exposes an HDMI PCM
and that the TV accepts sound. ALSA opening successfully cannot confirm audible
TV playback or any additional firmware mixer requirements.
