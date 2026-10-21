// SPDX-License-Identifier: MIT
//! The `export_drawing` command (mitcad#471, commands.md): the bodies at
//! the marker as a drawing on a sheet, written as PDF, SVG or DXF. The
//! kernel projects the views and finds the round holes; the `threads`
//! query gives the holes' threads and the sketches their dimensions;
//! `crate::drawing` lays the sheet out and writes it.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::ApiError;
use crate::datum::Vec3;
use crate::document::Document;
use crate::drawing::compose::{self, Composed, HoleInput, SheetInput, SketchDimension, ViewInput};
use crate::drawing::{
    DrawingOptions, RoundHole, SectionChoice, Units, ViewName, ViewSpec, cross, dot, pdf, svg,
    today, unit,
};
use crate::features::FeatureDef;
use crate::ids::BodyUid;
use crate::kernel::{Kernel, KernelError};
use crate::recompute::FeatureStatus;
use crate::sketch::DimensionKind;
use crate::sketch::geometry::Curve2;
use crate::transform::Transform;

/// `export_drawing`: the file, its format, the bodies and the drawing's
/// options (the other fields, [`DrawingOptions`]).
#[derive(Debug)]
pub(super) struct ExportDrawingCommand {
    path: String,
    format: Option<String>,
    bodies: Vec<String>,
    options: DrawingOptions,
}

impl<'de> Deserialize<'de> for ExportDrawingCommand {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut map = Map::<String, Value>::deserialize(deserializer)?;
        let path = match map.remove("path") {
            Some(Value::String(path)) => path,
            Some(_) => return Err(D::Error::custom("\"path\" must be a string")),
            None => return Err(D::Error::missing_field("path")),
        };
        let format = match map.remove("format") {
            None | Some(Value::Null) => None,
            Some(Value::String(format)) => Some(format),
            Some(_) => return Err(D::Error::custom("\"format\" must be a string")),
        };
        let bodies = match map.remove("bodies") {
            None | Some(Value::Null) => Vec::new(),
            Some(value) => serde_json::from_value(value).map_err(D::Error::custom)?,
        };
        let options = serde_json::from_value(Value::Object(map)).map_err(D::Error::custom)?;
        Ok(Self {
            path,
            format,
            bodies,
            options,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Pdf,
    Svg,
    Dxf,
}

impl Format {
    fn of(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "pdf" => Some(Self::Pdf),
            "svg" => Some(Self::Svg),
            "dxf" => Some(Self::Dxf),
            _ => None,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Svg => "svg",
            Self::Dxf => "dxf",
        }
    }
}

/// A body on the drawing: its shapes where the design shows it.
struct Drawn<S> {
    uid: BodyUid,
    name: String,
    placements: Vec<(Transform, S)>,
}

/// A thread of the `threads` query, for its hole.
struct ThreadOf {
    body: BodyUid,
    face: String,
    designation: String,
    internal: bool,
    modeled: bool,
    major: f64,
    start: Vec3,
    end: Vec3,
}

fn vec3(value: &Value) -> Option<Vec3> {
    let a = value.as_array()?;
    Some([
        a.first()?.as_f64()?,
        a.get(1)?.as_f64()?,
        a.get(2)?.as_f64()?,
    ])
}

fn threads_of(threads: &Value) -> Vec<ThreadOf> {
    threads
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| {
            Some(ThreadOf {
                body: t.get("body")?.as_str()?.parse().ok()?,
                face: t.get("face")?.as_str()?.to_owned(),
                designation: t.get("designation")?.as_str()?.to_owned(),
                internal: t.get("internal").and_then(Value::as_bool).unwrap_or(false),
                modeled: t.get("modeled").and_then(Value::as_bool).unwrap_or(false),
                major: t
                    .get("major_diameter")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                start: t.get("start").and_then(vec3)?,
                end: t.get("end").and_then(vec3)?,
            })
        })
        .collect()
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Whether a hole lies on a thread's axis (both in the design's
/// coordinates).
fn on_axis(hole: &RoundHole, start: Vec3, end: Vec3) -> bool {
    let Some(axis) = unit(sub(end, start)) else {
        return false;
    };
    let c = cross(axis, hole.direction);
    if dot(c, c).sqrt() > 1e-6 {
        return false;
    }
    let off = sub(hole.opening, start);
    let across = cross(off, axis);
    dot(across, across).sqrt() < 1e-4
}

impl<K: Kernel> Document<K> {
    pub(super) fn export_drawing_command(
        &self,
        command: ExportDrawingCommand,
    ) -> Result<Value, ApiError> {
        let extension = std::path::Path::new(&command.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_owned);
        let format = match (&command.format, &extension) {
            (Some(name), _) => Format::of(name).ok_or_else(|| {
                ApiError(format!(
                    "unknown drawing format '{name}' (expected pdf, svg or dxf)"
                ))
            })?,
            (None, Some(ext)) => Format::of(ext).ok_or_else(|| {
                ApiError(format!(
                    "{}: drawings are written to .pdf, .svg or .dxf files",
                    command.path
                ))
            })?,
            (None, None) => {
                return Err(ApiError(format!(
                    "{}: give a .pdf, .svg or .dxf file or \"format\"",
                    command.path
                )));
            }
        };
        let options = command.options;
        let units = match &options.units {
            Some(symbol) => Units::by_symbol(symbol)
                .ok_or_else(|| ApiError(format!("unknown length unit '{symbol}'")))?,
            None => Units::of(self.parameters().context().default_length_unit),
        };
        if options.views.is_empty() && options.section.is_none() {
            return Err(ApiError("a drawing needs at least one view".to_owned()));
        }
        let kernel = self.kernel();

        // The bodies where the design shows them.
        let all = self.bodies();
        let instances = self.instances();
        let chosen: Vec<BodyUid> = if command.bodies.is_empty() {
            all.iter()
                .filter(|b| instances.iter().any(|i| i.body == b.uid && i.visible))
                .map(|b| b.uid)
                .collect()
        } else {
            let mut chosen = Vec::new();
            for body in &command.bodies {
                let uid = self.body_by_uid_or_name(body)?;
                if chosen.contains(&uid) {
                    return Err(ApiError(format!("body {body} is listed more than once")));
                }
                chosen.push(uid);
            }
            chosen
        };
        let mut drawn: Vec<Drawn<K::Shape>> = Vec::new();
        for uid in chosen {
            let Some(body) = all.iter().find(|b| b.uid == uid) else {
                return Err(ApiError(format!(
                    "body {uid} does not exist at the timeline marker"
                )));
            };
            if matches!(
                kernel.body_kind(body.shape),
                Ok(crate::exchange::BodyKind::Empty)
            ) {
                continue;
            }
            let mine: Vec<_> = instances.iter().filter(|i| i.body == uid).collect();
            let shown: Vec<_> = mine.iter().copied().filter(|i| i.visible).collect();
            let mut transforms: Vec<Transform> = if shown.is_empty() { mine } else { shown }
                .iter()
                .map(|i| i.transform)
                .collect();
            if transforms.is_empty() {
                transforms.push(Transform::IDENTITY);
            }
            let mut placements = Vec::new();
            for t in transforms {
                let shape = if t.is_identity() {
                    body.shape.clone()
                } else {
                    kernel
                        .transform_shape(body.shape, &t, None)
                        .map_err(|e| ApiError(format!("{}: {e}", body.name)))?
                };
                placements.push((t, shape));
            }
            drawn.push(Drawn {
                uid,
                name: body.name.clone(),
                placements,
            });
        }
        if drawn.is_empty() {
            return Err(ApiError("there are no bodies to draw".to_owned()));
        }

        let threads = threads_of(&self.threads_json());
        let section = match &options.section {
            None => None,
            Some(SectionChoice::Plane { origin, normal }) => Some(
                compose::cut_plane(*origin, *normal)
                    .ok_or_else(|| ApiError("the section plane's normal is zero".to_owned()))?,
            ),
            Some(SectionChoice::Analysis { analysis }) => {
                let found = self
                    .analyses()
                    .iter()
                    .find(|a| &a.name == analysis)
                    .ok_or_else(|| ApiError(format!("there is no analysis '{analysis}'")))?;
                let crate::AnalysisDef::Section(def) = &found.def;
                let plane = self
                    .section_plane(def)
                    .map_err(|e| ApiError(format!("{analysis}: {e}")))?;
                Some(
                    compose::cut_plane(plane.origin, plane.normal)
                        .ok_or_else(|| ApiError(format!("{analysis}: no plane")))?,
                )
            }
        };
        let sketch_dimensions = if options.sketch_dimensions {
            self.sketch_dimensions()
        } else {
            Vec::new()
        };
        let mut views: Vec<ViewName> = Vec::new();
        for v in &options.views {
            if !views.contains(v) {
                views.push(*v);
            }
        }
        let title = options
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "Untitled".to_owned());
        let date = options.date.clone().unwrap_or_else(today);

        let sheets: Vec<Vec<&Drawn<K::Shape>>> = if options.per_body {
            drawn.iter().map(|d| vec![d]).collect()
        } else {
            vec![drawn.iter().collect()]
        };
        let mut composed: Vec<Composed> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        for bodies in &sheets {
            let shapes: Vec<&K::Shape> = bodies
                .iter()
                .flat_map(|d| d.placements.iter().map(|(_, s)| s))
                .collect();
            let mut inputs = Vec::new();
            for name in &views {
                let (toward_eye, x_axis) = name.axes();
                let spec = ViewSpec {
                    toward_eye,
                    x_axis,
                    hidden: options.hidden_lines && *name != ViewName::Iso,
                    smooth: options.tangent_edges,
                    exact: options.exact,
                    section: None,
                    deflection: 0.01,
                };
                let projection = kernel
                    .project_view(&shapes, &spec)
                    .map_err(|e| ApiError(format!("the {} view: {e}", name.as_str())))?;
                inputs.push(ViewInput {
                    name: Some(*name),
                    spec,
                    projection,
                });
            }
            if let Some(plane) = &section {
                let spec =
                    compose::section_spec(plane, false, options.tangent_edges, options.exact);
                let projection = kernel
                    .project_view(&shapes, &spec)
                    .map_err(|e| ApiError(format!("the section view: {e}")))?;
                inputs.push(ViewInput {
                    name: None,
                    spec,
                    projection,
                });
            }
            let mut holes = Vec::new();
            if options.dimensions || options.hole_table {
                for d in bodies {
                    for (t, shape) in &d.placements {
                        holes.extend(self.drawing_holes(d.uid, t, shape, &threads, &mut warnings));
                    }
                }
            }
            let input = SheetInput {
                title: title.clone(),
                bodies: bodies.iter().map(|d| d.name.clone()).collect(),
                date: date.clone(),
                views: inputs,
                holes,
                sketch_dimensions: sketch_dimensions.clone(),
            };
            let sheet = compose::compose(&options, units, &input);
            warnings.extend(sheet.warnings.iter().cloned());
            composed.push(sheet);
        }

        // The files: a PDF of every sheet, an SVG or DXF file per sheet
        // (the second and later named <name>-2.svg, …).
        let mut files = Vec::new();
        let write = |path: &str, bytes: &[u8]| {
            std::fs::write(path, bytes).map_err(|e| ApiError(format!("cannot write {path}: {e}")))
        };
        match format {
            Format::Pdf => {
                let sheets: Vec<_> = composed.iter().map(|c| c.sheet.clone()).collect();
                write(&command.path, &pdf::write(&sheets, &title))?;
                files.push(command.path.clone());
            }
            Format::Svg | Format::Dxf => {
                for (i, c) in composed.iter().enumerate() {
                    let path = if i == 0 {
                        command.path.clone()
                    } else {
                        numbered(&command.path, i + 1)
                    };
                    let bytes = if format == Format::Svg {
                        svg::write(&c.sheet).into_bytes()
                    } else {
                        let options = mitcad_dxf::WriteOptions::default();
                        mitcad_dxf::write_string(&crate::drawing::dxf::drawing(&c.sheet), &options)
                            .map_err(|e| ApiError(format!("cannot write {path}: {e}")))?
                            .into_bytes()
                    };
                    write(&path, &bytes)?;
                    files.push(path);
                }
            }
        }
        let first = &composed[0];
        let orientation = match options.orientation {
            crate::drawing::Orientation::Landscape => "landscape",
            crate::drawing::Orientation::Portrait => "portrait",
        };
        let scales: Vec<String> = composed
            .iter()
            .map(|c| crate::drawing::scale_text(c.scale))
            .collect();
        let bodies: Vec<BodyUid> = drawn.iter().map(|d| d.uid).collect();
        let names: Vec<String> = drawn.iter().map(|d| d.name.clone()).collect();
        let pages = if format == Format::Pdf {
            composed.len()
        } else {
            1
        };
        Ok(json!({
            "path": command.path,
            "format": format.as_str(),
            "files": files,
            "sheets": composed.len(),
            "pages": pages,
            "sheet": options.sheet.name(),
            "orientation": orientation,
            "scale": crate::drawing::scale_text(first.scale),
            "scales": scales,
            "views": first.views,
            "bodies": bodies,
            "body_names": names,
            "dimensions": composed.iter().map(|c| c.dimensions).sum::<usize>(),
            "holes": composed.iter().map(|c| c.holes).sum::<usize>(),
            "hole_table_rows": composed.iter().map(|c| c.hole_rows).sum::<usize>(),
            "units": units.symbol,
            "polygonal": composed.iter().any(|c| c.polygonal),
            "warnings": warnings,
        }))
    }

    /// A placed body's round holes in the design's coordinates, with their
    /// threads; a modelled thread in a hole that has no cylinder left is a
    /// hole of its major diameter.
    fn drawing_holes(
        &self,
        body: BodyUid,
        placement: &Transform,
        shape: &K::Shape,
        threads: &[ThreadOf],
        warnings: &mut Vec<String>,
    ) -> Vec<HoleInput> {
        let holes = match self.kernel().round_holes(shape) {
            Ok(holes) => holes,
            Err(KernelError::Unsupported(_)) => return Vec::new(),
            Err(e) => {
                warnings.push(format!("the holes of {}: {e}", self.body_name(body)));
                return Vec::new();
            }
        };
        let mine: Vec<&ThreadOf> = threads.iter().filter(|t| t.body == body).collect();
        let placed = |t: &ThreadOf| (placement.apply_point(t.start), placement.apply_point(t.end));
        let mut used = vec![false; mine.len()];
        let mut out: Vec<HoleInput> = holes
            .into_iter()
            .map(|hole| {
                let by_face = hole.face.as_ref().and_then(|f| {
                    let name = f.to_string();
                    mine.iter().position(|t| t.face == name)
                });
                let found = by_face.or_else(|| {
                    mine.iter().position(|t| {
                        let (s, e) = placed(t);
                        t.internal && on_axis(&hole, s, e)
                    })
                });
                if let Some(i) = found {
                    used[i] = true;
                }
                HoleInput {
                    thread: found.map(|i| mine[i].designation.clone()),
                    hole,
                }
            })
            .collect();
        for (i, t) in mine.iter().enumerate() {
            if used[i] || !t.internal || !t.modeled {
                continue;
            }
            let (s, e) = placed(t);
            let Some(axis) = unit(sub(e, s)) else {
                continue;
            };
            // Opening as a hole open at both ends does: towards +Z, +Y, +X.
            let key = if axis[2].abs() > 1e-9 {
                axis[2]
            } else if axis[1].abs() > 1e-9 {
                axis[1]
            } else {
                axis[0]
            };
            let (opening, direction) = if key > 0.0 {
                (e, axis.map(|v| -v))
            } else {
                (s, axis)
            };
            let depth = dot(sub(e, s), sub(e, s)).sqrt();
            out.push(HoleInput {
                hole: RoundHole {
                    face: t.face.parse().ok(),
                    opening,
                    direction,
                    diameter: t.major,
                    through: false,
                    depth,
                    counterbore: None,
                    countersink: None,
                },
                thread: Some(t.designation.clone()),
            });
        }
        out
    }

    /// The dimensions of the sketches at the marker, in the design's
    /// coordinates: distances, lengths, radii and diameters.
    fn sketch_dimensions(&self) -> Vec<SketchDimension> {
        let mut out = Vec::new();
        for entry in self.features() {
            let FeatureDef::Sketch(def) = &entry.def else {
                continue;
            };
            if self.status(entry.uid) != Some(&FeatureStatus::Ok) {
                continue;
            }
            let Some(output) = self.sketch_output(entry.uid) else {
                continue;
            };
            let frame = output.frame;
            let normal = frame.normal();
            let solved = &output.solved;
            let point = |id| solved.points.get(id).map(|p| frame.point(*p));
            let text = |d: &crate::sketch::Dimension<_>| d.text.map(|t| frame.point(t));
            for dim in &def.dimensions {
                let made = match &dim.kind {
                    DimensionKind::Distance { a, b } => {
                        point(a)
                            .zip(point(b))
                            .map(|(pa, pb)| SketchDimension::Linear {
                                a: pa,
                                b: pb,
                                along: None,
                                text: text(dim),
                                value: dot(sub(pb, pa), sub(pb, pa)).sqrt(),
                                normal,
                            })
                    }
                    DimensionKind::HorizontalDistance { a, b, .. }
                    | DimensionKind::VerticalDistance { a, b, .. } => {
                        let along = if matches!(dim.kind, DimensionKind::HorizontalDistance { .. })
                        {
                            frame.x_axis
                        } else {
                            frame.y_axis
                        };
                        point(a)
                            .zip(point(b))
                            .map(|(pa, pb)| SketchDimension::Linear {
                                a: pa,
                                b: pb,
                                along: Some(along),
                                text: text(dim),
                                value: dot(sub(pb, pa), along).abs(),
                                normal,
                            })
                    }
                    DimensionKind::Length { line } => match solved.curves.get(line) {
                        Some(Curve2::Line { a, b }) => {
                            let (pa, pb) = (frame.point(*a), frame.point(*b));
                            Some(SketchDimension::Linear {
                                a: pa,
                                b: pb,
                                along: None,
                                text: text(dim),
                                value: dot(sub(pb, pa), sub(pb, pa)).sqrt(),
                                normal,
                            })
                        }
                        _ => None,
                    },
                    DimensionKind::Radius { curve } | DimensionKind::Diameter { curve } => {
                        let diameter = matches!(dim.kind, DimensionKind::Diameter { .. });
                        match solved.curves.get(curve) {
                            Some(
                                Curve2::Circle { center, radius, .. }
                                | Curve2::Arc { center, radius, .. },
                            ) => Some(SketchDimension::Radial {
                                center: frame.point(*center),
                                radius: *radius,
                                diameter,
                                text: text(dim),
                                value: if diameter { 2.0 * radius } else { *radius },
                                normal,
                            }),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                out.extend(made);
            }
        }
        out
    }
}

/// `part.svg` as `part-2.svg`.
fn numbered(path: &str, n: usize) -> String {
    let p = std::path::Path::new(path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("drawing");
    let name = match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem}-{n}.{ext}"),
        None => format!("{stem}-{n}"),
    };
    p.with_file_name(name).to_string_lossy().into_owned()
}
