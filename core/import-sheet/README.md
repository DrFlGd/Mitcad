# mitcad-import-sheet

The sheet-metal family of the `.f3d` dump replay (mitcad#211): the
decoded sheet items of the dump (`.ipt` and `.f3d` alike) as MitCad's
native sheet features, and roundings and chamfers tried first as the sheet
corners they round or bevel. The module docs (`src/sheet.rs`) describe
what comes in how; the notes are in
[core/import/docs/f3d-sheet-metal.md](../import/docs/f3d-sheet-metal.md).

It sits above the replay's core
([mitcad-import-replay](../import-replay/README.md), mitcad#785): its
methods are the extension trait `Sheets` on the core's `Importer`, which
the core reaches only through its `Families` trait (`sheet_item`,
`dressups_on_sheets`), filled by `mitcad-import`. An edit here rechecks
this crate and `mitcad-import`, not the core.

Tests: `cargo test -p mitcad-import-sheet` (ctest `core.import-sheet`)
runs the module's unit tests; the replay's tests with sheet items are
`mitcad-import`'s (`core.import`).
