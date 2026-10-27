# mitcad-import-appearances

The appearances family of the `.f3d` dump replay (mitcad#228): after the
replay, the file's appearances and physical materials on the replay's
bodies, faces and occurrences. Each asset becomes one document appearance
on MitCad's parameters; a body gets its stored body's appearance, a face
its own where the replay's body has faces at the stored face's point, an
occurrence's override goes to the bodies of a component placed once, and a
body gets MitCad's material where the file's is one of MitCad's by name and
density. The module docs (`src/appearances.rs`) describe the rules.

It sits above the replay's core
([mitcad-import-replay](../import-replay/README.md), mitcad#791): its
method is the extension trait `Appearances` on the core's `Importer`,
which the core reaches only through its `Families` trait (`appearances`),
filled by `mitcad-import`. The report's types (`AppearanceReport` and its
entries) are the core's (`appearance_report`), since `DesignReport` holds
them; this crate re-exports them, and `mitcad-import` re-exports the crate
as `appearances`, so `mitcad_import::appearances::...` keeps working. An
edit here rechecks this crate and `mitcad-import`, not the core.

Tests: `cargo test -p mitcad-import-appearances` (ctest
`core.import-appearances`) runs the module's unit tests; the replay's tests
with appearances are `mitcad-import`'s (`tests::appearance_tests`,
`core.import`).
