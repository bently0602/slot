# Restore screen effects for all game systems

Status: implemented and packaged; physical H700 visual acceptance pending.

The user reports missing game overlays and confirmed this means the screen
effects (LCD3x, Grid, Dot), requesting them for every system.

Restore selectable Off, LCD3x, Grid, Dot, and Simpletex for GBA, GB/GBC, SNES,
and PlayStation. Add SNES and PlayStation rows under Menu > Screen and make the
existing shader shortcuts work for those systems. Save independent preferences
for GBA, GB/GBC, SNES, and PlayStation. Preserve existing saved choices and
defaults; new console preferences start at Off.

Verify menu navigation, cycling in both directions, persistence and old settings
files, active-platform selection, and rendering at handheld and console frame
sizes. Prepare a separate optimized H700 update package including the existing
run-ahead fixes. Do not replace installed SD-card files or unrelated changes.

## Changes

- `SlotState` now stores `shader_snes` and `shader_psx`. Missing or invalid console
  keys default to Off without changing existing preferences. Every shader name is
  accepted for GBA and GB/GBC too, using one shared five-effect choice list.
- Menu > Screen has SNES Shader and PlayStation Shader rows. Menu navigation,
  value display, arrow cycling, active-game routing, and both shader shortcuts
  support all five platforms. GB and GBC retain their shared preference.
- The existing compositor already supports these effects for both framebuffer
  sizes. No renderer or emulator-worker changes were required for this fix.

## Validation and package

- Store durability/persistence: 23 tests passed, including every effect on every
  setting, old files without console keys, unknown console values, and independent
  persistence.
- UI menu: 10 tests passed. App quick-menu: 32 tests passed, including all four
  shader rows, navigation/wrapping, restart persistence, and shortcuts for every
  platform. The three existing app shader-shortcut regressions also pass.
- Changed library Clippy targets pass with the existing unrelated PSX range-check
  and Bluetooth type-complexity warnings excluded, using `--no-deps`.
- A Mesa EGL readback probe compiled the repository's actual RECT_VERT/GAME_FRAG
  programs and rendered Off, LCD3x, Grid, Dot, and Simpletex at 240x160 and 640x480.
  Every enabled effect changed the output and the console side borders remained
  black. No OpenGL error occurred. Probe: `target/tmp/check-screen-effects.py`.
- The optimized ARM64/glibc-2.31 device build passes ELF inspection and an ARM64
  Bullseye `--init-root` startup check. The package checksum and ZIP integrity
  were verified.

The update is `target/slot-h700-screen-effects-test.zip`, containing System/slot,
installation/rollback instructions and SHA256SUMS. It includes the prior run-ahead
changes. No SD-card files were replaced. Hardware appearance and performance
remain for the user to check after selecting each desired effect.
