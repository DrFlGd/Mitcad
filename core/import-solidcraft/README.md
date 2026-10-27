# mitcad-import-solidcraft

The `.solidcraft` import (mitcad#450): a design read by
[`mitcad-solidcraft`](../solidcraft/README.md) and built again as MitCad
features through the model's commands, generic over the model's `Kernel`
(`mitcad-ffi` runs it with OCCT: `core/ffi/src/solidcraft_import.rs`).
The file keeps no bodies to check against, so every feature is evaluated
by MitCad. The crate docs (`src/lib.rs`) describe what comes in how;
[docs/solidcraft.md](docs/solidcraft.md) holds the notes.

It has an importer of its own, not the `.f3d` dump replay's, and depends
only on `mitcad-import-base` (vector helpers, the timeline notes' finish),
not on the replay (mitcad#785). `mitcad-import` re-exports it as
`mitcad_import::solidcraft`, so its callers keep that path.

Tests: `cargo test -p mitcad-import-solidcraft` (ctest
`core.import-solidcraft`), on the reader's test designs
(`core/solidcraft/tests/designs`) with the mock kernel. The corpus
(`MITCAD_SOLIDCRAFT_CORPUS`) runs through `mitcad-cli`
(`solidcraft.corpus`).
