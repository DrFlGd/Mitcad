// SPDX-License-Identifier: MIT
//! The class of every command and query of the API (mitcad#265): what it
//! works on besides the document's data. External clients (the MCP server,
//! `docs/mcp.md`) allow the model's own commands, features and queries and
//! refuse the rest by class, not by a list of names kept elsewhere, so a
//! new model command needs one line here and no other edit; the tests below
//! fail for a command or query without a class.

use self::ApiClass::{FileIo, Internal, Model, Remote};

/// What a command or query works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiClass {
    /// The document's own data: features, sketches, parameters, bodies'
    /// attributes, components, joints, views, analyses, undo and redo.
    Model,
    /// Reads or writes files: imports, exports, other designs (components,
    /// links), images and drawings by path, the saved state.
    FileIo,
    /// Reaches other repositories through the library resolver, which can
    /// fetch them over the network.
    Remote,
    /// The application's own workflows: a command panel's checkpoints and
    /// the caches of computed results.
    Internal,
}

impl ApiClass {
    pub fn name(self) -> &'static str {
        match self {
            Model => "model",
            FileIo => "file IO",
            Remote => "remote",
            Internal => "internal",
        }
    }
}

/// Every command (its `cmd`) with its class.
const COMMANDS: &[(&str, ApiClass)] = &[
    // A command panel's checkpoints and the caches (the application's own).
    ("begin_command", Internal),
    ("restore_command", Internal),
    ("discard_command", Internal),
    ("clear_cache", Internal),
    // Files: imports, exports, other designs, images by path, the saved
    // state of the design file.
    ("import_file", FileIo),
    ("export", FileIo),
    ("export_sketch", FileIo),
    ("export_flat_pattern_dxf", FileIo),
    ("mark_saved", FileIo),
    ("insert_component", FileIo),
    ("update_links", FileIo),
    ("sketch.import_dxf", FileIo),
    // A texture or an environment image may be read from its path: these
    // are the model's own when the call names no file, see
    // `command_class_with` (mitcad#383).
    ("create_appearance", FileIo),
    ("edit_appearance", FileIo),
    ("set_render_settings", FileIo),
    // Library parts come through the library resolver.
    ("update_library_parts", Remote),
    // The document's data.
    ("add_feature", Model),
    ("edit_feature", Model),
    ("delete_feature", Model),
    ("suppress_feature", Model),
    ("reorder_feature", Model),
    ("rename_feature", Model),
    ("rename_body", Model),
    ("set_marker", Model),
    ("add_parameter", Model),
    ("set_parameter", Model),
    ("set_units", Model),
    ("rename_parameter", Model),
    ("delete_parameter", Model),
    ("undo", Model),
    ("redo", Model),
    ("recompute", Model),
    ("merge_undo", Model),
    ("set_body_visible", Model),
    ("set_body_material", Model),
    ("set_body_appearance", Model),
    ("set_face_appearance", Model),
    ("clear_face_appearances", Model),
    ("set_feature_visible", Model),
    ("add_named_view", Model),
    ("delete_named_view", Model),
    ("rename_named_view", Model),
    ("set_origin_visible", Model),
    ("set_isolation", Model),
    ("group_features", Model),
    ("ungroup", Model),
    ("rename_group", Model),
    ("add_analysis", Model),
    ("edit_analysis", Model),
    ("set_analysis_visible", Model),
    ("rename_analysis", Model),
    ("delete_analysis", Model),
    ("delete_appearance", Model),
    ("reset_render_settings", Model),
    ("add_render_light", Model),
    ("edit_render_light", Model),
    ("delete_render_light", Model),
    ("set_flat_pattern", Model),
    ("remove_flat_pattern", Model),
    // Components and occurrences (`components.rs`).
    ("create_component", Model),
    ("components_from_bodies", Model),
    ("activate_component", Model),
    ("rename_component", Model),
    ("ground_occurrence", Model),
    ("set_occurrence_visible", Model),
    ("set_occurrence_transform", Model),
    ("copy_occurrence", Model),
    ("paste_new", Model),
    ("delete_occurrence", Model),
    ("reparent_occurrence", Model),
    // Joints (`joints.rs`).
    ("add_joint", Model),
    ("add_as_built_joint", Model),
    ("add_rigid_group", Model),
    ("drive_joint", Model),
    ("drag_occurrence", Model),
    ("add_constraint_set", Model),
    // Configurations (`libraries.rs`).
    ("set_configurations", Model),
    ("apply_configuration", Model),
    // Move Features into a Component (`move_features.rs`).
    ("move_to_component", Model),
    ("move_history_into_component", Model),
    ("set_hybrid_assembly", Model),
    // Design policy (`policy.rs`).
    ("set_policy", Model),
    // Sketch commands (`sketch.rs`).
    ("sketch.create", Model),
    ("sketch.add_rectangle", Model),
    ("sketch.add_circle", Model),
    ("sketch.add_point", Model),
    ("sketch.add_line", Model),
    ("sketch.rectangle", Model),
    ("sketch.circle", Model),
    ("sketch.arc", Model),
    ("sketch.polygon", Model),
    ("sketch.slot", Model),
    ("sketch.ellipse", Model),
    ("sketch.spline", Model),
    ("sketch.add_text", Model),
    ("sketch.edit_text", Model),
    ("sketch.add_constraint", Model),
    ("sketch.add_dimension", Model),
    ("sketch.set_dimension", Model),
    ("sketch.set_driven", Model),
    ("sketch.set_signed", Model),
    ("sketch.remove", Model),
    ("sketch.set_construction", Model),
    ("sketch.set_centerline", Model),
    ("sketch.set_fixed", Model),
    ("sketch.drag", Model),
    ("sketch.move", Model),
    ("sketch.trim", Model),
    ("sketch.extend", Model),
    ("sketch.fillet", Model),
    ("sketch.chamfer", Model),
    ("sketch.offset", Model),
    ("sketch.mirror", Model),
    ("sketch.circular_pattern", Model),
    ("sketch.rectangular_pattern", Model),
    ("sketch.edit_pattern", Model),
    ("sketch.edit_offset", Model),
    ("sketch.project", Model),
    ("sketch.set_dimension_text", Model),
    ("sketch.add_equal_spacing", Model),
    ("sketch.edit_equal_spacing", Model),
    ("sketch.clean_up_projections", Model),
    ("sketch.clean_up_stale_projections", Model),
    ("sketch.convert_projected", Model),
    // The BUILD tab (mitcad#375): placing reads a new source from a library
    // (with `part.source` it does not: [`command_class_with`]).
    ("build_place", Remote),
    ("build_connect", Remote),
    ("build_panel", Remote),
    ("build_edit", Model),
    ("build_remove", Model),
    // Motion studies and contact sets (mitcad#454).
    ("add_motion_study", Model),
    ("edit_motion_study", Model),
    ("delete_motion_study", Model),
    ("set_study_key", Model),
    ("remove_study_key", Model),
    ("play_study", Model),
    ("add_contact_set", Model),
    ("edit_contact_set", Model),
    ("delete_contact_set", Model),
    // Motion links between joints (mitcad#466).
    ("add_motion_link", Model),
];

/// Every query (its `query`) with its class. A `cache` query with `disk`
/// also reads the result store's files: an external client refuses that
/// form separately.
const QUERIES: &[(&str, ApiClass)] = &[
    // A file by its path.
    ("dxf_info", FileIo),
    ("compare_step", FileIo),
    ("document", Model),
    ("timeline", Model),
    ("evaluate", Model),
    ("parameters", Model),
    ("bodies", Model),
    ("profiles", Model),
    ("autoselect_profiles", Model),
    ("sheet_state", Model),
    ("sheet_rules", Model),
    ("feature", Model),
    ("faces", Model),
    ("edges", Model),
    ("report", Model),
    ("datums", Model),
    ("datum", Model),
    ("properties", Model),
    ("measure", Model),
    ("interference", Model),
    ("section", Model),
    ("section_plane", Model),
    ("threads", Model),
    ("sketch", Model),
    ("components", Model),
    ("instances", Model),
    ("dependents", Model),
    ("can_reorder", Model),
    ("named_views", Model),
    ("recompute_times", Model),
    ("thread_sizes", Model),
    ("cache", Model),
    ("changes_since_saved", Model),
    ("analyses", Model),
    ("appearances", Model),
    ("appearance_image", Model),
    ("render_settings", Model),
    ("joints", Model),
    ("joint_dof", Model),
    ("joint_drag", Model),
    ("joint_frame", Model),
    ("configurations", Model),
    ("library_parts", Model),
    ("parts_list", Model),
    ("constraint_row", Model),
    ("move_to_component", Model),
    ("feature_references", Model),
    ("profile", Model),
    ("flat_pattern", Model),
    ("flat_point", Model),
    ("sheet_reference", Model),
    ("sheet_seam", Model),
    ("sheet_bodies", Model),
    ("policy", Model),
    ("policies", Model),
    ("sketch_supports", Model),
    ("reparent_occurrence", Model),
    // Body edges and vertices in a sketch's plane (mitcad#256).
    ("plane_geometry", Model),
    ("tangent_chains", Model),
    // A thread's default end chamfer (mitcad#304).
    ("thread_chamfer", Model),
    // The BUILD tab (mitcad#375).
    ("build_preview", Model),
    ("build_parts", Model),
    ("build_junctions", Model),
    ("build_cut_list", Model),
    ("build_host", Model),
    // The library parts the design uses (mitcad#418).
    ("library_usage", Model),
    // Motion studies and contact sets (mitcad#454).
    ("motion_studies", Model),
    ("study_values", Model),
    ("contact_sets", Model),
    ("contact_check", Model),
    // Motion links between joints (mitcad#466).
    ("motion_link_offset", Model),
];

fn class_in(table: &[(&str, ApiClass)], name: &str) -> Option<ApiClass> {
    table
        .iter()
        .find(|(entry, _)| *entry == name)
        .map(|(_, class)| *class)
}

/// The class of a command (its `cmd`), or None for a name the API lacks.
pub fn command_class(name: &str) -> Option<ApiClass> {
    class_in(COMMANDS, name)
}

/// The maps of an appearance texture besides its base colour image.
const TEXTURE_MAPS: [&str; 4] = ["normal", "roughness", "metalness", "occlusion"];

/// The first file path a texture of a command would be read from: an
/// image (the base colour's or a map's) that is neither embedded (`data`)
/// nor kept from the design (`"embedded": true`). A texture that is null
/// or left out reads nothing.
fn texture_path(texture: Option<&serde_json::Value>) -> Option<String> {
    let texture = texture?.as_object()?;
    let reads = |slot: &serde_json::Map<String, serde_json::Value>| {
        slot.get("data").is_none_or(serde_json::Value::is_null)
            && slot.get("embedded").and_then(serde_json::Value::as_bool) != Some(true)
    };
    let path_of = |slot: &serde_json::Map<String, serde_json::Value>| {
        slot.get("path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    if reads(texture) {
        return Some(path_of(texture));
    }
    TEXTURE_MAPS
        .iter()
        .filter_map(|map| texture.get(*map)?.as_object())
        .find(|slot| reads(slot))
        .map(path_of)
}

/// The file a call would read, when its class depends on it (mitcad#383):
/// the texture path of `create_appearance` and `edit_appearance` (an image
/// not embedded) and the `environment.image` path of `set_render_settings`.
/// None for every other call, and for these when they name no file.
pub fn file_path_read(request: &serde_json::Value) -> Option<String> {
    match request.get("cmd")?.as_str()? {
        "create_appearance" | "edit_appearance" => texture_path(request.get("texture")),
        "set_render_settings" => request
            .pointer("/environment/image")?
            .as_str()
            .map(str::to_owned),
        _ => None,
    }
}

/// The class of a command with its fields: a BUILD command that places a
/// part from a source the design has (`part.source`) reads no library and
/// is the model's own (mitcad#375); `create_appearance`, `edit_appearance`
/// and `set_render_settings` are the model's own unless the call names a
/// file to read ([`file_path_read`], mitcad#383); others as
/// [`command_class`].
pub fn command_class_with(request: &serde_json::Value) -> Option<ApiClass> {
    let name = request.get("cmd")?.as_str()?;
    if matches!(
        name,
        "create_appearance" | "edit_appearance" | "set_render_settings"
    ) {
        return Some(if file_path_read(request).is_some() {
            FileIo
        } else {
            Model
        });
    }
    if matches!(name, "build_place" | "build_connect" | "build_panel")
        && request
            .pointer("/part/source")
            .is_some_and(serde_json::Value::is_string)
    {
        return Some(Model);
    }
    command_class(name)
}

/// The class of a query (its `query`), or None for a name the API lacks.
pub fn query_class(name: &str) -> Option<ApiClass> {
    class_in(QUERIES, name)
}

/// The class of a feature type: every feature evaluates on the document's
/// geometry and the bytes it embeds (a base feature's B-rep, a text's
/// installed fonts); none reads files or the network. A feature type that
/// comes to do so gets its own class here.
pub fn feature_class(_type_name: &str) -> ApiClass {
    Model
}

/// Whether an external client may run a command: one of the model's own
/// (`add_feature` and `edit_feature` of any feature, `feature_class`).
pub fn external_command_allowed(name: &str) -> bool {
    command_class(name) == Some(Model)
}

/// Whether an external client may run a query (not `cache` with `disk`,
/// which the caller checks).
pub fn external_query_allowed(name: &str) -> bool {
    query_class(name) == Some(Model)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use serde::Deserialize;
    use serde_json::json;

    use super::ApiClass::{FileIo, Internal, Model, Remote};
    use super::*;

    /// The names of an internally tagged enum's variants, as serde lists
    /// them when a tag is unknown.
    fn variants<T: for<'de> Deserialize<'de>>(tag: &str) -> Vec<String> {
        let message = match serde_json::from_value::<T>(json!({tag: "\u{1}unknown"})) {
            Ok(_) => panic!("an unknown {tag} was accepted"),
            Err(error) => error.to_string(),
        };
        let (_, expected) = message
            .split_once("expected ")
            .unwrap_or_else(|| panic!("no variants in {message}"));
        let names: Vec<String> = expected
            .split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect();
        assert!(names.len() > 3, "{message}");
        names
    }

    /// Every command the API runs, by name.
    fn commands() -> BTreeSet<String> {
        let mut names: BTreeSet<String> =
            variants::<crate::api::Command>("cmd").into_iter().collect();
        names.extend(variants::<crate::api::sketch::SketchCommand>("cmd"));
        for list in [
            &crate::api::components::COMMANDS[..],
            &crate::api::joints::COMMANDS[..],
            &crate::api::libraries::COMMANDS[..],
            &crate::api::move_features::COMMANDS[..],
            &crate::api::policy::COMMANDS[..],
            &crate::api::build::COMMANDS[..],
            &crate::api::motion::COMMANDS[..],
        ] {
            names.extend(list.iter().map(|name| (*name).to_owned()));
        }
        names
    }

    fn queries() -> BTreeSet<String> {
        variants::<crate::api::Query>("query").into_iter().collect()
    }

    #[test]
    fn every_command_has_a_class() {
        let commands = commands();
        let missing: Vec<&String> = commands
            .iter()
            .filter(|name| command_class(name).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "commands without a class in api/classes.rs: {missing:?}"
        );
        assert!(commands.contains("sketch.clean_up_stale_projections"));
    }

    #[test]
    fn every_query_has_a_class() {
        let queries = queries();
        let missing: Vec<&String> = queries
            .iter()
            .filter(|name| query_class(name).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "queries without a class in api/classes.rs: {missing:?}"
        );
        assert!(queries.contains("sketch"));
    }

    #[test]
    fn every_class_is_of_a_command_or_query_the_api_has() {
        // A renamed or removed command leaves no stale entry behind.
        let commands = commands();
        let queries = queries();
        for (name, _) in COMMANDS {
            assert!(
                commands.contains(*name),
                "{name} is classified but no command"
            );
        }
        for (name, _) in QUERIES {
            assert!(queries.contains(*name), "{name} is classified but no query");
        }
        let unique: BTreeSet<&str> = COMMANDS.iter().map(|(name, _)| *name).collect();
        assert_eq!(unique.len(), COMMANDS.len(), "a command classified twice");
        let unique: BTreeSet<&str> = QUERIES.iter().map(|(name, _)| *name).collect();
        assert_eq!(unique.len(), QUERIES.len(), "a query classified twice");
        assert_eq!(command_class("no_such_command"), None);
        assert_eq!(query_class("no_such_query"), None);
    }

    fn class_of(request: serde_json::Value) -> Option<ApiClass> {
        command_class_with(&request)
    }

    #[test]
    fn appearance_calls_without_a_file_are_the_models_own() {
        // The acceptance example of mitcad#383: parameters, no texture.
        assert_eq!(
            class_of(json!({"cmd": "create_appearance", "id": "led_red",
                "name": "LED red", "based_on": "emissive_white",
                "base_color": [1, 0.1, 0.08], "emission_color": [1, 0.1, 0.05],
                "emission": 5})),
            Some(Model)
        );
        assert_eq!(
            class_of(json!({"cmd": "edit_appearance", "id": "led_red", "roughness": 0.3})),
            Some(Model)
        );
        // No texture, or a removed one.
        assert_eq!(
            class_of(json!({"cmd": "edit_appearance", "id": "a", "texture": null})),
            Some(Model)
        );
        // Embedded images: bytes, or kept from the design.
        assert_eq!(
            class_of(json!({"cmd": "create_appearance", "texture": {
                "path": "oak.png", "data": "iVBORw0K",
                "normal": {"path": "n.png", "data": "iVBORw0K"},
                "roughness": {"path": "r.png", "embedded": true,
                    "image_sha256": "ab"}}})),
            Some(Model)
        );
        assert_eq!(
            class_of(json!({"cmd": "edit_appearance", "id": "a", "texture": {
                "path": "oak.png", "embedded": true, "size": [50, 50]}})),
            Some(Model)
        );
        assert_eq!(file_path_read(&json!({"cmd": "create_appearance"})), None);
    }

    #[test]
    fn appearance_calls_that_read_a_file_stay_file_io() {
        // A texture by path, also with a null `data`.
        for request in [
            json!({"cmd": "create_appearance", "texture": {"path": "/etc/x.png"}}),
            json!({"cmd": "edit_appearance", "id": "a",
                "texture": {"path": "/etc/x.png", "data": null}}),
            json!({"cmd": "edit_appearance", "id": "a",
                "texture": {"path": "/etc/x.png", "embedded": false}}),
        ] {
            assert_eq!(class_of(request.clone()), Some(FileIo), "{request}");
            assert_eq!(file_path_read(&request).as_deref(), Some("/etc/x.png"));
        }
        // A map by path beside an embedded base colour.
        let request = json!({"cmd": "create_appearance", "texture": {
            "path": "oak.png", "data": "iVBORw0K",
            "normal": {"path": "/etc/n.png"}}});
        assert_eq!(class_of(request.clone()), Some(FileIo));
        assert_eq!(file_path_read(&request).as_deref(), Some("/etc/n.png"));
    }

    #[test]
    fn render_settings_are_the_models_own_unless_they_name_an_image() {
        for request in [
            json!({"cmd": "set_render_settings", "environment": {"preset": "outdoor"}}),
            json!({"cmd": "set_render_settings", "output": {"width": 800}}),
            // null: the field's default, no file.
            json!({"cmd": "set_render_settings", "environment": {"image": null}}),
        ] {
            assert_eq!(class_of(request.clone()), Some(Model), "{request}");
        }
        let request = json!({"cmd": "set_render_settings",
            "environment": {"preset": "image", "image": "/env/sky.hdr"}});
        assert_eq!(class_of(request.clone()), Some(FileIo));
        assert_eq!(file_path_read(&request).as_deref(), Some("/env/sky.hdr"));
    }

    #[test]
    fn file_remote_and_internal_commands_are_refused_to_external_clients() {
        for name in [
            "import_file",
            "export",
            "export_sketch",
            "export_flat_pattern_dxf",
            "insert_component",
            "update_links",
            "sketch.import_dxf",
            "mark_saved",
            "create_appearance",
        ] {
            assert_eq!(command_class(name), Some(FileIo), "{name}");
            assert!(!external_command_allowed(name), "{name}");
        }
        assert_eq!(command_class("update_library_parts"), Some(Remote));
        assert!(!external_command_allowed("update_library_parts"));
        for name in [
            "begin_command",
            "restore_command",
            "discard_command",
            "clear_cache",
        ] {
            assert_eq!(command_class(name), Some(Internal), "{name}");
            assert!(!external_command_allowed(name), "{name}");
        }
        for name in ["dxf_info", "compare_step"] {
            assert_eq!(query_class(name), Some(FileIo), "{name}");
            assert!(!external_query_allowed(name), "{name}");
        }
        // Commands added after the MCP server's first list of names work
        // with no edit of it (mitcad#232, mitcad#252).
        for name in [
            "sketch.clean_up_projections",
            "sketch.clean_up_stale_projections",
            "add_feature",
            "undo",
        ] {
            assert!(external_command_allowed(name), "{name}");
        }
        assert!(external_query_allowed("sketch"));
        assert_eq!(feature_class("extrude"), Model);
        assert_eq!(FileIo.name(), "file IO");
    }
}
