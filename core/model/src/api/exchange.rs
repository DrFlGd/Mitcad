// SPDX-License-Identifier: MIT
//! Commands for data exchange: `import_file`, `export` and `export_sketch`
//! (see `commands.md`).

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use crate::document::Document;
use crate::exchange::{
    Coordinates, ExportFormat, ExportOptions, ExportPlacement, Imported, LengthUnit, ReadOptions,
    Refinement, StepSchema,
};
use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::Kernel;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImportFileCommand {
    path: String,
    /// The base feature's name.
    #[serde(default)]
    name: Option<String>,
    /// STL and OBJ: millimetres per file unit.
    #[serde(default)]
    unit_mm: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExportCommand {
    path: String,
    /// Bodies at the marker by uid or name; all when left out.
    #[serde(default)]
    bodies: Vec<String>,
    /// By the file extension when left out.
    #[serde(default)]
    format: Option<ExportFormat>,
    #[serde(default)]
    schema: StepSchema,
    #[serde(default)]
    unit: LengthUnit,
    #[serde(default)]
    refinement: Refinement,
    #[serde(default)]
    ascii: bool,
    /// Colours of bodies by uid or name (sRGB in [0, 1]), over imported
    /// ones: what the application shows (mitcad#13).
    #[serde(default)]
    colors: BTreeMap<String, [f64; 3]>,
    /// `design` (the default) or `component` (mitcad#17, mitcad#19).
    #[serde(default)]
    coordinates: Option<Coordinates>,
    /// Only the placements by this occurrence (`O1/O4` or `Arm:1/Pin:2`).
    #[serde(default)]
    occurrence: Option<String>,
    /// USDZ: the design file's folder, which relative texture paths are
    /// relative to (mitcad#123).
    #[serde(default)]
    design_folder: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExportSketchCommand {
    /// Uid or name.
    sketch: String,
    path: String,
    /// `r2000` (default) or `r12`.
    #[serde(default)]
    version: Option<String>,
}

pub(super) fn imported_json(imported: &Imported) -> Value {
    let bodies: Vec<Value> = imported
        .bodies
        .iter()
        .map(|b| json!({"uid": b.uid, "name": b.name, "kind": b.kind.as_str()}))
        .collect();
    let mut out =
        json!({"uid": imported.feature.uid, "name": imported.feature.name, "bodies": bodies});
    if !imported.warnings.is_empty() {
        out["warnings"] = json!(imported.warnings);
    }
    out
}

impl<K: Kernel> Document<K> {
    pub(super) fn import_file_command(
        &mut self,
        command: ImportFileCommand,
    ) -> Result<Value, ApiError> {
        let options = ReadOptions {
            unit_mm: command.unit_mm.unwrap_or(1.0),
        };
        let imported = self.import_file(&command.path, &options, command.name.as_deref())?;
        Ok(imported_json(&imported))
    }

    pub(super) fn export_command(&self, command: ExportCommand) -> Result<Value, ApiError> {
        let format = match command.format {
            Some(format) => format,
            None => ExportFormat::of_path(&command.path).ok_or_else(|| {
                ApiError(format!(
                    "{}: unknown file type; use .step, .iges, .stl, .obj, .brep, .3mf or .usdz, or give \"format\"",
                    command.path
                ))
            })?,
        };
        let options = ExportOptions {
            format,
            step_schema: command.schema,
            unit: command.unit,
            refinement: command.refinement,
            ascii: command.ascii,
            design_folder: command.design_folder.map(std::path::PathBuf::from),
        };
        let bodies = command
            .bodies
            .iter()
            .map(|body| self.body_by_uid_or_name(body))
            .collect::<Result<Vec<_>, _>>()?;
        let mut colors = BTreeMap::new();
        for (body, color) in &command.colors {
            if !color.iter().all(|c| (0.0..=1.0).contains(c)) {
                return Err(ApiError(format!(
                    "the colour of {body} must have components from 0 to 1"
                )));
            }
            colors.insert(self.body_by_uid_or_name(body)?, *color);
        }
        let within = match &command.occurrence {
            Some(path) => Some(
                self.assembly()
                    .find_path(path)
                    .ok_or_else(|| ApiError(format!("there is no occurrence '{path}'")))?,
            ),
            None => None,
        };
        let placement = ExportPlacement {
            coordinates: command.coordinates,
            within,
        };
        let exported = self.export_bodies(&command.path, &bodies, &options, &colors, &placement)?;
        let mut result =
            json!({"path": command.path, "format": format.as_str(), "bodies": exported.bodies});
        if format == ExportFormat::ThreeMf {
            // Each body's mesh, and the bodies left out (mitcad#13).
            result["meshes"] = exported
                .bodies
                .iter()
                .zip(&exported.meshes)
                .map(|(uid, mesh)| {
                    json!({"body": uid, "name": self.body_name(*uid),
                           "triangles": mesh.triangles, "volume": mesh.volume})
                })
                .collect();
            result["skipped"] = exported
                .skipped
                .iter()
                .map(|(uid, why)| json!({"body": uid, "name": self.body_name(*uid), "reason": why}))
                .collect();
        }
        if format == ExportFormat::Usdz {
            // The materials, the bodies left out and what was written
            // otherwise (mitcad#123).
            result["materials"] = json!(exported.materials);
            result["skipped"] = exported
                .skipped
                .iter()
                .map(|(uid, why)| json!({"body": uid, "name": self.body_name(*uid), "reason": why}))
                .collect();
            result["warnings"] = json!(exported.warnings);
        }
        Ok(result)
    }

    pub(super) fn export_sketch_command(
        &self,
        command: ExportSketchCommand,
    ) -> Result<Value, ApiError> {
        let extension = std::path::Path::new(&command.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if extension.as_deref() != Some("dxf") {
            return Err(ApiError(format!(
                "{}: sketches are exported to .dxf files",
                command.path
            )));
        }
        let version = match command.version.as_deref() {
            None | Some("r2000") => mitcad_dxf::DxfVersion::R2000,
            Some("r12") => mitcad_dxf::DxfVersion::R12,
            Some(other) => {
                return Err(ApiError(format!(
                    "unknown DXF version '{other}' (expected r2000 or r12)"
                )));
            }
        };
        let options = mitcad_dxf::WriteOptions {
            version,
            ..mitcad_dxf::WriteOptions::default()
        };
        let sketch = match command.sketch.parse::<FeatureUid>() {
            Ok(uid) => uid,
            Err(_) => self
                .features()
                .find(|f| f.name == command.sketch)
                .map(|f| f.uid)
                .ok_or_else(|| ApiError(format!("no feature is named '{}'", command.sketch)))?,
        };
        let entities = self.export_sketch_dxf(sketch, &command.path, &options)?;
        Ok(json!({"path": command.path, "entities": entities}))
    }

    /// A body at the marker by its uid or display name.
    fn body_by_uid_or_name(&self, body: &str) -> Result<BodyUid, ApiError> {
        if let Ok(uid) = body.parse::<BodyUid>() {
            return Ok(uid);
        }
        self.bodies()
            .into_iter()
            .find(|b| b.name == body)
            .map(|b| b.uid)
            .ok_or_else(|| ApiError(format!("no body at the timeline marker is named '{body}'")))
    }
}

/// `export_flat_pattern_dxf` (mitcad#210): a body's Flat Pattern as a DXF
/// drawing.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExportFlatPatternCommand {
    /// Uid or name of the sheet-metal body.
    body: String,
    path: String,
    /// The drawing unit (`mm` by default).
    #[serde(default)]
    unit: LengthUnit,
    /// `r2000` (default) or `r12`.
    #[serde(default)]
    version: Option<String>,
    /// Also the bends' tangent lines, on `BEND_TANGENT`.
    #[serde(default)]
    tangents: bool,
    /// The side the drawing looks at; the Flat Pattern's when left out.
    #[serde(default)]
    reference_side: Option<crate::sheet::SheetReferenceSide>,
}

impl<K: Kernel> Document<K> {
    pub(super) fn export_flat_pattern_command(
        &self,
        command: ExportFlatPatternCommand,
    ) -> Result<Value, ApiError> {
        use crate::sheet::flat_dxf::{self, FlatDxfOptions};
        let extension = std::path::Path::new(&command.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if extension.as_deref() != Some("dxf") {
            return Err(ApiError(format!(
                "{}: Flat Patterns are exported to .dxf files",
                command.path
            )));
        }
        let version = match command.version.as_deref() {
            None | Some("r2000") => mitcad_dxf::DxfVersion::R2000,
            Some("r12") => mitcad_dxf::DxfVersion::R12,
            Some(other) => {
                return Err(ApiError(format!(
                    "unknown DXF version '{other}' (expected r2000 or r12)"
                )));
            }
        };
        let write = mitcad_dxf::WriteOptions {
            version,
            ..mitcad_dxf::WriteOptions::default()
        };
        let body = self.body_by_uid_or_name(&command.body)?;
        let options = FlatDxfOptions {
            unit: command.unit,
            tangents: command.tangents,
        };
        let (def, drawing) = self.export_flat_pattern_dxf(
            body,
            &command.path,
            command.reference_side,
            &options,
            &write,
        )?;
        let count = |layer: &str| drawing.entities.iter().filter(|e| e.layer == layer).count();
        let layers: Vec<Value> = drawing
            .layers
            .iter()
            .map(|l| json!({"name": l.name, "entities": count(&l.name)}))
            .collect();
        let bounds = drawing
            .bounds()
            .map(|(min, max)| json!({"min": [min.x, min.y], "max": [max.x, max.y]}));
        Ok(json!({
            "path": command.path,
            "body": body,
            "unit": command.unit,
            "stationary": def.stationary,
            "reference_side": def.reference_side,
            "entities": drawing.entities.len(),
            "layers": layers,
            "bends": {"up": count(flat_dxf::BEND_UP), "down": count(flat_dxf::BEND_DOWN)},
            "bounds": bounds,
        }))
    }
}
