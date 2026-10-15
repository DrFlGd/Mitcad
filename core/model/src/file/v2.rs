// SPDX-License-Identifier: MIT
//! Design file versions 2 (a single-file design) and 3 (a design in a
//! project; see the module documentation of `file`): version 3 is version 2
//! with B-rep data in a project's store.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{FORMAT, FileError, SINGLE_VERSION, VERSION, has_references, legacy_name};
use crate::appearance::{Appearance, BASE_COLOR, ImageData, share_images, slot_name};
use crate::assembly::{
    Assembly, ComponentDef, ExternalLink, Occurrence, ROOT_NAME, from_rows, matrix_rows,
};
use crate::configurations::Configurations;
use crate::document::{
    Analysis, BodyAttributes, DisplayState, DocState, NamedView, TimelineGroup, check_components,
};
use crate::expr::{EvalContext, ParamSpec, TableError, Unit, value_to_expression};
use crate::features::{CheckContext, FeatureDef, FeatureEntry, HelixConstruction, is_false};
use crate::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use crate::parameters::{Parameters, default_context, slot_unit};
use crate::policy::Policy;
use crate::render_settings::RenderSettings;
use crate::sketch::legacy::{LegacyShape, convert, settle};
use crate::transform::Transform;

// Writing: field order is the order in the file.

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'static str,
    version: u64,
    units: UnitsOut,
    parameters: Vec<ParameterOut<'a>>,
    // Components and occurrences (F6), when there are any.
    #[serde(skip_serializing_if = "Option::is_none")]
    root_component: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    components: Vec<ComponentOut<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    occurrences: Vec<OccurrenceOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_component: Option<ComponentUid>,
    features: Vec<FeatureOut<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    bodies: Vec<BodyOut<'a>>,
    // Named views (U5), when there are any.
    #[serde(skip_serializing_if = "<[NamedView]>::is_empty")]
    views: &'a [NamedView],
    // The Origin folder and Isolate (P9), when not the defaults.
    #[serde(skip_serializing_if = "DisplayState::is_default")]
    display: &'a DisplayState,
    // Timeline groups (P9), when there are any.
    #[serde(skip_serializing_if = "<[TimelineGroup]>::is_empty")]
    groups: &'a [TimelineGroup],
    // Analyses (mitcad#41), when there are any.
    #[serde(skip_serializing_if = "<[Analysis]>::is_empty")]
    analyses: &'a [Analysis],
    // The document's appearances (mitcad#46), when there are any.
    #[serde(skip_serializing_if = "no_appearances")]
    appearances: Cow<'a, [Appearance]>,
    // Their embedded images, once each, when the textures refer to them by
    // digest (mitcad#313).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    images: Vec<ImageOut>,
    // Render settings (mitcad#47), when not the defaults.
    #[serde(skip_serializing_if = "RenderSettings::is_default")]
    render: &'a RenderSettings,
    // The configuration table (mitcad#64), when there is one.
    #[serde(skip_serializing_if = "Configurations::is_empty")]
    configurations: &'a Configurations,
    // A hybrid assembly allowed without asking (mitcad#171), when it is.
    #[serde(skip_serializing_if = "is_false")]
    hybrid_assembly: bool,
    // The joins' tolerance an importer set (mitcad#298), when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    join_tolerance: Option<f64>,
    // Flat Patterns of sheet-metal bodies (mitcad#208), when there are any.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    flat_patterns: Vec<FlatPatternOut<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    marker: Option<usize>,
    // The root component's policy (mitcad#220), when not the defaults.
    #[serde(skip_serializing_if = "Policy::is_default")]
    root_policy: Policy,
}

/// A Flat Pattern: its body and settings.
#[derive(Serialize)]
struct FlatPatternOut<'a> {
    body: BodyUid,
    #[serde(flatten)]
    def: &'a crate::sheet::flat::FlatPatternDef,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlatPatternIn {
    body: BodyUid,
    stationary: crate::sheet::PanelId,
    #[serde(default)]
    x_axis: crate::sheet::flat::FlatXAxis,
    #[serde(default)]
    reference_side: Option<crate::sheet::SheetReferenceSide>,
    #[serde(default)]
    shown: bool,
}

#[derive(Serialize)]
struct ComponentOut<'a> {
    uid: ComponentUid,
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_by: Option<FeatureUid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<&'a ExternalLink>,
    // The library it came from (mitcad#64).
    #[serde(skip_serializing_if = "Option::is_none")]
    library: Option<&'a crate::library::LibraryRef>,
    // Its policy (mitcad#220), when not the general defaults.
    #[serde(skip_serializing_if = "Policy::is_default")]
    policy: Policy,
}

#[derive(Serialize)]
struct OccurrenceOut {
    uid: OccurrenceUid,
    component: ComponentUid,
    parent: ComponentUid,
    number: u32,
    /// Rows of the rotation and the translation; left out for the
    /// identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    transform: Option<[[f64; 4]; 3]>,
    #[serde(skip_serializing_if = "is_false")]
    grounded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<crate::features::point_pattern::PatternInstance>,
}

fn is_root(component: &ComponentUid) -> bool {
    component.is_root()
}

#[derive(Serialize)]
struct UnitsOut {
    length: &'static str,
    angle: &'static str,
}

#[derive(Serialize)]
struct ParameterOut<'a> {
    name: &'a str,
    expression: &'a str,
    unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expression_units: Option<UnitsOut>,
    /// The value when saved (mm, rad), for readers; loading evaluates the
    /// expression.
    value: f64,
    comment: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<FeatureUid>,
    // Favourite parameters (P9).
    #[serde(skip_serializing_if = "is_false")]
    favorite: bool,
}

#[derive(Serialize)]
struct FeatureOut<'a> {
    uid: FeatureUid,
    name: &'a str,
    #[serde(skip_serializing_if = "is_false")]
    suppressed: bool,
    #[serde(skip_serializing_if = "is_root")]
    component: ComponentUid,
    /// Set by the user for sketches and construction features (U3).
    #[serde(skip_serializing_if = "Option::is_none")]
    visible: Option<bool>,
    /// What the importer that made it said (mitcad#170).
    #[serde(skip_serializing_if = "Option::is_none")]
    import_note: Option<&'a str>,
    #[serde(flatten)]
    def: FeatureDef<String>,
}

#[derive(Serialize)]
struct BodyOut<'a> {
    uid: BodyUid,
    name: &'a str,
    // Attributes, when not the default.
    #[serde(skip_serializing_if = "Option::is_none")]
    visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    appearance: Option<&'a str>,
    // Appearances of single faces by face name (mitcad#53).
    #[serde(skip_serializing_if = "Option::is_none")]
    face_appearances: Option<&'a BTreeMap<String, String>>,
}

pub(super) fn save(state: &DocState) -> String {
    let (appearances, images) = appearances_out(&state.appearances);
    let parameters = &state.parameters;
    let context = parameters.context();
    let assembly = &state.assembly;
    let file = FileOut {
        format: FORMAT,
        // Version 3 only when base features refer to a project's B-rep store.
        version: if has_references(state) {
            VERSION
        } else {
            SINGLE_VERSION
        },
        units: UnitsOut {
            length: context.default_length_unit.symbol(),
            angle: context.default_angle_unit.symbol(),
        },
        parameters: parameters
            .iter()
            .map(|p| ParameterOut {
                name: p.name(),
                expression: p.expression(),
                unit: p.unit().to_string(),
                expression_units: p.expression_context().map(|context| UnitsOut {
                    length: context.default_length_unit.symbol(),
                    angle: context.default_angle_unit.symbol(),
                }),
                value: p.value(),
                comment: p.comment(),
                owner: parameters.owner(p.id()),
                favorite: state.favorites.contains(&p.id()),
            })
            .collect(),
        root_component: (assembly.root_name != ROOT_NAME).then_some(assembly.root_name.as_str()),
        components: assembly
            .components
            .iter()
            .map(|c| ComponentOut {
                uid: c.uid,
                name: &c.name,
                created_by: c.created_by,
                link: c.link.as_ref(),
                library: c.library.as_ref(),
                policy: c.policy,
            })
            .collect(),
        occurrences: assembly
            .occurrences
            .iter()
            .map(|o| OccurrenceOut {
                uid: o.uid,
                component: o.component,
                parent: o.parent,
                number: o.number,
                transform: (!o.transform.is_identity()).then(|| matrix_rows(&o.transform)),
                grounded: o.grounded,
                visible: (!o.visible).then_some(false),
                pattern: o.pattern.clone(),
            })
            .collect(),
        active_component: (!assembly.active.is_root()).then_some(assembly.active),
        features: state
            .features
            .iter()
            .map(|f| FeatureOut {
                uid: f.uid,
                name: &f.name,
                suppressed: f.suppressed,
                component: f.component,
                visible: state.feature_visibility.get(&f.uid).copied(),
                import_note: state.import_notes.get(&f.uid).map(String::as_str),
                def: f
                    .def
                    .map_params(&mut |_, id| Ok::<_, ()>(parameters.name(*id)))
                    .expect("names never fail"),
            })
            .collect(),
        bodies: state
            .body_names
            .iter()
            .map(|(uid, name)| {
                let attributes = state.body_attributes.get(uid);
                BodyOut {
                    uid: *uid,
                    name,
                    visible: attributes.filter(|a| !a.visible).map(|_| false),
                    material: attributes.and_then(|a| a.material.as_deref()),
                    appearance: attributes.and_then(|a| a.appearance.as_deref()),
                    face_appearances: attributes
                        .map(|a| &a.face_appearances)
                        .filter(|faces| !faces.is_empty()),
                }
            })
            .collect(),
        views: &state.named_views,
        display: &state.display,
        groups: &state.groups,
        analyses: &state.analyses,
        appearances,
        images,
        render: &state.render,
        configurations: &state.configurations,
        hybrid_assembly: state.hybrid_assembly,
        join_tolerance: state.join_tolerance,
        flat_patterns: state
            .flat_patterns
            .iter()
            .map(|(body, def)| FlatPatternOut { body: *body, def })
            .collect(),
        marker: (state.marker < state.features.len()).then_some(state.marker),
        root_policy: assembly.root_policy,
    };
    let mut json =
        serde_json::to_string_pretty(&file).expect("the design file schema always serializes");
    json.push('\n');
    json
}

fn no_appearances(appearances: &[Appearance]) -> bool {
    appearances.is_empty()
}

/// An embedded image in the file's `images` (mitcad#313).
#[derive(Serialize)]
struct ImageOut {
    sha256: String,
    data: ImageData,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageIn {
    sha256: String,
    data: ImageData,
}

/// The appearances as the file keeps them, and the file's `images`: as
/// before mitcad#313 (each embedded image in its texture's `data`, no
/// `images`) unless an image of a map other than the base colour is
/// embedded or one image is embedded more than once; then each image is
/// kept once in `images`, in the order of first use, and the textures
/// refer to it by its digest (`image`).
fn appearances_out(appearances: &[Appearance]) -> (Cow<'_, [Appearance]>, Vec<ImageOut>) {
    let mut uses: HashMap<&str, usize> = HashMap::new();
    let mut maps = false;
    for texture in appearances.iter().filter_map(|a| a.texture.as_ref()) {
        for (map, data) in texture.images() {
            *uses.entry(data.sha256()).or_default() += 1;
            maps |= map != BASE_COLOR;
        }
    }
    if !maps && uses.values().all(|n| *n == 1) {
        return (Cow::Borrowed(appearances), Vec::new());
    }
    let mut images: Vec<ImageOut> = Vec::new();
    let mut listed = HashSet::new();
    let mut out = appearances.to_vec();
    for texture in out.iter_mut().filter_map(|a| a.texture.as_mut()) {
        for slot in texture.slots_mut() {
            if let Some(data) = slot.data.take() {
                let sha256 = data.sha256().to_owned();
                if listed.insert(sha256.clone()) {
                    images.push(ImageOut {
                        sha256: sha256.clone(),
                        data,
                    });
                }
                *slot.image = Some(sha256);
            }
        }
    }
    (Cow::Owned(out), images)
}

/// The file's `images` by digest, each checked: a PNG or JPEG file of at
/// most 32 MB whose SHA-256 is the one given, listed once.
fn images_in(top: &mut Map<String, Value>) -> Result<HashMap<String, ImageData>, FileError> {
    let mut images = HashMap::new();
    for (i, image) in array(top, "images", false)?.into_iter().enumerate() {
        let image: ImageIn =
            serde_json::from_value(image).map_err(|e| schema(format!("images[{i}]: {e}")))?;
        image
            .data
            .bytes()
            .map_err(|e| invalid(format!("images[{i}]: {e}")))?;
        if image.data.sha256() != image.sha256 {
            return Err(invalid(format!(
                "images[{i}]: the image's SHA-256 is {}, not {}",
                image.data.sha256(),
                image.sha256
            )));
        }
        if images.insert(image.sha256.clone(), image.data).is_some() {
            return Err(invalid(format!(
                "images[{i}]: image {} is listed more than once",
                image.sha256
            )));
        }
    }
    Ok(images)
}

/// An appearance of the file, its textures' images (`image`) taken from
/// the file's `images`, shared.
fn appearance_in(
    i: usize,
    value: Value,
    images: &HashMap<String, ImageData>,
) -> Result<Appearance, FileError> {
    let mut appearance = serde_json::from_value::<Appearance>(value)
        .map_err(|e| schema(format!("appearances[{i}]: {e}")))?;
    if let Some(texture) = appearance.texture.as_mut() {
        for slot in texture.slots_mut() {
            let Some(digest) = slot.image.take() else {
                continue;
            };
            let what = slot_name(slot.map);
            if slot.data.is_some() {
                return Err(schema(format!(
                    "appearances[{i}]: {what}: both `data` and `image`"
                )));
            }
            *slot.data = Some(images.get(&digest).cloned().ok_or_else(|| {
                invalid(format!(
                    "appearances[{i}]: {what}: image {digest} is not in `images`"
                ))
            })?);
        }
    }
    appearance
        .check()
        .map_err(|e| invalid(format!("appearances[{i}]: {e}")))?;
    Ok(appearance)
}

// Reading: the JSON value is walked, so every error can say where it is.

/// A parameter: `expression` and `unit`, or only a `value` (millimetres or
/// radians) in files written before expressions.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParameterIn {
    name: String,
    #[serde(default)]
    expression: Option<String>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    expression_units: Option<UnitsIn>,
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    comment: String,
    #[serde(default)]
    owner: Option<FeatureUid>,
    #[serde(default)]
    favorite: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitsIn {
    #[serde(default)]
    length: Option<String>,
    #[serde(default)]
    angle: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyIn {
    uid: BodyUid,
    name: String,
    #[serde(default = "visible")]
    visible: bool,
    #[serde(default)]
    material: Option<String>,
    #[serde(default)]
    appearance: Option<String>,
    #[serde(default)]
    face_appearances: BTreeMap<String, String>,
}

fn visible() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentIn {
    uid: ComponentUid,
    name: String,
    #[serde(default)]
    created_by: Option<FeatureUid>,
    #[serde(default)]
    link: Option<ExternalLink>,
    #[serde(default)]
    library: Option<crate::library::LibraryRef>,
    #[serde(default)]
    policy: Policy,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OccurrenceIn {
    uid: OccurrenceUid,
    component: ComponentUid,
    #[serde(default)]
    parent: ComponentUid,
    number: u32,
    #[serde(default)]
    transform: Option<Vec<Vec<f64>>>,
    #[serde(default)]
    grounded: bool,
    #[serde(default = "visible")]
    visible: bool,
    #[serde(default)]
    pattern: Option<crate::features::point_pattern::PatternInstance>,
}

struct FeatureIn {
    uid: FeatureUid,
    name: String,
    suppressed: bool,
    component: ComponentUid,
    visible: Option<bool>,
    /// What the importer that made it said (mitcad#170).
    import_note: Option<String>,
    def: FeatureDef<String>,
    /// A sketch of rectangles and circles (`"shapes"`, early version 2):
    /// converted again with the parameter values once they are loaded, so
    /// that the stored positions are exact.
    shapes: Option<Value>,
}

const TOP_FIELDS: [&str; 23] = [
    "format",
    "version",
    "units",
    "parameters",
    "root_component",
    "components",
    "occurrences",
    "active_component",
    "features",
    "bodies",
    "marker",
    "views",
    "display",
    "groups",
    "analyses",
    "appearances",
    "render",
    // The configuration table (mitcad#64).
    "configurations",
    // A hybrid assembly allowed without asking (mitcad#171).
    "hybrid_assembly",
    // The joins' tolerance an importer set (mitcad#298).
    "join_tolerance",
    // Flat Patterns of sheet-metal bodies (mitcad#208).
    "flat_patterns",
    // The root component's policy (mitcad#220), when not the defaults.
    "root_policy",
    // Embedded images by digest (mitcad#313).
    "images",
];

fn schema(message: impl Into<String>) -> FileError {
    FileError::Schema(message.into())
}

fn invalid(message: impl Into<String>) -> FileError {
    FileError::Invalid(message.into())
}

fn array(
    top: &mut Map<String, Value>,
    field: &str,
    required: bool,
) -> Result<Vec<Value>, FileError> {
    match top.remove(field) {
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(schema(format!("\"{field}\" must be an array"))),
        None if required => Err(schema(format!("missing field `{field}`"))),
        None => Ok(Vec::new()),
    }
}

fn feature_in(i: usize, value: Value) -> Result<FeatureIn, FileError> {
    let Value::Object(mut fields) = value else {
        return Err(schema(format!("features[{i}]: expected an object")));
    };
    let text = |fields: &mut Map<String, Value>, field: &str| match fields.remove(field) {
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(schema(format!(
            "features[{i}]: \"{field}\" must be a string"
        ))),
        None => Err(schema(format!("features[{i}]: missing field `{field}`"))),
    };
    let uid_text = text(&mut fields, "uid")?;
    let name = text(&mut fields, "name")?;
    let uid = uid_text
        .parse()
        .map_err(|e| schema(format!("features[{i}] ({name}): {e}")))?;
    let suppressed = match fields.remove("suppressed") {
        None => false,
        Some(Value::Bool(value)) => value,
        Some(_) => {
            return Err(schema(format!(
                "features[{i}] ({name}): \"suppressed\" must be true or false"
            )));
        }
    };
    let component = match fields.remove("component") {
        None => ComponentUid::ROOT,
        Some(Value::String(text)) => text
            .parse()
            .map_err(|e| schema(format!("features[{i}] ({name}): {e}")))?,
        Some(_) => {
            return Err(schema(format!(
                "features[{i}] ({name}): \"component\" must be a component id"
            )));
        }
    };
    let visible = match fields.remove("visible") {
        None => None,
        Some(Value::Bool(value)) => Some(value),
        Some(_) => {
            return Err(schema(format!(
                "features[{i}] ({name}): \"visible\" must be true or false"
            )));
        }
    };
    let import_note = match fields.remove("import_note") {
        None => None,
        Some(Value::String(note)) => Some(note).filter(|n| !n.trim().is_empty()),
        Some(_) => {
            return Err(schema(format!(
                "features[{i}] ({name}): \"import_note\" must be a string"
            )));
        }
    };
    let shapes = fields.get("shapes").cloned();
    let def = serde_json::from_value(Value::Object(fields))
        .map_err(|e| schema(format!("features[{i}] ({name}): {e}")))?;
    Ok(FeatureIn {
        uid,
        name,
        suppressed,
        component,
        visible,
        import_note,
        def,
        shapes,
    })
}

/// The components and occurrences of a file; files without them have
/// only the root component, which holds every feature.
fn assembly_in(top: &mut Map<String, Value>) -> Result<Assembly, FileError> {
    let mut assembly = Assembly::default();
    match top.remove("root_component") {
        None => {}
        Some(Value::String(name)) if !name.trim().is_empty() => assembly.root_name = name,
        Some(_) => return Err(schema("\"root_component\" must be a name")),
    }
    // Files without it have the general defaults (mitcad#220).
    if let Some(policy) = top.remove("root_policy") {
        assembly.root_policy =
            serde_json::from_value(policy).map_err(|e| schema(format!("root_policy: {e}")))?;
    }
    for (i, c) in array(top, "components", false)?.into_iter().enumerate() {
        let c: ComponentIn =
            serde_json::from_value(c).map_err(|e| schema(format!("components[{i}]: {e}")))?;
        if assembly.exists(c.uid) {
            return Err(invalid(format!(
                "components[{i}]: component {} is listed more than once",
                c.uid
            )));
        }
        assembly.next_component = assembly.next_component.max(c.uid.0 + 1);
        assembly.components.push(ComponentDef {
            uid: c.uid,
            name: c.name,
            created_by: c.created_by,
            link: c.link,
            library: c.library,
            policy: c.policy,
        });
    }
    for (i, o) in array(top, "occurrences", false)?.into_iter().enumerate() {
        let o: OccurrenceIn =
            serde_json::from_value(o).map_err(|e| schema(format!("occurrences[{i}]: {e}")))?;
        if assembly.occurrence(o.uid).is_some() {
            return Err(invalid(format!(
                "occurrences[{i}]: occurrence {} is listed more than once",
                o.uid
            )));
        }
        let transform = match &o.transform {
            None => Transform::IDENTITY,
            Some(rows) => from_rows(rows).map_err(|e| invalid(format!("occurrences[{i}]: {e}")))?,
        };
        assembly.next_occurrence = assembly.next_occurrence.max(o.uid.0 + 1);
        assembly.occurrences.push(Occurrence {
            uid: o.uid,
            component: o.component,
            parent: o.parent,
            transform,
            grounded: o.grounded,
            visible: o.visible,
            number: o.number,
            pattern: o.pattern,
        });
    }
    match top.remove("active_component") {
        None => {}
        Some(value) => {
            assembly.active = serde_json::from_value(value)
                .map_err(|e| schema(format!("active_component: {e}")))?;
        }
    }
    Ok(assembly)
}

pub(super) fn load(value: Value) -> Result<(DocState, Vec<String>), FileError> {
    let Value::Object(mut top) = value else {
        return Err(FileError::NotADesign);
    };
    if let Some(field) = top.keys().find(|k| !TOP_FIELDS.contains(&k.as_str())) {
        return Err(schema(format!(
            "unknown field `{field}`, expected one of {}",
            TOP_FIELDS.map(|f| format!("`{f}`")).join(", ")
        )));
    }
    let context = match top.remove("units") {
        None => default_context(),
        Some(units) => {
            let units: UnitsIn =
                serde_json::from_value(units).map_err(|e| schema(format!("units: {e}")))?;
            units_context(&units)?
        }
    };
    let parameters = array(&mut top, "parameters", true)?
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            serde_json::from_value::<ParameterIn>(p)
                .map_err(|e| schema(format!("parameters[{i}]: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let assembly = assembly_in(&mut top)?;
    let features = array(&mut top, "features", true)?
        .into_iter()
        .enumerate()
        .map(|(i, f)| feature_in(i, f))
        .collect::<Result<Vec<_>, _>>()?;
    let bodies = array(&mut top, "bodies", false)?
        .into_iter()
        .enumerate()
        .map(|(i, b)| {
            serde_json::from_value::<BodyIn>(b).map_err(|e| schema(format!("bodies[{i}]: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let views = array(&mut top, "views", false)?
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            let view = serde_json::from_value::<NamedView>(v)
                .map_err(|e| schema(format!("views[{i}]: {e}")))?;
            view.check()
                .map_err(|e| invalid(format!("views[{i}]: {e}")))?;
            Ok(view)
        })
        .collect::<Result<Vec<_>, FileError>>()?;
    let display = match top.remove("display") {
        None => DisplayState::default(),
        Some(value) => serde_json::from_value::<DisplayState>(value)
            .map_err(|e| schema(format!("display: {e}")))?,
    };
    let groups = array(&mut top, "groups", false)?
        .into_iter()
        .enumerate()
        .map(|(i, g)| {
            serde_json::from_value::<TimelineGroup>(g)
                .map_err(|e| schema(format!("groups[{i}]: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let analyses = array(&mut top, "analyses", false)?
        .into_iter()
        .enumerate()
        .map(|(i, a)| {
            let analysis = serde_json::from_value::<Analysis>(a)
                .map_err(|e| schema(format!("analyses[{i}]: {e}")))?;
            analysis
                .check()
                .map_err(|e| invalid(format!("analyses[{i}]: {e}")))?;
            Ok(analysis)
        })
        .collect::<Result<Vec<_>, FileError>>()?;
    let images = images_in(&mut top)?;
    let mut appearances = array(&mut top, "appearances", false)?
        .into_iter()
        .enumerate()
        .map(|(i, a)| appearance_in(i, a, &images))
        .collect::<Result<Vec<_>, FileError>>()?;
    // One copy of each image, also of one embedded twice in an older file.
    let mut shared = HashMap::new();
    for texture in appearances.iter_mut().filter_map(|a| a.texture.as_mut()) {
        share_images(texture, &mut shared);
    }
    let render = match top.remove("render") {
        None => RenderSettings::default(),
        Some(value) => {
            let render = serde_json::from_value::<RenderSettings>(value)
                .map_err(|e| schema(format!("render: {e}")))?;
            render
                .check()
                .map_err(|e| invalid(format!("render: {e}")))?;
            render
        }
    };
    let configurations = match top.remove("configurations") {
        None => Configurations::default(),
        Some(value) => serde_json::from_value::<Configurations>(value)
            .map_err(|e| schema(format!("configurations: {e}")))?,
    };
    let hybrid_assembly = match top.remove("hybrid_assembly") {
        None => false,
        Some(Value::Bool(allowed)) => allowed,
        Some(_) => return Err(schema("\"hybrid_assembly\" must be true or false")),
    };
    let join_tolerance = match top.remove("join_tolerance") {
        None => None,
        Some(value) => match value.as_f64() {
            Some(t) if t.is_finite() && t > 0.0 => Some(t),
            _ => return Err(schema("\"join_tolerance\" must be a positive number")),
        },
    };
    let flat_patterns = array(&mut top, "flat_patterns", false)?
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            serde_json::from_value::<FlatPatternIn>(f)
                .map_err(|e| schema(format!("flat_patterns[{i}]: {e}")))
        })
        .collect::<Result<Vec<_>, FileError>>()?;
    let marker = match top.remove("marker") {
        None => None,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(|m| usize::try_from(m).ok())
                .ok_or_else(|| schema("\"marker\" must be a feature count"))?,
        ),
    };

    let mut state = DocState::default();
    let mut warnings = Vec::new();
    let renamed = load_parameters(&mut state, context, &parameters, &features, &mut warnings)?;

    // Uids and names first, so references can tell a missing feature from a
    // later one.
    let mut uids = HashSet::new();
    let mut names = HashSet::new();
    for (i, f) in features.iter().enumerate() {
        if !uids.insert(f.uid) {
            return Err(invalid(format!(
                "features[{i}] ({}): uid {} is used more than once",
                f.name, f.uid
            )));
        }
        if f.name.trim().is_empty() {
            return Err(invalid(format!("features[{i}]: the name is empty")));
        }
        if !names.insert(f.name.as_str()) {
            return Err(invalid(format!(
                "features[{i}]: feature name '{}' is used more than once",
                f.name
            )));
        }
    }
    let mut entries = Vec::with_capacity(features.len());
    for (i, f) in features.iter().enumerate() {
        let params = &state.parameters;
        let mut def = f
            .def
            .map_params(&mut |slot, name: &String| {
                let actual = renamed.get(name).unwrap_or(name);
                params
                    .find(actual)
                    .ok_or_else(|| format!("{slot}: parameter '{name}' does not exist"))
            })
            .map_err(|e| invalid(format!("features[{i}] ({}): {e}", f.name)))?;
        // A growing helix of a file written before mitcad#83 is FreeCAD's.
        if let FeatureDef::Helix(helix) = &mut def {
            helix.settle_construction(HelixConstruction::Freecad);
        }
        if let FeatureDef::SheetFace(face) = &mut def {
            face.settle(None);
        }
        crate::features::sheet_folded::settle(&mut def, None);
        if let (Some(shapes), FeatureDef::Sketch(sketch)) = (&f.shapes, &mut def)
            && let Ok(shapes) = serde_json::from_value::<Vec<LegacyShape<String>>>(shapes.clone())
        {
            let size = |name: &String| {
                params
                    .find(renamed.get(name).unwrap_or(name))
                    .and_then(|id| params.value(id))
            };
            let exact = convert(shapes, &size);
            for (entity, placed) in sketch.entities.iter_mut().zip(exact.entities) {
                entity.kind = placed.kind;
            }
            // Values that do not solve load and fail at recompute.
            let _ = settle(sketch, params);
        }
        entries.push(Arc::new(FeatureEntry {
            uid: f.uid,
            name: f.name.clone(),
            suppressed: f.suppressed,
            component: f.component,
            def,
        }));
        if let Some(visible) = f.visible {
            state.feature_visibility.insert(f.uid, visible);
        }
        if let Some(note) = &f.import_note {
            state.import_notes.insert(f.uid, note.clone());
        }
    }
    let feature_uids: BTreeSet<FeatureUid> = uids.iter().copied().collect();
    assembly.check(&feature_uids).map_err(invalid)?;
    for (i, entry) in entries.iter().enumerate() {
        let ctx = CheckContext {
            uid: entry.uid,
            earlier: &entries[..i],
            all: &entries,
        };
        entry
            .def
            .info()
            .check(&ctx)
            .and_then(|()| crate::joints::check_generated_references(&assembly, &ctx, &entry.def))
            .and_then(|()| match &entry.def {
                FeatureDef::Extrude(extrude) => {
                    extrude.cut_scope.as_ref().map_or(Ok(()), |scope| {
                        scope.check_components(&assembly, &ctx, entry.component)
                    })
                }
                _ => Ok(()),
            })
            .and_then(|()| check_components(&assembly, &entries, entry))
            .map_err(|e| invalid(format!("features[{i}] ({}): {e}", entry.name)))?;
    }
    state.features = entries;
    state.assembly = assembly;

    for (i, p) in parameters.iter().enumerate() {
        if let Some(owner) = p.owner.filter(|o| !uids.contains(o)) {
            return Err(invalid(format!(
                "parameters[{i}]: the owner {owner} is not a feature of the file"
            )));
        }
    }
    for (i, body) in bodies.into_iter().enumerate() {
        if !uids.contains(&body.uid.feature) {
            return Err(invalid(format!(
                "bodies[{i}]: feature {} does not exist",
                body.uid.feature
            )));
        }
        if body.name.trim().is_empty() {
            return Err(invalid(format!("bodies[{i}]: the name is empty")));
        }
        if state.body_names.insert(body.uid, body.name).is_some() {
            return Err(invalid(format!(
                "bodies[{i}]: body {} is listed more than once",
                body.uid
            )));
        }
        if let Some(id) = body
            .material
            .as_deref()
            .filter(|id| crate::analysis::material(id).is_none())
        {
            return Err(invalid(format!("bodies[{i}]: unknown material '{id}'")));
        }
        if let Some(face) = body
            .face_appearances
            .keys()
            .find(|face| face.parse::<crate::topo::FaceName>().is_err())
        {
            return Err(invalid(format!(
                "bodies[{i}]: '{face}' is no face name in face_appearances"
            )));
        }
        let attributes = BodyAttributes {
            visible: body.visible,
            material: body.material,
            appearance: body.appearance,
            face_appearances: body.face_appearances,
        };
        if !attributes.is_default() {
            state.body_attributes.insert(body.uid, attributes);
        }
    }
    state.marker = match marker {
        Some(m) if m > state.features.len() => {
            return Err(invalid(format!(
                "the marker {m} is past the end of the timeline ({} features)",
                state.features.len()
            )));
        }
        Some(m) => m,
        None => state.features.len(),
    };
    state.next_uid = state
        .features
        .iter()
        .map(|f| f.uid.0 + 1)
        .max()
        .unwrap_or(1);
    for (i, view) in views.iter().enumerate() {
        if views[..i].iter().any(|v| v.name == view.name) {
            return Err(invalid(format!(
                "views[{i}]: named view '{}' is listed more than once",
                view.name
            )));
        }
    }
    state.named_views = views;
    state.display = display;
    for (i, group) in groups.iter().enumerate() {
        if let Some(missing) = group
            .features
            .iter()
            .find(|uid| state.position(**uid).is_none())
        {
            return Err(invalid(format!(
                "groups[{i}] ({}): feature {missing} does not exist",
                group.name
            )));
        }
    }
    state.groups = groups;
    state.normalize_groups();
    // Analyses (mitcad#41): their references are resolved when they are
    // asked for, so one whose plane is gone loads and says why.
    for (i, analysis) in analyses.iter().enumerate() {
        if analyses[..i].iter().any(|a| a.name == analysis.name) {
            return Err(invalid(format!(
                "analyses[{i}]: analysis '{}' is listed more than once",
                analysis.name
            )));
        }
    }
    state.analyses = analyses;
    // Appearances (mitcad#46): ids of their own, not the library's.
    for (i, appearance) in appearances.iter().enumerate() {
        let id = &appearance.id;
        if !crate::appearance::valid_id(id) {
            return Err(invalid(format!(
                "appearances[{i}]: '{id}' is no appearance id"
            )));
        }
        if crate::appearance::library_appearance(id).is_some() {
            return Err(invalid(format!(
                "appearances[{i}]: '{id}' is a library appearance's id"
            )));
        }
        if appearances[..i].iter().any(|a| a.id == *id) {
            return Err(invalid(format!(
                "appearances[{i}]: appearance '{id}' is listed more than once"
            )));
        }
    }
    state.appearances = appearances;
    state.render = render;
    // The configuration table (mitcad#64): rows of the file's parameters.
    if let Some(problem) = configurations.problems(&state.parameters).first() {
        return Err(invalid(format!("configurations: {problem}")));
    }
    state.configurations = configurations;
    state.hybrid_assembly = hybrid_assembly;
    state.join_tolerance = join_tolerance;
    // Flat Patterns (mitcad#208): evaluated when asked for, so one whose
    // body or panel is gone loads and says why.
    for (i, f) in flat_patterns.into_iter().enumerate() {
        let def = crate::sheet::flat::FlatPatternDef {
            stationary: f.stationary,
            x_axis: f.x_axis,
            reference_side: f
                .reference_side
                .unwrap_or(crate::sheet::SheetReferenceSide::Front),
            shown: f.shown,
        };
        if state.flat_patterns.insert(f.body, def).is_some() {
            return Err(invalid(format!(
                "flat_patterns[{i}]: body {} is listed more than once",
                f.body
            )));
        }
    }
    // One shown at a time: the first shown one.
    let mut shown = false;
    for analysis in &mut state.analyses {
        analysis.visible = analysis.visible && !shown;
        shown = shown || analysis.visible;
    }
    Ok((state, warnings))
}

/// The default units of a file.
fn units_context(units: &UnitsIn) -> Result<EvalContext, FileError> {
    let mut context = default_context();
    if let Some(text) = &units.length {
        context.default_length_unit = Unit::parse(text)
            .ok()
            .and_then(|u| match (u.length(), u.angle()) {
                (Some((unit, 1)), None) => Some(unit),
                _ => None,
            })
            .ok_or_else(|| invalid(format!("units: '{text}' is not a length unit")))?;
    }
    if let Some(text) = &units.angle {
        context.default_angle_unit = Unit::parse(text)
            .ok()
            .and_then(|u| match (u.length(), u.angle()) {
                (None, Some((unit, 1))) => Some(unit),
                _ => None,
            })
            .ok_or_else(|| invalid(format!("units: '{text}' is not an angle unit")))?;
    }
    Ok(context)
}

/// Builds the parameters. Files from before expressions store values: they
/// become expressions in millimetres, or degrees when a feature uses the
/// parameter as an angle, and names that are now reserved (`m`, `E`, `in`,
/// ...) are renamed with a warning. Returns the renames.
fn load_parameters(
    state: &mut DocState,
    context: EvalContext,
    parameters: &[ParameterIn],
    features: &[FeatureIn],
    warnings: &mut Vec<String>,
) -> Result<HashMap<String, String>, FileError> {
    let mut angles = HashSet::new();
    for f in features {
        let _ = f.def.map_params(&mut |slot, name: &String| {
            if slot_unit(slot) == Unit::DEG {
                angles.insert(name.clone());
            }
            Ok::<_, ()>(())
        });
    }
    let mut taken: HashSet<String> = parameters.iter().map(|p| p.name.clone()).collect();
    let mut renamed = HashMap::new();
    let mut seen = HashSet::new();
    let mut specs = Vec::with_capacity(parameters.len());
    for (i, p) in parameters.iter().enumerate() {
        let at = |message: String| invalid(format!("parameters[{i}]: {message}"));
        if p.name.is_empty() {
            return Err(at("the name is empty".to_owned()));
        }
        if !seen.insert(p.name.as_str()) {
            return Err(at(format!("parameter '{}' already exists", p.name)));
        }
        let spec = match (&p.expression, p.value) {
            (Some(expression), _) => {
                let unit = Unit::parse(p.unit.as_deref().unwrap_or(""))
                    .map_err(|e| at(format!("unit: {e}")))?;
                ParamSpec::user(&p.name, expression, unit)
            }
            (None, Some(value)) => {
                if p.unit.is_some() {
                    return Err(at("a unit needs an expression".to_owned()));
                }
                let unit = if angles.contains(&p.name) {
                    Unit::DEG
                } else {
                    Unit::MM
                };
                let name = match legacy_name(&p.name, &taken) {
                    Some(new) => {
                        warnings.push(format!(
                            "parameter '{}' is renamed to '{new}': '{}' is a unit, function \
                             or constant in expressions",
                            p.name, p.name
                        ));
                        taken.insert(new.clone());
                        renamed.insert(p.name.clone(), new.clone());
                        new
                    }
                    None => p.name.clone(),
                };
                ParamSpec::user(&name, &value_to_expression(value, unit), unit)
            }
            (None, None) => {
                return Err(schema(format!(
                    "parameters[{i}]: missing field `expression`"
                )));
            }
        };
        let expression_context = p
            .expression_units
            .as_ref()
            .map(units_context)
            .transpose()
            .map_err(|e| at(format!("expression_units: {e}")))?;
        specs.push((
            spec.with_comment(&p.comment)
                .with_expression_context(expression_context),
            p.owner,
        ));
    }
    let index_of = |name: &str| {
        parameters
            .iter()
            .position(|p| renamed.get(&p.name).unwrap_or(&p.name) == name)
    };
    state.parameters = Parameters::build(context, specs).map_err(|e| {
        let name = match &e {
            TableError::InvalidName(name)
            | TableError::ReservedName(name)
            | TableError::DuplicateName(name) => Some(name.as_str()),
            TableError::Parse { parameter, .. } | TableError::Eval { parameter, .. } => {
                Some(parameter.as_str())
            }
            _ => None,
        };
        match name.and_then(index_of) {
            Some(i) => invalid(format!("parameters[{i}]: {e}")),
            None => invalid(format!("parameters: {e}")),
        }
    })?;
    // Favourites (P9).
    for p in parameters.iter().filter(|p| p.favorite) {
        if let Some(id) = state
            .parameters
            .find(renamed.get(&p.name).unwrap_or(&p.name))
        {
            state.favorites.insert(id);
        }
    }
    Ok(renamed)
}
