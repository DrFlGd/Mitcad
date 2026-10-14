// SPDX-License-Identifier: MIT
//! The `.rvt` / `.rfa` import, phase 1 (mitcad#274): `mitcad_rvt` reads the
//! file (metadata, schema, element table, element records), and
//! `mitcad_import::rvt` brings a project's building elements in as one
//! component per level, each element a body: its saved mesh, or its box
//! at its transform (OCCT through the model's kernel). A family file
//! (`.rfa`, `.rft`) opens with its metadata and report only: its forms are
//! a later ticket. The `import_rvt` document command and `mitcad-cli
//! import-rvt`; the memory guard watches the reading and the building
//! (mitcad#80), and an import that runs low on memory leaves the remaining
//! elements out and says so.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use mitcad_model::api::ApiError;
use mitcad_rvt::elements::Model;
use mitcad_rvt::{Kind, RvtFile};

use crate::Document;
use crate::f3d_import::MemoryGuard;

/// The options of `import_rvt` (JSON).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    /// A readable report instead of JSON.
    #[serde(default)]
    text: bool,
    /// Also write the JSON report to this file.
    #[serde(default)]
    report_path: Option<String>,
    /// Read and report only; import no elements.
    #[serde(default)]
    report_only: bool,
}

fn file_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

impl Document {
    fn import_rvt_report(&mut self, path: &str, options: &Options) -> Result<Value, ApiError> {
        let file = file_name(path);
        let fail = |e: &dyn std::fmt::Display| ApiError(format!("{file}: {e}"));
        let started = std::time::Instant::now();
        let progress = Arc::new(mitcad_import::Progress::new());
        let rvt = RvtFile::open(std::path::Path::new(path)).map_err(|e| fail(&e))?;
        let schema = rvt.schema().map_err(|e| fail(&e))?;
        let unknown = schema.unknown_encodings();
        // The reading and the building, watched by the memory guard.
        let (model, import) = MemoryGuard::with_limit(None)
            .around(&progress, || {
                let model = Model::read(&rvt, &schema)?;
                let import = if rvt.kind == Kind::Project && !options.report_only {
                    Some(
                        mitcad_import::rvt::import_rvt(&mut self.0, file, &model, Some(&*progress))
                            .map_err(|e| mitcad_rvt::RvtError::Stream(e.to_string()))?,
                    )
                } else {
                    None
                };
                Ok::<_, mitcad_rvt::RvtError>((model, import))
            })
            .map_err(|e| fail(&e))?;
        let mut warnings: Vec<String> = Vec::new();
        if rvt.release_by_table() != rvt.release() {
            warnings.push(format!(
                "BasicFileInfo names release {:?}, the element table's class release {:?}",
                rvt.release(),
                rvt.release_by_table()
            ));
        }
        if !matches!(rvt.release(), Some(2024 | 2025)) {
            warnings.push(format!(
                "saved by release {}: element names, levels and types are checked against the \
                 2024 and 2025 releases' files; this release's are read the same way",
                rvt.release().map_or("unknown".into(), |r| r.to_string())
            ));
        }
        if !unknown.is_empty() {
            warnings.push(format!(
                "{} field encodings of the schema are unknown",
                unknown.len()
            ));
        }
        if model.objects_partial > 0 {
            warnings.push(format!(
                "{} element records were read in part: data of extensible storage schemas, which \
                 the file's class schema does not describe, stopped them after the element's own \
                 fields",
                model.objects_partial
            ));
        }
        if model.object_errors > 0 || model.header_errors > 0 {
            warnings.push(format!(
                "{} header and {} element records did not read",
                model.header_errors, model.object_errors
            ));
        }
        if rvt.kind == Kind::Family {
            warnings.push(
                "a family file: its metadata only; its forms and parameters are not imported yet"
                    .into(),
            );
        }
        if let Some(import) = &import {
            warnings.extend(import.warnings.iter().cloned());
        }
        let atom = rvt.part_atom();
        let preview = rvt.preview();
        let mut report = json!({
            "file": file,
            "format": if rvt.kind == Kind::Family { "rfa" } else { "rvt" },
            "kind": rvt.kind,
            "release": rvt.release(),
            "build": rvt.info.build,
            "family": atom.as_ref().and_then(|a| a.title.clone()),
            "family_categories": atom.map(|a| a.categories).unwrap_or_default(),
            "external_files": rvt.external_files().iter().map(|f| json!({
                "name": f.name, "kind": f.kind, "element": f.element_id,
            })).collect::<Vec<_>>(),
            "preview": preview.as_ref().map(|p| json!({
                "bytes": p.len(), "size": mitcad_rvt::info::png_size(p),
            })),
            "schema": {
                "classes": schema.classes.len(),
                "id_width": schema.id_width,
                "unknown_encodings": unknown.len(),
            },
            "elements": {
                "declared": model.declared.len(),
                "with_records": model.elements.iter().filter(|e| e.partition.is_some()).count(),
                "walked": model.objects_walked,
                "read_in_part": model.objects_partial,
                "not_read": model.header_errors + model.object_errors,
            },
            "levels": model.levels.iter().map(|l| json!({
                "id": l.id, "name": l.name, "elevation": l.elevation * mitcad_rvt::elements::FOOT_MM,
            })).collect::<Vec<_>>(),
            "materials": model.materials.len(),
            "import": import,
            "warnings": warnings,
            "seconds": started.elapsed().as_secs_f64(),
        });
        if let Some(path) = &options.report_path {
            let text = serde_json::to_string_pretty(&report).expect("the report serializes");
            std::fs::write(path, format!("{text}\n"))
                .map_err(|e| ApiError(format!("{path}: {e}")))?;
        }
        // The imported bodies, for the application's report.
        report["bodies"] = json!(report["import"]["bodies"].as_u64().unwrap_or(0));
        Ok(report)
    }

    /// The bridge's `import_rvt`: the report as JSON, or text.
    pub(crate) fn import_rvt(&mut self, path: &str, json: &str) -> Result<String, ApiError> {
        let json = if json.trim().is_empty() { "{}" } else { json };
        let options: Options = serde_json::from_str(json)
            .map_err(|e| ApiError(format!("invalid import options: {e}")))?;
        let report = self.without_job(|document| document.import_rvt_report(path, &options))?;
        Ok(if options.text {
            summary(&report)
        } else {
            serde_json::to_string_pretty(&report).expect("the report serializes")
        })
    }

    /// `{"cmd": "import_rvt", "path": ..., ...}`: the import as a document
    /// command.
    pub(crate) fn import_rvt_command(&mut self, mut command: Value) -> Result<Value, ApiError> {
        let path = command
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError("import_rvt needs a \"path\"".to_owned()))?
            .to_owned();
        if let Some(map) = command.as_object_mut() {
            map.remove("cmd");
            map.remove("path");
        }
        let options: Options = serde_json::from_value(command)
            .map_err(|e| ApiError(format!("invalid import options: {e}")))?;
        let report = self.without_job(|document| document.import_rvt_report(&path, &options))?;
        Ok(json!({
            "file": report["file"],
            "design": report["family"].as_str().map_or_else(|| report["file"].clone(), |f| json!(f)),
            "bodies": report["bodies"],
            "report": report,
        }))
    }
}

/// The report as text.
fn summary(report: &Value) -> String {
    let mut text = String::new();
    let import = &report["import"];
    text += &format!(
        "{} ({}), saved by release {} (build {})\n",
        report["file"].as_str().unwrap_or_default(),
        report["format"].as_str().unwrap_or_default(),
        report["release"],
        report["build"].as_str().unwrap_or("?"),
    );
    let e = &report["elements"];
    text += &format!(
        "  elements: {} declared, {} with records, {} read whole, {} read in part, {} not read\n",
        e["declared"], e["with_records"], e["walked"], e["read_in_part"], e["not_read"]
    );
    text += &format!(
        "  schema: {} classes, ElementId of {} bytes, {} unknown encodings\n",
        report["schema"]["classes"],
        report["schema"]["id_width"],
        report["schema"]["unknown_encodings"]
    );
    if let Some(family) = report["family"].as_str() {
        text += &format!("  family: {family}\n");
    }
    if import.is_object() {
        text += &format!(
            "Imported {} bodies: {} saved meshes, {} boxes at a family type's transform, {} boxes; {} placed on a level by elevation\n",
            import["bodies"],
            import["meshes"],
            import["oriented_boxes"],
            import["boxes"],
            import["by_elevation"]
        );
        for level in import["levels"].as_array().into_iter().flatten() {
            text += &format!(
                "  {}: {} bodies ({} meshes, {} boxes){}\n",
                level["name"].as_str().unwrap_or_default(),
                level["bodies"],
                level["meshes"],
                level["boxes"],
                level["elevation"]
                    .as_f64()
                    .map_or(String::new(), |e| format!(" at {e:.1} mm"))
            );
        }
        for (category, n) in import["categories"].as_object().into_iter().flatten() {
            text += &format!("  {n:>7} {category}\n");
        }
        for (why, n) in import["left_out"].as_object().into_iter().flatten() {
            text += &format!("  left out: {n} {why}\n");
        }
        for not_built in import["not_built"].as_array().into_iter().flatten() {
            text += &format!(
                "  not built: {} {}\n",
                not_built[0],
                not_built[1].as_str().unwrap_or_default()
            );
        }
    }
    for link in report["external_files"].as_array().into_iter().flatten() {
        if !link["name"].as_str().unwrap_or_default().is_empty() {
            text += &format!(
                "  external file: {} ({})\n",
                link["name"].as_str().unwrap_or_default(),
                link["kind"].as_str().unwrap_or_default()
            );
        }
    }
    for warning in report["warnings"].as_array().into_iter().flatten() {
        text += &format!("  warning: {}\n", warning.as_str().unwrap_or_default());
    }
    text
}

/// The bridge's `rvt_write_test_file`: the reader's test project or
/// family (`mitcad_rvt::testfile`).
pub(crate) fn rvt_write_test_file(path: &str, family: bool) -> Result<(), String> {
    let data = if family {
        mitcad_rvt::testfile::family()
    } else {
        mitcad_rvt::testfile::project()
    };
    std::fs::write(path, data).map_err(|e| format!("{path}: {e}"))
}
