# Move console-shelf music status to the top

Place music playback status at the top center between the L/R category hints.
Use a play/pause icon and track title with a smaller `Y · Now Playing` hint,
truncate long titles with an ellipsis, and give both lines enough room to avoid
clipped glyphs. Keep the Music browser's controls and layout intact.

Status: complete. Requested by the user.

Console shelves have a separate 520×48 status texture at (100, 24), with a
play/pause icon, a title ending in an ellipsis when needed and the smaller
Now Playing shortcut. The status is hidden in the core picker and game menu.
Tall cartridges move down just enough to clear the header; the slot, category
name, battery and clock retain their positions. The Music browser still uses
its existing footer and playback controls.

Validation: formatting and whitespace checks passed. All 169 focused tests
passed across app, music, shelf hints, shelf geometry, coverflow and chrome.
Generated and inspected previews for GBA/playing, GB/paused with a long title,
and PlayStation/playing. A temporary preview harness verified text has blank
top/bottom margins and cartridge artwork clears the header, then was removed.
The previews remain in `target/music-status-*.png`. Rebuild the device bundle
before deploying this change.
