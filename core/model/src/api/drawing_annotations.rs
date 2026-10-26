// SPDX-License-Identifier: MIT
//! Annotations on drawings' views (mitcad#707 phase 2,
//! `crate::drawing::annotate`; commands.md, "Annotations on drawing
//! views"): the commands `add_drawing_annotation`,
//! `edit_drawing_annotation` and `delete_drawing_annotation`, the
//! `drawing_pick` query, and the annotations worked out when a sheet is
//! drawn.
//!
//! A pick names model topology through its view: a body the view shows
//! (with the occurrence that places it) and one of its edges or faces by
//! its topological name. A click on a sheet is turned into such a pick
//! here: the kernel lists the body's named edges, round faces and planar
//! faces (`Kernel::edge_geometries`, `Kernel::face_geometries`, kept per
//! shape version in the projection cache), each is projected as the view
//! sees it, and the one nearest the click is taken. A view sees what it
//! shows (mitcad#717): a section what lies behind its plane (and a planar
//! face where its plane cuts it), a detail what lies in its circle, a
//! broken view what lies beside its bands; of picks as near as one
//! another, one the view draws as a visible line goes first, then the one
//! nearest the eye. When the sheet is drawn each pick is looked up again
//! by its name (`Kernel::edge_geometry`, `Kernel::face_geometry`), so the
//! annotation follows the model; a pick that is gone makes it broken.
//!
//! Values are written by #702's engine (`annotations.rs`): the same
//! prefixes, decimals, own texts, tolerances and fit codes as the
//! dimensions of the model. On a pictorial view (an isometric one) a
//! length seen along the view's x or y, an ordinate and an angle would be
//! foreshortened: they are refused there, and an aligned dimension
//! measures the true distance of its picks in 3D.

use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use super::annotations::{FormatEdit, Units as WordUnits, frame_words, present, value_text};
use super::drawing_tables::{PartsList, RowKey, TableEdit, row_of};
use super::drawings::{Evaluated, Shown};
use crate::datum::{CurveGeometry, SurfaceGeometry, Vec3};
use crate::document::{AnnotationFormat, Document, GeometricTolerance, MeasureKind, Tolerance};
use crate::drawing::annotate::{
    self, Along, AnnotationType, BodyGeometry, FlatFace, Frame, PickPoint, RoundFace, Seen,
    SheetNote, SurfaceFinish, ViewAnnotation, ViewPick, Want,
};
use crate::drawing::compose::{
    Bounds, PlacedView, SketchDimension, ViewRole, sketch_dimension, view_bounds,
};
use crate::drawing::def::{Drawing, DrawingSheet, DrawingUid, Subject, ViewKind};
use crate::drawing::sheet::Group;
use crate::drawing::tables::{
    HOLE_COLUMNS, Levels, PARTS_COLUMNS, TableSettings, header, lay_out_table,
};
use crate::drawing::{BodyInView, CutFace, CutPlane, P2, Units, ViewCurve, ViewSpec, clip};
use crate::features::OccurrencePath;
use crate::fingerprint::Fingerprint;
use crate::ids::{BodyUid, FeatureUid, OccurrenceUid};
use crate::kernel::{Kernel, KernelError};
use crate::topo::{EdgeName, FaceName};
use crate::transform::Transform;

/// A pick as a command gives it: model topology, or a point on the sheet
/// (mm) where the nearest pick of the view is found.
// Read from a command and gone with it: one value, so a boxed pick would
// save nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum PickInput {
    Pick(ViewPick),
    Point(P2),
}

/// Linear dimensions made at once: from the first pick to each other one
/// (baseline, stacked) or from each to the next (chain, in a row).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Series {
    Baseline,
    Chain,
}

/// The paper between the dimension lines of a baseline series, mm.
const BASELINE_STEP: f64 = 7.0;
/// How far a model point may lie in front of a section's plane and still
/// be seen, mm.
const CUT_TOLERANCE: f64 = 1e-6;
/// How far a section's cut edge may lie from a planar face to be its cut,
/// mm.
const PLANE_TOLERANCE: f64 = 1e-3;
/// Picks within this of the nearest (paper mm) are as near as it: the one
/// drawn as a visible line goes first, then the one nearest the eye.
const PICK_TIE: f64 = 0.25;
/// How near a visible line of the view a pick's point must lie to count as
/// seen on it, paper mm.
const VISIBLE_NEAR: f64 = 0.05;
/// A parts list's balloons (mitcad#725): their column's distance right
/// of the view's box and the room of each, paper mm.
const BALLOON_COLUMN: f64 = 15.0;
const BALLOON_ROW: f64 = 12.0;

/// `add_drawing_annotation`: an annotation on a view, or (`"type":
/// "text"`) a text note on a sheet.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AddAnnotationCommand {
    drawing: String,
    #[serde(default)]
    view: Option<u32>,
    /// A text note's sheet.
    #[serde(default)]
    sheet: usize,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    picks: Vec<PickInput>,
    #[serde(default)]
    along: Option<Along>,
    /// Where its dimension line or text goes on the sheet; a text note's
    /// place.
    #[serde(default)]
    at: Option<P2>,
    #[serde(default)]
    format: Option<AnnotationFormat>,
    #[serde(default)]
    tolerance: Option<Tolerance>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    letter: Option<String>,
    #[serde(default)]
    frame: Option<GeometricTolerance>,
    #[serde(default)]
    finish: Option<SurfaceFinish>,
    /// A text note's letters' height, mm.
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    series: Option<Series>,
    /// A hole table's or a parts list's settings (`at`: its place).
    #[serde(default)]
    table: Option<TableEdit>,
    /// A parts list's balloons (mitcad#725): one on each row's bodies the
    /// view shows, as the Balloon tool places them.
    #[serde(default)]
    balloons: bool,
}

/// `edit_drawing_annotation`: what is given changes; `null` sets a field
/// back to its default.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EditAnnotationCommand {
    drawing: String,
    annotation: u32,
    /// A drag on the sheet: its line or text moves.
    #[serde(default, rename = "move")]
    shift: Option<P2>,
    /// Its dimension line or text through this sheet point; a text note's
    /// new place.
    #[serde(default)]
    at: Option<P2>,
    #[serde(default)]
    picks: Option<Vec<PickInput>>,
    #[serde(default)]
    along: Option<Along>,
    #[serde(default)]
    format: Option<FormatEdit>,
    #[serde(default, deserialize_with = "present")]
    tolerance: Option<Option<Tolerance>>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    letter: Option<String>,
    #[serde(default)]
    frame: Option<GeometricTolerance>,
    #[serde(default, deserialize_with = "present")]
    finish: Option<Option<SurfaceFinish>>,
    #[serde(default, deserialize_with = "present")]
    line_offset: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    text_offset: Option<Option<P2>>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    table: Option<TableEdit>,
}

/// The `drawing_pick` query: what a click at a sheet point (mm) picks on
/// a view (the one given, else the smallest drawn view there), for an
/// annotation of a type or what is wanted.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PickQuery {
    drawing: String,
    #[serde(default)]
    sheet: usize,
    #[serde(default)]
    view: Option<u32>,
    at: P2,
    #[serde(default, rename = "type")]
    kind: Option<AnnotationType>,
    #[serde(default)]
    want: Option<Want>,
}

/// A sheet's annotations worked out: their groups to draw, their entries
/// for the `drawing` query, why those that are broken are, and the sketch
/// dimensions its views retrieved (indices into the list given).
pub(super) struct Annotated {
    pub(super) groups: Vec<Group>,
    pub(super) entries: Vec<Value>,
    pub(super) warnings: Vec<String>,
    pub(super) retrieved: Vec<usize>,
}

/// How a view sees a pick: in its plane, and the model point (the
/// drawing's coordinates) that stands for it there (its point, a line's
/// middle, a circle's centre, a round face's axis' middle).
#[derive(Debug, Clone, Copy)]
struct Found {
    seen: Seen,
    at: Vec3,
}

/// A pick a click can find and how its view sees it.
#[derive(Debug, Clone)]
struct Candidate {
    pick: ViewPick,
    found: Found,
}

/// A view drawn, with what its annotations need: its index among the
/// sheet's drawn views and the bodies it shows.
pub(super) struct ViewAt<'a> {
    pub(super) placed: &'a PlacedView,
    pub(super) shown: Vec<Shown>,
    /// A detail's circle (view mm): picks outside it are not seen.
    pub(super) circle: Option<(P2, f64)>,
    /// It looks at the model from an angle (an isometric view, a detail of
    /// one): its lengths along x and y are foreshortened.
    pictorial: bool,
    /// Along no axis and not a section, whatever faces it looks square at:
    /// an aligned dimension measures its picks' true distance in 3D, as it
    /// always has there (mitcad#722).
    angled: bool,
    /// On a view along a given direction, the bodies (with their
    /// placements) that have a planar face it looks square at
    /// (mitcad#718): their annotations take the view as orthographic.
    square: Vec<(BodyUid, Vec<OccurrenceUid>)>,
    /// How many occurrences below the drawing's component its subject lies
    /// (an occurrence's path; 0 for the design or chosen bodies).
    pub(super) subject_depth: usize,
}

impl ViewAt<'_> {
    /// Whether an annotation of these picks sees the model from an angle:
    /// the view does, unless it looks square at a planar face of each
    /// pick's body (mitcad#722: not of any body it shows).
    fn pictorial_for(&self, picks: &[ViewPick]) -> bool {
        if !self.angled {
            return false;
        }
        if picks.is_empty() {
            return self.pictorial;
        }
        !picks.iter().all(|p| {
            self.square
                .iter()
                .any(|(body, path)| *body == p.body && path.as_slice() == p.path())
        })
    }

    /// Whether an annotation takes its picks as points and its value as
    /// their true distance in 3D: on a view from an angle for its picks,
    /// and an aligned dimension on any view along no axis (one square to a
    /// face too, so that files' aligned dimensions keep their values,
    /// mitcad#722).
    fn true_distance(
        &self,
        kind: AnnotationType,
        along: Option<Along>,
        picks: &[ViewPick],
    ) -> bool {
        self.pictorial_for(picks)
            || (self.angled && kind == AnnotationType::Linear && along == Some(Along::Aligned))
    }

    pub(super) fn frame(&self) -> Frame<'_> {
        Frame {
            map: &self.placed.map,
            view_box: self.placed.sheet_box(),
        }
    }

    /// Whether a view point is seen: inside a detail's circle, outside a
    /// break's band.
    pub(super) fn sees(&self, p: P2) -> bool {
        let inside = self.circle.is_none_or(|(c, r)| {
            let d = [p[0] - c[0], p[1] - c[1]];
            (d[0] * d[0] + d[1] * d[1]).sqrt() <= r + 1e-9
        });
        inside && !self.placed.map.in_break(p)
    }

    /// A section view's plane.
    fn cut(&self) -> Option<CutPlane> {
        self.placed.spec.section
    }

    /// Whether a model point is not taken away by the section's plane.
    pub(super) fn kept(&self, p: Vec3) -> bool {
        self.cut().is_none_or(|c| side(&c, p) <= CUT_TOLERANCE)
    }

    /// Whether a view point lies on a line the view draws as visible (its
    /// visible edges and the outlines of its cut faces).
    fn on_visible(&self, p: P2) -> bool {
        let near = VISIBLE_NEAR / self.placed.map.scale;
        let projection = &self.placed.projection;
        let on_edge = projection
            .edges
            .iter()
            .filter(|e| e.visible)
            .any(|e| curve_distance(&e.curve, p) <= near);
        on_edge
            || projection.cuts.iter().flatten().any(|lp| {
                (0..lp.len())
                    .any(|i| annotate::segment_distance(p, lp[i], lp[(i + 1) % lp.len()]) <= near)
            })
    }
}

/// How far a point is from a projected curve.
fn curve_distance(curve: &ViewCurve, p: P2) -> f64 {
    match curve {
        ViewCurve::Line(a, b) => annotate::segment_distance(p, *a, *b),
        ViewCurve::Polyline(points) => points
            .windows(2)
            .map(|w| annotate::segment_distance(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min),
        ViewCurve::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let d = [p[0] - center[0], p[1] - center[1]];
            let angle = d[1].atan2(d[0]);
            if (angle - start).rem_euclid(TAU) <= end - start + 1e-12 {
                ((d[0] * d[0] + d[1] * d[1]).sqrt() - radius).abs()
            } else {
                let at = |a: f64| [center[0] + radius * a.cos(), center[1] + radius * a.sin()];
                let (s, e) = (at(*start), at(*end));
                ((p[0] - s[0]).hypot(p[1] - s[1])).min((p[0] - e[0]).hypot(p[1] - e[1]))
            }
        }
    }
}

/// How far a model point lies on the normal's side of a section's plane
/// (the side taken away).
fn side(cut: &CutPlane, p: Vec3) -> f64 {
    mitcad_vec3::dot(sub3(p, cut.origin), cut.normal)
}

/// A curve of a body moved by its placement into the drawing's
/// coordinates.
fn placed_curve(curve: &CurveGeometry, t: &Transform) -> CurveGeometry {
    match curve {
        CurveGeometry::Line { start, end } => CurveGeometry::Line {
            start: t.apply_point(*start),
            end: t.apply_point(*end),
        },
        CurveGeometry::Circle {
            center,
            normal,
            radius,
            start,
            end,
        } => CurveGeometry::Circle {
            center: t.apply_point(*center),
            normal: t.apply_vector(*normal),
            radius: *radius,
            start: t.apply_point(*start),
            end: t.apply_point(*end),
        },
        CurveGeometry::Other { kind, start, end } => CurveGeometry::Other {
            kind: kind.clone(),
            start: t.apply_point(*start),
            end: t.apply_point(*end),
        },
    }
}

fn sub3(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mid3(a: Vec3, b: Vec3) -> Vec3 {
    [
        (a[0] + b[0]) / 2.0,
        (a[1] + b[1]) / 2.0,
        (a[2] + b[2]) / 2.0,
    ]
}

fn ends(curve: &CurveGeometry) -> (Vec3, Vec3) {
    match curve {
        CurveGeometry::Line { start, end }
        | CurveGeometry::Circle { start, end, .. }
        | CurveGeometry::Other { start, end, .. } => (*start, *end),
    }
}

/// What a section's plane leaves of a curve: a straight edge cut where it
/// crosses the plane, a circle or another curve whole while some of it
/// lies behind the plane; none when all of it lies in front.
fn kept_curve(curve: &CurveGeometry, cut: &CutPlane) -> Option<CurveGeometry> {
    let kept = |p: Vec3| side(cut, p) <= CUT_TOLERANCE;
    match curve {
        CurveGeometry::Line { start, end } => {
            let (s, e) = (side(cut, *start), side(cut, *end));
            match (s <= CUT_TOLERANCE, e <= CUT_TOLERANCE) {
                (false, false) => None,
                (true, true) => Some(curve.clone()),
                (from_start, _) => {
                    let at = mitcad_vec3::add_scaled(*start, sub3(*end, *start), s / (s - e));
                    Some(if from_start {
                        CurveGeometry::Line {
                            start: *start,
                            end: at,
                        }
                    } else {
                        CurveGeometry::Line {
                            start: at,
                            end: *end,
                        }
                    })
                }
            }
        }
        CurveGeometry::Circle {
            center,
            normal,
            radius,
            start,
            end,
        } => annotate::arc_points3(*center, *normal, *radius, *start, *end)
            .into_iter()
            .any(kept)
            .then(|| curve.clone()),
        CurveGeometry::Other { start, end, .. } => {
            (kept(*start) || kept(*end)).then(|| curve.clone())
        }
    }
}

/// How a view sees an edge (in the drawing's coordinates) or a point of
/// it: a section's what lies behind its plane.
fn seen_edge(
    view: &ViewAt<'_>,
    curve: &CurveGeometry,
    point: Option<PickPoint>,
) -> Result<Found, String> {
    let spec = &view.placed.spec;
    let (start, end) = ends(curve);
    let curve = match view.cut() {
        Some(cut) => {
            let gone = match point {
                Some(PickPoint::Start) => !view.kept(start),
                Some(PickPoint::End) => !view.kept(end),
                _ => false,
            };
            if gone {
                return Err("the point lies in front of the section's plane".to_owned());
            }
            kept_curve(curve, &cut)
                .ok_or_else(|| "the edge lies in front of the section's plane".to_owned())?
        }
        None => curve.clone(),
    };
    let seen = annotate::seen_curve(&curve, point, |p| spec.project(p), spec.toward_eye)?;
    let at = match (point, &curve) {
        (Some(PickPoint::Start), _) => start,
        (Some(PickPoint::End), _) => end,
        (_, CurveGeometry::Circle { center, .. }) => *center,
        (_, CurveGeometry::Line { start, end } | CurveGeometry::Other { start, end, .. }) => {
            mid3(*start, *end)
        }
    };
    Ok(Found { seen, at })
}

/// How far a round face reaches along its axis (from its axis point): the
/// circles that bound it (its own edges, mitcad#717: not another face's
/// on the same axis) with its radius, by their centres; how far the widest
/// of them turns (2 pi for a whole circle) and that circle; none when it
/// has none.
fn round_extent(
    geometry: &BodyGeometry,
    face: &RoundFace,
) -> Option<(f64, f64, f64, CurveGeometry)> {
    let tolerance = 1e-6 * face.radius.max(1.0);
    let bounds = |edge: &EdgeName| {
        edge.faces()
            .iter()
            .any(|f| *f == face.face || face.names.contains(f))
    };
    let mut out: Option<(f64, f64, f64, CurveGeometry)> = None;
    for (name, curve) in &geometry.edges {
        let CurveGeometry::Circle {
            center,
            normal,
            radius: r,
            start,
            end,
        } = curve
        else {
            continue;
        };
        if !bounds(name) {
            continue;
        }
        let along = mitcad_vec3::dot(sub3(*center, face.origin), face.axis);
        let off = mitcad_vec3::norm(sub3(
            sub3(*center, face.origin),
            mitcad_vec3::scale(face.axis, along),
        ));
        let n = mitcad_vec3::unit(*normal).unwrap_or([0.0, 0.0, 1.0]);
        if off > tolerance
            || (r - face.radius).abs() > tolerance
            || mitcad_vec3::dot(n, face.axis).abs() < 1.0 - 1e-6
        {
            continue;
        }
        let sweep = annotate::arc_sweep(*center, n, *r, *start, *end);
        out = Some(match out {
            None => (along, along, sweep, curve.clone()),
            Some((lo, hi, widest, arc)) => {
                let (widest, arc) = if sweep > widest {
                    (sweep, curve.clone())
                } else {
                    (widest, arc)
                };
                (lo.min(along), hi.max(along), widest, arc)
            }
        });
    }
    out
}

/// How a view sees a round face: end on, as its circle (an arc of it where
/// its ends are arcs); from the side, as its axis over the face's length
/// (none when it is seen at a slant or its ends are not circles); a
/// section's what lies behind its plane. With how far its widest end
/// turns.
fn seen_round(
    view: &ViewAt<'_>,
    geometry: &BodyGeometry,
    face: &RoundFace,
    transform: &Transform,
) -> Option<(Found, f64)> {
    let spec = &view.placed.spec;
    let (mut lo, mut hi, sweep, widest) = round_extent(geometry, face)?;
    let placed_axis = mitcad_vec3::unit(transform.apply_vector(face.axis))?;
    let point3 = |t: f64| transform.apply_point(mitcad_vec3::add_scaled(face.origin, face.axis, t));
    if let Some(cut) = view.cut() {
        let (s0, s1) = (side(&cut, point3(lo)), side(&cut, point3(hi)));
        if s0 > CUT_TOLERANCE && s1 > CUT_TOLERANCE {
            return None;
        }
        if (s0 - s1).abs() > 1e-12 {
            let t = lo + (hi - lo) * (s0 / (s0 - s1));
            if s0 > CUT_TOLERANCE {
                lo = t;
            } else if s1 > CUT_TOLERANCE {
                hi = t;
            }
        }
    }
    let facing = mitcad_vec3::dot(placed_axis, spec.toward_eye);
    if facing.abs() > 1.0 - 1e-6 {
        // The end nearer the eye stands for it.
        let near = if facing > 0.0 { hi } else { lo };
        let arc = if sweep < TAU - 1e-9 {
            match annotate::seen_curve(
                &placed_curve(&widest, transform),
                None,
                |p| spec.project(p),
                spec.toward_eye,
            ) {
                Ok(Seen::Circle { arc, .. }) => arc,
                _ => None,
            }
        } else {
            None
        };
        return Some((
            Found {
                seen: Seen::Circle {
                    center: spec.project(point3(near)),
                    radius: face.radius,
                    end_on: true,
                    across: [1.0, 0.0],
                    arc,
                },
                at: point3(near),
            },
            sweep,
        ));
    }
    if facing.abs() > 1e-6 || hi - lo < 1e-9 {
        return None;
    }
    Some((
        Found {
            seen: Seen::Axis {
                a: spec.project(point3(lo)),
                b: spec.project(point3(hi)),
                radius: face.radius,
            },
            at: point3((lo + hi) / 2.0),
        },
        sweep,
    ))
}

/// Where a section's plane cuts a planar face, as the view draws it: the
/// edges of the cut faces that lie in the face's plane, as one line from
/// the first to the last (mitcad#717: the cut edges are picked as the
/// faces they cut). `cuts`: the cut faces of the face's own body as the
/// view shows them (mitcad#718: another body's coplanar cut faces made the
/// line longer). None off a section, for a face parallel to the plane or
/// one it does not cut.
fn cut_line(
    view: &ViewAt<'_>,
    cuts: &[CutFace],
    face: &FlatFace,
    transform: &Transform,
) -> Option<Found> {
    let cut = view.cut()?;
    let spec = &view.placed.spec;
    let n = mitcad_vec3::unit(transform.apply_vector(face.normal))?;
    let o = transform.apply_point(face.origin);
    let direction = mitcad_vec3::cross(n, cut.normal);
    if mitcad_vec3::norm(direction) < 1e-9 {
        return None;
    }
    // A view point of the cut, in the model: on the plane, which the view
    // looks at square on.
    let depth = mitcad_vec3::dot(cut.origin, spec.toward_eye);
    let y = spec.y_axis();
    let lift = |p: P2| {
        mitcad_vec3::add(
            mitcad_vec3::add(
                mitcad_vec3::scale(spec.x_axis, p[0]),
                mitcad_vec3::scale(y, p[1]),
            ),
            mitcad_vec3::scale(spec.toward_eye, depth),
        )
    };
    let on = |p: P2| mitcad_vec3::dot(sub3(lift(p), o), n).abs() <= PLANE_TOLERANCE;
    let along = spec.project(direction);
    let length = (along[0] * along[0] + along[1] * along[1]).sqrt();
    if length < 1e-12 {
        return None;
    }
    let along = [along[0] / length, along[1] / length];
    let mut points: Vec<P2> = Vec::new();
    for lp in cuts.iter().flatten() {
        for i in 0..lp.len() {
            let (a, b) = (lp[i], lp[(i + 1) % lp.len()]);
            if (a[0] - b[0]).hypot(a[1] - b[1]) > 1e-9 && on(a) && on(b) {
                points.push(a);
                points.push(b);
            }
        }
    }
    let first = *points.first()?;
    let t = |p: &P2| (p[0] - first[0]) * along[0] + (p[1] - first[1]) * along[1];
    let low = points.iter().map(t).fold(f64::INFINITY, f64::min);
    let high = points.iter().map(t).fold(f64::NEG_INFINITY, f64::max);
    let at = |s: f64| [first[0] + along[0] * s, first[1] + along[1] * s];
    let (a, b) = (at(low), at(high));
    Some(Found {
        seen: Seen::Line(a, b),
        at: lift([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]),
    })
}

/// Its name in undo steps: a dimension, a centre mark, a note, ….
fn noun(kind: AnnotationType) -> &'static str {
    match kind {
        AnnotationType::CenterMark => "centre mark",
        AnnotationType::Centerline => "centreline",
        AnnotationType::Note => "note",
        AnnotationType::Datum => "datum label",
        AnnotationType::Frame => "feature control frame",
        AnnotationType::Finish => "surface finish symbol",
        AnnotationType::HoleTable => "hole table",
        AnnotationType::PartsList => "parts list",
        AnnotationType::Balloon => "balloon",
        _ => "dimension",
    }
}

/// Whether a view looks along none of its component's axes and is no
/// section: seen from an angle unless it looks square at a face.
fn angled(spec: &ViewSpec) -> bool {
    spec.section.is_none() && !super::drawings::along_an_axis(spec)
}

/// Why an annotation of this type, along this direction, is not made on a
/// pictorial view (its value would be foreshortened), if it is not.
fn pictorial_refusal(kind: AnnotationType, along: Option<Along>) -> Option<String> {
    let refused = match kind {
        AnnotationType::Linear => along != Some(Along::Aligned),
        AnnotationType::Ordinate | AnnotationType::Angle => true,
        _ => false,
    };
    refused.then(|| {
        "the view looks at the model from an angle: its lengths along x and y, ordinates and \
         angles would be foreshortened; an aligned dimension measures the true length"
            .to_owned()
    })
}

impl<K: Kernel> Document<K> {
    /// A body's named edges, round faces and planar faces, kept by its
    /// shape version.
    pub(super) fn body_geometry(&self, body: BodyUid) -> Result<Arc<BodyGeometry>, String> {
        let shape = self
            .body_shape(body)
            .ok_or_else(|| format!("the body {body} is gone"))?;
        let mut fp = Fingerprint::new("drawing-body-geometry-v2");
        fp.str(&body.to_string());
        let own = fp.finish();
        fp.u128(self.body_version(body).unwrap_or(0));
        let key = fp.finish();
        if let Some(geometry) = self.drawing_cache().geometry(key) {
            return Ok(geometry);
        }
        let mut geometry = BodyGeometry::default();
        match self.kernel().edge_geometries(shape) {
            Ok(edges) => {
                for (name, curve) in edges {
                    if let Some(name) = name.and_then(|n| n.parse::<EdgeName>().ok()) {
                        geometry.edges.push((name, curve));
                    }
                }
            }
            Err(KernelError::Unsupported(_)) => {}
            Err(e) => return Err(format!("the edges of {body}: {e}")),
        }
        match self.kernel().face_geometries(shape) {
            Ok(faces) => {
                for (names, surface) in faces {
                    let names: Vec<FaceName> = names
                        .iter()
                        .filter_map(|n| n.parse::<FaceName>().ok())
                        .collect();
                    let Some(face) = names.first().cloned() else {
                        continue;
                    };
                    match surface {
                        SurfaceGeometry::Cylinder {
                            origin,
                            axis,
                            radius,
                        } => {
                            if let Some(axis) = mitcad_vec3::unit(axis) {
                                geometry.cylinders.push(RoundFace {
                                    face,
                                    names,
                                    origin,
                                    axis,
                                    radius,
                                });
                            }
                        }
                        SurfaceGeometry::Plane { origin, normal } => {
                            if let Some(normal) = mitcad_vec3::unit(normal) {
                                geometry.planes.push(FlatFace {
                                    face,
                                    names,
                                    origin,
                                    normal,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(KernelError::Unsupported(_)) => {}
            Err(e) => return Err(format!("the faces of {body}: {e}")),
        }
        let geometry = Arc::new(geometry);
        self.drawing_cache()
            .keep_geometry(own, key, Arc::clone(&geometry));
        Ok(geometry)
    }

    /// The bodies a view looks square at a planar face of (mitcad#718):
    /// their lengths in that face's plane are true, so a view along a
    /// given direction straight at an inclined face is drawn and
    /// dimensioned as an orthographic one. Only on a view along a given
    /// direction (`along_a_given_direction`) that is no section and along
    /// no axis (mitcad#722: the isometric presets and the corner views
    /// stay pictorial whatever faces they happen to look square at).
    pub(super) fn square_bodies(
        &self,
        sheet: &DrawingSheet,
        id: u32,
        shown: &[Shown],
        spec: &ViewSpec,
    ) -> Vec<(BodyUid, Vec<OccurrenceUid>)> {
        if !angled(spec) || !sheet.along_a_given_direction(id) {
            return Vec::new();
        }
        shown
            .iter()
            .filter(|s| {
                self.body_geometry(s.body).is_ok_and(|geometry| {
                    geometry.planes.iter().any(|plane| {
                        mitcad_vec3::unit(s.transform.apply_vector(plane.normal)).is_some_and(|n| {
                            mitcad_vec3::dot(n, spec.toward_eye).abs() > 1.0 - 1e-9
                        })
                    })
                })
            })
            .map(|s| (s.body, s.path.clone()))
            .collect()
    }

    /// The cut faces of one body of a section as the view shows them (in a
    /// detail's circle, beside breaks); the view's own when it shows that
    /// body alone or the body's projection fails.
    fn own_cuts(&self, view: &ViewAt<'_>, s: &Shown) -> Vec<CutFace> {
        let placed = view.placed;
        if view.shown.len() < 2 {
            return placed.projection.cuts.clone();
        }
        match self.body_in_view(view, s) {
            Ok(own) => own.cuts.clone(),
            Err(_) => placed.projection.cuts.clone(),
        }
    }

    /// What one body of a view shows there, from its own projection cut
    /// to the detail's circle and beside the breaks (`own_cuts`,
    /// `bodies_seen`). Kept in a bounded cache of its own by the body's
    /// version and placement and the view, apart from the views'
    /// projections (mitcad#722: an assembly's section with a parts list
    /// projected each body again on every draw and pushed the views'
    /// projections out). A failed projection is not kept.
    pub(super) fn body_in_view(
        &self,
        view: &ViewAt<'_>,
        s: &Shown,
    ) -> Result<Arc<BodyInView>, String> {
        let placed = view.placed;
        let version = self.body_version(s.body).unwrap_or(0);
        let mut fp = Fingerprint::new("drawing-body-in-view-v1");
        fp.u128(super::drawings::view_key(
            "drawing-body-in-view-v1",
            &[(s.body, version, s.transform)],
            &placed.spec,
        ));
        fp.option(view.circle, |fp, (center, radius)| {
            fp.f64(center[0]).f64(center[1]).f64(radius);
        });
        fp.u64(placed.map.breaks.len() as u64);
        for band in &placed.map.breaks {
            fp.u64(band.axis as u64)
                .f64(band.from)
                .f64(band.to)
                .f64(band.gap);
        }
        let key = fp.finish();
        if let Some(own) = self.drawing_cache().body_view(key) {
            return Ok(own);
        }
        let projection = self.project_now(std::slice::from_ref(s), &placed.spec)?;
        let inside = match view.circle {
            Some((center, radius)) => clip::inside_circle(&projection, center, radius),
            None => projection,
        };
        let shown = clip::without_bands(&inside, &placed.map.breaks);
        let own = Arc::new(BodyInView {
            seen: !view_bounds(&shown).is_empty(),
            cuts: shown.cuts,
        });
        self.drawing_cache().keep_body_view(key, Arc::clone(&own));
        Ok(own)
    }

    /// Whether a view is seen from an angle: neither along an axis of its
    /// component nor square to a face it shows (`square_bodies`), and not
    /// a section.
    pub(super) fn seen_from_an_angle(
        &self,
        sheet: &DrawingSheet,
        id: u32,
        shown: &[Shown],
        spec: &ViewSpec,
    ) -> bool {
        angled(spec) && self.square_bodies(sheet, id, shown, spec).is_empty()
    }

    /// The view `id` of a sheet as drawn, with its bodies; why not, when
    /// it is not drawn.
    pub(super) fn view_at<'e>(
        &self,
        drawing: &Drawing,
        si: usize,
        evaluated: &'e Evaluated,
        id: u32,
    ) -> Result<ViewAt<'e>, String> {
        if let Some(error) = evaluated.errors.get(&id) {
            return Err(format!("view {id} is not drawn: {error}"));
        }
        let index = *evaluated
            .index_of
            .get(&id)
            .ok_or_else(|| format!("there is no view {id} on the sheet"))?;
        let shown: Vec<Shown> = evaluated
            .view_shown
            .get(&id)
            .map(|own| own.iter().map(|i| evaluated.shown[*i].clone()).collect())
            .unwrap_or_default();
        let circle = match drawing.sheets[si].view(id).map(|v| &v.kind) {
            Some(ViewKind::Detail { center, radius, .. }) => Some((*center, *radius)),
            _ => None,
        };
        let placed = &evaluated.views[index];
        let sheet = &drawing.sheets[si];
        let square = self.square_bodies(sheet, id, &shown, &placed.spec);
        let from_an_angle = angled(&placed.spec);
        Ok(ViewAt {
            placed,
            shown,
            circle,
            pictorial: from_an_angle && square.is_empty(),
            angled: from_an_angle,
            square,
            subject_depth: match drawing.sheets[si].subject(id) {
                Subject::Occurrence { occurrence } => occurrence.0.len(),
                _ => 0,
            },
        })
    }

    /// Everything a click on a view can pick: each shown body's named
    /// edges (and their ends and a circle's centre), its round faces seen
    /// from the side or end on, and on a section the planar faces its
    /// plane cuts; what the section takes away left out.
    fn candidates(&self, view: &ViewAt<'_>) -> Vec<Candidate> {
        let mut out = Vec::new();
        for s in &view.shown {
            let Ok(geometry) = self.body_geometry(s.body) else {
                continue;
            };
            let occurrence = (!s.path.is_empty()).then(|| OccurrencePath(s.path.clone()));
            let pick = |edge: Option<&EdgeName>, face: Option<&FaceName>, point| ViewPick {
                body: s.body,
                occurrence: occurrence.clone(),
                edge: edge.cloned(),
                face: face.cloned(),
                point,
            };
            for (edge, curve) in &geometry.edges {
                let curve = placed_curve(curve, &s.transform);
                let points: &[Option<PickPoint>] = match &curve {
                    CurveGeometry::Circle {
                        center,
                        normal,
                        radius,
                        start,
                        end,
                    } => {
                        // A whole circle has no ends to pick (mitcad#717).
                        if annotate::arc_sweep(*center, *normal, *radius, *start, *end)
                            >= TAU - 1e-9
                        {
                            &[None, Some(PickPoint::Center)]
                        } else {
                            &[
                                None,
                                Some(PickPoint::Start),
                                Some(PickPoint::End),
                                Some(PickPoint::Center),
                            ]
                        }
                    }
                    _ => &[None, Some(PickPoint::Start), Some(PickPoint::End)],
                };
                for point in points {
                    if let Ok(found) = seen_edge(view, &curve, *point) {
                        out.push(Candidate {
                            pick: pick(Some(edge), None, *point),
                            found,
                        });
                    }
                }
            }
            for face in &geometry.cylinders {
                if let Some((found, _)) = seen_round(view, &geometry, face, &s.transform) {
                    out.push(Candidate {
                        pick: pick(None, Some(&face.face), None),
                        found,
                    });
                }
            }
            if view.cut().is_some() {
                let cuts = self.own_cuts(view, s);
                for face in &geometry.planes {
                    if let Some(found) = cut_line(view, &cuts, face, &s.transform) {
                        out.push(Candidate {
                            pick: pick(None, Some(&face.face), None),
                            found,
                        });
                    }
                }
            }
        }
        out
    }

    /// The pick nearest a sheet point on a view, of what is wanted: a
    /// point within 2 mm of paper first, else an edge or a face within
    /// 3 mm; where the view shows it (its part nearest the click inside a
    /// detail's circle, beside a break's bands). Of picks as near, the one
    /// the view draws as a visible line, then the one nearest the eye.
    fn pick_at(&self, view: &ViewAt<'_>, at: P2, want: Want) -> Option<Candidate> {
        let map = &view.placed.map;
        let q = map.view_point(at);
        let reach = annotate::PICK_REACH / map.scale;
        let point_reach = annotate::POINT_REACH / map.scale;
        let tie = PICK_TIE / map.scale;
        let eye = view.placed.spec.toward_eye;
        let candidates = self.candidates(view);
        let nearest = |keep: &dyn Fn(&Candidate) -> bool, within: f64| {
            let mut found: Vec<(f64, &Candidate)> = candidates
                .iter()
                .filter(|c| keep(c))
                .filter_map(|c| {
                    let d = c.found.seen.distance(q);
                    (d <= within && view.sees(c.found.seen.nearest(q))).then_some((d, c))
                })
                .collect();
            let best = found.iter().map(|(d, _)| *d).fold(f64::INFINITY, f64::min);
            found.retain(|(d, _)| *d <= best + tie);
            found
                .into_iter()
                .map(|(d, c)| {
                    let visible = view.on_visible(c.found.seen.nearest(q));
                    let depth = mitcad_vec3::dot(c.found.at, eye);
                    (visible, depth, d, c)
                })
                .max_by(|a, b| {
                    a.0.cmp(&b.0)
                        .then(a.1.total_cmp(&b.1))
                        .then(b.2.total_cmp(&a.2))
                })
                .map(|(.., c)| c.clone())
        };
        let is_point = |c: &Candidate| c.pick.point.is_some();
        let is_line =
            |c: &Candidate| c.pick.point.is_none() && matches!(c.found.seen, Seen::Line(..));
        let is_round = |c: &Candidate| {
            c.pick.point.is_none()
                && matches!(c.found.seen, Seen::Circle { .. } | Seen::Axis { .. })
        };
        match want {
            Want::Any => nearest(&is_point, point_reach)
                .or_else(|| nearest(&|c: &Candidate| c.pick.point.is_none(), reach)),
            Want::Point => nearest(&is_point, point_reach.max(reach)).or_else(|| {
                // A circle's rim or a round face: its centre.
                nearest(&is_round, reach).map(|mut c| {
                    c.pick.point = Some(PickPoint::Center);
                    c.found.seen = Seen::Point(c.found.seen.point());
                    c
                })
            }),
            Want::Line => nearest(&is_line, reach),
            Want::Circle => nearest(&is_round, reach),
        }
    }

    /// How a view sees a pick now, found again by its name; why not, when
    /// it is gone (or, on a section, in front of its plane).
    fn resolve_found(&self, view: &ViewAt<'_>, pick: &ViewPick) -> Result<Found, String> {
        let s = view
            .shown
            .iter()
            .find(|s| s.body == pick.body && s.path.as_slice() == pick.path())
            .ok_or_else(|| {
                let at = pick
                    .occurrence
                    .as_ref()
                    .map_or(String::new(), |p| format!(" in {p}"));
                format!("the view does not show the body {}{at}", pick.body)
            })?;
        let shape = self
            .body_shape(s.body)
            .ok_or_else(|| format!("the body {} is gone", s.body))?;
        if let Some(edge) = &pick.edge {
            let curve = self
                .kernel()
                .edge_geometry(shape, edge)
                .map_err(|_| format!("the edge {edge} of {} is gone", s.name))?;
            return seen_edge(view, &placed_curve(&curve, &s.transform), pick.point)
                .map_err(|e| format!("the edge {edge} of {}: {e}", s.name));
        }
        let face = pick
            .face
            .as_ref()
            .ok_or_else(|| "a pick is a body's edge or face".to_owned())?;
        let surface = self
            .kernel()
            .face_geometry(shape, face)
            .map_err(|_| format!("the face {face} of {} is gone", s.name))?;
        let geometry = self.body_geometry(s.body)?;
        let found = match surface {
            SurfaceGeometry::Cylinder {
                origin,
                axis,
                radius,
            } => {
                let axis = mitcad_vec3::unit(axis)
                    .ok_or_else(|| format!("the face {face} has no axis"))?;
                let round = RoundFace {
                    face: face.clone(),
                    names: geometry
                        .cylinder(face)
                        .map_or_else(|| vec![face.clone()], |c| c.names.clone()),
                    origin,
                    axis,
                    radius,
                };
                seen_round(view, &geometry, &round, &s.transform)
                    .map(|(found, _)| found)
                    .ok_or_else(|| {
                        format!(
                            "the face {face} of {} is seen neither end on nor from the side",
                            s.name
                        )
                    })?
            }
            SurfaceGeometry::Plane { origin, normal } if view.cut().is_some() => {
                let flat = FlatFace {
                    face: face.clone(),
                    names: vec![face.clone()],
                    origin,
                    normal: mitcad_vec3::unit(normal).unwrap_or([0.0, 0.0, 1.0]),
                };
                cut_line(view, &self.own_cuts(view, s), &flat, &s.transform).ok_or_else(|| {
                    format!(
                        "the section's plane does not cut the face {face} of {}",
                        s.name
                    )
                })?
            }
            _ => return Err(format!("the face {face} of {} is not round", s.name)),
        };
        Ok(match pick.point {
            Some(PickPoint::Center) => Found {
                seen: Seen::Point(found.seen.point()),
                at: found.at,
            },
            _ => found,
        })
    }

    /// The picks of a command found on a view: model topology as given,
    /// a sheet point as the pick nearest it; with the view point of the
    /// first one clicked (a leader's arrow goes nearest it).
    pub(super) fn picks_of(
        &self,
        view: &ViewAt<'_>,
        inputs: &[PickInput],
        want: Want,
    ) -> Result<(Vec<ViewPick>, Option<P2>), ApiError> {
        let mut picks = Vec::with_capacity(inputs.len());
        let mut clicked = None;
        for (i, input) in inputs.iter().enumerate() {
            match input {
                PickInput::Pick(pick) => picks.push(pick.clone()),
                PickInput::Point(at) => {
                    let found = self.pick_at(view, *at, want).ok_or_else(|| {
                        ApiError(format!(
                            "nothing to pick at [{:.1}, {:.1}] on the view",
                            at[0], at[1]
                        ))
                    })?;
                    if i == 0 {
                        clicked = Some(view.placed.map.view_point(*at));
                    }
                    picks.push(found.pick);
                }
            }
        }
        Ok((picks, clicked))
    }

    /// Why a datum label of this letter cannot go on a drawing, if it
    /// cannot: the model or another drawing has one (mitcad#717).
    fn datum_taken(&self, letter: &str, drawing: DrawingUid) -> Option<String> {
        if let Some(datum) = self.datum_named(letter) {
            return Some(format!("datum {letter} exists: {}", datum.name));
        }
        self.drawing_with_datum(letter, Some(drawing))
            .map(|d| format!("datum {letter} exists: a datum label of {}", d.name))
    }

    pub(super) fn add_drawing_annotation(
        &mut self,
        c: AddAnnotationCommand,
    ) -> Result<Value, ApiError> {
        let old = self.find_drawing(&c.drawing)?.clone();
        let first = old.next_annotation_id();
        if c.kind == "text" {
            return self.add_text_note(old, c, first);
        }
        let kind: AnnotationType = serde_json::from_value(json!(c.kind)).map_err(|_| {
            ApiError(format!(
                "unknown annotation type '{}': {} or text",
                c.kind,
                AnnotationType::ALL.map(AnnotationType::key).join(", ")
            ))
        })?;
        let view_id = c
            .view
            .ok_or_else(|| ApiError(format!("a {} is on a view", noun(kind))))?;
        if c.table.is_some() && !kind.is_table() {
            return Err(ApiError(format!("a {} has no table settings", noun(kind))));
        }
        if c.balloons && kind != AnnotationType::PartsList {
            return Err(ApiError(format!("a {} places no balloons", noun(kind))));
        }
        let (si, _) = old
            .find_view(view_id)
            .ok_or_else(|| ApiError(format!("{} has no view {view_id}", old.name)))?;
        let evaluated = self.evaluate_views(&old, si);
        let view = self
            .view_at(&old, si, &evaluated, view_id)
            .map_err(ApiError)?;
        let (picks, clicked) = self.picks_of(&view, &c.picks, kind.wants())?;
        let found: Vec<Found> = picks
            .iter()
            .map(|p| self.resolve_found(&view, p))
            .collect::<Result<_, _>>()
            .map_err(ApiError)?;
        let frame = view.frame();
        let mut template = ViewAnnotation::new(first, kind, Vec::new());
        template.format = c.format.clone().unwrap_or_default();
        template.tolerance = c.tolerance.clone();
        template.text = c.text.clone();
        template.frame = c.frame.clone();
        template.finish = match kind {
            AnnotationType::Finish => Some(c.finish.clone().unwrap_or_default()),
            _ => c.finish.clone(),
        };
        template.at = clicked.filter(|_| {
            matches!(
                kind,
                AnnotationType::Note
                    | AnnotationType::Datum
                    | AnnotationType::Frame
                    | AnnotationType::Finish
                    | AnnotationType::Balloon
            )
        });
        // A table: its place on the sheet and its settings.
        match (kind.is_table(), &c.table) {
            (true, edit) => {
                let at = c.at.ok_or_else(|| {
                    ApiError(format!(
                        "a {} needs its place on the sheet, `at`",
                        noun(kind)
                    ))
                })?;
                if kind == AnnotationType::HoleTable && view.pictorial {
                    return Err(ApiError(
                        "a hole table lists the holes its view sees end on: this view looks \
                         at the model from an angle"
                            .to_owned(),
                    ));
                }
                let mut settings = TableSettings::at(at);
                if let Some(edit) = edit {
                    edit.apply(&mut settings);
                }
                template.table = Some(settings);
            }
            (false, Some(_)) => {
                return Err(ApiError(format!("a {} has no table settings", noun(kind))));
            }
            (false, None) => {}
        }
        if kind == AnnotationType::Datum {
            let letter = match &c.letter {
                Some(letter) => letter.trim().to_owned(),
                // One helper for the model's letters and the drawings'.
                None => self.next_datum_letter().map_err(ApiError)?,
            };
            if let Some(why) = self.datum_taken(&letter, old.uid) {
                return Err(ApiError(why));
            }
            template.letter = Some(letter);
        } else {
            template.letter = c.letter.clone();
        }
        // One annotation of the picks, or a series of linear ones.
        let sets: Vec<Vec<usize>> = match c.series {
            None => vec![(0..picks.len()).collect()],
            Some(_) if kind != AnnotationType::Linear => {
                return Err(ApiError(
                    "a baseline or chain is of linear dimensions".to_owned(),
                ));
            }
            Some(_) if picks.len() < 2 => {
                return Err(ApiError(
                    "a baseline or chain takes two picks at least".to_owned(),
                ));
            }
            Some(Series::Baseline) => (1..picks.len()).map(|i| vec![0, i]).collect(),
            Some(Series::Chain) => (1..picks.len()).map(|i| vec![i - 1, i]).collect(),
        };
        let along = match kind {
            // On a pictorial view, aligned unless given: its picks' true
            // distance.
            AnnotationType::Linear => {
                Some(
                    c.along
                        .unwrap_or_else(|| match (found.first(), found.last()) {
                            _ if view.pictorial_for(&picks) => Along::Aligned,
                            (Some(a), Some(b)) if found.len() >= 2 => {
                                annotate::default_along(&[a.seen, b.seen])
                            }
                            _ => Along::Horizontal,
                        }),
                )
            }
            AnnotationType::Ordinate => Some(c.along.unwrap_or(Along::Horizontal)),
            _ => c.along,
        };
        if view.pictorial_for(&picks)
            && let Some(why) = pictorial_refusal(kind, along)
        {
            return Err(ApiError(why));
        }
        let mut added: Vec<ViewAnnotation> = Vec::new();
        // A series' first line on the sheet (across it) and the side away
        // from its picks: the others stacked beyond it or in a row with it.
        let mut base: Option<(f64, f64)> = None;
        for (k, set) in sets.iter().enumerate() {
            let mut annotation = template.clone();
            annotation.id = first + k as u32;
            annotation.picks = set.iter().map(|i| picks[*i].clone()).collect();
            annotation.along = along;
            // Its picks and fields first: what follows takes them as they
            // are.
            annotation.check().map_err(ApiError)?;
            let points = view.true_distance(kind, along, &annotation.picks);
            let own: Vec<Seen> = set.iter().map(|i| shown_seen(points, &found[*i])).collect();
            // Its place: through `at`, or after the first of a series.
            if k == 0
                && let Some(at) = c.at
            {
                annotate::placed_at(&mut annotation, &own, &frame, at);
            }
            if c.series.is_some() {
                match base {
                    None => {
                        // The first one's line, as it is drawn.
                        if let Some((position, offset)) =
                            annotate::line_position(&annotation, &own, &frame)
                        {
                            annotation.line_offset = Some(offset);
                            base = Some((position, if offset < 0.0 { -1.0 } else { 1.0 }));
                        }
                    }
                    Some((position, away)) => {
                        let step = match c.series {
                            Some(Series::Baseline) => BASELINE_STEP * k as f64,
                            _ => 0.0,
                        };
                        if let Some(offset) =
                            annotate::offset_for(&annotation, &own, &frame, position + away * step)
                        {
                            annotation.line_offset = Some(offset);
                        }
                    }
                }
            }
            annotate::measure(&annotation, &own).map_err(ApiError)?;
            annotation.check().map_err(ApiError)?;
            added.push(annotation);
        }
        // A parts list's balloons (mitcad#725), in the same undo step.
        let balloons = if c.balloons {
            let levels = template
                .table
                .as_ref()
                .map(|t| t.levels)
                .unwrap_or_default();
            self.balloons_for(&view, levels, first + added.len() as u32)
        } else {
            Vec::new()
        };
        let ids: Vec<u32> = added.iter().map(|a| a.id).collect();
        let balloon_ids: Vec<u32> = balloons.iter().map(|a| a.id).collect();
        let mut changed = old.clone();
        let i = changed.sheets[si].view_index(view_id).expect("found above");
        changed.sheets[si].views[i].annotations.extend(added);
        changed.sheets[si].views[i].annotations.extend(balloons);
        let label = match ids.as_slice() {
            [id] if balloon_ids.is_empty() => format!("Add {} {id} to {}", noun(kind), old.name),
            [id] => format!(
                "Add {} {id} and {} balloons to {}",
                noun(kind),
                balloon_ids.len(),
                old.name
            ),
            _ => format!("Add {} dimensions to {}", ids.len(), old.name),
        };
        self.set_drawing(old.uid, label, changed)?;
        Ok(
            json!({"drawing": old.uid, "annotation": ids[0], "annotations": ids,
                  "balloons": balloon_ids}),
        )
    }

    /// A balloon on each parts list row's bodies a view shows (mitcad#725):
    /// the bodies the list counts (`bodies_seen`), on a straight edge of
    /// them the view draws as a visible line, the first one found, numbered
    /// from `first`. Their circles stand in one column right of the view's
    /// box, from its top down, in the order of their arrows' heights, so
    /// that they cover neither one another nor the view. A row whose bodies
    /// show no such edge gets none.
    fn balloons_for(&self, view: &ViewAt<'_>, levels: Levels, first: u32) -> Vec<ViewAnnotation> {
        let assembly = self.assembly();
        let depth = view.subject_depth;
        let candidates = self.candidates(view);
        let map = &view.placed.map;
        let mut done: Vec<RowKey> = Vec::new();
        // Each row's pick with its arrow's point on the sheet.
        let mut found: Vec<(ViewPick, P2, P2)> = Vec::new();
        for s in &self.bodies_seen(view) {
            let Some(key) = row_of(assembly, s.body, &s.path, levels, depth) else {
                continue;
            };
            if done.contains(&key) {
                continue;
            }
            // A line of the row's bodies, a visible one if there is one.
            let line_of = |visible: bool| {
                candidates.iter().find(|c| {
                    let point = c.found.seen.point();
                    c.pick.point.is_none()
                        && matches!(c.found.seen, Seen::Line(..))
                        && row_of(assembly, c.pick.body, c.pick.path(), levels, depth) == Some(key)
                        && view.sees(point)
                        && (!visible || view.on_visible(point))
                })
            };
            let Some(c) = line_of(true).or_else(|| line_of(false)) else {
                continue;
            };
            done.push(key);
            let point = c.found.seen.point();
            found.push((c.pick.clone(), point, map.at(point)));
        }
        // The highest arrow gets the top place of the column.
        found.sort_by(|a, b| b.2[1].total_cmp(&a.2[1]));
        let bounds = &view.placed.bounds;
        let (low, high) = (map.at(bounds.min), map.at(bounds.max));
        let right = low[0].max(high[0]) + BALLOON_COLUMN;
        let top = low[1].max(high[1]) - BALLOON_ROW / 2.0;
        found
            .into_iter()
            .enumerate()
            .map(|(k, (pick, at, tip))| {
                let mut balloon =
                    ViewAnnotation::new(first + k as u32, AnnotationType::Balloon, vec![pick]);
                balloon.at = Some(at);
                let row = top - BALLOON_ROW * k as f64;
                // A view with nothing drawn has no box: beside the arrows.
                balloon.text_offset = Some(if right.is_finite() && top.is_finite() {
                    [right - tip[0], row - tip[1]]
                } else {
                    [12.0, 10.0 + BALLOON_ROW * k as f64]
                });
                balloon
            })
            .collect()
    }

    fn add_text_note(
        &mut self,
        old: Drawing,
        c: AddAnnotationCommand,
        id: u32,
    ) -> Result<Value, ApiError> {
        if c.view.is_some() || !c.picks.is_empty() {
            return Err(ApiError(
                "a text note is on the sheet: give its `sheet` and `at`, no view or picks"
                    .to_owned(),
            ));
        }
        let si = c.sheet;
        if si >= old.sheets.len() {
            return Err(ApiError(format!("{} has no sheet {si}", old.name)));
        }
        let note = SheetNote {
            id,
            text: c.text.clone().unwrap_or_default(),
            at: c
                .at
                .ok_or_else(|| ApiError("a text note needs its place, `at`".to_owned()))?,
            height: c.height,
        };
        note.check().map_err(ApiError)?;
        let mut changed = old.clone();
        changed.sheets[si].notes.push(note);
        self.set_drawing(old.uid, format!("Add note {id} to {}", old.name), changed)?;
        Ok(json!({"drawing": old.uid, "annotation": id, "annotations": [id]}))
    }

    pub(super) fn edit_drawing_annotation(
        &mut self,
        c: EditAnnotationCommand,
    ) -> Result<Value, ApiError> {
        let old = self.find_drawing(&c.drawing)?.clone();
        let id = c.annotation;
        let (si, view_id) = old
            .find_annotation(id)
            .ok_or_else(|| ApiError(format!("{} has no annotation {id}", old.name)))?;
        let mut changed = old.clone();
        let only_moved = c.picks.is_none()
            && c.along.is_none()
            && c.format.is_none()
            && c.tolerance.is_none()
            && c.text.is_none()
            && c.letter.is_none()
            && c.frame.is_none()
            && c.finish.is_none()
            && c.height.is_none()
            && c.table.is_none();
        let Some(view_id) = view_id else {
            // A text note.
            let note = changed.sheets[si]
                .notes
                .iter_mut()
                .find(|n| n.id == id)
                .expect("found above");
            if c.picks.is_some()
                || c.along.is_some()
                || c.format.is_some()
                || c.tolerance.is_some()
                || c.letter.is_some()
                || c.frame.is_some()
                || c.finish.is_some()
                || c.line_offset.is_some()
                || c.text_offset.is_some()
                || c.table.is_some()
            {
                return Err(ApiError(
                    "a text note has a text, a place and a height only".to_owned(),
                ));
            }
            if let Some(text) = &c.text {
                note.text.clone_from(text);
            }
            if let Some(at) = c.at {
                note.at = at;
            }
            if let Some(d) = c.shift {
                note.at = [note.at[0] + d[0], note.at[1] + d[1]];
            }
            if let Some(height) = c.height {
                note.height = Some(height);
            }
            note.check().map_err(ApiError)?;
            let verb = if only_moved { "Move" } else { "Edit" };
            self.set_drawing(
                old.uid,
                format!("{verb} note {id} of {}", old.name),
                changed,
            )?;
            return Ok(json!({"drawing": old.uid, "annotation": id}));
        };
        let evaluated = self.evaluate_views(&old, si);
        let view = self
            .view_at(&old, si, &evaluated, view_id)
            .map_err(ApiError)?;
        let vi = changed.sheets[si].view_index(view_id).expect("found");
        // An unchanged letter is no new claim (mitcad#718: files where a
        // letter is shared refused every edit that named it).
        let current = changed.sheets[si].views[vi]
            .annotations
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.letter.clone());
        if let Some(letter) = &c.letter
            && current.as_deref() != Some(letter.trim())
            && let Some(why) = self.datum_taken(letter.trim(), old.uid)
        {
            return Err(ApiError(why));
        }
        let annotation = changed.sheets[si].views[vi]
            .annotations
            .iter_mut()
            .find(|a| a.id == id)
            .expect("found above");
        let kind = annotation.kind;
        if let Some(inputs) = &c.picks {
            let (picks, clicked) = self.picks_of(&view, inputs, kind.wants())?;
            annotation.picks = picks;
            if clicked.is_some() {
                annotation.at = clicked;
            }
            // Drawn afresh about the new picks.
            annotation.line_offset = None;
            annotation.text_offset = None;
        }
        if let Some(along) = c.along {
            if along != annotation.along.unwrap_or(along) {
                annotation.line_offset = None;
                annotation.text_offset = None;
            }
            annotation.along = Some(along);
        }
        if let Some(format) = &c.format {
            format.apply(&mut annotation.format);
        }
        if let Some(tolerance) = &c.tolerance {
            annotation.tolerance.clone_from(tolerance);
        }
        if let Some(text) = &c.text {
            annotation.text = Some(text.clone());
        }
        if let Some(letter) = &c.letter {
            annotation.letter = Some(letter.trim().to_owned());
        }
        if let Some(frame) = &c.frame {
            annotation.frame = Some(frame.clone());
        }
        if let Some(finish) = &c.finish {
            annotation.finish = match kind {
                AnnotationType::Finish => Some(finish.clone().unwrap_or_default()),
                _ => finish.clone(),
            };
        }
        if let Some(offset) = c.line_offset {
            // An angle's arc keeps a radius, as a drag and a place keep it
            // (mitcad#717).
            annotation.line_offset = match kind {
                AnnotationType::Angle => offset.map(|v| v.max(2.0)),
                _ => offset,
            };
        }
        if let Some(offset) = c.text_offset {
            annotation.text_offset = offset;
        }
        if c.height.is_some() {
            return Err(ApiError("only a text note has a height".to_owned()));
        }
        if let Some(edit) = &c.table {
            let table = annotation
                .table
                .as_mut()
                .ok_or_else(|| ApiError(format!("a {} has no table settings", noun(kind))))?;
            edit.apply(table);
        }
        annotation.check().map_err(ApiError)?;
        if view.pictorial_for(&annotation.picks)
            && let Some(why) = pictorial_refusal(kind, annotation.along)
        {
            return Err(ApiError(why));
        }
        if kind.is_table() {
            // A table is placed on the sheet, whatever its picks.
            if let Some(at) = c.at {
                annotate::placed_at(annotation, &[], &view.frame(), at);
            }
            if let Some(d) = c.shift {
                annotate::moved(annotation, &[], &view.frame(), d);
            }
        } else if c.at.is_some() || c.shift.is_some() {
            let points = view.true_distance(kind, annotation.along, &annotation.picks);
            let seen: Vec<Seen> = annotation
                .picks
                .iter()
                .map(|p| self.resolve_found(&view, p).map(|f| shown_seen(points, &f)))
                .collect::<Result<_, _>>()
                .map_err(|e| ApiError(format!("annotation {id} is broken: {e}")))?;
            let frame = view.frame();
            if let Some(at) = c.at {
                annotate::placed_at(annotation, &seen, &frame, at);
            }
            if let Some(d) = c.shift {
                annotate::moved(annotation, &seen, &frame, d);
            }
        }
        annotation.check().map_err(ApiError)?;
        let verb = if only_moved && (c.at.is_some() || c.shift.is_some()) {
            "Move"
        } else {
            "Edit"
        };
        self.set_drawing(
            old.uid,
            format!("{verb} {} {id} of {}", noun(kind), old.name),
            changed,
        )?;
        Ok(json!({"drawing": old.uid, "annotation": id}))
    }

    pub(super) fn delete_drawing_annotations(
        &mut self,
        drawing: &str,
        one: Option<u32>,
        several: Option<Vec<u32>>,
    ) -> Result<Value, ApiError> {
        let ids: Vec<u32> = match (one, several) {
            (Some(id), None) => vec![id],
            (None, Some(ids)) if !ids.is_empty() => ids,
            _ => {
                return Err(ApiError(
                    "give the `annotation` to delete, or the `annotations`".to_owned(),
                ));
            }
        };
        let old = self.find_drawing(drawing)?.clone();
        for id in &ids {
            if old.find_annotation(*id).is_none() {
                return Err(ApiError(format!("{} has no annotation {id}", old.name)));
            }
        }
        let mut changed = old.clone();
        for sheet in &mut changed.sheets {
            sheet.notes.retain(|n| !ids.contains(&n.id));
            for view in &mut sheet.views {
                view.annotations.retain(|a| !ids.contains(&a.id));
            }
        }
        let label = match ids.as_slice() {
            [id] => format!("Delete annotation {id} of {}", old.name),
            _ => format!("Delete {} annotations of {}", ids.len(), old.name),
        };
        self.set_drawing(old.uid, label, changed)?;
        Ok(json!({"drawing": old.uid, "deleted": ids}))
    }

    /// `drawing_pick`: what a click picks on a view, with what to show of
    /// it on the sheet (`items`, sheet mm), or `"pick": null`.
    pub(super) fn drawing_pick_json(&self, query: &PickQuery) -> Result<Value, ApiError> {
        let drawing = self.find_drawing(&query.drawing)?;
        let si = query.sheet;
        if si >= drawing.sheets.len() {
            return Err(ApiError(format!("{} has no sheet {si}", drawing.name)));
        }
        let evaluated = self.evaluate_views(drawing, si);
        // The view given, else the smallest drawn one there.
        let id = match query.view {
            Some(id) => Some(id),
            None => {
                let at = query.at;
                drawing.sheets[si]
                    .views
                    .iter()
                    .filter_map(|v| {
                        let i = *evaluated.index_of.get(&v.id)?;
                        let b = evaluated.views[i].sheet_box();
                        let inside = !b.is_empty()
                            && (0..2).all(|k| at[k] >= b.min[k] - 2.0 && at[k] <= b.max[k] + 2.0);
                        inside.then_some((b.width() * b.height(), v.id))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .map(|(_, id)| id)
            }
        };
        let Some(id) = id else {
            return Ok(json!({"pick": null}));
        };
        let view = self
            .view_at(drawing, si, &evaluated, id)
            .map_err(ApiError)?;
        let want = query
            .want
            .or_else(|| query.kind.map(AnnotationType::wants))
            .unwrap_or_default();
        let Some(found) = self.pick_at(&view, query.at, want) else {
            return Ok(json!({"view": id, "pick": null}));
        };
        let map = &view.placed.map;
        let scale = map.scale;
        let seen = found.found.seen;
        let (what, items) = match seen {
            Seen::Point(p) => ("point", vec![json!({"point": map.at(p)})]),
            Seen::Line(a, b) => ("line", vec![json!({"line": [map.at(a), map.at(b)]})]),
            Seen::Circle {
                center,
                radius,
                end_on: true,
                arc,
                ..
            } => (
                "circle",
                vec![match arc {
                    Some((start, end)) => json!({"arc": {"center": map.at(center),
                        "radius": radius * scale, "start": start, "end": end}}),
                    None => json!({"circle": {"center": map.at(center), "radius": radius * scale}}),
                }],
            ),
            Seen::Circle {
                center,
                radius,
                across,
                arc,
                ..
            } => {
                let (low, high) = arc.unwrap_or((-radius, radius));
                let a = [center[0] + across[0] * low, center[1] + across[1] * low];
                let b = [center[0] + across[0] * high, center[1] + across[1] * high];
                ("circle", vec![json!({"line": [map.at(a), map.at(b)]})])
            }
            Seen::Axis { a, b, radius } => {
                let d = [b[0] - a[0], b[1] - a[1]];
                let length = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-12);
                let n = [-d[1] / length * radius, d[0] / length * radius];
                let edge = |s: f64| {
                    json!({"line": [map.at([a[0] + s * n[0], a[1] + s * n[1]]),
                                    map.at([b[0] + s * n[0], b[1] + s * n[1]])]})
                };
                ("axis", vec![edge(1.0), edge(-1.0)])
            }
        };
        Ok(json!({
            "view": id,
            "pick": found.pick,
            "seen": what,
            "at": map.at(seen.point()),
            "items": items,
        }))
    }

    /// A sheet's annotations worked out for drawing it: each view's (and
    /// its automatic centre marks and retrieved dimensions) and the text
    /// notes. `sketches`: the design's sketch dimensions (what views
    /// retrieve), when the sheet's views retrieve any.
    pub(super) fn annotate_sheet(
        &self,
        drawing: &Drawing,
        si: usize,
        evaluated: &Evaluated,
        units: Units,
        sketches: &[(FeatureUid, SketchDimension)],
    ) -> Annotated {
        let sheet = &drawing.sheets[si];
        let words_units = WordUnits::new(units.symbol, units.mm);
        let mut out = Annotated {
            groups: Vec::new(),
            entries: Vec::new(),
            warnings: Vec::new(),
            retrieved: Vec::new(),
        };
        // The datum letters a frame may refer to: the model's and every
        // drawing's, as the frame dialog offers them.
        let letters = self.all_datum_letters();
        let datums: Vec<&str> = letters.iter().map(String::as_str).collect();
        // The sheet's parts lists first: its balloons take their numbers
        // (mitcad#707 phase 3).
        let mut parts_lists: Vec<(u32, Result<PartsList, String>)> = Vec::new();
        for view in &sheet.views {
            for a in view
                .annotations
                .iter()
                .filter(|a| a.kind == AnnotationType::PartsList)
            {
                let list = self
                    .view_at(drawing, si, evaluated, view.id)
                    .and_then(|v| self.parts_list_of(a, view.id, &v));
                parts_lists.push((a.id, list));
            }
        }
        // The holes' threads, once, when a hole table asks.
        let mut threads: Option<Vec<super::drawing::ThreadOf>> = None;
        for view in &sheet.views {
            let drawn = self.view_at(drawing, si, evaluated, view.id);
            // Where the broken ones are listed: beside the view's box, or
            // its place.
            let mut broken_at = match &drawn {
                Ok(v) => {
                    let b = v.placed.sheet_box();
                    if b.is_empty() {
                        view.place
                    } else {
                        [b.max[0] + 2.0, b.max[1]]
                    }
                }
                Err(_) => view.place,
            };
            if let Ok(v) = &drawn {
                if view.center_marks {
                    out.groups.extend(self.automatic_marks(v));
                }
                if view.retrieved_dimensions
                    && drawing.component.is_root()
                    && v.placed.role != ViewRole::Pictorial
                    && !v.pictorial
                {
                    for (i, g) in self.retrieved_dimensions(v, sketches, words_units) {
                        out.groups.push(g);
                        if !out.retrieved.contains(&i) {
                            out.retrieved.push(i);
                        }
                    }
                }
            }
            for annotation in &view.annotations {
                let mut entry = serde_json::to_value(annotation).unwrap_or(Value::Null);
                entry["view"] = json!(view.id);
                let laid = match &drawn {
                    Err(e) => Err(e.clone()),
                    Ok(v) => match annotation.kind {
                        AnnotationType::HoleTable => {
                            let threads = threads.get_or_insert_with(|| {
                                super::drawing::threads_of(&self.threads_json())
                            });
                            self.lay_out_hole_table(annotation, v, units, threads, &mut entry)
                                .map(|(laid, label, warnings)| {
                                    out.warnings.extend(warnings);
                                    (laid, None, label, None)
                                })
                        }
                        AnnotationType::PartsList => {
                            match parts_lists.iter().find(|(id, _)| *id == annotation.id) {
                                Some((_, Ok(list))) => {
                                    table_entry(
                                        &mut entry,
                                        annotation,
                                        false,
                                        list.data.rows.len(),
                                    );
                                    let place =
                                        annotation.table.as_ref().map_or([0.0; 2], |t| t.place);
                                    let laid = lay_out_table(
                                        annotation.id,
                                        "parts-list",
                                        &list.data,
                                        place,
                                    );
                                    Ok((laid, None, list.label.clone(), None))
                                }
                                Some((_, Err(e))) => Err(e.clone()),
                                None => Err("the parts list is not worked out".to_owned()),
                            }
                        }
                        AnnotationType::Balloon => {
                            let number = annotation.picks.first().and_then(|pick| {
                                let own = parts_lists
                                    .iter()
                                    .filter(|(_, l)| l.as_ref().is_ok_and(|l| l.view == view.id));
                                let others = parts_lists
                                    .iter()
                                    .filter(|(_, l)| l.as_ref().is_ok_and(|l| l.view != view.id));
                                own.chain(others)
                                    .filter_map(|(_, l)| l.as_ref().ok())
                                    .find_map(|l| {
                                        let key = row_of(
                                            self.assembly(),
                                            pick.body,
                                            pick.path(),
                                            l.levels,
                                            l.depth,
                                        )?;
                                        l.item(key)
                                    })
                            });
                            match number {
                                Some(n) => self.lay_out_annotation(
                                    annotation,
                                    v,
                                    words_units,
                                    &datums,
                                    Some(n.to_string().as_str()),
                                ),
                                // Gone, or listed nowhere: why.
                                None => self
                                    .lay_out_annotation(
                                        annotation,
                                        v,
                                        words_units,
                                        &datums,
                                        Some("?"),
                                    )
                                    .and_then(|_| Err(self.unlisted(annotation))),
                            }
                        }
                        _ => self.lay_out_annotation(annotation, v, words_units, &datums, None),
                    },
                };
                match laid {
                    Ok((laid, value, text, warning)) => {
                        entry["broken"] = json!(false);
                        if let Some(value) = value {
                            entry["value"] = json!(value);
                        }
                        entry["label"] = json!(text);
                        if let Some(warning) = warning {
                            entry["warning"] = json!(warning);
                        }
                        entry["box"] = bounds_json(&laid.bounds);
                        out.groups.push(laid.group);
                    }
                    Err(error) => {
                        entry["broken"] = json!(true);
                        entry["error"] = json!(error);
                        let at = broken_at;
                        entry["box"] = json!([[at[0], at[1] - 6.0], [at[0] + 14.0, at[1]]]);
                        broken_at = [at[0], at[1] - 8.0];
                        out.warnings.push(format!(
                            "annotation {} on view {}: {error}",
                            annotation.id, view.id
                        ));
                    }
                }
                out.entries.push(entry);
            }
        }
        for note in &sheet.notes {
            let laid = annotate::lay_out_note(note);
            let mut entry = serde_json::to_value(note).unwrap_or(Value::Null);
            entry["type"] = json!("text");
            entry["view"] = Value::Null;
            entry["broken"] = json!(false);
            entry["label"] = json!(note.text.replace('\n', " / "));
            entry["box"] = bounds_json(&laid.bounds);
            out.groups.push(laid.group);
            out.entries.push(entry);
        }
        out
    }

    /// The sketch dimensions a view retrieves (mitcad#717): of the
    /// sketches that shape its bodies, those it sees square on, where it
    /// shows them (behind a section's plane, in a detail's circle, beside
    /// breaks); written by #702's engine. Each with its index in
    /// `sketches`.
    fn retrieved_dimensions(
        &self,
        view: &ViewAt<'_>,
        sketches: &[(FeatureUid, SketchDimension)],
        units: WordUnits,
    ) -> Vec<(usize, Group)> {
        let spec = &view.placed.spec;
        let bodies: Vec<BodyUid> = view.shown.iter().map(|s| s.body).collect();
        let own = self.sketches_shaping(&bodies);
        let mut out = Vec::new();
        for (i, (sketch, dim)) in sketches.iter().enumerate() {
            if !own.contains(sketch) {
                continue;
            }
            let (normal, points, kind, value) = match dim {
                SketchDimension::Linear {
                    a,
                    b,
                    normal,
                    value,
                    ..
                } => (*normal, vec![*a, *b], MeasureKind::Distance, value.abs()),
                SketchDimension::Radial {
                    center,
                    normal,
                    diameter,
                    value,
                    ..
                } => (
                    *normal,
                    vec![*center],
                    if *diameter {
                        MeasureKind::Diameter
                    } else {
                        MeasureKind::Radius
                    },
                    *value,
                ),
            };
            let square = mitcad_vec3::dot(spec.toward_eye, normal).abs();
            if square <= 1.0 - 1e-9
                || !points
                    .iter()
                    .all(|p| view.kept(*p) && view.sees(spec.project(*p)))
            {
                continue;
            }
            let (words, _) = value_text(kind, &AnnotationFormat::default(), None, value, units);
            if let Some(mut g) = sketch_dimension(dim, spec, &view.placed.map, &words.text) {
                g.class = "retrieved-dimension";
                out.push((i, g));
            }
        }
        out
    }

    /// An annotation laid out on its view: its group and box, its value,
    /// its words on one line and a warning of its words (a fit code out of
    /// range, a datum referred to that no label has). `number`: a
    /// balloon's.
    #[allow(clippy::type_complexity)]
    fn lay_out_annotation(
        &self,
        annotation: &ViewAnnotation,
        view: &ViewAt<'_>,
        units: WordUnits,
        datums: &[&str],
        number: Option<&str>,
    ) -> Result<(annotate::Laid, Option<f64>, String, Option<String>), String> {
        if view.pictorial_for(&annotation.picks)
            && let Some(why) = pictorial_refusal(annotation.kind, annotation.along)
        {
            return Err(why);
        }
        let found: Vec<Found> = annotation
            .picks
            .iter()
            .map(|p| self.resolve_found(view, p))
            .collect::<Result<_, _>>()?;
        let points = view.true_distance(annotation.kind, annotation.along, &annotation.picks);
        let seen: Vec<Seen> = found.iter().map(|f| shown_seen(points, f)).collect();
        let mut value = annotate::measure(annotation, &seen)?;
        // On a pictorial view an aligned dimension's value is the true
        // distance of its picks; on any view along no axis too.
        if points && annotation.kind == AnnotationType::Linear && found.len() == 2 {
            value = Some(mitcad_vec3::distance(found[0].at, found[1].at));
        }
        let (words, mut warning) = match (value, number) {
            (Some(v), _) => {
                let (words, warning) = value_text(
                    annotation.kind.measure_kind(),
                    &annotation.format,
                    annotation.tolerance.as_ref(),
                    v,
                    units,
                );
                (Some(words), warning)
            }
            (None, Some(n)) => (
                Some(annotate::ValueText {
                    text: n.to_owned(),
                    main: n.to_owned(),
                    ..annotate::ValueText::default()
                }),
                None,
            ),
            (None, None) => (None, None),
        };
        let (cells, text) = match (&annotation.frame, annotation.kind) {
            (Some(frame), AnnotationType::Frame) => {
                let (cells, text, frame_warning) = frame_words(frame, units, datums);
                warning = warning.or(frame_warning);
                (cells, text)
            }
            _ => (
                Vec::new(),
                match annotation.kind {
                    AnnotationType::Note => annotation
                        .text
                        .as_deref()
                        .unwrap_or("")
                        .lines()
                        .map(str::trim_end)
                        .collect::<Vec<_>>()
                        .join(" / "),
                    AnnotationType::Datum => annotation.letter.clone().unwrap_or_default(),
                    AnnotationType::Finish => annotation
                        .finish
                        .as_ref()
                        .and_then(|f| f.text.clone())
                        .unwrap_or_default(),
                    _ => words.as_ref().map(|w| w.text.clone()).unwrap_or_default(),
                },
            ),
        };
        let laid = annotate::lay_out(annotation, &seen, &view.frame(), words.as_ref(), &cells)?;
        Ok((laid, value, text, warning))
    }

    /// A hole table laid out with its rows (its entry given its columns),
    /// its words on one line (its rows, `no holes`) and the warnings of
    /// finding its holes.
    fn lay_out_hole_table(
        &self,
        annotation: &ViewAnnotation,
        view: &ViewAt<'_>,
        units: Units,
        threads: &[super::drawing::ThreadOf],
        entry: &mut Value,
    ) -> Result<(annotate::Laid, String, Vec<String>), String> {
        // Its origin pick, found again: the origin follows the model.
        let origin = annotation
            .picks
            .first()
            .map(|pick| self.resolve_found(view, pick).map(|f| f.seen.point()))
            .transpose()?;
        let (data, warnings) = self.hole_table(annotation, origin, view, units, threads)?;
        table_entry(entry, annotation, true, data.rows.len());
        let label = if data.rows.is_empty() {
            "no holes".to_owned()
        } else {
            data.rows
                .iter()
                .map(|r| r.join(" "))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let place = annotation.table.as_ref().map_or([0.0; 2], |t| t.place);
        Ok((
            lay_out_table(annotation.id, "hole-table", &data, place),
            label,
            warnings,
        ))
    }

    /// Why a balloon has no number: no parts list on its sheet lists what
    /// it picks.
    fn unlisted(&self, annotation: &ViewAnnotation) -> String {
        let what = annotation.picks.first().map_or_else(String::new, |pick| {
            match pick
                .path()
                .first()
                .and_then(|o| self.assembly().occurrence(*o))
            {
                Some(o) => self.assembly().name(o.component),
                None => self.body_name(pick.body),
            }
        });
        format!("no parts list on the sheet lists {what}")
    }

    /// A view's automatic centre marks: on each circle it sees end on that
    /// turns half way round at least (concentric ones once, the largest;
    /// mitcad#717: a half-cylinder hole's too), and a centreline on each
    /// round face it sees from the side whose ends turn half way round at
    /// least; inside a detail's circle, beside breaks and behind a
    /// section's plane.
    fn automatic_marks(&self, view: &ViewAt<'_>) -> Vec<Group> {
        let spec = &view.placed.spec;
        let map = &view.placed.map;
        let mut circles: Vec<(P2, f64)> = Vec::new();
        let mut axes: Vec<(P2, P2)> = Vec::new();
        for s in &view.shown {
            let Ok(geometry) = self.body_geometry(s.body) else {
                continue;
            };
            for (_, curve) in &geometry.edges {
                let CurveGeometry::Circle {
                    center,
                    normal,
                    radius,
                    start,
                    end,
                } = placed_curve(curve, &s.transform)
                else {
                    continue;
                };
                let half = annotate::arc_sweep(center, normal, radius, start, end) >= PI - 1e-6;
                let n = mitcad_vec3::unit(normal).unwrap_or([0.0, 0.0, 1.0]);
                if !half
                    || mitcad_vec3::dot(n, spec.toward_eye).abs() < 1.0 - 1e-6
                    || !view.kept(center)
                {
                    continue;
                }
                let c = spec.project(center);
                if !view.sees(c) {
                    continue;
                }
                match circles
                    .iter_mut()
                    .find(|(o, _)| (o[0] - c[0]).abs() < 1e-6 && (o[1] - c[1]).abs() < 1e-6)
                {
                    Some(found) => found.1 = found.1.max(radius),
                    None => circles.push((c, radius)),
                }
            }
            for face in &geometry.cylinders {
                if let Some((
                    Found {
                        seen: Seen::Axis { a, b, .. },
                        ..
                    },
                    sweep,
                )) = seen_round(view, &geometry, face, &s.transform)
                    && sweep >= PI - 1e-6
                    && view.sees(a)
                    && view.sees(b)
                {
                    let same =
                        |p: P2, q: P2| (p[0] - q[0]).abs() < 1e-6 && (p[1] - q[1]).abs() < 1e-6;
                    if !axes
                        .iter()
                        .any(|(p, q)| (same(*p, a) && same(*q, b)) || (same(*p, b) && same(*q, a)))
                    {
                        axes.push((a, b));
                    }
                }
            }
        }
        let mut out = Vec::new();
        for (c, r) in circles {
            let mut g = Group::new("center-mark");
            annotate::center_mark(&mut g, map.at(c), r * map.scale);
            out.push(g);
        }
        for (a, b) in axes {
            let mut g = Group::new("center-mark");
            annotate::centerline(&mut g, map.at(a), map.at(b), 0.0, 0.0);
            out.push(g);
        }
        out
    }
}

/// A table's entry in the `drawing` query: the columns it shows, those it
/// can show (its type's own, and a parts list's property columns) with
/// their headers, and how many rows it has.
fn table_entry(entry: &mut Value, annotation: &ViewAnnotation, holes: bool, rows: usize) {
    let Some(table) = &annotation.table else {
        return;
    };
    let shown = table.shown_columns(holes);
    let own: &[&str] = if holes { &HOLE_COLUMNS } else { &PARTS_COLUMNS };
    let mut available: Vec<String> = own.iter().map(|c| (*c).to_owned()).collect();
    for column in &shown {
        if !available.contains(column) {
            available.push(column.clone());
        }
    }
    entry["columns"] = json!(shown);
    entry["available"] = json!(
        available
            .iter()
            .map(|c| json!({"column": c, "header": header(c)}))
            .collect::<Vec<_>>()
    );
    entry["sort"] = json!(table.sort_column(holes));
    entry["rows"] = json!(rows);
}

/// A pick as an annotation of a pictorial view takes it: its point, so
/// that an aligned dimension runs from one to the other; as seen elsewhere.
/// `points`: [`ViewAt::true_distance`].
fn shown_seen(points: bool, found: &Found) -> Seen {
    if points {
        match found.seen {
            Seen::Line(..) | Seen::Axis { .. } => Seen::Point(found.seen.point()),
            other => other,
        }
    } else {
        found.seen
    }
}

pub(super) fn bounds_json(b: &Bounds) -> Value {
    if b.is_empty() {
        Value::Null
    } else {
        json!([b.min, b.max])
    }
}
