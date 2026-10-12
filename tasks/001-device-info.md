# Device Info from the system menu

Add a small Go application using Gabagool, opened with Menu → Device Info from
the shelf. It must not appear as a game cartridge. Show read-only Linux hardware,
OS, CPU, memory, battery, display and SD-card storage details; A refreshes and B
returns to the same system-menu row. Missing metrics should say unavailable.

Slot must release graphics, audio and input before handing off, and recover after
normal exit, startup failure or app crash. Bundle the ARM64 app and its SDL runtime
with the device build; provide a desktop build for development. Preserve existing
workspace changes.

Status: implementation complete; handheld hardware validation pending.

Implemented the system-menu row, Go/Gabagool app, process handoff and return to
the same menu row. Device distribution/deployment tasks include the ARM64 binary,
private Mali SDL runtime, license notices and corresponding sources. A desktop
Linux build is available for development.

Validation passed: ARM64 and desktop Go builds/tests, packaged headless runs,
desktop UI render/refresh/scroll/back checks, Slot menu/app/power tests, scoped
Rust Clippy and device checks, formatting, card-script and setup-device smoke
tests. Workspace all-target Clippy still encounters the pre-existing macOS-only
HeadlessSurface import in examples/record.rs. A complete SD-card distribution was
not rebuilt or deployed during this task.

Still to validate on H700: framebuffer output, physical buttons, lid/power events,
and HDMI handoff.
