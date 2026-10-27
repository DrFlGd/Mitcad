# mitcad-model-api

The JSON command and query interface of the MitCad model: everything the
application, `mitcad-cli` and the MCP server send to a document goes
through `command`, `preview` and `query` here. The commands are described
in [commands.md](src/commands.md).

The crate sits above `mitcad-model` (mitcad#777), so that an edit of a
command recompiles only this crate and the ones above it. The document's
api methods are extension traits of `mitcad_model::Document`: import
`mitcad_model_api::DocumentApi` (or `mitcad_model_api::prelude::*`) to call
them. Each file's methods form traits by their visibility: `<File>Api`
(public), `<File>CrateOps` and `<File>Ops` (the crate's), `<File>Local`
(the file's own).

Its own unit tests are the inline `mod tests` of its files (`core.model_api`);
the tests that drive the model through the api live in the small test
crates under [`core/model-tests/`](../model-tests/README.md).
