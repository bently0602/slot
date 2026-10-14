# Show save-slot dates and times

Status: complete; scope confirmed by the user.

Replace the generic Saved label on occupied numbered save slots with the date
and time that slot was saved. Apply this to both Save State and Load State.

## Behavior

- Show an occupied slot as `Slot 1: 2026-10-14 14:35`, using a compact,
  unambiguous date and 24-hour time in the device's configured UTC offset.
- Keep `Slot 1: Empty` for an unused slot.
- Read the save file's modification time so existing saves gain dates without
  requiring a save-file migration. If a timestamp cannot be read, keep the
  occupied slot available and show `Date Unknown` instead of treating it as empty.
- Refresh labels when the picker opens and after an undo. Saving over a slot
  updates its timestamp; undoing that overwrite restores its original timestamp.
- Preserve slot numbering, per-game/core isolation, saved history, loading,
  saving, and the existing undo grace period.
- Keep each label readable within the existing menu width.

## Implementation and validation

- Extend the slot metadata cached by App when refreshing the save-slot picker.
- Format timestamps using the project's existing civil-date utilities and the
  device's configured offset; avoid adding a date/time dependency.
- Preserve the replaced slot's modification time in the existing undo data.
- Update the save-slot menu tests to cover populated and empty labels, date/time
  formatting across a date boundary, overwrites and undo timestamp restoration.
- Run the focused state-menu, save-storage and undo tests, plus formatting and
  whitespace checks. Check label dimensions against the existing menu layout.
- Update README.md and CHANGELOG.md and preserve unrelated workspace changes.

This extends the four-slot picker introduced in task 002.

## Completed implementation

App caches each slot's file metadata when the picker opens or refreshes, then
formats its saved date and time using the configured device offset. Existing
save files work without migration. Empty slots and Saved History retain their
existing behavior. Undo captures the overwritten file's modification time and
restores it along with the original save bytes.

Validation: all 54 focused tests passed (24 state-menu, 10 undo, 20 save-storage).
Tests cover both menus, empty slots, existing saves, positive/negative and
half-hour offsets, year-boundary rollover, overwrites and timestamp restoration
after undo. Rasterized labels fit within the menu width, and their drawn bounds
stay on screen. Rust formatting and whitespace checks passed. A device-bundle
rebuild and handheld verification were not performed.
