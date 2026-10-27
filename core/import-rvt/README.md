# mitcad-import-rvt

The `.rvt` import (mitcad#274, mitcad#275, mitcad#333): a project's
building elements, read by [`mitcad-rvt`](../rvt/README.md), as a MitCad
design, one component per level, generic over the model's `Kernel`
(`mitcad-ffi` runs it with OCCT: `core/ffi/src/rvt_import.rs`, the
`import_rvt` command, `mitcad-cli import-rvt`). The crate docs
(`src/lib.rs`) describe what comes in how.

| Module | What it holds |
|---|---|
| `lib.rs` | `import_rvt`, `RvtOptions`, the report, links and IFC links, the IFC reference check (`Reference`, `reference_openings`). |
| `plan`, `footprint` | Walls, floors, roofs, ceilings, columns, beams, stairs and rails rebuilt from their parameters as IFC geometry items, built with the IFC import's builder. |
| `geometry` | The meshes of the geometry the file keeps of an element. |
| `properties` | Elements' properties (mitcad#496). |

It depends on `mitcad-import-ifc` (its builder, meshes, transforms and the
IFC import of links and references) and `mitcad-import-base` (the
progress, the default geometric tolerance), not on the `.f3d` replay.

Tests: `cargo test -p mitcad-import-rvt` (ctest `core.import-rvt`): the
unit tests in `src/`, and `tests/rvt_corpus.rs`, `tests/rvt_rails.rs`,
`tests/rvt_mullion_trims.rs`, which read `MITCAD_RVT_CORPUS` and skip
without it.
