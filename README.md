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

Supports GBA, GBC, GB, and original PlayStation (PS1) titles.

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
scroll. It starts Off, and your choice is saved across restarts for all shelves.
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

To preview HDMI scaling on a computer, run
`SLOT_HDMI_PREVIEW=1 SLOT_WINDOW=1280x720 bash setup-desktop.sh --run`.
The preview never changes the computer's display configuration.

## Save states

While playing, tap **Menu** to pause and open the state menu. **Save State** adds
a new state (the ten most recent are kept per game), **Load State** opens the saved
states to pick one with Left/Right and A, and **Back**, B or another Menu tap
returns to the game. Double-tapping Menu still jumps straight to the saved states.
The Select + R1 (save) and Select + L1 (load newest) shortcuts are unchanged.
On desktop, Menu is **Tab**.

## External controllers and docking

USB gamepads can connect and disconnect while Slot is running. Use the handheld's
USB data/OTG port and a suitable hub for the controller or its wireless receiver.
Built-in buttons remain available, and disconnecting a controller releases its
held buttons. One external controller controls player one alongside the handheld.

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

# Hardware build (Ubuntu / WSL)

Start Docker Desktop with Linux containers and WSL integration enabled for Ubuntu,
then run from the repository root in Ubuntu:

```sh
bash setup-device.sh
```

The script installs missing build prerequisites and Go Task, checks ARM64 container
support, then builds Slot and all three emulator cores into `dist-device`.
Repeated runs reuse build caches and preserve existing games, BIOS, saves and
settings in that folder. Native ARM64 Ubuntu/Debian builds install Rust instead
of using Docker. An x86 Linux host needs a working Docker daemon with ARM64 emulation.

Power off the handheld and merge `dist-device/System/` into **SD2's `System/`**,
replacing software files. Keep the card's `Games`, `BIOS`, `Config`, `Saves` and
`States` folders, and leave **BaseOS on SD1** in place. The script prints docking
test steps when it finishes; see [DOCKING.md](DOCKING.md) for the hardware checklist.

# Desktop testing (Ubuntu / WSL)

Run from an Ubuntu or Debian terminal, including Ubuntu in WSL on Windows:

```bash
bash setup-desktop.sh --run
```

The script installs missing system prerequisites (using sudo when needed), installs
the pinned Rust toolchain, builds the desktop app, patched mGBA and PCSX ReARMed cores, and prepares
`sdcard/`. Omit `--run` to set up without launching. It is safe to rerun: Cargo reuses
its build cache, an up-to-date mGBA core is reused, and existing ROMs, saves, BIOS files
and settings are preserved. Desktop build files are kept separate from device builds.

Put unzipped ROMs in `sdcard/Games/GBA`, `sdcard/Games/GB` or `sdcard/Games/GBC`, and
select mGBA for Game Boy gameplay. PlayStation images go in `sdcard/Games/PSX`.
WSL launches need WSLg; the script configures WSL
audio locally without changing your personal ALSA settings.

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

This version uses a single digital controller and 4:3 video. A/B/X/Y map to
Circle/Cross/Triangle/Square; L1/R1 and L2/R2 are sent to the game. Desktop keys
are Z/X/C/V, A/S, and Q/W respectively. PlayStation disables the Game Boy turbo,
rewind, fast-forward, and LCD shader behavior. Analog sticks and multiplayer are
not implemented. The x86 Linux and macOS desktop cores use the interpreter; the ARM64 device core
uses the recompiler. Performance and game compatibility still need device testing.

`task dist:device` builds and bundles the PlayStation core and its corresponding
source archive. After updating the repo, rerun `bash setup-desktop.sh --run` for
desktop testing. The new core build is cached on subsequent runs.

WSL launches default to X11 with software rendering to avoid Wayland and GPU
driver failures. This uses more CPU than hardware rendering. If your WSL graphics
drivers work, use `bash setup-desktop.sh --run --native-graphics` to keep the normal
display and graphics configuration. Native Linux launches keep their normal graphics.

If a graphics failure left WSLg unresponsive, save your work in WSL, run
`wsl --shutdown` in Windows PowerShell, reopen Ubuntu, and retry the script. Shutdown
stops all running WSL distributions, including WSL sessions used by Docker.

---

# AI Disclosure

The Rust frontend was put together by Claude Opus. I reviewed everything that was
produced. All documentation is 100% free-range, meatbag prose.

The project is extremely low stakes. I wanted a bespoke frontend for my RG SP and thought
that something that evokes the feeling of using my GBA SP as a kid would be pretty neat.

Use it, don't use it, I don't care.

Figured I should share the end result of all the wasted water. ✌🏻
