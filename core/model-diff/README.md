# mitcad-model-diff

Comparison of two MitCad designs (P12c): what differs between two
documents, two design files or two versions, in the model's terms
(parameters, timeline, sketches, components, occurrences, bodies,
materials, drawings and the others) rather than as lines of JSON, with a
text for people. The version history (`mitcad-vcs`, `mitcad-cli diff`, the
Version History window through the C++ bridge) uses it;
[commands.md](../model-api/src/commands.md) describes the comparison.

The crate sits above `mitcad-model` (mitcad#777): nothing in the model
uses it, so a change here recompiles only this crate and its users. It
reads the document's state (`mitcad_model::DocState`) and the
design file's loader; the model items it needs are public and hidden from
the model's docs.

The crate has no tests of its own: the comparison is tested in
`core/model-tests/files` (`diff_tests.rs`) and in the areas whose items it
compares ([README](../model-tests/README.md)).
