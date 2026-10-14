# Turn screen off from slot selection

Add Turn Screen Off to the main shelf menu. Blank the display and turn off the
handheld backlight while keeping music playing. Any button restores the screen
at its previous brightness; consume the wake press so it cannot launch a game
or change the selection. Keep lid sleep and the power menu working normally.

Status: complete. Requested by the user.

Turn Screen Off appears directly below Screen in the main menu. Compact main
menu rows fit all eleven options above the shortcut legend. The screen-off
state is temporary and separate from lid sleep; the saved brightness stays
intact. Wake presses are consumed through release, including held Menu/Power
buttons. Opening the lid restores the display; normal lid sleep still applies.

Validation: 178 focused tests passed across app, menu, music, power, docking,
docked-lid audio and menu artwork suites. The two screen-off integration tests
also passed after the final input cleanup. Formatting and whitespace checks
passed. Generated and inspected `target/screen-off-menu-preview.png`, then
removed the temporary preview harness. Physical handheld testing was not run.
