# mitcad-print-keys

The print settings' key map (mitcad#640): MitCad's keys of the three
layers of print settings (the printer, the process, the filament), each
with its type, unit, choices, a plain name, its place in the settings
form, and per slicer dialect (the Bambu Studio family, PrusaSlicer, Cura)
the slicer's key and how a value converts. Export writes a dialect's keys
from MitCad's values and import reads them back through this table.

The crate's documentation (`src/lib.rs`) describes the dialects and the
categories; the table itself is `src/table.rs`. The model re-exports the
crate as `mitcad_model::print_keys`. Its only dependency is `serde_json`.

Its tests (`cargo test -p mitcad-print-keys`) run as the ctest
`core.print-keys`.
