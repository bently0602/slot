<div align="center">

<img src=".github/resources/banner.png" width="100%" alt="slot. A bespoke, Game Boy-centric frontend for the Anbernic RG SP.">

<a href="LICENSE"><img src="https://img.shields.io/github/license/BrandonKowalski/slot?style=for-the-badge&labelColor=242429&color=4d4d57" height="36" alt="License"></a>
<a href="https://github.com/BrandonKowalski/slot/stargazers"><img src="https://img.shields.io/github/stars/BrandonKowalski/slot?style=for-the-badge&labelColor=242429&color=4d4d57" height="36" alt="Stars"></a>
<a href="https://github.com/BrandonKowalski/slot/releases"><img src="https://img.shields.io/github/downloads/BrandonKowalski/slot/total?style=for-the-badge&labelColor=242429&color=4d4d57" height="36" alt="Downloads"></a>
<a href="https://slot-cfw.fyi"><img src="https://img.shields.io/badge/Guide-slot--cfw.fyi-4d4d57?style=for-the-badge&labelColor=242429" height="36" alt="Guide"></a>
<a href="CHANGELOG.md"><img src="https://img.shields.io/badge/Changelog-4d4d57?style=for-the-badge&labelColor=242429" height="36" alt="Changelog"></a>

</div>

---

# What is slot?

A bespoke, Game Boy-centric frontend for the Anbernic RG SP.

Supports GBA, GBC, GB, original PlayStation (PS1), and Super Nintendo (SNES) titles.

> [!IMPORTANT]
> ROMs must be unzipped. slot does not read `.zip` or `.7z` files.

A full user guide can be found at [slot-cfw.fyi](https://slot-cfw.fyi).

Release notes can be found in the [changelog](CHANGELOG.md).

---

# What does it look like?

<p align="center">
<img src=".github/resources/screenshots/carousel.gif" width="48%" alt="Scrolling the cart carousel across the Game Boy Advance, Game Boy and Game Boy Color shelves">
<img src=".github/resources/screenshots/core-picker.gif" width="48%" alt="Flipping a cart to its circuit board to choose mGBA or gpSP">
</p>

<p align="center">
<img src=".github/resources/screenshots/link.gif" width="96.5%" alt="Two SPs racing in Mario Kart over the emulated link">
</p>

---

# Supported Devices

| Device             | Supported                  | Since Version |
|--------------------|----------------------------|---------------|
| Anbernic RG SP     | Yes                        | Always        |
| Anbernic RG34XX    | Yes                        | 1.4.0         |
| Anbernic RG34XXSP  | Yes (No Stick Support Yet) | 1.4.0         |
| Anbernic RG35XXSP  | Yes                        | 1.5.0         |
| Anbernic 35XX Plus | Yes                        | 1.5.0         |

The RG34XX will likely work from 1.4.0. The RG34XXSP may not map its sticks correctly.

If you have one of the Untested devices, or have tried slot on a device not listed,
please [open an issue on GitHub](https://github.com/BrandonKowalski/slot/issues) and let me know how it went.

---

# Cart shelf settings

On the cart shelf, press **Menu → Screen → Cover Flow** and use Left or Right to
switch between On and Off. Cover Flow angles and overlaps neighboring carts as you
scroll. It starts Off, and your choice is saved across restarts for all console shelves.
On desktop, Menu is **Tab** and the A button is **Z**.

On RG SP with BaseOS, **Menu → Video Out** controls HDMI. **Auto** selects HDMI
when a cable is connected; **Internal** keeps the built-in screen.
Choose **720p / 60** (default) or **1080p / 60**, then restart the device to apply
output or resolution settings. In Auto mode, connecting or disconnecting HDMI
saves the current game and restarts Slot to recreate the display, then resumes
the game. Expect a brief pause rather than a device reboot. Switching waits for
loading, cartridge animations, sleep, or a link session to finish. If saving
fails, Slot keeps the current session and logs the error in `slot.log`.
HDMI setup failures attempt to restore the internal screen.

**HDMI Scaling** offers Fit (preserve aspect ratio), Integer (whole multiples of
the 720×480 canvas), or Stretch (fill the TV). **HDMI Safe Area** adds 0%, 5%, or
10% padding on each edge for TVs that crop the picture. These two settings apply
immediately on HDMI and do not change the built-in screen. The 1080p signal uses
a 720p framebuffer scaled by the display hardware; HDMI integer scaling refers
to that framebuffer.

**Keep Awake Docked** defaults to On: closing the lid keeps Slot running on an
active HDMI display and turns off the internal backlight. Set it to Off to sleep
on every lid close. Handheld lid sleep and the power button still work. If HDMI
disconnects with the lid closed, Slot sleeps; open the lid to finish switching
back to the handheld. A failed display setup or Internal output never enables
the docked lid exception.

**Audio Output** applies immediately on the device:

- **Auto** (default): prefer TV audio while HDMI is active; use handheld audio
  otherwise.
- **Device**: keep audio on the handheld speaker/headphone output, useful for a
  monitor without speakers.
- **HDMI**: prefer the TV output, with handheld fallback when HDMI is inactive
  or its audio endpoint cannot open.

Slot discovers ALSA playback endpoints instead of assuming card numbers, and
falls back to the internal codec if TV audio cannot open.
Handheld audio prefers the firmware's configured default PCM to preserve speaker
mixer controls, checking that its underlying card is the onboard codec before use.
Slot's volume buttons and mute still apply. TV audio uses the existing speaker
volume/mute preference;
headphone volume/mute remain separate, including with a headphone cable inserted.
The selected PCM and requested route are recorded in `slot.log`. Hardware
verification on RG SP/BaseOS is still pending. Desktop previews keep using the
computer's normal audio output.

### Bluetooth headphones

On BaseOS devices with Bluetooth, open **Menu → Bluetooth** on the cart shelf.
Put your headphones in pairing mode, choose **Scan for Headphones**, then use
Left/Right on **Headphones** to select them and press A on **Connect**. Scanning
takes about 15 seconds, followed by device identification. The Status row shows
progress or a failure. B returns to the system menu while operations finish.

With **Video Out → Audio Output → Auto**, the connected selected headphones take
priority over HDMI and the handheld. They use the existing headphone volume and
mute settings. Device and HDMI choices retain their explicit routing. Browsing
another headset does not switch audio until you connect it. Music pauses when
Bluetooth playback loses its connection, including while docked; reconnecting
does not automatically resume music. If a connected headset's audio transport
cannot open, Slot reports an audio error and retries without sending that audio
to speakers.

**Disconnect** ends the selected connection and disables reconnect at startup.
**Forget Headphones** removes its pairing. A successful connection saves the
address in `Config/bluetooth.txt` and attempts to reconnect at the next device
startup; pairing keys are managed by the firmware's BlueZ service. After a lost
connection, select **Connect** again. Slot starts the firmware's Bluetooth and
BlueALSA services on demand, so rebuild and copy the full device bundle first.

This supports A2DP headphone playback through BlueALSA. It requires firmware
Bluetooth drivers, `bluetoothctl`, and the BlueALSA daemon and ALSA plugin. Models
without a Bluetooth radio show an unavailable message. Desktop previews use the
computer's audio output and direct pairing to the computer's settings. Physical
pairing, playback, latency and suspend/resume still need handheld validation;
the checklist is in [BLUETOOTH.md](BLUETOOTH.md).

To preview HDMI scaling on a computer, run
`SLOT_HDMI_PREVIEW=1 SLOT_WINDOW=1280x720 bash setup-desktop.sh --run`.
The preview never changes the computer's display configuration.

## Music

Put MP3 and FLAC files in `Games/Music`, for example
`Games/Music/Artist/Album/01 - Track.flac`. Each folder with directly contained
audio files is one album; nested folders (including separate discs) are separate
albums. Files directly in `Games/Music` also form an album. Tags supply album,
artist, title and disc/track ordering; folder names and filenames fill missing
tags. Albums sort by artist/title. Artwork prefers `cover.jpg`, `cover.jpeg`,
then `cover.png` (case insensitive), then an embedded cover, then a placeholder.
Resized artwork is cached in `System/Cache/Music`.

Music is the last shelf after the consoles and always uses square coverflow.
Left/Right browse albums, Up/Down jump initials, and A flips a cover into six
scrolling track rows. Up/Down select a track, A plays it, and B flips back.
Browsing another album leaves the current album playing until you select a track.

| Where | Music controls |
|-------|----------------|
| Music covers | L1/R1 switch shelves |
| Track panel or full-screen visualizer | L1/R1 previous/next track |
| Any music screen | Start play/pause; X toggle full-screen visualizer; L2/R2 seek back/forward 10 seconds |
| Full-screen visualizer | Left/Right choose spectrum bars or waveform; B return |
| Game shelves | Y return to the loaded album; Start keeps its game core-picker action |

With no loaded track, Start plays the selected album's first track. Previous
restarts a track after three seconds; otherwise it follows playback history.
Music continues while browsing and in menus, and stops before launching a game
or external app. A footer on game shelves shows the current track and pause state.

**Menu → Music** provides playback controls, album shuffle, repeat Off/Album/Track,
visualizer style, **Play With Lid Closed**, and **Rescan Library**. Shuffle visits
each track once per cycle. Manual skipping overrides Repeat Track. The default
visualizer is a retro spectrum with falling peaks; the other is a green waveform.

Playback/settings are saved atomically in `Config/music.json`, including relative
album/track paths and position. Normal startup restores the track paused. HDMI
display restarts preserve its playing/paused state. Sleep resumes music only if
it was playing before sleep. Removing headphones pauses headphone playback;
reconnecting them does not resume it.

**Play With Lid Closed** defaults On: while music plays, lid close turns off the
internal screen and keeps playback running. Pause, stop or album completion
returns a closed lid to normal sleep unless the docked keep-awake setting applies.
The power button retains its normal sleep/shutdown behavior. Volume, mute and
speaker/headphone/HDMI routing use Slot's existing controls.

The first version supports mono/stereo MP3 and FLAC through Slot's 16-bit stereo
output. It does not provide cross-album queues, music during games, online artwork,
tag editing, crossfade, guaranteed gapless playback, or MilkDrop effects.
Implementation and validation notes are in [MUSIC.md](MUSIC.md).

## Battery and standby

Open **Menu → Battery** on the cart shelf for a live percentage and status:
Charging, Full, Discharging, Status Unknown, or Unavailable when the battery
cannot be read. Percentage refreshes every ten seconds; charging status refreshes
every second. The shelf's charging bolt and status LED remain available.

**Sleep to Shutdown** offers **1, 3, 5, 10, or 30 minutes**, or **Never**.
The default remains three minutes. **While Charging** defaults Off;
Charging or Full keeps standby alive, and unplugging starts a fresh delay.
Turn it On to use the selected delay while charging too. Both settings survive
restarts. Active closed-lid music and docked play do not start the standby timer.
Waking with the lid or Power cancels it; the next sleep starts a new delay.

At **10% or less**, Slot saves the current game's automatic resume and battery
save and shows **Battery Low: Game Saved** after a successful write. It does this
once per game launch until charging or recovery above 15% resets the warning.
Failed saves show **Battery Low: Save Failed**, log the error, and retry after
30 seconds. Link play defers this early save until the link ends; loading games
wait until they are ready. Manual save slots and saved history are unchanged.
At **5% or less**, Slot attempts a final save and powers off, even with Never
selected. Charging and Full suppress low-battery saving and shutdown.

This is Slot's existing **software standby**: the game pauses and the backlight
turns off while Slot remains running until shutdown. It does not enter kernel
suspend-to-RAM. BaseOS deep sleep changes SP wake behavior to Power and needs a
separate wake/timer integration. Physical battery and standby checks are listed
in [POWER.md](POWER.md).

## Save states

While playing, tap **Menu** to pause and open the state menu. **Save State** and
**Load State** offer four numbered slots per game and emulator core. Use Up/Down
to pick a slot and A to save or load it. Slots show **Empty** or **Saved**; saving
to an occupied slot replaces only that slot. Empty slots cannot be loaded.
B or **Back** returns to the state menu; another Menu tap resumes the game.
Press X in the slot picker within 30 seconds to undo the latest save or load,
including restoring the prior state after an overwrite.

Existing timestamped saves are preserved under **Load State → Saved History**.
Choose one with Left/Right and A. Double-tapping Menu still opens this history.
The Select + R1 (save) and Select + L1 (load newest) shortcuts are unchanged.
They use the separate history, which keeps the ten most recent states per game.
On desktop, Menu is **Tab**.

## Run ahead

The same menu has a **Run Ahead** row. Press A, Left or Right on it to choose
**Off** (default), **1 Frame** or **2 Frames**. Run ahead hides input lag that is
built into a game by showing the picture one or two frames early. The choice is
saved for that game only, in `Config/run_ahead.txt`, and applies as soon as the
menu closes.

Each frame of run ahead makes the device emulate one extra frame per screen
refresh, so a demanding game can slow down or crackle. Turn it back down if it
does. A setting higher than the game's own lag shows as skipped animation frames
when a button is pressed. Run ahead pauses itself during fast-forward, rewind and
link sessions. It is available on PlayStation, where it is the most expensive.

## External controllers and docking

USB gamepads can connect and disconnect while Slot is running. Use the handheld's
USB data/OTG port and a suitable hub for the controller or its wireless receiver.
Built-in buttons remain available, and disconnecting a controller releases its
held buttons. One external controller controls player one alongside the handheld.

Every connected controller can drive the menus. In a game, each controller
plays as one player, and any controller's Guide button opens the menu.

To choose who plays as whom, open **Menu → Controllers → Players** on the cart
shelf, or tap **Menu** during a game and choose **Players**. The handheld and each controller have a row of their own under four
columns, **1** to **4**. Press Left or Right **on your own controller**, or on
the handheld's own D-pad for the handheld, to move it to the player you want;
to swap players one and two, each moves to the other's column. A or B goes
back. Everything starts as players 1, 2, 3 and 4 in the order it connects: the
handheld and the controller chosen under **Controller** are player 1. Two in one
column share that player. The choice lasts until the controllers are unplugged
or Slot restarts.

Players beyond the first work in **PlayStation and SNES games**. Two players use
the console's two controller ports. Moving a controller to player 3 or 4 plugs a
multitap into port 1, which is how four-player PlayStation games expect it;
some two-player games do not work with a multitap attached, so move back to 1
and 2 for those. SNES uses a multitap in **port 2** when anyone selects player
3 or 4, and switches back to two standard pads when everyone selects 1 or 2.
The game must support multiplayer or a multitap. Two controllers of the same model work together and share one
saved profile. Game Boy games stay single-player; use the link screen for
those.

From the cart shelf, open **Menu → Controllers**:

- **Controller**: use Left/Right to choose Auto or a connected pad. Auto selects
  the first pad and waits for that pad if it disconnects, keeping other pads
  from taking over. An explicit selection is remembered across restarts.
- **Swap A / B** and **Swap X / Y**: swap those buttons on the selected external
  controller, including menu confirm/back. Handheld buttons keep their mapping.
- **Stick Dead Zone**: choose 10%, 15%, 20%, 25% (default), 30%, or 40%. This affects
  the left stick's digital directions; the D-pad is unchanged.
- **Pause if Lost**: On by default. A docked game pauses while its controller is
  missing and resumes when it reconnects. A docked game started without a pad
  also waits. Press **Start on the handheld** to resume with built-in controls;
  this press is consumed so it does not also press Start in the game.
- **Reset Profile**: press A to restore the selected pad's mapping and dead zone.

Profiles save automatically in `Config/controllers.conf` and apply after releasing
held inputs. Connect a controller before editing its profile. Profiles use the
reported device identity; identical models without serial numbers share settings.
Switching USB modes can report a different identity. Handheld play continues
normally when unplugging a pad. Link sessions continue without disconnect pausing.

For an **8BitDo Ultimate**, use a USB cable or the 2.4 GHz receiver in **X-input**
mode (**X** on variants with an X/D switch). The firmware must recognize the pad
and expose a Linux input device; controller detection and HDMI switching still
need verification on RG SP hardware. Bluetooth pairing is managed by the OS.
This change does not add a pairing menu or firmware drivers.

By default, buttons follow the handheld's physical positions, regardless of the printed
Xbox or Nintendo labels:

| External control | Slot control |
|------------------|--------------|
| D-pad or left stick | Directions |
| East / right face button | A (confirm) |
| South / bottom face button | B (back) |
| North / top face button | X |
| West / left face button | Y |
| LB / RB, LT / RT | L1 / R1, L2 / R2 |
| Start / Back (or + / −) | Start / Select |
| Guide / Home or left-stick click | Menu |

Leave **Video Out → Output** on **Auto** for handheld-to-TV play. Connect the hub
and HDMI cable, wait for the saved game to reload, then use the external pad.
Unplug HDMI to resume on the handheld screen. Keep the lid open; closing it
still puts Slot to sleep. HDMI audio continues to use the existing device output.

If a controller is not detected, enable `SLOT_TRACE_INPUT=1` when launching Slot
and reconnect it. `input-trace.log` at the SD card root records discovered devices
and mapped events; `slot.log` records connections and disconnections. A pad that
never appears in `/dev/input` requires OS/driver support before Slot can use it.
Desktop builds also support gamepads, left-stick directions and unplugging;
USB access from WSL depends on passing the device through to Linux.

The remaining docking work and hardware checks are tracked in [DOCKING.md](DOCKING.md).

---

## Device Info

From the cart shelf, choose **Menu → Device Info** to open the bundled Go app
built with Gabagool. It shows hardware, OS, CPU, memory, battery, framebuffer and
SD-card storage information. Up/Down scrolls, A refreshes, and B returns to the
system menu. Device Info is a system-menu app and never appears as a cartridge.

The device build bundles the app and its SDL runtime automatically. For a desktop
preview, run `task app:device-info:host` before starting Slot. See
[apps/device-info/README.md](apps/device-info/README.md) for build and test details.
H700 display, controls, lid/power and HDMI handoff still need hardware validation.

# Hardware build (Ubuntu / WSL)

Start Docker Desktop with Linux containers and WSL integration enabled for Ubuntu,
then run from the repository root in Ubuntu:

```sh
bash setup-device.sh
```

The script installs missing build prerequisites and Go Task, checks ARM64 container
support, then builds Slot and all four emulator cores into `dist-device`.
Repeated runs reuse build caches and clean `dist-device` after a successful build,
removing stale files before copying the new bundle. Keep games, BIOS, saves and
settings on your SD card, outside this generated folder. A failed build leaves
the previous bundle intact. Native ARM64 Ubuntu/Debian builds install Rust instead
of using Docker for Slot itself. The Device Info app uses Docker on both host
architectures. An x86 Linux host needs a working Docker daemon with ARM64 emulation.

Power off the handheld and merge `dist-device/System/` into **SD2's `System/`**,
replacing software files. Keep the card's `Games`, `BIOS`, `Config`, `Saves` and
`States` folders, and leave **BaseOS on SD1** in place. The script prints docking
test steps when it finishes; see [DOCKING.md](DOCKING.md) for the hardware checklist.

# Build and run locally (Ubuntu / WSL)

The desktop build runs the same frontend in a window, with the keyboard standing
in for the handheld's buttons. It needs Ubuntu or Debian, including Ubuntu in
WSL2 on Windows (with WSLg for the window and sound). Nothing is installed on,
or copied to, the handheld.

## 1. Build and launch

From the repository root, in an Ubuntu terminal:

```bash
bash setup-desktop.sh --run
```

On the first run the script:

1. installs any missing system packages with `apt` (it asks for sudo only then),
2. installs the Rust toolchain pinned in `rust-toolchain.toml`,
3. builds the desktop app and the mGBA, PCSX ReARMed and Snes9x cores for your computer,
4. creates the `sdcard/` folder that stands in for the SD card, and
5. opens the Slot window.

The first build takes several minutes. Later runs reuse the build caches and keep
your ROMs, saves, BIOS files and settings. Leave off `--run` to build without
launching. Desktop build output lives in `target/desktop`, separate from the
device build.

## 2. Add games

Put unzipped ROMs in `sdcard/Games/GBA`, `sdcard/Games/GB` or `sdcard/Games/GBC`,
PlayStation images in `sdcard/Games/PSX`, and SNES ROMs in `sdcard/Games/SNES`,
then start Slot again. Game Boy
games use the mGBA core on the desktop; gpSP is not built by this script.

## 3. Controls

| Key | Button |
|-----|--------|
| Arrow keys | D-pad |
| Z / X / C / V | A / B / X / Y |
| A / S | L1 / R1 |
| Q / W | L2 / R2 |
| Enter | Start |
| Right Shift | Select |
| Tab or backtick | Menu |
| `=` / `-` | Volume up / down |
| Backslash | Power |
| `]` | Lid |

USB gamepads also work when the computer (or WSL) can see them.

While playing, tap **Menu** (desktop: **Tab**) to pause and open the in-game menu.
Select **Rewind** or **Fast Forward** and hold **A** (desktop: **Z**) to show the
game and run that mode. Release A to return to the paused menu; **B / Back**
resumes normal play. Playback uses these menu controls on every system, including
PlayStation. Fast-forward uses the speed and sound choices in the shelf's
**Menu > Game** settings; the Rewind setting enables or disables recording.
Sleep, ejecting the cart, or a controller disconnect that pauses the game stops
playback. L2/R2 and Menu + L2 no longer activate playback shortcuts.

## 4. After changing the code

Run the same command again; only what changed is rebuilt:

```bash
bash setup-desktop.sh --run
```

To run the tests, use the Rust toolchain the script installed:

```bash
cargo test --workspace
```

Tests that load a real emulator core expect one built for your computer in
`vendor/`. If that folder holds the ARM64 device cores from a hardware build,
those tests fail on an x86 computer; the rest are unaffected.

## Troubleshooting on WSL

WSL launches default to X11 with software rendering to avoid Wayland and GPU
driver failures. This uses more CPU than hardware rendering. If your WSL graphics
drivers work, use `bash setup-desktop.sh --run --native-graphics` to keep the normal
display and graphics configuration. Native Linux launches keep their normal graphics.
The script also points Slot's audio at WSLg without changing your personal ALSA
settings.

If a graphics failure left WSLg unresponsive, save your work in WSL, run
`wsl --shutdown` in Windows PowerShell, reopen Ubuntu, and retry the script. Shutdown
stops all running WSL distributions, including WSL sessions used by Docker.

## PlayStation games and disc swapping

Put `.chd`, `.cue` (alongside all referenced `.bin` tracks), `.pbp`, `.iso`, or
homebrew `.exe` files in `Games/PSX`. PCSX ReARMed is selected automatically.
Raw `.bin` tracks are hidden from the shelf. Optional square cover art goes in
`Labels/PSX` with the same stem as the game or playlist and a `.png` extension.

For multi-disc games (up to eight discs), create a `.m3u` playlist in `Games/PSX`, with one relative
disc filename per line, in disc order. For example, `Adventure.m3u` contains:

```text
Adventure (Disc 1).chd
Adventure (Disc 2).chd
```

The playlist appears as one game; its individual discs are hidden from the shelf.
It shares one memory card and resume history across all discs. While playing,
hold **Select** and press **Menu**, then choose **Previous Disc** or **Next Disc**
with Up/Down and A. B or Back closes the menu. On desktop this is **right Shift +
Tab**, arrows, **Z** to confirm, and **X** to go back. Disc changes keep the game
loaded; perform them when the game prompts you. Resume states restore the selected
disc. Keep the playlist's disc order unchanged after saving.

Supply your own PlayStation BIOS in `BIOS` (for example `scph5500.bin`,
`scph5501.bin`, or `scph5502.bin`). PCSX ReARMed falls back to its built-in HLE BIOS
when no compatible file is found; a real BIOS is recommended for compatibility.
See the [PCSX ReARMed documentation](https://docs.libretro.com/library/pcsx_rearmed/).
Memory cards use the existing save system under `Saves/PSX`; resume states and
screenshots use `States/PSX`.

This version uses digital controllers (up to four players with external pads) and 4:3 video. A/B/X/Y map to
Circle/Cross/Triangle/Square; L1/R1 and L2/R2 are sent to the game. Desktop keys
are Z/X/C/V, A/S, and Q/W respectively. Rewind and fast-forward use the same
in-game menu controls described above; the triggers stay available to the game.
From the shelf, **Menu > Game > Rewind** turns recording on or off.
PS1 captures ten snapshots per second and replays about twice as fast, with the
existing 20 MiB compressed history budget; the available duration depends on the
game. Captures are skipped if compression falls behind. Loading a state or
changing discs clears history. Audio is silent while
rewinding. PS1 rewind performance still needs testing on the handheld.
PlayStation disables the Game Boy turbo and LCD shader behavior. Analog sticks are
not implemented. The x86 Linux and macOS desktop cores use the interpreter; the ARM64 device core
uses the recompiler. Performance and game compatibility still need device testing.

`task dist:device` builds and bundles the PlayStation core and its corresponding
source archive. After updating the repo, rerun `bash setup-desktop.sh --run` for
desktop testing. The new core build is cached on subsequent runs.

---

## Super Nintendo

Put unzipped `.sfc`, `.smc`, `.swc` or `.fig` ROMs in `Games/SNES`. Snes9x is
selected automatically; normal cartridge games need no BIOS. Optional label art
goes in `Labels/SNES/<game stem>.png`. Desktop setup and device bundles build the
same pinned core, with its source archive and non-commercial license included.

SNES uses the same four manual save slots, save history and undo, automatic
resume, rewind, fast-forward, run ahead, controller profiles, Players menu,
volume, sleep and HDMI behavior as the other systems. Battery saves go in
`Saves/SNES`; states go in `States/SNES/snes9x`. A/B/X/Y and L1/R1 are ordinary
SNES game buttons, including when Game Boy turbo is enabled. Use the in-game
menu for rewind and fast-forward. Two players use standard controller ports;
players 3 or 4 automatically attach a port-two multitap for compatible games.
Slot exposes up to four players. Game Boy link features do not apply to SNES.

Output uses a 4:3 canvas and supports NTSC, PAL, hi-res and interlaced games.
Run ahead adds CPU cost; performance, special-chip games and physical controller
operation still need verification on RG SP hardware.

To run the real-core homebrew regression after building Snes9x:

```sh
SLOT_TEST_SNES_CORE="$PWD/target/desktop/snes9x_libretro.so" \
  cargo test -p slot-retro --test snes --target-dir target/desktop
SLOT_TEST_SNES_CORE="$PWD/target/desktop/snes9x_libretro.so" \
  cargo test -p slot --test snes --target-dir target/desktop
```

# AI Disclosure

The Rust frontend was put together by Claude Opus. I reviewed everything that was
produced. All documentation is 100% free-range, meatbag prose.

The project is extremely low stakes. I wanted a bespoke frontend for my RG SP and thought
that something that evokes the feeling of using my GBA SP as a kid would be pretty neat.

Use it, don't use it, I don't care.

Figured I should share the end result of all the wasted water. ✌🏻
