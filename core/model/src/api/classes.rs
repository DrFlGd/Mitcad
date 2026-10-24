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
    // A timber frame (mitcad#479): its parts and fasteners from a library.
    ("build_frame", Remote),
    // Fasteners swapped in place (mitcad#497): the new one from a library.
    ("build_swap_fasteners", Remote),
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
    // Bodies moved into another component on the timeline (mitcad#465).
    ("transfer_bodies", Model),
    // Drawings as PDF, SVG or DXF files (mitcad#471).
    ("export_drawing", FileIo),
    // Gears: the motion link of a mesh (mitcad#472).
    ("link_gears", Model),
    // SVG drawings and QR codes in sketches (mitcad#473).
    ("sketch.import_svg", FileIo),
    ("sketch.add_code", Model),
    ("sketch.edit_code", Model),
    // Typed property sets (mitcad#496).
    ("set_property", Model),
    ("remove_property", Model),
    ("define_property_type", Model),
    ("edit_property_type", Model),
    ("remove_property_type", Model),
    // The sketch's own axes and origin as entities (mitcad#500).
    ("sketch.axis", Model),
    // Reorient Design (mitcad#502).
    ("reorient_design", Model),
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
    ("printed_fit_parameters", Model),
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
    // Bodies moved into another component on the timeline (mitcad#465).
    ("transfer_bodies", Model),
    // Gears: derived values (mitcad#472).
    ("gear_values", Model),
    ("gear_link", Model),
    // An SVG drawing for Insert SVG (mitcad#473).
    ("svg_info", FileIo),
    // Typed property sets (mitcad#496).
    ("property_sets", Model),
    ("property_types", Model),
    ("property_search", Model),
    // Mount frames (mitcad#525); a preview reads its part as
    // insert_component does: from a library or a file.
    ("mount_frames", Model),
    ("mount_preview", FileIo),
    // Pulleys: derived values and belts (mitcad#562).
    ("pulley_values", Model),
    // Printable bearings: derived values and sizes (mitcad#577).
    ("bearing_values", Model),
    ("bearing_sizes", Model),
    // Seats on shafts (mitcad#578).
    ("seat_frame", Model),
    // Print Analysis (mitcad#620).
    ("print_overhangs", Model),
    ("print_thickness", Model),
    ("print_estimate", Model),
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
/// not embedded), the `environment.image` path of `set_render_settings`,
/// and sketch font files, including those inside feature definitions.
/// None for every other call, and for these when they name no file.
pub fn file_path_read(request: &serde_json::Value) -> Option<String> {
    match request.get("cmd")?.as_str()? {
        "create_appearance" | "edit_appearance" => texture_path(request.get("texture")),
        "set_render_settings" => request
            .pointer("/environment/image")?
            .as_str()
            .map(str::to_owned),
        "add_feature" | "edit_feature" => feature_file_path(request.get("def")?),
        "sketch.add_text" | "sketch.edit_text" => request
            .get("font_file")?
            .as_str()
            .filter(|path| !path.is_empty())
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
        "create_appearance"
            | "edit_appearance"
            | "set_render_settings"
            | "add_feature"
            | "edit_feature"
            | "sketch.add_text"
            | "sketch.edit_text"
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
    // Joinery cuts the design's members and reads no library (mitcad#478).
    if name == "build_connect"
        && request
            .get("joint")
            .is_some_and(serde_json::Value::is_object)
    {
        return Some(Model);
    }
    // A timber frame laid out again drives its fasteners from a library
    // (mitcad#479).
    if name == "build_edit"
        && request
            .get("frame")
            .is_some_and(serde_json::Value::is_object)
    {
        return Some(Remote);
    }
    // Fasteners swapped for a source of the design read no library; a set
    // laid out again with its fasteners does (mitcad#497).
    if name == "build_swap_fasteners"
        && request
            .pointer("/part/source")
            .is_some_and(serde_json::Value::is_string)
    {
        return Some(Model);
    }
    if name == "build_edit"
        && request
            .pointer("/spacing/fastener")
            .is_some_and(serde_json::Value::is_object)
    {
        return Some(Remote);
    }
    command_class(name)
}

/// The class of a query (its `query`), or None for a name the API lacks.
pub fn query_class(name: &str) -> Option<ApiClass> {
    class_in(QUERIES, name)
}

/// The default class of a feature type. Sketch text may read a font file:
/// callers accepting definitions must also check [`feature_class_with`].
/// Base B-reps are embedded; geometric paths and occurrence links name
/// document geometry, not filesystem paths. External components are
/// refreshed only by separately classified file/remote commands.
pub fn feature_class(_type_name: &str) -> ApiClass {
    Model
}

/// A file path in a feature definition that its evaluation may read.
/// Font-file metadata is refused even with stored outlines: a later text
/// edit can invalidate those outlines and read the file again.
pub fn feature_file_path(definition: &serde_json::Value) -> Option<String> {
    if definition.get("type")?.as_str()? != "sketch" {
        return None;
    }
    definition
        .get("texts")?
        .as_array()?
        .iter()
        .find_map(|text| {
            text.get("font_file")?
                .as_object()?
                .get("path")?
                .as_str()
                .map(str::to_owned)
        })
}

/// The class of a feature with its fields, including sketch font files.
pub fn feature_class_with(definition: &serde_json::Value) -> ApiClass {
    if feature_file_path(definition).is_some() {
        FileIo
    } else {
        feature_class(
            definition
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default(),
        )
    }
}

/// The same audit on an already parsed feature, without evaluating it.
pub fn feature_class_of<P>(definition: &crate::features::FeatureDef<P>) -> ApiClass {
    if let crate::features::FeatureDef::Sketch(sketch) = definition
        && sketch.texts.iter().any(|text| text.font_file.is_some())
    {
        FileIo
    } else {
        feature_class(definition.type_name())
    }
}

/// Includes implicit font-file reads from a text already in a live
/// document. Changing its string or style can invalidate stored outlines,
/// even when the command does not repeat the font path.
pub fn command_class_in<K: crate::kernel::Kernel>(
    document: &crate::document::Document<K>,
    request: &serde_json::Value,
) -> Option<ApiClass> {
    let class = command_class_with(request)?;
    if class != Model {
        return Some(class);
    }
    let unstored_font = |definition: &crate::features::FeatureDef| {
        if let crate::features::FeatureDef::Sketch(sketch) = definition {
            sketch.texts.iter().any(|text| {
                text.font_file.is_some() && !text.glyphs.as_ref().is_some_and(|g| g.fit(text))
            })
        } else {
            false
        }
    };
    if matches!(request["cmd"].as_str(), Some("undo" | "redo"))
        && document
            .history_state(request["cmd"] == "undo")
            .is_some_and(|state| {
                state
                    .features
                    .iter()
                    .any(|feature| unstored_font(&feature.def))
            })
    {
        return Some(FileIo);
    }
    for feature in document.features() {
        let crate::features::FeatureDef::Sketch(sketch) = &feature.def else {
            continue;
        };
        for text in &sketch.texts {
            if text.font_file.is_none() {
                continue;
            }
            if !text.glyphs.as_ref().is_some_and(|g| g.fit(text)) {
                return Some(FileIo);
            }
            let target = request["sketch"].as_str();
            if request["cmd"] != "sketch.edit_text"
                || (target != Some(feature.uid.to_string().as_str())
                    && target != Some(feature.name.as_str()))
                || request["id"].as_str() != Some(format!("t{}", text.id.0).as_str())
            {
                continue;
            }
            // The ordinary edit leaves the file for an explicitly chosen
            // different family, or for font_file: "".
            if request["font_file"] == ""
                || request["font"]
                    .as_str()
                    .is_some_and(|font| font != text.font)
            {
                continue;
            }
            let mut edited = text.clone();
            if let Some(value) = request["text"].as_str() {
                edited.text = value.to_owned();
            }
            if let Some(value) = request["bold"].as_bool() {
                edited.bold = value;
            }
            if let Some(value) = request["italic"].as_bool() {
                edited.italic = value;
            }
            if !edited.glyphs.as_ref().is_some_and(|g| g.fit(&edited)) {
                return Some(FileIo);
            }
        }
    }
    Some(Model)
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

    use serde_json::json;

    use super::ApiClass::{FileIo, Internal, Model, Remote};
    use super::*;
    use crate::document::Document;
    use crate::testing::MockKernel;

    /// Every command the API runs, by name: the document's own and those
    /// of every family the dispatch sends commands to (mitcad#585).
    fn commands() -> BTreeSet<String> {
        let mut names: BTreeSet<String> = crate::api::Command::NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        for family in Document::<MockKernel>::command_families() {
            names.extend(family.names.iter().map(|name| (*name).to_owned()));
        }
        names
    }

    fn queries() -> BTreeSet<String> {
        crate::api::Query::NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
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
    fn sketch_definitions_with_font_files_are_file_io() {
        let mut definition = json!({"type": "sketch", "texts": [{
            "id": "t1", "text": "A", "at": [0, 0], "height": 10,
            "font_file": {"path": "outside/font.ttf", "index": 0, "licence": ""}
        }]});
        for stored in [false, true] {
            if stored {
                definition["texts"][0]["glyphs"] = json!({
                    "text": "A", "font": "", "glyphs": [{}]});
            }
            assert_eq!(feature_class_with(&definition), FileIo);
            for command in ["add_feature", "edit_feature"] {
                let request = json!({"cmd": command, "def": definition});
                assert_eq!(command_class_with(&request), Some(FileIo));
                assert_eq!(
                    file_path_read(&request).as_deref(),
                    Some("outside/font.ttf")
                );
            }
        }
        definition["texts"][0]
            .as_object_mut()
            .unwrap()
            .remove("font_file");
        assert_eq!(feature_class_with(&definition), Model);
    }

    #[test]
    fn editing_a_stored_file_font_cannot_implicitly_read_it_through_mcp() {
        let text = json!({"id": "t1", "text": "A", "at": [0, 0], "height": 10,
            "font": "Synthetic", "font_file": {"path": "missing.ttf", "index": 0, "licence": ""},
            "glyphs": {"text": "A", "font": "Synthetic", "glyphs": [{}]}});
        let fixture = json!({"format": "mitcad", "version": 2,
            "parameters": [],
            "features": [{"uid": "F1", "name": "Sketch1", "type": "sketch", "plane": "xy", "texts": [text]}]});
        let document = Document::from_json(&fixture.to_string(), MockKernel::default()).unwrap();
        let feature = document.features().next().unwrap();
        assert_eq!(feature_class_of(&feature.def), FileIo);
        let base =
            json!({"cmd": "sketch.edit_text", "sketch": feature.uid.to_string(), "id": "t1"});
        for (key, value) in [
            ("text", json!("B")),
            ("bold", json!(true)),
            ("italic", json!(true)),
        ] {
            let mut request = base.clone();
            request[key] = value;
            assert_eq!(command_class_with(&request), Some(Model));
            assert_eq!(command_class_in(&document, &request), Some(FileIo));
        }
        for (key, value) in [
            ("height", json!(20)),
            ("text", json!("A")),
            ("font", json!("Installed")),
            ("font_file", json!("")),
        ] {
            let mut request = base.clone();
            request[key] = value;
            assert_eq!(command_class_in(&document, &request), Some(Model));
        }
        assert_eq!(
            command_class_in(&document, &json!({"cmd": "recompute"})),
            Some(Model)
        );
        let mut by_name = base.clone();
        by_name["sketch"] = json!(feature.name);
        by_name["italic"] = json!(true);
        assert_eq!(command_class_in(&document, &by_name), Some(FileIo));

        let mut stale = fixture.clone();
        stale["features"][0]["texts"][0]["text"] = json!("B");
        let mut document = Document::from_json(&stale.to_string(), MockKernel::default()).unwrap();
        assert_eq!(
            command_class_in(&document, &json!({"cmd": "recompute"})),
            Some(FileIo)
        );
        assert_eq!(
            command_class_in(&document, &json!({"cmd": "set_marker", "index": 0})),
            Some(FileIo)
        );
        let uid = document.features().next().unwrap().uid.to_string();
        document
            .command(&json!({"cmd": "delete_feature", "uid": uid}).to_string())
            .unwrap();
        assert!(document.features().next().is_none());
        assert_eq!(
            command_class_in(&document, &json!({"cmd": "undo"})),
            Some(FileIo)
        );
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
