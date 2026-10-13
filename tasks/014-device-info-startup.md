# Repair Device Info startup on the handheld

Investigate Device Info failing to open from the carousel menu, with empty
`device-info.log` and `device-info-ui.log` files on the device.

Status: implementation complete; handheld validation pending. Requested by the user.

The packaged SDL config has `SDL_VIDEO_DRIVER_MALI` undefined. In the pinned
SDL fork, the Autoconf Mali check is nested inside KMSDRM and does not include
the Mali source files; the build disables KMSDRM. Gabagool exits without printing
the SDL initialization error, accounting for the empty logs.

Patch the SDL configuration to build Mali independently, verify the driver in
both build headers and the packaged runtime, and preserve the patch alongside
the original SDL source archive. Initialize SDL with explicit error logging in
the app. On the handheld, launch through the ARM64 loader as Slot does so app
files do not require executable bits. Preserve frontend restart behavior.

Reproduced the reported behavior with the prior ARM64 binary: selecting the
Mali driver exited with status 1 and produced zero-byte application/UI logs.
The rebuilt package enumerates `mali` and attempts framebuffer initialization.
Without hardware it now reports `mali-fbdev: Could not open framebuffer device`
instead of exiting silently.

Validation passed: ARM64 and desktop Go tests/builds, packaged Mali-driver check,
shell syntax and launcher recovery tests, and an actual non-executable ARM64
binary launched through the loader with captured SDL failure and preserved
frontend arguments. Desktop UI render/refresh/scroll/Back tests also passed;
the virtual-display wrapper needed its readiness signal after the X server was
confirmed ready. Inspected the rendered Device Info window.

Prepared `target/device-info-update.zip` (84.1 MiB), containing the launcher,
complete ARM64 app/runtime/license/source tree and installation instructions.
ZIP CRC and required-file checks passed, and its app matches the tested binary.
The user must replace SD2's `System/device-info/` and `System/device-info.sh`
and test opening the app; the handheld is not connected to this workspace.
