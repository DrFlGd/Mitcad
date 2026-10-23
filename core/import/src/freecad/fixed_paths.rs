// SPDX-License-Identifier: MIT
//! Fixed paths of objects that keep no sketch (mitcad#547, mitcad#548),
//! shared by Part extrusions and Part sweeps:
//!
//! - [`Importer::part_helix`]: a Part helix (`Part::Helix`). A cylindrical
//!   one is a fixed `helix` path from its sizes (Pitch, Height, Radius,
//!   LocalCoord) and placement: FreeCAD's helix starts at Radius along its
//!   placement's x axis and rises along its z axis, counter-clockwise
//!   about it (Right-handed) or clockwise (Left-handed). A conical one
//!   (Angle) has no fixed helix path: its stored edges (the B-splines
//!   FreeCAD builds of it) are a path of fixed curves.
//! - [`fixed_curves`]: curves in model space (an object's stored edges in
//!   no plane) as a `fixed_curves` path.

use mitcad_freecad::Object;
use mitcad_model::{Curve3, ElementKind, Kernel};
use serde_json::{Value, json};

use super::Importer;
use super::history::Target;
use crate::geom::unit;

/// The most edges of an object a fixed path reads.
const MAX_EDGES: usize = 10_000;

/// A Part helix as a path.
pub(super) struct PartHelix {
    /// The whole path: `{"helix": {...}}`, or `{"fixed_curves": [...]}`
    /// for a conical helix.
    pub path: Value,
    /// Where the helix starts (Radius along its placement's x axis).
    pub start: [f64; 3],
    /// What the report says of it (the import is partial: FreeCAD's sizes
    /// are not carried over).
    pub note: String,
}

impl<K: Kernel> Importer<'_, '_, K> {
    /// A Part helix (`h`, a `Part::Helix`) as a path in the target's
    /// component: the one reader of Part helices.
    pub(super) fn part_helix(&mut self, h: &Object, target: &Target) -> Result<PartHelix, String> {
        let (pitch, height, radius) = (
            h.f64("Pitch").ok_or("no pitch")?,
            h.f64("Height").ok_or("no height")?,
            h.f64("Radius").ok_or("no radius")?,
        );
        if pitch <= 0.0 || height <= 0.0 || radius <= 0.0 {
            return Err(format!(
                "{} is a helix without a pitch, a height or a radius",
                h.name
            ));
        }
        let frame = super::sketches::frame_of(&target.place.transform, &super::placement_of(h));
        let axis = unit(frame.normal()).ok_or("the helix has no axis")?;
        let x = unit(frame.x_axis).ok_or("the helix has no axes")?;
        let start: [f64; 3] = std::array::from_fn(|i| frame.origin[i] + radius * x[i]);
        // A conical helix: its stored curves (mitcad#548).
        if h.f64("Angle").is_some_and(|a| a != 0.0) {
            let shape = self
                .stored_in(&h.name, &target.place.transform)
                .ok_or_else(|| format!("{} has no stored shape", h.name))?;
            let mut curves = Vec::new();
            {
                let kernel = self.doc.kernel();
                for index in 0..MAX_EDGES {
                    match kernel.indexed_element(&shape, ElementKind::Edge, index) {
                        Ok(Some(edge)) => curves.extend(edge.curves),
                        Ok(None) => break,
                        Err(e) => return Err(format!("the edges of {}: {e}", h.name)),
                    }
                }
            }
            if curves.is_empty() {
                return Err(format!("{}'s stored shape has no edges", h.name));
            }
            return Ok(PartHelix {
                path: fixed_curves(&curves)?,
                start,
                note: format!(
                    "{}'s conical helix is its stored curves as a fixed path (FreeCAD's sizes \
                     are not carried over)",
                    h.name
                ),
            });
        }
        let left = self
            .enum_text(h, "LocalCoord")
            .is_some_and(|t| t.starts_with("Left"));
        let turn = std::f64::consts::TAU * height / pitch;
        let (pitch, end) = if left { (-pitch, -turn) } else { (pitch, turn) };
        Ok(PartHelix {
            path: json!({"helix": {"center": frame.origin, "axis": axis, "x_axis": x,
                "radius": radius, "pitch": pitch, "start_angle": 0.0, "end_angle": end}}),
            start,
            note: format!(
                "{}'s helix is a fixed helix path (FreeCAD's sizes are not carried over)",
                h.name
            ),
        })
    }
}

/// Where a clamped spline starts (mitcad#548): its first pole, when its
/// first `degree + 1` knots are the same; None for other curves and for a
/// spline that is not clamped there (its start is not a pole).
pub(super) fn spline_start(curve: &Curve3) -> Option<[f64; 3]> {
    let Curve3::BSpline {
        degree,
        poles,
        knots,
        ..
    } = curve
    else {
        return None;
    };
    let d = *degree as usize;
    let first = *poles.first()?;
    let clamped = knots.len() == poles.len() + d + 1
        && d < knots.len()
        && knots[..=d].iter().all(|k| *k == knots[0]);
    clamped.then_some(first)
}

/// Curves in model space as a path of fixed curves (`{"fixed_curves":
/// [...]}`, mitcad#548): lines, circle and ellipse arcs, B-splines. Err for
/// points and helices (a helix has a `helix` path of its own).
pub(super) fn fixed_curves(curves: &[Curve3]) -> Result<Value, String> {
    let mut out = Vec::new();
    for curve in curves {
        out.push(match curve {
            Curve3::Line { start, end } => json!({"type": "line", "start": start, "end": end}),
            Curve3::Conic {
                center,
                normal,
                x_axis,
                major,
                minor,
                start,
                end,
                closed,
            } => json!({"type": "conic", "center": center, "normal": normal,
                        "x_axis": x_axis, "major": major, "minor": minor,
                        "start": start, "end": end, "closed": closed}),
            Curve3::BSpline {
                degree,
                poles,
                weights,
                knots,
            } => {
                let mut spline = json!({"type": "spline", "degree": degree, "poles": poles,
                                        "knots": knots});
                if !weights.is_empty() {
                    spline["weights"] = json!(weights);
                }
                spline
            }
            Curve3::Helix { .. } => {
                return Err("a helix among other curves has no fixed path".to_owned());
            }
            Curve3::Point(_) => return Err("a point is no curve of a path".to_owned()),
        });
    }
    if out.is_empty() {
        return Err("no curves for a fixed path".to_owned());
    }
    Ok(json!({"fixed_curves": out}))
}
