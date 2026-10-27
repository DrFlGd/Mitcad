# mitcad-model-drawing

The drawing layout and writers of the MitCad model, a crate of its own
above `mitcad-model` (mitcad#777): a drawing's views laid out on a sheet
with their dimensions, annotations, hole tables, parts lists, frame and
title block, and the sheet written as PDF, SVG, DXF or DWG. No geometry
library: the kernel projects the views (`Kernel::project_view`) and the
JSON api (`mitcad-model-api`, its `drawing*.rs`) asks this crate to lay
them out.

| Module | What it does |
|---|---|
| `compose` | The layout of a sheet: views in their places, their marks, the automatic annotations, the frame and the title block |
| `clip` | Detail views and breaks cut in the view's plane |
| `annotate` | Annotations on views laid out from their picks |
| `tables` | Hole tables and parts lists drawn from their rows |
| `template` | Sheet templates drawn; the built-in ones (`templates/*.json`) and the user's and projects' libraries |
| `sheet` | The sheet's groups, pens and texts that the writers write |
| `pdf`, `svg`, `dxf`, `dwg` | The writers |
| `lib.rs` | The projection cache in the document's side cache (`DrawingCache`) |

What a design file keeps of a drawing stays in the model
(`core/model/src/drawing`): the definition (`def.rs`) with its stored
annotations (`annotate_def.rs`), table settings (`tables_def.rs`) and
templates and revisions (`template_def.rs`), and the options of
`export_drawing`. This crate re-exports all of it under the same paths,
so `mitcad_model_drawing::…` names everything of drawings.

Where code goes: a new stored field or its check into the model's
`drawing/`; layout and writing here; a command into the api crate. Tests
of the layout without a document live beside the code here
(`core.model_drawing`); tests through the JSON api in
`core/model-tests/drawing` and `drawing-views`.
