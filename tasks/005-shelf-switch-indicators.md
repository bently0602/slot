# Shelf category switch indicators

Add L and R button indicators to the top left and top right of the shelf screen,
with outward arrows indicating previous and next shelf categories. Match the
existing button hint style. Show them on console shelves and music covers; hide
them in menus, cartridge insertion/playback, the core picker, music tracks, and
the music visualizer where shelf switching is unavailable.

Status: complete. Requested by the user.

The frontend uploads matching L/R button badges with outward carets. The shelf
renderer places them 24 pixels from the top and side edges. Visibility follows
the shoulder controls on console shelves and the music cover browser.

Validation: 180 focused tests passed across shelf hints, app, core picker, music,
quick menu, chrome, legend, and plate suites. Formatting and whitespace checks
passed. The generated button textures were visually inspected. Hardware display
verification was not run.
