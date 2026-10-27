# mitcad-import-replay

The core of the `.f3d` dump replay (also of the `.ipt` import's dumps):
`Importer`'s state, the timeline walk, the translation of features and
operations, references, the history check, fallbacks, retries, the
deviations, the learning dump, the timeline notes and the report. The
crate docs (`src/lib.rs`) describe the replay; the notes are indexed in
[core/import](../import/README.md#documents).

Split from `mitcad-import` (mitcad#785; the map it was cut by is on that
ticket): the families that the core reaches only through a few entry
points live in crates above it, as extension traits on its `Importer`,
and its timeline walk reaches them through its `Families` trait, which
`mitcad-import` fills ([mitcad-import-joints](../import-joints/README.md),
[mitcad-import-sheet](../import-sheet/README.md),
[mitcad-import-primitives](../import-primitives/README.md),
[mitcad-import-appearances](../import-appearances/README.md), mitcad#791;
the appearances' report types stay here in `appearance_report`, since
`DesignReport` holds them).
So the core does not depend on them, and an edit in a family does not
rebuild it. `replay_design` takes the families; callers use
`mitcad-import`'s `import_design` and `import_design_with`, which give
them, and keep the paths `mitcad_import::...` (it re-exports this crate).

`Importer`, its fields and the modules are public for the families and
for the replay's tests in `mitcad-import` (the modules hidden from the
docs).

Tests: the unit tests beside the modules, `cargo test -p
mitcad-import-replay` (ctest `core.import-replay`), on the model's mock
kernel; the replay's tests on synthetic dumps are `mitcad-import`'s
(`core.import`), where the whole replay is.
