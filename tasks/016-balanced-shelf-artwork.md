# Smaller, balanced game artwork

Make the console shelves' game artwork about 10% smaller, while preserving
the relative sizes and each system's cartridge or case proportions.

Scale GBA, GB/GBC, PlayStation and SNES together in the artwork dimensions so
shelf spacing, Cover Flow, shadows and cartridge insertion use the same sizes.
Keep the seated dimensions appropriate to the slot. Inspect a comparison preview
and run relevant checks.

Status: complete. Requested by the user.

Shelf sizes are now GBA 324×190, GB/GBC 281×303, PlayStation 281×281 and
SNES 324×203. PlayStation and SNES labels and SNES shell details scale with
their artwork. Existing artwork caches rebuild when label dimensions change.

Validation: all 229 focused tests passed across cartridge artwork, shelf,
Cover Flow, chrome, board, SNES, app, core picker, face loading and label cache.
Formatting and whitespace checks passed. Generated and visually inspected
`target/shelf-size-preview.png`, then removed the temporary preview harness.
