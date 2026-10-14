# Recover a stuck game from the in-game menu

Status: implementation in progress. Requested by the user.

Add Reset Game immediately above Back in the menu shown by tapping Menu while
playing. Restart the current cartridge cleanly, bypassing saved-state resume,
and keep existing game saves, manual slots and state history on disk.

Emulator snapshot requests currently wait forever. Bound those waits so a
stalled serialize call releases the UI. Recovery replaces the Slot process
without joining or unloading the wedged native core or saving its broken state.
Reload only the last successfully persisted in-game save. Progress since that
save is lost. Preserve BaseOS's explicit loader, card root, and selected console;
the reset flag applies only to this boot and is removed on later restarts.

Validate menu layout/navigation, one-shot reset requests, unchanged state/save
files, clean startup of the correct cartridge, loader invocation and bounded
waits with an unresponsive emulator. Physical handheld validation is pending.
