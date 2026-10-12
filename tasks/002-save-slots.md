# Four manual save-state slots

Expand Save State / Load State to offer four numbered manual saves per game and
core. Keep existing timestamped saves available, preserve automatic resume, and
support loading, overwriting and undoing manual saves. Empty slots must be clear
and must not load. Preserve all existing workspace changes.

Status: complete. User confirmed four selectable save slots.

Save State and Load State open a numbered picker with Empty/Saved indicators.
Each game/platform/core keeps four atomic slot files outside the existing
timestamped history and resume file. Saving replaces only the selected slot;
loading an empty slot or failing a write leaves the picker open and game paused.
X undoes the latest save/load within the existing 30-second grace period,
restoring overwritten slot data. The picker displays the available undo hint.

Existing states remain under Load State → Saved History, with double-Menu and
Select shortcuts preserved. README usage instructions are updated.

Validation: 20 storage tests, 18 state-menu tests, and 109 existing app/power/undo
tests passed. Scoped host Clippy (libraries, binaries and tests), device Clippy,
formatting and diff whitespace checks passed. Hardware deployment was not run.
