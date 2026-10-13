# Center menu text vertically in its highlight

Correct the vertical alignment of labels and highlight bars in the menu opened
from the cartridge carousel. Keep labels, values and carets on the same baseline.

Status: complete. Requested by the user.

The main menu's 38-pixel rows retained the fixed text offset used by 52-pixel
submenu rows. Rendered glyph measurements showed the main menu's text center
six pixels below its highlight center. Center the 40-pixel text texture within
the current row pitch and account for the font's one-pixel optical offset.

Validation: 10 UI menu tests and 29 app menu tests passed, along with formatting
and whitespace checks. A temporary raster preview measured the main, Music and
Screen page labels exactly at their highlight centers after the fix. Inspected
the main page and Screen submenu, including values and carets. The temporary
harness was removed; before/after previews remain under `target/quick-menu-*.png`.
Rebuild the device bundle to deploy the adjustment.
