// SPDX-License-Identifier: MIT
//! Data exchange: base features from imported files and from bodies an
//! importer built, export of bodies (STEP, IGES, STL, OBJ, BRep, 3MF,
//! USDZ) and of a sketch's geometry (DXF).
//!
//! File formats are the kernel's business ([`Kernel::read_file`],
//! [`Kernel::write_file`], which takes each body's placements, mitcad#19);
//! DXF is written by `mitcad-dxf` from the evaluated sketch, and 3MF by
//! `mitcad-3mf` and STL by the model (`stl`) from the kernel's triangle
//! meshes ([`Kernel::triangle_mesh`], mitcad#13, mitcad#17); USDZ by
//! `mitcad-usdz` from the kernel's face meshes ([`Kernel::face_meshes`],
//! `usdz`, mitcad#123). Every format has the bodies where the design shows
//! them unless the components' coordinates are asked for.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::document::{Added, BodyView, Document, InstanceView, ModelError};
use crate::features::{BaseBody, BaseDef, Brep, FeatureDef, Operation, ValueInput};
use crate::ids::{BodyUid, ComponentUid, FeatureUid, OccurrenceUid};
use crate::kernel::Kernel;
use crate::profile::SegmentGeometry;
use crate::transform::Transform;

mod stl;
mod usdz;

// The bodies on the print bed by their print bed faces (mitcad#731).
mod bed;
pub use bed::{DEFAULT_PLATE, OnBed};

// Meshes with a longest edge (mitcad#734, `MeshTolerance::max_edge`).
mod edges;

/// What a body is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    /// One or more closed solids.
    Solid,
    /// Faces or open shells with surfaces (a surface body).
    Sheet,
    /// Triangles without surfaces (from STL or OBJ).
    Mesh,
    Empty,
}

impl BodyKind {
    /// `solid`, `sheet`, `mesh` or `empty`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Sheet => "sheet",
            Self::Mesh => "mesh",
            Self::Empty => "empty",
        }
    }
}

/// A body read from a file or built by an importer.
#[derive(Debug, Clone)]
pub struct ImportBody<S> {
    pub name: Option<String>,
    /// sRGB components in [0, 1].
    pub color: Option<[f64; 3]>,
    pub shape: S,
}

/// Options of [`Kernel::read_file`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReadOptions {
    /// The length of one file unit in millimetres for formats without
    /// units (STL, OBJ).
    pub unit_mm: f64,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self { unit_mm: 1.0 }
    }
}

/// A body to write, with its display name and colour, and where it goes
/// (mitcad#19): once per placement, or once as it is without placements.
#[derive(Debug, Clone, Copy)]
pub struct ExportBody<'a, S> {
    pub name: &'a str,
    pub color: Option<[f64; 3]>,
    pub shape: &'a S,
    pub placements: &'a [BodyPlacement],
}

/// Where [`Kernel::write_file`] puts a body: a rotation and a translation
/// from its component's coordinates to the design's (an occurrence's
/// placement), and the occurrences' path that places it (`Arm:1/Pin:2`;
/// empty for the root's bodies). STEP writes a body's placements as the
/// instances of one part in an assembly, the other formats as moved
/// copies.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyPlacement {
    pub transform: Transform,
    pub name: String,
}

/// File formats for bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Step,
    Iges,
    Stl,
    Obj,
    /// OCCT's native B-rep text format.
    Brep,
    /// 3D Manufacturing Format for slicers (mitcad#13): one object with a
    /// part per body.
    #[serde(rename = "3mf")]
    ThreeMf,
    /// Universal Scene Description package for AR viewers and other
    /// programs (mitcad#123): the bodies as shaded meshes in the component
    /// hierarchy, with their appearances.
    Usdz,
}

impl ExportFormat {
    /// The format of a file name's extension (`.step`/`.stp`, `.iges`/`.igs`,
    /// `.stl`, `.obj`, `.brep`/`.brp`, `.3mf`, `.usdz`), ignoring case.
    pub fn of_path(path: &str) -> Option<Self> {
        let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
        Some(match extension.as_str() {
            "step" | "stp" => Self::Step,
            "iges" | "igs" => Self::Iges,
            "stl" => Self::Stl,
            "obj" => Self::Obj,
            "brep" | "brp" => Self::Brep,
            "3mf" => Self::ThreeMf,
            "usdz" => Self::Usdz,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Iges => "iges",
            Self::Stl => "stl",
            Self::Obj => "obj",
            Self::Brep => "brep",
            Self::ThreeMf => "3mf",
            Self::Usdz => "usdz",
        }
    }

    /// Formats that write triangles, which mesh bodies can go to.
    pub fn takes_meshes(self) -> bool {
        matches!(
            self,
            Self::Stl | Self::Obj | Self::Brep | Self::ThreeMf | Self::Usdz
        )
    }
}

/// Where exported bodies are (mitcad#17, mitcad#19).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coordinates {
    /// Where the design shows them: placed by their components'
    /// occurrences, a body placed twice written twice (the default).
    Design,
    /// Each body once, in its own component's coordinates.
    Component,
}

/// Which placements of the bodies [`Document::export_bodies`] writes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExportPlacement {
    /// The design's coordinates when `None`.
    pub coordinates: Option<Coordinates>,
    /// Only the placements by this occurrence (a path from the root) and
    /// the occurrences in it; in the design's coordinates.
    pub within: Option<Vec<OccurrenceUid>>,
    /// 3MF and STL: the bodies on the print bed by their print bed faces
    /// (mitcad#731, `bed.rs`).
    pub on_bed: bool,
}

/// A body as one triangle mesh ([`Kernel::triangle_mesh`]), millimetres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriangleMesh {
    pub vertices: Vec<[f64; 3]>,
    /// Indices into `vertices`, counter-clockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

/// A body's faces as triangles for display ([`Kernel::face_meshes`],
/// mitcad#123), millimetres.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FaceMeshes {
    pub vertices: Vec<[f64; 3]>,
    /// A unit normal per vertex, out of the body.
    pub normals: Vec<[f64; 3]>,
    /// Counter-clockwise seen from outside, face by face.
    pub triangles: Vec<[u32; 3]>,
    /// Each face's number of triangles, in the order [`Kernel::faces`]
    /// lists the faces.
    pub face_triangles: Vec<u32>,
}

/// What [`Document::export_bodies`] wrote.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Exported {
    /// The bodies written, in order.
    pub bodies: Vec<BodyUid>,
    /// Bodies left out and why (3MF of all bodies: sheet and empty bodies;
    /// USDZ of all bodies: hidden and empty bodies).
    pub skipped: Vec<(BodyUid, String)>,
    /// 3MF: the mesh of each written body, in `bodies` order.
    pub meshes: Vec<MeshSummary>,
    /// USDZ: the materials written (appearance ids, `color rrggbb`,
    /// `default look`).
    pub materials: Vec<String>,
    /// USDZ: what was written otherwise than the design has it (a texture
    /// whose image cannot be read); 3MF with a slicer's settings: what the
    /// slicer does not take as the design has it (mitcad#640).
    pub warnings: Vec<String>,
    /// 3MF with a slicer's settings (mitcad#640): the filament slot of each
    /// written body (0 the first).
    pub slots: Vec<usize>,
    /// 3MF and STL on the print bed (mitcad#731): how the bodies were put
    /// there.
    pub on_bed: Option<OnBed>,
}

/// A body's mesh in a 3MF file: its triangles and the volume they enclose
/// (mm³), which differs from the body's by the refinement's deviation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshSummary {
    pub triangles: usize,
    pub volume: f64,
}

/// STEP application protocol.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepSchema {
    /// AP214 (automotive design), read by nearly every program.
    #[default]
    Ap214,
    /// AP242 (managed model based 3D engineering).
    Ap242,
}

/// Length unit of a STEP, IGES, STL, OBJ or 3MF file (the mesh formats
/// since mitcad#734).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    #[default]
    #[serde(rename = "mm")]
    Millimeter,
    #[serde(rename = "cm")]
    Centimeter,
    #[serde(rename = "m")]
    Meter,
    #[serde(rename = "in")]
    Inch,
    #[serde(rename = "ft")]
    Foot,
}

impl LengthUnit {
    /// The length of one unit in millimetres.
    pub fn millimeters(self) -> f64 {
        match self {
            Self::Millimeter => 1.0,
            Self::Centimeter => 10.0,
            Self::Meter => 1000.0,
            Self::Inch => 25.4,
            Self::Foot => 304.8,
        }
    }

    /// The unit of a 3MF file's model.
    fn three_mf(self) -> mitcad_3mf::Unit {
        match self {
            Self::Millimeter => mitcad_3mf::Unit::Millimeter,
            Self::Centimeter => mitcad_3mf::Unit::Centimeter,
            Self::Meter => mitcad_3mf::Unit::Meter,
            Self::Inch => mitcad_3mf::Unit::Inch,
            Self::Foot => mitcad_3mf::Unit::Foot,
        }
    }
}

/// Mesh refinement of STL and OBJ export: a preset or the
/// tolerances themselves.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Refinement {
    Preset(RefinementPreset),
    Custom(MeshTolerance),
}

impl Default for Refinement {
    fn default() -> Self {
        Self::Preset(RefinementPreset::Medium)
    }
}

/// The presets bound each doubly curved face's triangles (mitcad#398): a
/// face that would get more at the preset's deviation, which only faces
/// larger than about 15 cm can, meshes with a larger deviation that
/// grows with its size, so that a dome of metres exports in about the
/// triangles of one of centimetres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefinementPreset {
    /// 0.1 mm surface deviation, 30 degrees between facet normals, about
    /// 5 000 triangles a face at most.
    Low,
    /// 0.03 mm, 15 degrees, 20 000 triangles.
    Medium,
    /// 0.01 mm, 8 degrees, 50 000 triangles.
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshTolerance {
    /// Largest distance between the mesh and the surface, millimetres.
    pub deviation: f64,
    /// Largest angle between neighbouring facet normals, radians.
    pub angle: f64,
    /// About how many triangles a doubly curved face gets at most; such a
    /// face that would get more meshes with a larger deviation (mitcad#398,
    /// `geometry/io` `MeshOptions::face_triangles`). 0 (the default of a
    /// tolerance given as numbers): `deviation` everywhere.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub face_triangles: u64,
    /// The longest edge a triangle of an exported mesh may have, mm
    /// (mitcad#734): STL, OBJ and 3MF split longer edges at their middles
    /// after the kernel's meshing (`edges.rs`), which keeps the mesh's shape
    /// and adds triangles only. 0 (the default, and the presets'): no limit.
    /// The kernel's meshing does not use it.
    #[serde(default, skip_serializing_if = "is_no_length")]
    pub max_edge: f64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

fn is_no_length(length: &f64) -> bool {
    *length == 0.0
}

impl Refinement {
    /// The tolerances: (surface deviation in mm, normal angle in radians,
    /// triangles a face at most).
    pub fn tolerance(self) -> MeshTolerance {
        let (deviation, degrees, face_triangles) = match self {
            Self::Custom(tolerance) => return tolerance,
            Self::Preset(RefinementPreset::Low) => (0.1, 30.0_f64, 5_000),
            Self::Preset(RefinementPreset::Medium) => (0.03, 15.0, 20_000),
            Self::Preset(RefinementPreset::High) => (0.01, 8.0, 50_000),
        };
        MeshTolerance {
            deviation,
            angle: degrees.to_radians(),
            face_triangles,
            max_edge: 0.0,
        }
    }
}

/// Options of [`Kernel::write_file`]; each format uses its own.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub step_schema: StepSchema,
    /// STEP, IGES, STL, OBJ and 3MF (the kernel's OBJ writer scales to it).
    pub unit: LengthUnit,
    /// STL, OBJ, 3MF and USDZ.
    pub refinement: Refinement,
    /// STL: text instead of binary.
    pub ascii: bool,
    /// USDZ: the design file's folder, which relative texture paths are
    /// relative to.
    pub design_folder: Option<PathBuf>,
    /// 3MF: the slicer whose settings go with the bodies (mitcad#640):
    /// each body's effective printer, process and filament in its dialect.
    pub dialect: Option<crate::print_keys::Dialect>,
}

impl ExportOptions {
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            step_schema: StepSchema::default(),
            unit: LengthUnit::default(),
            refinement: Refinement::default(),
            ascii: false,
            design_folder: None,
            dialect: None,
        }
    }
}

/// A base feature to add from shapes (see [`Document::add_base_feature`]).
#[derive(Debug, Clone)]
pub struct BaseInput<S> {
    /// The feature's name; `Base<n>` when None.
    pub name: Option<String>,
    /// Where the bodies come from, e.g. a file name.
    pub source: Option<String>,
    pub bodies: Vec<ImportBody<S>>,
    pub operation: Operation,
    /// For join, cut and intersect; empty for all bodies.
    pub participants: Vec<BodyUid>,
    /// New bodies only: the bodies they replace, body i taking the id and
    /// name of `replaces[i]` (see [`crate::features::BaseDef::replaces`]).
    pub replaces: Vec<BodyUid>,
    /// The component it goes into (F6); the active component when None.
    pub component: Option<ComponentUid>,
}

impl<S> BaseInput<S> {
    /// New bodies from the shapes.
    pub fn new(bodies: Vec<ImportBody<S>>) -> Self {
        Self {
            name: None,
            source: None,
            bodies,
            operation: Operation::NewBody,
            participants: Vec::new(),
            replaces: Vec::new(),
            component: None,
        }
    }
}

/// A body a base feature created.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedBody {
    pub uid: BodyUid,
    pub name: String,
    pub kind: BodyKind,
}

/// The result of an import: the base feature and the bodies it created.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub feature: Added,
    pub bodies: Vec<ImportedBody>,
    /// What the import left out ([`Document::import_file`]: empty bodies).
    pub warnings: Vec<String>,
}

fn invalid(message: impl Into<String>) -> ModelError {
    ModelError::Invalid(message.into())
}

fn kernel_error(e: crate::kernel::KernelError) -> ModelError {
    invalid(e.to_string())
}

/// The file name without directories, for the `source` of a base feature.
fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_owned()
}

/// `name`, or `name (2)`, `name (3)`, ... the first one not taken.
pub(crate) fn unique_name(name: &str, taken: &mut BTreeSet<String>) -> String {
    let name = name.trim();
    let unique = std::iter::once(name.to_owned())
        .chain((2..).map(|n| format!("{name} ({n})")))
        .find(|n| !taken.contains(n))
        .expect("an unused name exists");
    taken.insert(unique.clone());
    unique
}

impl<K: Kernel> Document<K> {
    /// Shapes as the bodies of a base feature: B-rep data with their names
    /// and colours.
    pub(crate) fn base_bodies(
        &self,
        bodies: &[ImportBody<K::Shape>],
    ) -> Result<Vec<BaseBody>, ModelError> {
        let mut out = Vec::with_capacity(bodies.len());
        for (i, body) in bodies.iter().enumerate() {
            let fail = |e: crate::kernel::KernelError| invalid(format!("body {i}: {e}"));
            let kind = self.kernel().body_kind(&body.shape).map_err(fail)?;
            if kind == BodyKind::Empty {
                return Err(invalid(format!("body {i} is empty")));
            }
            let data = self.kernel().brep_data(&body.shape).map_err(fail)?;
            out.push(BaseBody {
                name: body.name.clone().filter(|n| !n.trim().is_empty()),
                color: body.color,
                mesh: kind == BodyKind::Mesh,
                brep: Brep::new(data),
            });
        }
        Ok(out)
    }

    /// Adds a base feature holding the shapes as bodies, at the marker:
    /// new bodies named after the shapes (unnamed ones get `Body<n>`), or a
    /// join, cut or intersect with participant bodies. This is how an
    /// importer adds bodies it built itself, such as the .f3d importer's
    /// bodies without history and its fallback for features it cannot
    /// replay. The shapes are stored as B-rep data in the feature.
    pub fn add_base_feature(&mut self, base: BaseInput<K::Shape>) -> Result<Imported, ModelError> {
        let label = match &base.name {
            Some(name) => format!("Add {name}"),
            None => "Add Base Feature".to_owned(),
        };
        let mut imported = self.add_base_features(&label, vec![base])?;
        Ok(imported.remove(0))
    }

    /// Adds several base features as one undo step named `label`.
    pub fn add_base_features(
        &mut self,
        label: &str,
        features: Vec<BaseInput<K::Shape>>,
    ) -> Result<Vec<Imported>, ModelError> {
        if features.is_empty() {
            return Err(invalid("nothing to import"));
        }
        let mut defs = Vec::with_capacity(features.len());
        // Each body's data with the shape it was written from: the
        // evaluation adopts the shape rather than reading the data back
        // (mitcad#244).
        let mut written_from = Vec::new();
        for base in &features {
            let component = base.component.unwrap_or(self.active_component());
            self.state().editable(component)?;
            let bodies = self.base_bodies(&base.bodies)?;
            for (body, input) in bodies.iter().zip(&base.bodies) {
                written_from.push((body.brep.clone(), input.shape.clone()));
            }
            let def = FeatureDef::<ValueInput>::Base(BaseDef {
                operation: base.operation,
                participants: base.participants.clone(),
                source: base.source.clone(),
                replaces: base.replaces.clone(),
                ..BaseDef::new(bodies)
            });
            defs.push((def, base.name.clone(), component));
        }
        let mut taken: BTreeSet<String> = self.bodies().into_iter().map(|b| b.name).collect();
        let added = self.with_written_from(written_from, |doc| {
            doc.apply(|state| {
                taken.extend(state.body_names.values().cloned());
                let mut added = Vec::with_capacity(defs.len());
                for (def, name, component) in &defs {
                    let feature = state.add_feature_in(def, name.as_deref(), *component)?;
                    if let FeatureDef::Base(base) = def
                        && base.operation == Operation::NewBody
                    {
                        // Bodies that replace others keep those bodies' names.
                        let replacing = base.replaces.len();
                        for (i, body) in (0..).zip(&base.bodies).skip(replacing) {
                            if let Some(name) = &body.name {
                                let name = unique_name(name, &mut taken);
                                state.body_names.insert(BodyUid::new(feature.uid, i), name);
                            }
                        }
                    }
                    added.push(feature);
                }
                Ok((label.to_owned(), added))
            })
        })?;
        let all = self.bodies();
        let mut result = Vec::with_capacity(added.len());
        for (feature, base) in added.into_iter().zip(&features) {
            let mut bodies = Vec::new();
            let mine = |uid: BodyUid| uid.feature == feature.uid || base.replaces.contains(&uid);
            for body in all.iter().filter(|b| mine(b.uid)) {
                bodies.push(ImportedBody {
                    uid: body.uid,
                    name: body.name.clone(),
                    kind: self.kernel().body_kind(body.shape).map_err(kernel_error)?,
                });
            }
            result.push(Imported {
                feature,
                bodies,
                warnings: Vec::new(),
            });
        }
        Ok(result)
    }

    /// Imports a STEP, IGES, BRep, STL or OBJ file as a base feature: one
    /// body per part or solid (STEP, IGES, BRep) or object (OBJ), a mesh
    /// body for STL. Empty bodies (a part without geometry) are left out
    /// with a warning; a file of only empty bodies is refused.
    pub fn import_file(
        &mut self,
        path: &str,
        options: &ReadOptions,
        name: Option<&str>,
    ) -> Result<Imported, ModelError> {
        if !(options.unit_mm > 0.0 && options.unit_mm.is_finite()) {
            return Err(invalid(format!(
                "the file unit must be greater than zero, got {} mm",
                options.unit_mm
            )));
        }
        let bodies = self
            .kernel()
            .read_file(path, options)
            .map_err(kernel_error)?;
        if bodies.is_empty() {
            return Err(invalid(format!("{path} has no bodies")));
        }
        // Here, unlike in base_bodies, an empty body is no error: nothing
        // refers to the file's bodies by index yet.
        let count = bodies.len();
        let mut kept = Vec::with_capacity(count);
        let mut warnings = Vec::new();
        for (i, body) in bodies.into_iter().enumerate() {
            let kind = self
                .kernel()
                .body_kind(&body.shape)
                .map_err(|e| invalid(format!("body {i}: {e}")))?;
            if kind == BodyKind::Empty {
                let named = match body.name.as_deref().map(str::trim) {
                    Some(name) if !name.is_empty() => format!("body {i} ({name})"),
                    _ => format!("body {i}"),
                };
                warnings.push(format!("{named} is empty: left out"));
            } else {
                kept.push(body);
            }
        }
        if kept.is_empty() {
            return Err(invalid(format!(
                "{path} has only empty bodies ({count}): nothing to import"
            )));
        }
        let source = file_name(path);
        let base = BaseInput {
            name: name.map(str::to_owned),
            source: Some(source.clone()),
            ..BaseInput::new(kept)
        };
        let mut imported = self
            .add_base_features(&format!("Import {source}"), vec![base])?
            .remove(0);
        imported.warnings = warnings;
        Ok(imported)
    }

    /// Writes bodies at the marker to a file; all bodies when `bodies` is
    /// empty. Bodies keep their display names, and `colors` or else the
    /// colours of imported bodies. Mesh bodies go only to STL, OBJ, BRep,
    /// 3MF and USDZ files. Every format has the bodies where the design
    /// shows them (mitcad#17, mitcad#19), unless `placement` asks for the components'
    /// coordinates; with an occurrence, only its placements (all bodies:
    /// those it places). A 3MF file holds the solids and meshes (of all
    /// bodies, sheet and empty bodies are left out) as the parts of one
    /// object, each placed where its component's occurrences place it; a
    /// STEP file of moved bodies is an assembly of their parts; a USDZ file
    /// keeps the occurrences as a hierarchy and the bodies' appearances (of
    /// all bodies, hidden ones are left out; `usdz`).
    pub fn export_bodies(
        &self,
        path: &str,
        bodies: &[BodyUid],
        options: &ExportOptions,
        colors: &BTreeMap<BodyUid, [f64; 3]>,
        placement: &ExportPlacement,
    ) -> Result<Exported, ModelError> {
        let format = options.format;
        let design = placement.coordinates != Some(Coordinates::Component);
        let within = placement.within.as_deref();
        if within.is_some() && !design {
            return Err(invalid(
                "the placements of an occurrence are in the design's coordinates",
            ));
        }
        if placement.on_bed {
            if !matches!(
                format,
                ExportFormat::ThreeMf | ExportFormat::Stl | ExportFormat::Obj
            ) {
                return Err(invalid(
                    "only 3MF, STL and OBJ files put the bodies on the print bed",
                ));
            }
            if !design {
                return Err(invalid(
                    "bodies are put on the print bed from the design's coordinates",
                ));
            }
        }
        let all = self.bodies();
        let mut chosen: Vec<_> = if bodies.is_empty() {
            all.iter().collect()
        } else {
            let mut chosen = Vec::new();
            for (i, uid) in bodies.iter().enumerate() {
                if bodies[..i].contains(uid) {
                    return Err(invalid(format!("body {uid} is listed more than once")));
                }
                chosen.push(all.iter().find(|b| b.uid == *uid).ok_or_else(|| {
                    invalid(format!("body {uid} does not exist at the timeline marker"))
                })?);
            }
            chosen
        };
        // Where the design shows the bodies: the instances of their
        // components (within the occurrence).
        let instances = if design { self.instances() } else { Vec::new() };
        let inside = |i: &InstanceView| within.is_none_or(|w| i.path.starts_with(w));
        let placed = |body: BodyUid| instances.iter().any(|i| i.body == body && inside(i));
        if let Some(occurrence) = within {
            if bodies.is_empty() {
                chosen.retain(|b| placed(b.uid));
            } else if let Some(body) = chosen.iter().find(|b| !placed(b.uid)) {
                return Err(invalid(format!(
                    "{} is not placed in {}",
                    body.name,
                    self.assembly().path_name(occurrence)
                )));
            }
        }
        if chosen.is_empty() {
            return Err(invalid("there are no bodies to export"));
        }
        let tolerance = options.refinement.tolerance();
        if !(tolerance.deviation > 0.0 && tolerance.angle > 0.0) {
            return Err(invalid(
                "the mesh deviation and angle must be greater than zero",
            ));
        }
        if !(tolerance.max_edge >= 0.0 && tolerance.max_edge.is_finite()) {
            return Err(invalid(
                "the maximum edge length must be greater than zero (or 0 for none)",
            ));
        }
        let color = |uid: BodyUid| {
            colors
                .get(&uid)
                .copied()
                .or_else(|| self.imported_color(uid))
        };
        // A body's placements: by the shown occurrences of its component,
        // by all of them when none is shown; once in component coordinates.
        let placed_at = |body: BodyUid| {
            let mine: Vec<&InstanceView> = instances
                .iter()
                .filter(|i| i.body == body && inside(i))
                .collect();
            let shown: Vec<&InstanceView> = mine.iter().copied().filter(|i| i.visible).collect();
            let mut out: Vec<BodyPlacement> = if shown.is_empty() { mine } else { shown }
                .iter()
                .map(|i| BodyPlacement {
                    transform: i.transform,
                    name: i.path_name.clone(),
                })
                .collect();
            if out.is_empty() {
                out.push(BodyPlacement {
                    transform: Transform::IDENTITY,
                    name: String::new(),
                });
            }
            out
        };
        let placements = |body: BodyUid| -> Vec<Transform> {
            placed_at(body).into_iter().map(|p| p.transform).collect()
        };
        // On the print bed (mitcad#731): the bodies' bed faces, and the turn
        // that takes the one face down.
        let mut on_bed = if placement.on_bed {
            let uids: Vec<BodyUid> = chosen.iter().map(|b| b.uid).collect();
            Some(self.bed_plan(&uids, &placements))
        } else {
            None
        };
        if format == ExportFormat::ThreeMf {
            // The occurrence path a body's print settings are of: its first
            // shown placement's (mitcad#640).
            let path_of = |body: BodyUid| -> Option<Vec<OccurrenceUid>> {
                let mine: Vec<&InstanceView> = instances
                    .iter()
                    .filter(|i| i.body == body && inside(i))
                    .collect();
                mine.iter()
                    .find(|i| i.visible)
                    .or_else(|| mine.first())
                    .map(|i| i.path.clone())
            };
            let mut exported = self.export_3mf(
                path,
                &chosen,
                bodies.is_empty(),
                &tolerance,
                options.unit,
                color,
                placements,
                options.dialect.map(|d| {
                    (
                        d,
                        &path_of as &dyn Fn(BodyUid) -> Option<Vec<OccurrenceUid>>,
                    )
                }),
                on_bed.as_mut(),
            )?;
            self.report_bed(&mut exported, on_bed);
            return Ok(exported);
        }
        if format == ExportFormat::Stl {
            let mut exported = self.export_stl(
                path,
                &chosen,
                &tolerance,
                options,
                placements,
                on_bed.as_mut(),
            )?;
            self.report_bed(&mut exported, on_bed);
            return Ok(exported);
        }
        if format == ExportFormat::Obj && (on_bed.is_some() || tolerance.max_edge > 0.0) {
            // The model's meshes (mitcad#734): split to the longest edge,
            // and placed on the print bed; the kernel writes them.
            let placed: Vec<Vec<BodyPlacement>> = chosen
                .iter()
                .map(|b| if design { placed_at(b.uid) } else { Vec::new() })
                .collect();
            let mut exported = self.export_obj_meshes(
                path,
                &chosen,
                &placed,
                &tolerance,
                options,
                color,
                on_bed.as_mut(),
            )?;
            self.report_bed(&mut exported, on_bed);
            return Ok(exported);
        }
        if format == ExportFormat::Usdz {
            // The placements with their occurrence paths (mitcad#123).
            let inside: Vec<InstanceView> =
                instances.iter().filter(|i| inside(i)).cloned().collect();
            return self.export_usdz(
                path,
                &chosen,
                bodies.is_empty(),
                &tolerance,
                color,
                design.then_some(inside.as_slice()),
                within.map_or(0, <[OccurrenceUid]>::len),
                options.design_folder.as_deref(),
            );
        }
        // The kernel's formats (mitcad#19): each body with its placements in
        // the design's coordinates, without any in the components'.
        let placed: Vec<Vec<BodyPlacement>> = chosen
            .iter()
            .map(|b| if design { placed_at(b.uid) } else { Vec::new() })
            .collect();
        let mut out = Vec::with_capacity(chosen.len());
        for (body, placements) in chosen.iter().zip(&placed) {
            let kind = self.kernel().body_kind(body.shape).map_err(kernel_error)?;
            if kind == BodyKind::Mesh && !options.format.takes_meshes() {
                return Err(invalid(format!(
                    "{} is a mesh body, which cannot be written to {} (only to STL, OBJ, BRep or 3MF)",
                    body.name,
                    options.format.as_str().to_uppercase()
                )));
            }
            if kind == BodyKind::Empty {
                return Err(invalid(format!("{} is empty", body.name)));
            }
            out.push(ExportBody {
                name: &body.name,
                color: color(body.uid),
                shape: body.shape,
                placements,
            });
        }
        self.kernel()
            .write_file(path, &out, options)
            .map_err(kernel_error)?;
        Ok(Exported {
            bodies: chosen.iter().map(|b| b.uid).collect(),
            ..Exported::default()
        })
    }

    /// A body's triangle mesh for a mesh file: the kernel's within the
    /// tolerance, its edges split to `max_edge` (mitcad#734). An empty
    /// body, or one without triangles, is an error.
    fn export_mesh(
        &self,
        body: &BodyView<'_, K::Shape>,
        tolerance: &MeshTolerance,
    ) -> Result<TriangleMesh, ModelError> {
        let kind = self.kernel().body_kind(body.shape).map_err(kernel_error)?;
        if kind == BodyKind::Empty {
            return Err(invalid(format!("{} is empty", body.name)));
        }
        let mesh = self
            .kernel()
            .triangle_mesh(body.shape, tolerance)
            .map_err(|e| invalid(format!("{}: {e}", body.name)))?;
        if mesh.triangles.is_empty() {
            return Err(invalid(format!("{} has no triangles", body.name)));
        }
        edges::split_long_edges(mesh, tolerance.max_edge, edges::MOST_TRIANGLES).map_err(|e| {
            invalid(format!(
                "{}: the maximum edge length {} mm would make more than {} triangles",
                body.name, tolerance.max_edge, e.most
            ))
        })
    }

    /// An STL file of the bodies (mitcad#17): their triangle meshes as one
    /// solid, each body once per placement (in the design's coordinates,
    /// or once in its component's); turned onto the print bed face, the
    /// lowest point on the bed and centred on the plate, with `bed`'s turn
    /// (mitcad#731); text with `options.ascii`, in `options.unit`
    /// (mitcad#734).
    fn export_stl(
        &self,
        path: &str,
        chosen: &[&BodyView<'_, K::Shape>],
        tolerance: &MeshTolerance,
        options: &ExportOptions,
        placements: impl Fn(BodyUid) -> Vec<Transform>,
        on_bed: Option<&mut OnBed>,
    ) -> Result<Exported, ModelError> {
        let mut all = TriangleMesh::default();
        for body in chosen {
            let mesh = self.export_mesh(body, tolerance)?;
            for placement in placements(body.uid) {
                // The first vertex's index; every index must fit in a u32.
                let first = u32::try_from(all.vertices.len() + mesh.vertices.len())
                    .map(|_| all.vertices.len() as u32)
                    .map_err(|_| invalid("the bodies have too many vertices for one STL file"))?;
                all.vertices
                    .extend(mesh.vertices.iter().map(|v| placement.apply_point(*v)));
                // A mirroring placement would turn the triangles inside out.
                let mirrored = placement.determinant() < 0.0;
                all.triangles.extend(mesh.triangles.iter().map(|t| {
                    let [a, b, c] = t.map(|i| first + i);
                    if mirrored { [a, c, b] } else { [a, b, c] }
                }));
            }
        }
        // On the print bed (mitcad#731): a turn keeps the triangles facing
        // out.
        if let Some(on_bed) = on_bed
            && let Some(turn) = on_bed.transform
        {
            let moved = bed::settled(&turn, all.vertices.iter().copied(), on_bed.plate);
            for vertex in &mut all.vertices {
                *vertex = moved.apply_point(*vertex);
            }
            on_bed.transform = Some(moved);
        }
        // In the file's unit (mitcad#734).
        let scale = 1.0 / options.unit.millimeters();
        if scale != 1.0 {
            for vertex in &mut all.vertices {
                *vertex = vertex.map(|v| v * scale);
            }
        }
        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Mitcad");
        std::fs::write(path, stl::write(&all, name, options.ascii))
            .map_err(|e| invalid(format!("cannot write {path}: {e}")))?;
        Ok(Exported {
            bodies: chosen.iter().map(|b| b.uid).collect(),
            ..Exported::default()
        })
    }

    /// An OBJ file of the model's meshes (mitcad#734): each body's mesh
    /// split to the longest edge, made a mesh body
    /// ([`Kernel::triangle_body`]) and written by the kernel at its
    /// `placed` placements, which `on_bed`'s turn moves onto the print bed
    /// face, the lowest point on the bed and centred on the plate, as STL
    /// does (mitcad#731). The kernel scales to `options.unit`.
    #[allow(clippy::too_many_arguments)]
    fn export_obj_meshes(
        &self,
        path: &str,
        chosen: &[&BodyView<'_, K::Shape>],
        placed: &[Vec<BodyPlacement>],
        tolerance: &MeshTolerance,
        options: &ExportOptions,
        color: impl Fn(BodyUid) -> Option<[f64; 3]>,
        on_bed: Option<&mut OnBed>,
    ) -> Result<Exported, ModelError> {
        let mut meshes = Vec::with_capacity(chosen.len());
        for body in chosen {
            let mesh = self.export_mesh(body, tolerance)?;
            let shape = self
                .kernel()
                .triangle_body(&mesh.vertices, &mesh.triangles)
                .map_err(|e| invalid(format!("{}: {e}", body.name)))?;
            meshes.push((mesh, shape));
        }
        let mut placed = placed.to_vec();
        // On the print bed (mitcad#731): every placement moved alike, as
        // the bodies are one piece.
        if let Some(on_bed) = on_bed
            && let Some(turn) = on_bed.transform
        {
            let mut points = Vec::new();
            for ((mesh, _), placements) in meshes.iter().zip(&placed) {
                if placements.is_empty() {
                    points.extend(mesh.vertices.iter().copied());
                }
                for placement in placements {
                    points.extend(
                        mesh.vertices
                            .iter()
                            .map(|v| placement.transform.apply_point(*v)),
                    );
                }
            }
            let moved = bed::settled(&turn, points.into_iter(), on_bed.plate);
            for placements in &mut placed {
                if placements.is_empty() {
                    placements.push(BodyPlacement {
                        transform: Transform::IDENTITY,
                        name: String::new(),
                    });
                }
                for placement in placements.iter_mut() {
                    placement.transform = moved.after(&placement.transform);
                }
            }
            on_bed.transform = Some(moved);
        }
        let out: Vec<ExportBody<'_, K::Shape>> = chosen
            .iter()
            .zip(&meshes)
            .zip(&placed)
            .map(|((body, (_, shape)), placements)| ExportBody {
                name: &body.name,
                color: color(body.uid),
                shape,
                placements,
            })
            .collect();
        self.kernel()
            .write_file(path, &out, options)
            .map_err(kernel_error)?;
        Ok(Exported {
            bodies: chosen.iter().map(|b| b.uid).collect(),
            ..Exported::default()
        })
    }

    /// A 3MF file of the bodies (mitcad#13): each body's triangle mesh a
    /// part, placed by `placements` (the visible occurrences of its
    /// component, all of them when none is visible), named after the body,
    /// in its colour. Sheet and empty bodies are an error when chosen, and
    /// left out when all bodies are written (`all`). With `print` (a
    /// slicer, and the occurrence path whose settings a body prints with),
    /// each body's effective print settings go with it in the slicer's
    /// dialect (mitcad#640, `crate::print_files`). With `on_bed`'s turn the
    /// parts' placements turn onto the print bed face, the lowest point on
    /// the bed and the middle over the plate's centre (mitcad#731): the
    /// meshes stay as the kernel made them. The model is in `unit`, its
    /// meshes and placements scaled to it (mitcad#734).
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::type_complexity)]
    fn export_3mf(
        &self,
        path: &str,
        chosen: &[&BodyView<'_, K::Shape>],
        all: bool,
        tolerance: &MeshTolerance,
        unit: LengthUnit,
        color: impl Fn(BodyUid) -> Option<[f64; 3]>,
        placements: impl Fn(BodyUid) -> Vec<Transform>,
        print: Option<(
            crate::print_keys::Dialect,
            &dyn Fn(BodyUid) -> Option<Vec<OccurrenceUid>>,
        )>,
        on_bed: Option<&mut OnBed>,
    ) -> Result<Exported, ModelError> {
        let mut exported = Exported::default();
        let mut parts = Vec::with_capacity(chosen.len());
        for body in chosen {
            let kind = self.kernel().body_kind(body.shape).map_err(kernel_error)?;
            if matches!(kind, BodyKind::Sheet | BodyKind::Empty) {
                let why = format!("a {} body", kind.as_str());
                if !all {
                    return Err(invalid(format!(
                        "{} is {why}; 3MF takes solids and mesh bodies",
                        body.name
                    )));
                }
                exported.skipped.push((body.uid, why));
                continue;
            }
            let mesh = self.export_mesh(body, tolerance)?;
            let placements = placements(body.uid)
                .iter()
                .map(|t| mitcad_3mf::Placement {
                    linear: t.linear,
                    translation: t.translation,
                })
                .collect();
            let mesh = mitcad_3mf::Mesh {
                vertices: mesh.vertices,
                triangles: mesh.triangles,
            };
            exported.bodies.push(body.uid);
            exported.meshes.push(MeshSummary {
                triangles: mesh.triangles.len(),
                volume: mesh.volume(),
            });
            parts.push(mitcad_3mf::Part {
                name: body.name.clone(),
                color: color(body.uid),
                mesh,
                placements,
                metadata: Vec::new(),
            });
        }
        if parts.is_empty() {
            return Err(invalid("there are no solid or mesh bodies to export"));
        }
        // On the print bed (mitcad#731): every placement moved alike, as the
        // bodies are one object.
        if let Some(on_bed) = on_bed
            && let Some(turn) = on_bed.transform
        {
            let mut points = Vec::new();
            for part in &parts {
                for placement in &part.placements {
                    points.extend(part.mesh.vertices.iter().map(|v| placement.apply(*v)));
                }
            }
            let moved = bed::settled(&turn, points.into_iter(), on_bed.plate);
            let outer = mitcad_3mf::Placement {
                linear: moved.linear,
                translation: moved.translation,
            };
            for part in &mut parts {
                for placement in &mut part.placements {
                    *placement = outer.after(placement);
                }
            }
            on_bed.transform = Some(moved);
        }
        // In the file's unit (mitcad#734): the meshes and the placements'
        // moves scaled, the turns as they are.
        let scale = 1.0 / unit.millimeters();
        if scale != 1.0 {
            for part in &mut parts {
                for vertex in &mut part.mesh.vertices {
                    *vertex = vertex.map(|v| v * scale);
                }
                for placement in &mut part.placements {
                    placement.translation = placement.translation.map(|v| v * scale);
                }
            }
        }
        let mut model = mitcad_3mf::Model {
            name: Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Mitcad")
                .to_owned(),
            parts,
            unit: unit.three_mf(),
            application: format!("Mitcad {}", env!("CARGO_PKG_VERSION")),
            ..mitcad_3mf::Model::default()
        };
        // The slicer's settings (mitcad#640): each body's effective ones.
        if let Some((dialect, path_of)) = print {
            let settings: Vec<crate::document::EffectivePrint> = exported
                .bodies
                .iter()
                .map(|uid| {
                    self.effective_print(&crate::document::PrintTarget::Body(*uid, path_of(*uid)))
                })
                .collect();
            let parts: Vec<crate::print_files::ExportPart<'_>> = model
                .parts
                .iter()
                .zip(&settings)
                .map(|(part, settings)| crate::print_files::ExportPart {
                    name: &part.name,
                    settings,
                    triangles: part.mesh.triangles.len(),
                    placements: part.placements.len(),
                })
                .collect();
            // With the purge volumes set by hand (mitcad#705).
            let written = crate::print_files::export(
                dialect,
                &model.name,
                &parts,
                &self.state().purge_volumes,
            );
            if let Some(application) = written.application {
                model.application = application;
            }
            model.metadata = written.metadata;
            model.namespaces = written.namespaces;
            model.files = written.files;
            for (part, metadata) in model.parts.iter_mut().zip(written.part_metadata) {
                part.metadata = metadata;
            }
            exported.slots = written.slots;
            exported.warnings.extend(written.warnings);
        }
        let data = mitcad_3mf::write(&model).map_err(|e| invalid(e.to_string()))?;
        std::fs::write(path, data).map_err(|e| invalid(format!("cannot write {path}: {e}")))?;
        Ok(exported)
    }

    /// The colour a base feature gave a new body.
    fn imported_color(&self, body: BodyUid) -> Option<[f64; 3]> {
        match &self.feature(body.feature)?.def {
            FeatureDef::Base(base) if base.operation == Operation::NewBody => {
                base.bodies.get(body.index as usize)?.color
            }
            _ => None,
        }
    }

    /// Writes the geometry of an evaluated sketch to a DXF file, in sketch
    /// coordinates and millimetres. Returns the number of entities.
    pub fn export_sketch_dxf(
        &self,
        sketch: FeatureUid,
        path: &str,
        options: &mitcad_dxf::WriteOptions,
    ) -> Result<usize, ModelError> {
        let entry = self
            .feature(sketch)
            .ok_or(ModelError::NoSuchFeature(sketch))?;
        if !matches!(entry.def, FeatureDef::Sketch(_)) {
            return Err(invalid(format!(
                "{} ({sketch}) is not a sketch",
                entry.name
            )));
        }
        if !self.status(sketch).is_some_and(|s| s.is_ok()) {
            return Err(invalid(format!(
                "{} has no result to export (it failed, is suppressed or is after the marker)",
                entry.name
            )));
        }
        let FeatureDef::Sketch(def) = &entry.def else {
            unreachable!("checked above");
        };
        let output = self
            .sketch_output(sketch)
            .ok_or_else(|| invalid(format!("{} has no result to export", entry.name)))?;
        // Every curve of the sketch once, in its solved geometry;
        // construction geometry on its own layer; the lines that stand for
        // the sketch's axes (mitcad#500) are not drawn.
        let curves: Vec<(&str, String, SegmentGeometry)> = def
            .entities
            .iter()
            .filter(|e| e.axis.is_none())
            .filter_map(|e| {
                let curve = output.solved.curves.get(&e.id)?;
                let layer = if e.construction { "construction" } else { "0" };
                Some((layer, e.as_ref().to_string(), curve.whole()))
            })
            .collect();
        let drawing = sketch_drawing(curves.iter().map(|(l, n, g)| (*l, n.as_str(), g)))
            .map_err(|e| invalid(format!("{}: {e}", entry.name)))?;
        mitcad_dxf::write_file(&drawing, path, options)
            .map_err(|e| invalid(format!("cannot write {path}: {e}")))?;
        Ok(drawing.entities.len())
    }
}

/// The DXF drawing of a sketch's solved curves: each curve (its layer,
/// its name for messages and its geometry) once, in sketch coordinates and
/// millimetres.
///
/// This is the one place that maps sketch geometry to DXF.
pub(crate) fn sketch_drawing<'a>(
    curves: impl IntoIterator<Item = (&'a str, &'a str, &'a SegmentGeometry)>,
) -> Result<mitcad_dxf::Drawing, String> {
    use mitcad_dxf::{Drawing, Geometry, Point2, Spline, Units};

    let point = |p: [f64; 2]| Point2::new(p[0], p[1]);
    let mut drawing = Drawing::new(Units::Millimeters);
    {
        for (layer, name, segment_geometry) in curves {
            let geometry = match segment_geometry {
                SegmentGeometry::Line { start, end } => Geometry::Line {
                    start: point(*start),
                    end: point(*end),
                },
                SegmentGeometry::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => Geometry::arc(point(*center), *radius, *start_angle, *end_angle),
                SegmentGeometry::Circle { center, radius, .. } => Geometry::Circle {
                    center: point(*center),
                    radius: *radius,
                },
                SegmentGeometry::Ellipse {
                    center,
                    major_radius,
                    minor_radius,
                    rotation,
                } => ellipse(
                    point(*center),
                    *major_radius,
                    *minor_radius,
                    *rotation,
                    None,
                ),
                SegmentGeometry::EllipseArc {
                    center,
                    major_radius,
                    minor_radius,
                    rotation,
                    start_angle,
                    end_angle,
                } => ellipse(
                    point(*center),
                    *major_radius,
                    *minor_radius,
                    *rotation,
                    Some((*start_angle, *end_angle)),
                ),
                SegmentGeometry::BSpline {
                    degree,
                    poles,
                    weights,
                    knots,
                    multiplicities,
                    periodic,
                } => {
                    if *periodic {
                        return Err(format!(
                            "{name}: periodic splines cannot be written to DXF yet"
                        ));
                    }
                    Geometry::Spline(Spline {
                        degree: *degree,
                        control_points: poles.iter().map(|p| point(*p)).collect(),
                        knots: knots
                            .iter()
                            .zip(multiplicities)
                            .flat_map(|(k, m)| std::iter::repeat_n(*k, *m as usize))
                            .collect(),
                        weights: weights.clone(),
                        ..Spline::default()
                    })
                }
            };
            drawing.push(layer, geometry);
        }
    }
    Ok(drawing)
}

/// An ellipse, or the arc between two parameter angles counter-clockwise,
/// with the first axis `rotation` from the x axis. DXF needs the major axis
/// to be the longer one, so a longer second axis turns the frame a quarter
/// and the parameters back by as much.
fn ellipse(
    center: mitcad_dxf::Point2,
    first: f64,
    second: f64,
    rotation: f64,
    arc: Option<(f64, f64)>,
) -> mitcad_dxf::Geometry {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let turn = if second > first { FRAC_PI_2 } else { 0.0 };
    let (major, minor) = (first.max(second), first.min(second));
    let (start_param, end_param) = match arc {
        None => (0.0, TAU),
        Some((start, end)) => {
            let span = if end <= start {
                end + TAU - start
            } else {
                end - start
            };
            let start = (start - turn).rem_euclid(TAU);
            (start, start + span)
        }
    };
    mitcad_dxf::Geometry::Ellipse {
        center,
        major_axis: mitcad_dxf::Point2::polar(rotation + turn) * major,
        ratio: minor / major,
        start_param,
        end_param,
    }
}

#[cfg(test)]
#[path = "exchange_tests.rs"]
mod tests;
