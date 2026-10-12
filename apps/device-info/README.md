# Device Info

A read-only Go app built with Gabagool v2.26.4. Open it from the cart shelf with
**Menu → Device Info**. Up/Down scrolls, A refreshes, and B returns to the system
menu. It is not scanned as a cartridge and does not run alongside a game.

The app reports the BaseOS model, Slot version/build, OS, kernel, CPU, available
memory, system uptime, battery, framebuffer size and free space on the content
card. Missing Linux metrics display `Unavailable`. Metrics refresh on A.

Slot releases its EGL surface, audio and input before replacing itself with
`System/device-info.sh`. The script runs the app with a private library path,
then restarts Slot on the Device Info menu row, including after app failure.
Errors are recorded in `device-info.log` and `device-info-ui.log` at the card root.
Lid closure returns to Slot to use its existing lid policy. Releasing Power
returns to Slot and sleeps; this app does not implement its own power menu.

## Device build

From Linux/WSL with Docker running:

```sh
task app:device-info:device
# Or include the app in the normal complete device bundle:
bash setup-device.sh
```

On x86 Linux, the device app uses an ARM64 cross compiler; native ARM64 hosts
compile it directly. Both paths use a pinned SDL Mali fbdev source revision with
Debian Bullseye companion libraries, bundled privately under `System/device-info/lib`. BaseOS's
loader, glibc and EGL/GLES drivers remain in use. License notices and corresponding
app, Go dependency and SDL sources are included under `System/device-info/licenses`. Building does not
change BaseOS or require SDL libraries to be installed on the card separately.

`task dist:device` packages the app; `task deploy:device` pushes it with Slot.
Hardware testing on H700 is still needed for the framebuffer, built-in button
mapping, lid/power events, and HDMI handoff.

## Desktop build

```sh
task app:device-info:host
bash setup-desktop.sh --run
```

The host task installs the desktop binary/runtime in `sdcard/System` so the same
menu entry works. Keyboard controls match Slot: arrows, Z for A, X or Esc for B.
For headless diagnostics on Linux, run:

```sh
LD_LIBRARY_PATH=target/desktop/device-info/lib SLOT_ROOT=./sdcard \
    target/desktop/device-info/device-info --text
```

The app's Go tests run during either build. Menu and process recovery tests are
part of Slot's Rust tests and `task test:card` respectively.
