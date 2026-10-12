# SNES support

Add SNES as a shelf using a pinned Snes9x libretro core. Preserve the shared
save RAM, four manual save slots, history/undo, resume, rewind, fast-forward,
run ahead, controller profiles and Players assignment features. Support two
console ports and automatic port-two multitap for players three and four.
Include the core in desktop and device builds, with source and license notices.
Preserve existing workspace changes.

Status: complete. Physical RG SP validation remains pending.

SNES ROMs and labels have their own shelf and directories. Snes9x is the platform
default, and incompatible saved core choices fall back to it. The shared session
provides all four manual slots, state history/undo, automatic resume, battery
SRAM, rewind, fast-forward and run ahead. X/Y stay native SNES buttons. Players
assignment routes four independent pads; choosing player 3 or 4 attaches a
multitap to console port 2 and returning to players 1/2 removes it. Video uses a
4:3 canvas with PAL/NTSC timing and hi-res/interlace support. A wide cartridge
face supports generated labels and supplied artwork.

Desktop setup, macOS Task builds, ARM64 device bundles and release packaging
include the pinned core, corresponding source and Snes9x license. README covers
ROM placement, controls, persistence and real-core test commands.

Validation:

- Built the pinned Linux desktop core; tested the build cache and source archive.
- Cross-compiled the identical source/platform recipe into an ARM aarch64 shared
  library. The native ARM container build was stopped after slow QEMU compilation;
  the cross-build verified device compilation instead.
- Real, original homebrew tests cover SNES video/audio, PAL/NTSC, hi-res/interlace,
  two ports and four-player multitap, SRAM, state round trips, player assignment,
  all four manual slots, load/undo, run ahead, rewind, fast-forward and resume.
- 509 core/storage/UI tests and 212 frontend regressions passed. Frontend session
  fixtures used an explicit missing SLOT_CORE to select the intended mock; real
  SNES tests used the actual Snes9x library. Inspected the generated cartridge PNG.
- Host lint for Slot, UI, storage and libretro libraries/binaries/tests, device
  lint, formatting, whitespace, setup smoke tests and all card script tests passed.
- Worker tests initially encountered three timing-sensitive failures under
  concurrent builds. Their isolated budget and fast-forward rechecks all passed.

Full workspace checks are not Linux-clean: the recording example and graphics
pipeline tests import the macOS-only HeadlessSurface. Existing real-mGBA fixture
tests also encountered an unavailable vendor library while a separate device
build was running. Those unrelated files and builds were preserved. No handheld
deployment or physical controller/performance verification was performed.
