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

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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
    /// The IFC export of the same project: the elements built as features
    /// are checked against its solids, by GlobalId (mitcad#275).
    #[serde(default)]
    reference_ifc: Option<String>,
    /// Without `reference_ifc`, no IFC export next to the project (of its
    /// name) is taken as the reference either (mitcad#333).
    #[serde(default)]
    no_reference_ifc: bool,
    /// Files of linked models, by the link's file name (the
    /// application's file dialogs).
    #[serde(default)]
    link_files: BTreeMap<String, String>,
    /// A folder to look for linked models in (besides the project's).
    #[serde(default)]
    link_folder: Option<String>,
    /// List the links and where their files are; import nothing.
    #[serde(default)]
    list_links: bool,
    /// Every element as its saved mesh or box (no features).
    #[serde(default)]
    mesh_only: bool,
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
                let import =
                    if rvt.kind == Kind::Project && !options.report_only && !options.list_links {
                        Some(
                            self.import_rvt_model(path, file, &model, options, &progress)
                                .map_err(|e| mitcad_rvt::RvtError::Stream(e.to_string()))?,
                        )
                    } else {
                        None
                    };
                Ok::<_, mitcad_rvt::RvtError>((model, import))
            })
            .map_err(|e| fail(&e))?;
        if options.list_links {
            return Ok(Self::list_rvt_links(path, &model, options));
        }
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
        // The imported bodies, for the application's report: the meshes
        // and boxes, and the bodies of the elements built as features.
        let featured: u64 = report["import"]["elements"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|l| l["outcome"] == "features")
            .filter_map(|l| l["bodies"].as_u64())
            .sum();
        report["bodies"] = json!(report["import"]["bodies"].as_u64().unwrap_or(0) + featured);
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
        let counts = |v: &Value| {
            let parts: Vec<String> = v
                .as_object()
                .into_iter()
                .flatten()
                .map(|(k, n)| format!("{n} {k}"))
                .collect();
            if parts.is_empty() {
                "none".to_string()
            } else {
                parts.join(", ")
            }
        };
        text += &format!(
            "Built {} elements as features ({}), checked against {}; of those kinds {} came in as meshes or boxes\n",
            import["features"],
            counts(&import["built"]),
            import["reference"]
                .as_str()
                .map_or("their stored boxes".to_string(), |r| format!(
                    "the reference {r}"
                )),
            counts(&import["fallbacks"]),
        );
        if import["reference"].is_string() {
            text += &format!(
                "  the reference's solids agreed with: {}; not agreed with: {}\n",
                counts(&import["reference_agree"]),
                import["reference_unmatched"].as_array().map_or(0, Vec::len)
            );
            for line in import["reference_unmatched"]
                .as_array()
                .into_iter()
                .flatten()
            {
                text += &format!("    {}\n", line.as_str().unwrap_or_default());
            }
        }
        text += &format!(
            "  {} level planes, {} grid axes, {} grid arcs, {} appearances\n",
            import["level_planes"], import["grid_axes"], import["grid_arcs"], import["appearances"]
        );
        for link in import["links"].as_array().into_iter().flatten() {
            text += &format!(
                "  link {}: {}\n",
                link["file"].as_str().unwrap_or_default(),
                match link["found"].as_str() {
                    Some(found) => format!("{found}, {} elements", link["elements"]),
                    None => link["note"].as_str().unwrap_or("not imported").to_string(),
                }
            );
        }
        text += &format!(
            "Imported {} bodies: {} saved meshes, {} boxes at a family type's transform, {} boxes; {} meshes of the geometry the file keeps, {} reference bodies, {} boxes hidden; {} placed on a level by elevation\n",
            import["bodies"],
            import["meshes"],
            import["oriented_boxes"],
            import["boxes"],
            import["geometry_meshes"],
            import["reference_bodies"],
            import["hidden_boxes"],
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

/// The solids of an IFC export, by GlobalId: each product's volume and
/// box (mm, the design's coordinates), imported with the kernel into a
/// document of their own (its type's maps copied into each element).
fn ifc_reference(path: &str) -> Result<mitcad_import::rvt::Reference, String> {
    use mitcad_ifc::{IfcFile, ReadOptions};
    let name = file_name(path).to_string();
    let file = IfcFile::open_with(Path::new(path), &ReadOptions::import())
        .map_err(|e| format!("{name}: {e}"))?;
    crate::kernel::exchange::ffi::catch_occt_crashes();
    let mut doc = mitcad_model::Document::new(crate::kernel::OcctKernel);
    let options = mitcad_import::ifc::IfcOptions {
        occurrences: false,
        ..mitcad_import::ifc::IfcOptions::default()
    };
    let report = mitcad_import::ifc::import_ifc(&mut doc, &file, &name, &options);
    let kernel = doc.kernel();
    let instances = doc.instances();
    let mut solids = std::collections::HashMap::new();
    for line in &report.products {
        if line.bodies.is_empty() || line.global_id.is_empty() {
            continue;
        }
        let mut bbox: Option<([f64; 3], [f64; 3])> = None;
        let mut bodies = Vec::new();
        for inst in instances
            .iter()
            .filter(|i| line.bodies.contains(&i.body.to_string()))
        {
            let Some(shape) = doc.body_shape(inst.body) else {
                continue;
            };
            // The body placed, for elements the project keeps no
            // geometry of (mitcad#333).
            if let Ok(placed) =
                mitcad_model::Kernel::transform_shape(kernel, shape, &inst.transform, None)
                && let Ok(data) = mitcad_model::Kernel::brep_data(kernel, &placed)
            {
                bodies.push(data);
            }
            let Ok(Some(b)) = mitcad_model::Kernel::bounding_box(kernel, shape) else {
                continue;
            };
            for c in 0..8 {
                let p = [
                    if c & 1 == 0 { b.min[0] } else { b.max[0] },
                    if c & 2 == 0 { b.min[1] } else { b.max[1] },
                    if c & 4 == 0 { b.min[2] } else { b.max[2] },
                ];
                let q = inst.transform.apply_point(p);
                bbox = Some(match bbox {
                    None => (q, q),
                    Some((lo, hi)) => (
                        std::array::from_fn(|k| lo[k].min(q[k])),
                        std::array::from_fn(|k| hi[k].max(q[k])),
                    ),
                });
            }
        }
        solids.insert(
            line.global_id.clone(),
            mitcad_import::rvt::ReferenceSolid {
                kind: line.kind.clone(),
                name: line.name.clone(),
                volume: line.volume,
                bbox,
                bodies,
            },
        );
    }
    Ok(mitcad_import::rvt::Reference { file: name, solids })
}

/// Where a link's file is: the file given for it, else next to the
/// project (by its name, then by the path the project stores), else in
/// the link folder.
fn find_link(
    project: &str,
    link: &mitcad_rvt::building::Link,
    options: &Options,
) -> Option<PathBuf> {
    let name = link.file.as_deref()?;
    if let Some(given) = options.link_files.get(name) {
        return Some(PathBuf::from(given)).filter(|p| p.is_file());
    }
    let dir = Path::new(project).parent().unwrap_or(Path::new("."));
    let stored = link.path.as_deref().map(|p| p.replace('\\', "/"));
    let mut candidates = vec![dir.join(name)];
    if let Some(stored) = &stored
        && !stored.contains(':')
        && !stored.starts_with('/')
    {
        candidates.push(dir.join(stored));
    }
    if let Some(folder) = &options.link_folder {
        candidates.push(Path::new(folder).join(name));
    }
    candidates.into_iter().find(|p| p.is_file())
}

impl Document {
    /// The links of a project and where their files are, as JSON.
    fn list_rvt_links(path: &str, model: &Model, options: &Options) -> Value {
        json!({"links": model.building.links.iter().map(|l| json!({
            "id": l.id,
            "file": l.file,
            "stored_path": l.path,
            "found": find_link(path, l, options).map(|p| p.display().to_string()),
        })).collect::<Vec<_>>()})
    }

    /// The import of a project's model, with its reference and its links.
    fn import_rvt_model(
        &mut self,
        path: &str,
        file: &str,
        model: &Model,
        options: &Options,
        progress: &Arc<mitcad_import::Progress>,
    ) -> Result<mitcad_import::rvt::Report, String> {
        // The reference: the IFC export given, else one next to the project
        // by its name (mitcad#333).
        let beside = Path::new(path).with_extension("ifc");
        let (reference_path, found_beside) = match &options.reference_ifc {
            Some(r) => (Some(r.clone()), false),
            None if !options.no_reference_ifc && beside.is_file() => {
                (Some(beside.display().to_string()), true)
            }
            None => (None, false),
        };
        let reference = match &reference_path {
            Some(r) => Some(ifc_reference(r)?),
            None => None,
        };
        let mut import_options = mitcad_import::rvt::RvtOptions {
            progress: Some(progress),
            reference: reference.as_ref(),
            mesh_only: options.mesh_only,
            ..Default::default()
        };
        for link in &model.building.links {
            let name = link
                .file
                .clone()
                .unwrap_or_else(|| format!("link {}", link.id));
            // A linked IFC model (mitcad#333): its cached project model
            // (`<name>.RVT`) when there is one, else the IFC file itself.
            let is_ifc = name.to_ascii_lowercase().ends_with(".ifc");
            let cached = is_ifc
                .then(|| {
                    let mut l = link.clone();
                    l.file = Some(format!("{name}.RVT"));
                    l.path = None;
                    find_link(path, &l, options)
                })
                .flatten();
            if is_ifc && cached.is_none() {
                match find_link(path, link, options) {
                    Some(found) => {
                        let read = mitcad_ifc::IfcFile::open_with(
                            &found,
                            &mitcad_ifc::ReadOptions::import(),
                        );
                        match read {
                            Ok(ifc) => import_options.ifc_links.push(mitcad_import::rvt::IfcLink {
                                link: link.clone(),
                                file: file_name(&found.display().to_string()).to_string(),
                                ifc,
                            }),
                            Err(e) => import_options.missing_links.push((
                                link.clone(),
                                format!("{} did not read: {e}", found.display()),
                            )),
                        }
                    }
                    None => import_options.missing_links.push((
                        link.clone(),
                        format!("the IFC model {name} (or its cached {name}.RVT) was not found: its component is empty"),
                    )),
                }
                continue;
            }
            let Some(found) = cached.or_else(|| find_link(path, link, options)) else {
                import_options.missing_links.push((
                    link.clone(),
                    format!("{name} was not found next to the project or where it was given: its component is empty"),
                ));
                continue;
            };
            let read = RvtFile::open(&found).and_then(|f| {
                let schema = f.schema()?;
                if f.kind != Kind::Project {
                    return Err(mitcad_rvt::RvtError::Stream("not a project".into()));
                }
                Model::read(&f, &schema)
            });
            match read {
                Ok(m) => import_options.links.push(mitcad_import::rvt::LinkedModel {
                    link: link.clone(),
                    file: file_name(&found.display().to_string()).to_string(),
                    model: m,
                }),
                Err(e) => import_options.missing_links.push((
                    link.clone(),
                    format!("{} did not read: {e}", found.display()),
                )),
            }
        }
        let mut report = mitcad_import::rvt::import_rvt(&mut self.0, file, model, &import_options)
            .map_err(|e| e.to_string())?;
        if found_beside {
            report.warnings.push(format!(
                "the IFC export next to the project ({}) is the reference the elements are checked against",
                file_name(&beside.display().to_string())
            ));
        }
        Ok(report)
    }
}

/// The bridge's `rvt_write_building_test`: the reader's building test
/// project (`mitcad_rvt::testfile::building`) as `building.rvt` in `dir`,
/// with the model it links next to it (`linked.rvt`, the same project).
pub(crate) fn rvt_write_building_test(dir: &str) -> Result<(), String> {
    use mitcad_rvt::testfile::building;
    let dir = Path::new(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for name in ["building.rvt", building::LINKED] {
        let path = dir.join(name);
        std::fs::write(&path, building::project())
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}
