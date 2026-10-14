// SPDX-License-Identifier: MIT
use serde_json::{Value, json};

pub(crate) fn tools() -> Vec<Value> {
    let document = json!({"type": "string", "minLength": 1, "description": "Explicit document handle returned by document_create or document_open"});
    let path = json!({"type": "string", "minLength": 1, "description": "Path within the configured workspace"});
    vec![
        tool(
            "document_create",
            "Create an empty metric CAD document and return its handle. Documents remain open until closed or the server exits.",
            json!({}),
            &[],
            false,
            false,
        ),
        tool(
            "document_open",
            "Open a Mitcad project within the configured workspace and return its document handle. The source file is not changed.",
            json!({"path": path}),
            &["path"],
            false,
            false,
        ),
        tool(
            "document_list",
            "List open document handles and their saved paths and modification state.",
            json!({}),
            &[],
            true,
            false,
        ),
        tool(
            "document_close",
            "Close a document. Unsaved changes prevent closing unless discard is explicitly true.",
            json!({
                "document": document,
                "discard": {"type": "boolean", "default": false, "description": "Explicitly discard unsaved changes"}
            }),
            &["document"],
            false,
            true,
        ),
        tool(
            "model_query",
            "Read a document using one existing JSON model query. Read mitcad://reference/commands for query names, arguments, identities and units.",
            json!({"document": document, "query": {"type": "object", "description": "One JSON query object from the model API reference"}}),
            &["document", "query"],
            true,
            false,
        ),
        tool(
            "model_command",
            "Apply one JSON model command to a document. Read mitcad://reference/commands first. Script arrays, file imports and external links are not supported through this tool.",
            json!({"document": document, "command": {"type": "object", "description": "One JSON command object; lengths use millimetres unless explicitly specified"}}),
            &["document", "command"],
            false,
            true,
        ),
        tool(
            "document_save",
            "Save a document as a Mitcad project inside the workspace. An existing file requires overwrite:true.",
            json!({
                "document": document,
                "path": path,
                "overwrite": {"type": "boolean", "default": false, "description": "Explicitly allow replacement of an existing file"}
            }),
            &["document", "path"],
            false,
            true,
        ),
        tool(
            "model_export",
            "Export geometry to a new file inside the workspace. Existing output files are refused. Optionally select bodies; DXF requires a sketch handle, or a sheet-metal body whose Flat Pattern is written, instead of bodies. Defaults are metric.",
            json!({
                "document": document,
                "path": path,
                "format": {"type": "string", "enum": ["step", "iges", "brep", "stl", "obj", "3mf", "usdz", "dxf"]},
                "bodies": {"type": "array", "items": {"type": "string", "minLength": 1}, "description": "Optional body identities, for example F7.b0"},
                "sketch": {"type": "string", "minLength": 1, "description": "Sketch feature identity for DXF, for example F2"},
                "flat_pattern": {"type": "string", "minLength": 1, "description": "Sheet-metal body identity for a Flat Pattern DXF, for example F3.b0"}
            }),
            &["document", "path", "format"],
            false,
            false,
        ),
    ]
}

pub(crate) fn live_tools() -> Vec<Value> {
    let mut tools: Vec<Value> = tools()
        .into_iter()
        .filter(|tool| {
            matches!(
                tool["name"].as_str(),
                Some("document_list" | "model_query" | "model_command")
            )
        })
        .collect();
    for tool in &mut tools {
        match tool["name"].as_str() {
            Some("model_query") => {
                tool["description"] = json!(
                    "Read the open document with one JSON model query, also while a sketch or command is open. Refused only while the model computes: the result then says busy and retry_after_ms. Read mitcad://reference/commands for query names, arguments, identities and units."
                );
            }
            Some("model_command") => {
                tool["description"] = json!(
                    "Apply one JSON model command to the open document through its normal undo workflow; the change is labelled (agent) in the undo text. While a sketch is open, sketch.* commands on it (and edit_feature of it, undo, redo) go through the sketch session as the user's would; other commands wait until it is finished. Read mitcad://reference/commands first. File, remote and internal commands are refused."
                );
            }
            _ => {}
        }
        if let Some(document) = tool["inputSchema"]["properties"].get_mut("document") {
            document["description"] = json!(LIVE_DOCUMENT);
        }
    }
    tools.extend(ui_tools());
    tools
}

const LIVE_DOCUMENT: &str = "Explicit live document handle returned by document_list";

/// The application's own state and actions on the open document
/// (mitcad#265); the application executes them.
fn ui_tools() -> Vec<Value> {
    let document = json!({"type": "string", "minLength": 1, "description": LIVE_DOCUMENT});
    let kinds = json!([
        "face",
        "edge",
        "vertex",
        "body",
        "profile",
        "sketch_curve",
        "sketch_point",
        "sketch_constraint",
        "sketch_dimension",
        "plane",
        "axis",
        "point",
        "component",
        "sketch",
        "feature"
    ]);
    let item = json!({
        "type": "object",
        "description": "An entity as ui_state lists the selection: kind, owner (a body such as F2.b0, a sketch such as F4, a datum, a component), name (a face, edge or vertex name, a region key, a curve, point, constraint or dimension id; empty for bodies, datums and sketches), occurrence (\"O1/O4\"; empty in the root)",
        "properties": {
            "kind": {"type": "string", "enum": kinds},
            "owner": {"type": "string", "minLength": 1},
            "name": {"type": "string"},
            "occurrence": {"type": "string"}
        },
        "required": ["kind", "owner"],
        "additionalProperties": false
    });
    vec![
        tool(
            "document_save",
            "Save the open document to its own path as File > Save does, with the project's version and commit message when it is in a project. Refused for a document never saved (no path), a read-only document and while a dialog would have to ask the user.",
            json!({"document": document}),
            &["document"],
            false,
            true,
        ),
        tool(
            "ui_state",
            "The application's state: mode (idle, sketch, command, pick_plane), the active component, the sketch being edited with its Sketch Palette status and warnings (each with the entities it is about), the running command with its inputs and panel messages, the selection, the hint or error line, read-only state, unsaved changes, undo and redo labels, connected agents, and the application's version and build.",
            json!({}),
            &[],
            true,
            false,
        ),
        tool(
            "ui_log",
            "The application's latest log lines (its own messages, command refusals), each with a sequence number. Pass since to get only lines after that number.",
            json!({
                "since": {"type": "integer", "minimum": 0, "description": "Only lines with a larger sequence number"},
                "limit": {"type": "integer", "minimum": 1, "maximum": 2000, "default": 200, "description": "At most this many of the newest lines"}
            }),
            &[],
            true,
            false,
        ),
        tool(
            "ui_screenshot",
            "A PNG of the 3D view as the user sees it, including the sketch overlay, or of the whole window; scaled to fit max_size pixels and the response limit. The image is base64 in png_base64.",
            json!({
                "window": {"type": "boolean", "default": false, "description": "The whole window instead of the 3D view"},
                "max_size": {"type": "integer", "minimum": 64, "maximum": 4096, "default": 1280, "description": "The longer side's largest size in pixels"}
            }),
            &[],
            true,
            false,
        ),
        tool(
            "ui_select",
            "Set the application's selection to the given entities, as clicks would, to show the user something; the user can then press Delete or run a command on them. Optionally pan or zoom the view to them.",
            json!({
                "document": document,
                "items": {"type": "array", "items": item, "description": "The entities; empty clears the selection"},
                "reveal": {"type": "string", "enum": ["none", "pan", "fit"], "default": "pan", "description": "pan: move the view to them when none is in sight, keeping the zoom unless they do not fit; fit: zoom to them"}
            }),
            &["document", "items"],
            false,
            false,
        ),
        tool(
            "ui_command",
            "Run a command of the application's command registry by its id (as app/COMMANDS.md lists them: sketch.clean_up_stale_projections, sketch.project, sketch.finish, ...) with input values, then OK or Cancel; or with edit, open a feature for editing (a sketch in sketch mode) as a double-click in the timeline does; or with only finish, end the open command panel, or with cancel a sketch being edited (its changes taken back). The result carries the panel's validation, preview and warnings. Commands that open a file dialog or another modal dialog are refused.",
            json!({
                "document": document,
                "id": {"type": "string", "minLength": 1, "description": "A command id of the registry"},
                "edit": {"type": "string", "minLength": 1, "description": "A feature uid to edit instead of running a command"},
                "inputs": {"type": "object", "description": "Input values by the panel's input keys: text for values and texts, a choice's value, true or false for checks, an array of entities (as ui_select's items) for selections"},
                "finish": {"type": "string", "enum": ["ok", "cancel", "keep_open"], "default": "ok", "description": "What ends a command panel"}
            }),
            &["document"],
            false,
            true,
        ),
    ]
}

fn tool(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
    destructive: bool,
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "openWorldHint": false
        }
    })
}

/// Validate the small, fixed schema vocabulary used by our tool catalog.
/// CAD command and query contents remain the document backend's responsibility.
pub(crate) fn validate(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
    let valid_type = match schema.get("type").and_then(Value::as_str) {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        Some("integer") => value.is_i64() || value.is_u64(),
        _ => true,
    };
    if !valid_type {
        return Err(format!("{path} must be {}", schema["type"]));
    }
    if let Some(number) = value.as_f64() {
        if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64)
            && number < minimum
        {
            return Err(format!("{path} must be at least {minimum}"));
        }
        if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64)
            && number > maximum
        {
            return Err(format!("{path} must be at most {maximum}"));
        }
    }
    if let Some(options) = schema.get("enum").and_then(Value::as_array)
        && !options.contains(value)
    {
        return Err(format!("{path} must be one of {}", schema["enum"]));
    }
    if let Some(string) = value.as_str()
        && let Some(minimum) = schema.get("minLength").and_then(Value::as_u64)
        && (string.chars().count() as u64) < minimum
    {
        return Err(format!("{path} must not be empty"));
    }
    if let Some(object) = value.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    return Err(format!("{path}.{key} is required"));
                }
            }
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        for (key, value) in object {
            if let Some(property) = properties.and_then(|properties| properties.get(key)) {
                validate(value, property, &format!("{path}.{key}"))?;
            } else if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
                return Err(format!("Unknown argument {path}.{key}"));
            }
        }
    }
    if let Some(array) = value.as_array()
        && let Some(items) = schema.get("items")
    {
        for (index, value) in array.iter().enumerate() {
            validate(value, items, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}
