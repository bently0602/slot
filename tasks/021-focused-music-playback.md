# Keep music playback on the Music shelf

Status: complete; scope confirmed by the user.

Requested behavior: remove music playback while browsing other slot categories
and reclaim the space above game artwork. Keep playback available while focused
on the Music category and its album, track and visualizer screens.

## Scope

- Remove the play/pause status, track title and Y / Now Playing shortcut from
  console shelves, including the textures and artwork clearance used for them.
- Pause music when leaving the Music category for a console shelf. Retain the
  loaded album, track and position so the user can return and resume explicitly.
- Keep album browsing, track selection, playback controls and the visualizer
  within Music. Switching between albums within Music remains available.
- Ensure Menu playback actions cannot start audio over another category; opening
  playback controls should focus Music before starting or resuming playback.
- Preserve music settings, screen-off listening, closed-lid listening and the
  existing headphone, sleep and display-restart behavior while focused on Music.
- Update README.md, MUSIC.md and CHANGELOG.md to describe the new behavior.

## Implementation locations

- `crates/slot/src/app.rs`: shelf transitions, Y shortcut, menu playback actions
  and console artwork drawing.
- `crates/slot/src/session.rs`: enforce the playback focus rule during audio
  synchronization, including queued commands and wake/resume paths.
- `crates/slot/src/music_ui.rs`: remove console-only status rendering and texture
  generation while preserving Music's own heading and footer.
- `crates/slot-ui/src/shelf.rs`: remove the music header clearance from artwork
  placement and restore the direct shelf drawing path.
- Existing music and shelf tests: replace cross-category playback expectations
  with pause/resume coverage and verify console art no longer reserves header space.

## Acceptance and validation

- Playing music pauses when L1/R1 switches from Music to a game category.
- Console shelves show game artwork at its normal position without a music banner
  or Now Playing shortcut.
- Returning to Music retains the track and position and waits for explicit resume.
- Playback and navigation still work on Music's cover, track and visualizer screens.
- Menus and automatic resume cannot bypass the category focus rule.
- Run Rust formatting and the focused music, app, menu and shelf rendering tests.
- Preserve the workspace's existing unrelated changes.

This supersedes the console status behavior documented in task 012.

## Implementation

Removed console music status textures, the Y shortcut, and the special header
clearance above cartridges. The session pauses and checkpoints music outside
the Music category, rejects playback-start commands there, and clears pending
sleep resume. Opening Menu → Music focuses Music before its controls can play.
Album browsing, Music's own heading/footer and visualizers remain available.

Added five regression tests covering both shelf-switch directions, explicit
resume at the saved position, queued playback commands, waking on a console,
Music menu focus, and unchanged console artwork with a loaded album.

Validation: 287 focused tests passed across music (28), application controls (77),
library unit tests (60), quick menu (30), shelf hints (1), docking/lid audio (12),
and UI shelf/coverflow/chrome/menu checks (79). Rust formatting and whitespace
checks passed. Validation uses simulated audio and drawing geometry; handheld
playback and a device-bundle rebuild remain to be performed before deployment.
