# PlayStation case and disc animation

Requested by the user: animate the PSX CD case opening with a CD inside,
then slide the CD into the slot when launching a game.

Use the existing cover as the hinged lid, reveal a jewel-case tray and optical
disc, lift the disc out, then insert only the disc. Give PSX its own animation
duration, preserve instant resume, and reverse the sequence for eject and a
failed launch. Other systems keep their existing animation.

Status: complete.

The cover hinges open to reveal a jewel-case tray and reflective CD. The CD
lifts free in front of the lid, centres over the slot, and slides behind the
housing while the empty case fades. Shared textures are generated once at
startup; the selected game's existing cover remains the outside of the lid.

PSX movement takes 1.5 seconds, followed by the existing 0.28-second launch
hold. Insert audio follows disc seating. Resume skips the animation. Eject and
failed launches reverse the sequence, including cancellation while opening.
Animation timing uses the selected shelf so emulator startup cannot alter it.

Validation: 203 focused tests passed across PSX rendering, cart artwork, chrome,
Cover Flow, app, eject, launch refusal, resume and sound effects. Host tests used
`SLOT_SILENT=1` for the WSL audio environment; the 15 sound-effect tests also
passed with `--no-default-features`. Formatting and whitespace checks passed.
Rendered and inspected the opening, lift and insertion frames. Preview artifacts:
`target/psx-animation.png` (animated) and `target/psx-animation-preview.png`.
The temporary preview example was removed after rendering. Handheld playback
has not been checked.
