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
    // A texture or an environment image may be read from its path.
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
