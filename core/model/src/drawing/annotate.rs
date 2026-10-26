// SPDX-License-Identifier: MIT
//! Annotations on a drawing's views (mitcad#707, phase 2): dimensions
//! (linear, aligned, angular, radius, diameter, ordinate), centre marks and
//! centrelines, leader notes, datum labels, feature control frames and
//! surface finish symbols, each on one view and kept with it; and text
//! notes on the sheet.
//!
//! The annotations of mitcad#702 and these are one engine: the same value
//! formats, tolerances and ISO 286 fit codes, datum letters and geometric
//! tolerances (`crate::document::annotations`), and the same words for a
//! value (`api/annotations.rs` writes them). What a view adds is where it
//! is drawn: on the sheet, in the view's plane.
//!
//! A pick ([`ViewPick`]) names model topology, not a point of the sheet: a
//! body (with the occurrence that places it, from the drawing's component)
//! and one of its edges or faces by its topological name, or a point of an
//! edge (its start, end or a circle's centre). The application picks on the
//! sheet; the model finds the edge or face whose projection passes there
//! (`api/drawing_annotations.rs`), so the annotation follows the model.
//! When a pick is gone the annotation is broken: it is not drawn, and the
//! `drawing` query says why.
//!
//! Places are kept relative to the picks in millimetres of paper: a
//! dimension line's distance from its measured points (`line_offset`), the
//! text's shift from its default place (`text_offset`), a leader's text
//! from its arrow. So an annotation keeps its look at any scale and moves
//! with its view and with the model.
//!
//! This module holds the definitions and lays an annotation out on the
//! sheet from its picks as its view sees them ([`Seen`]), without the
//! model.

use std::f64::consts::{FRAC_1_SQRT_2, PI, TAU};

use mitcad_vec3::vec2::{self, perp};
use mitcad_vec3::vec2::{add as add2, dot as dot2, norm as len2, scale as mul2, sub as sub2};
use serde::{Deserialize, Serialize};

use super::P2;
use super::clip::ViewMap;
use super::compose::{
    ARROW_LENGTH, Bounds, DIMENSION_TEXT, EXTENSION_BEYOND, EXTENSION_GAP, arrow, upright,
};
use super::sheet::{Anchor, Group, Item, Pen, text_width};
use super::tables::TableSettings;
use crate::datum::{CurveGeometry, Vec3};
use crate::document::{
    AnnotationFormat, Characteristic, GeometricTolerance, MeasureKind, Tolerance, datum_letter,
};
use crate::features::OccurrencePath;
use crate::ids::BodyUid;
use crate::topo::{EdgeName, FaceName};

/// A dimension line's distance from what it measures (paper mm) when it is
/// placed for itself, beyond the view's box.
pub const LINE_GAP: f64 = 10.0;
/// A leader's text from its arrow (paper mm) when not moved.
pub const NOTE_SHIFT: P2 = [10.0, 8.0];
/// A datum label's square from its triangle (paper mm) when not moved.
pub const DATUM_SHIFT: P2 = [0.0, 12.0];
/// How far centre lines reach beyond their circle or cylinder, paper mm.
pub const CENTER_BEYOND: f64 = 2.5;
/// The side of a datum label's square and a frame's cells, paper mm.
const CELL: f64 = 7.0;
/// A balloon's circle from its arrow (paper mm) when not moved, and its
/// radius at least.
pub const BALLOON_SHIFT: P2 = [12.0, 10.0];
const BALLOON_RADIUS: f64 = 3.5;
/// How near a pick must be to the click (paper mm), and a point nearer
/// than an edge to win over it.
pub const PICK_REACH: f64 = 3.0;
pub const POINT_REACH: f64 = 2.0;

/// What an annotation on a view is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationType {
    /// Between two picks, horizontal, vertical or aligned in the view.
    Linear,
    /// Between two straight edges seen in the view.
    Angle,
    /// Of a circle or a round face.
    Radius,
    Diameter,
    /// From an origin pick to another, along the view's x or y.
    Ordinate,
    /// Crossed centre lines of a circle seen end on; the centreline of a
    /// round face seen from the side.
    CenterMark,
    /// Through the centres of two circles, between two edges, or a round
    /// face's axis.
    Centerline,
    /// A leader note: text on an arrow to a pick.
    Note,
    /// A datum label (ISO 5459).
    Datum,
    /// A feature control frame (ISO 1101).
    Frame,
    /// A surface finish symbol (ISO 1302).
    Finish,
    /// The round holes the view sees end on, tagged and listed
    /// (mitcad#707 phase 3, [`super::tables`]).
    HoleTable,
    /// The components the view shows, numbered and listed.
    PartsList,
    /// A component's number in a parts list, in a circle on a leader to
    /// it.
    Balloon,
}

impl AnnotationType {
    pub const ALL: [Self; 14] = [
        Self::Linear,
        Self::Angle,
        Self::Radius,
        Self::Diameter,
        Self::Ordinate,
        Self::CenterMark,
        Self::Centerline,
        Self::Note,
        Self::Datum,
        Self::Frame,
        Self::Finish,
        Self::HoleTable,
        Self::PartsList,
        Self::Balloon,
    ];

    /// Its name in commands and files.
    pub fn key(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Angle => "angle",
            Self::Radius => "radius",
            Self::Diameter => "diameter",
            Self::Ordinate => "ordinate",
            Self::CenterMark => "center_mark",
            Self::Centerline => "centerline",
            Self::Note => "note",
            Self::Datum => "datum",
            Self::Frame => "frame",
            Self::Finish => "finish",
            Self::HoleTable => "hole_table",
            Self::PartsList => "parts_list",
            Self::Balloon => "balloon",
        }
    }

    /// How many picks it takes, at least and at most: a hole table its
    /// origin or none, a parts list none.
    pub fn picks(self) -> (usize, usize) {
        match self {
            Self::Linear | Self::Angle | Self::Ordinate => (2, 2),
            Self::Centerline => (1, 2),
            Self::HoleTable => (0, 1),
            Self::PartsList => (0, 0),
            _ => (1, 1),
        }
    }

    /// Whether it is a table ([`super::tables`]): placed on the sheet, its
    /// rows worked out from the model.
    pub fn is_table(self) -> bool {
        matches!(self, Self::HoleTable | Self::PartsList)
    }

    /// Whether it shows a measured value.
    pub fn is_dimension(self) -> bool {
        matches!(
            self,
            Self::Linear | Self::Angle | Self::Radius | Self::Diameter | Self::Ordinate
        )
    }

    /// The kind of #702's dimensions whose words it takes: its prefix (Ø,
    /// R), whether its value is a length or an angle.
    pub fn measure_kind(self) -> MeasureKind {
        match self {
            Self::Angle => MeasureKind::Angle,
            Self::Radius => MeasureKind::Radius,
            Self::Diameter => MeasureKind::Diameter,
            _ => MeasureKind::Distance,
        }
    }

    /// What a click for it picks: points first, lines, circles or round
    /// faces, or anything.
    pub fn wants(self) -> Want {
        match self {
            Self::Ordinate | Self::HoleTable => Want::Point,
            Self::Angle => Want::Line,
            Self::Radius | Self::Diameter | Self::CenterMark => Want::Circle,
            _ => Want::Any,
        }
    }
}

/// What a click on a view picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Want {
    /// A point near the click if there is one, else an edge or a face.
    #[default]
    Any,
    /// An edge's end or a circle's centre (a circle's rim gives its
    /// centre).
    Point,
    /// A straight edge.
    Line,
    /// A circle or a round face.
    Circle,
}

/// The direction a linear or an ordinate dimension measures along: the
/// view's x, its y, or the line between its picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Along {
    Horizontal,
    Vertical,
    Aligned,
}

/// A point of an edge or a round face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickPoint {
    Start,
    End,
    /// A circle's or a round face's centre, a straight edge's middle.
    Center,
}

/// Model topology picked on a view: a body where an occurrence places it
/// (a path from the drawing's component; its own placement when left
/// out) and an edge or a face of it, or a point of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewPick {
    pub body: BodyUid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<OccurrencePath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<EdgeName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<PickPoint>,
}

impl ViewPick {
    /// The occurrence path it names, empty for none.
    pub fn path(&self) -> &[crate::ids::OccurrenceUid] {
        self.occurrence.as_ref().map_or(&[], |p| p.0.as_slice())
    }

    fn check(&self) -> Result<(), String> {
        match (&self.edge, &self.face, self.point) {
            (Some(_), None, _) => Ok(()),
            (None, Some(_), None | Some(PickPoint::Center)) => Ok(()),
            (None, Some(_), Some(_)) => Err("a face's point is its centre".to_owned()),
            _ => Err("a pick is a body's edge or face".to_owned()),
        }
    }
}

/// How a surface finish symbol says the surface is made (ISO 1302): any
/// way, by removing material (a bar across the symbol) or without (a
/// circle in it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishProcess {
    #[default]
    Any,
    Removal,
    NoRemoval,
}

/// A surface finish symbol's requirements: the process and the text over
/// it (`Ra 1.6`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceFinish {
    #[serde(default, skip_serializing_if = "is_any")]
    pub process: FinishProcess,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

fn is_any(process: &FinishProcess) -> bool {
    *process == FinishProcess::Any
}

/// An annotation on a view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewAnnotation {
    /// Unique among the drawing's annotations and text notes, from 1.
    pub id: u32,
    #[serde(rename = "type")]
    pub kind: AnnotationType,
    pub picks: Vec<ViewPick>,
    /// A linear dimension's (and an ordinate's: horizontal or vertical).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub along: Option<Along>,
    /// A dimension line's place across it from its default one's anchor
    /// (paper mm, signed): a linear dimension's from the middle of its
    /// picks, an ordinate's from its origin, an angle's arc radius; the
    /// default (beyond the view's box) when left out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_offset: Option<f64>,
    /// The text's shift from its default place, paper mm (along and across
    /// a dimension line); a leader's text, a datum's square, a frame, a
    /// finish symbol from the arrow's point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_offset: Option<P2>,
    /// Where on the view (view mm) a leader's pick was clicked: its arrow
    /// points at the pick's point nearest it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<P2>,
    #[serde(default, skip_serializing_if = "AnnotationFormat::is_default")]
    pub format: AnnotationFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<Tolerance>,
    /// A note's text, lines split at `\n`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// A datum label's letter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<GeometricTolerance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<SurfaceFinish>,
    /// A hole table's or a parts list's settings (its place on the sheet,
    /// columns, sort, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<TableSettings>,
}

/// A text note on the sheet, without a leader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SheetNote {
    /// Unique among the drawing's annotations and text notes.
    pub id: u32,
    /// Lines split at `\n`.
    pub text: String,
    /// Where its first line starts on the sheet, mm.
    pub at: P2,
    /// Its letters' height, mm (3.5 when left out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
}

impl SheetNote {
    pub(crate) fn check(&self) -> Result<(), String> {
        let at = format!("note {}", self.id);
        if self.text.trim().is_empty() {
            return Err(format!("{at}: a note needs a text"));
        }
        if !self.at.iter().all(|v| v.is_finite()) {
            return Err(format!("{at}: its place is not a point"));
        }
        if let Some(h) = self.height
            && !(h.is_finite() && h > 0.0)
        {
            return Err(format!("{at}: its height is more than 0"));
        }
        Ok(())
    }
}

impl ViewAnnotation {
    /// An annotation of this type on these picks, every other field left
    /// to its default.
    pub fn new(id: u32, kind: AnnotationType, picks: Vec<ViewPick>) -> Self {
        Self {
            id,
            kind,
            picks,
            along: None,
            line_offset: None,
            text_offset: None,
            at: None,
            format: AnnotationFormat::default(),
            tolerance: None,
            text: None,
            letter: None,
            frame: None,
            finish: None,
            table: None,
        }
    }

    /// Why it cannot be kept, if it cannot (not its geometry).
    pub(crate) fn check(&self) -> Result<(), String> {
        let at = format!("annotation {}", self.id);
        self.check_fields().map_err(|e| format!("{at}: {e}"))
    }

    fn check_fields(&self) -> Result<(), String> {
        let kind = self.kind;
        let name = kind.key().replace('_', " ");
        let (least, most) = kind.picks();
        if !(least..=most).contains(&self.picks.len()) {
            return Err(if least == most {
                format!(
                    "a {name} takes {least} pick{}",
                    if least == 1 { "" } else { "s" }
                )
            } else {
                format!("a {name} takes {least} to {most} picks")
            });
        }
        for pick in &self.picks {
            pick.check()?;
        }
        match (kind, self.along) {
            (AnnotationType::Linear, None) => {
                return Err("a linear dimension measures along a direction".to_owned());
            }
            (AnnotationType::Ordinate, None | Some(Along::Aligned)) => {
                return Err("an ordinate dimension measures along the view's x or y".to_owned());
            }
            (AnnotationType::Linear | AnnotationType::Ordinate, Some(_)) | (_, None) => {}
            (_, Some(_)) => return Err(format!("a {name} has no direction to measure along")),
        }
        if let Some(offset) = self.line_offset
            && !offset.is_finite()
        {
            return Err("the line's offset is a number".to_owned());
        }
        if self
            .text_offset
            .is_some_and(|o| o.iter().any(|v| !v.is_finite()))
            || self.at.is_some_and(|o| o.iter().any(|v| !v.is_finite()))
        {
            return Err("a place is two numbers".to_owned());
        }
        if kind.is_dimension() {
            crate::document::check_format(&self.format)?;
            if let Some(tolerance) = &self.tolerance {
                crate::document::check_tolerance(tolerance, kind.measure_kind())?;
            }
        } else if !self.format.is_default() || self.tolerance.is_some() {
            return Err(format!("a {name} has no value's format or tolerance"));
        }
        if self.text.is_some() != (kind == AnnotationType::Note) {
            return Err(if kind == AnnotationType::Note {
                "a note needs a text".to_owned()
            } else {
                format!("a {name} has no text of its own")
            });
        }
        if let Some(text) = &self.text
            && text.trim().is_empty()
        {
            return Err("a note needs a text".to_owned());
        }
        match (&self.letter, kind) {
            (Some(letter), AnnotationType::Datum) => {
                if datum_letter(letter)? != *letter {
                    return Err("a datum's letter is written without spaces".to_owned());
                }
            }
            (None, AnnotationType::Datum) => return Err("a datum needs its letter".to_owned()),
            (Some(_), _) => return Err("only a datum has a letter".to_owned()),
            (None, _) => {}
        }
        match (&self.frame, kind) {
            (Some(frame), AnnotationType::Frame) => crate::document::check_frame(frame)?,
            (None, AnnotationType::Frame) => {
                return Err("a feature control frame needs its tolerance".to_owned());
            }
            (Some(_), _) => {
                return Err("only a feature control frame has a geometric tolerance".to_owned());
            }
            (None, _) => {}
        }
        if self.finish.is_some() && kind != AnnotationType::Finish {
            return Err("only a surface finish symbol has a finish".to_owned());
        }
        match (&self.table, kind.is_table()) {
            (Some(table), true) => table.check(kind == AnnotationType::HoleTable)?,
            (None, true) => return Err(format!("a {name} needs its place on the sheet")),
            (Some(_), false) => return Err(format!("a {name} is no table")),
            (None, false) => {}
        }
        Ok(())
    }
}

/// A body's named edges, round faces and planar faces, in its own
/// coordinates, as the kernel gives them: what picks on views are found
/// among and centre marks are drawn on (planar faces: the cut edges of a
/// section view). Kept by the body's shape version (`ProjectionCache`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BodyGeometry {
    pub edges: Vec<(EdgeName, CurveGeometry)>,
    pub cylinders: Vec<RoundFace>,
    pub planes: Vec<FlatFace>,
}

/// A cylinder's face: its name (and the others the kernel gives it), a
/// point of its axis, the unit axis and its radius.
#[derive(Debug, Clone, PartialEq)]
pub struct RoundFace {
    pub face: FaceName,
    pub names: Vec<FaceName>,
    pub origin: Vec3,
    pub axis: Vec3,
    pub radius: f64,
}

/// A planar face: its name (and its others), a point of it and its unit
/// normal.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatFace {
    pub face: FaceName,
    pub names: Vec<FaceName>,
    pub origin: Vec3,
    pub normal: Vec3,
}

impl BodyGeometry {
    /// The round face of this name, if the body has one.
    pub fn cylinder(&self, face: &FaceName) -> Option<&RoundFace> {
        self.cylinders
            .iter()
            .find(|c| c.face == *face || c.names.contains(face))
    }

    /// The planar face of this name, if the body has one.
    pub fn plane(&self, face: &FaceName) -> Option<&FlatFace> {
        self.planes
            .iter()
            .find(|p| p.face == *face || p.names.contains(face))
    }
}

// --- What a view sees of a pick ---------------------------------------------

/// A pick as its view sees it, in the view's coordinates (model mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seen {
    Point(P2),
    /// A straight edge, or a curve's chord.
    Line(P2, P2),
    /// A circle or an arc: its centre and radius (its true radius: a
    /// circle's diameter across `across` is seen at full length), whether
    /// it is seen end on (as a circle), and that direction (unit). An arc
    /// (`arc`, none for a whole circle) is where it is: seen end on, its
    /// angles in the view, counter-clockwise from the first to the second;
    /// from the side, how far it reaches along `across` from the centre,
    /// the lower and the higher (mitcad#717: arcs were whole circles).
    Circle {
        center: P2,
        radius: f64,
        end_on: bool,
        across: P2,
        arc: Option<(f64, f64)>,
    },
    /// A round face seen from the side: its axis from `a` to `b` and its
    /// radius.
    Axis {
        a: P2,
        b: P2,
        radius: f64,
    },
}

impl Seen {
    /// The point that stands for it: a point, a line's middle, a circle's
    /// centre, the middle of an axis.
    pub fn point(&self) -> P2 {
        match *self {
            Self::Point(p) => p,
            Self::Line(a, b) | Self::Axis { a, b, .. } => mid(a, b),
            Self::Circle { center, .. } => center,
        }
    }

    /// How far a view point is from it, view mm.
    pub fn distance(&self, q: P2) -> f64 {
        match *self {
            Self::Axis { a, b, radius } => {
                let n = mul2(perp(unit2(sub2(b, a))), radius);
                segment_distance(q, add2(a, n), add2(b, n))
                    .min(segment_distance(q, sub2(a, n), sub2(b, n)))
                    .min(segment_distance(q, a, b))
            }
            _ => len2(sub2(q, self.nearest(q))),
        }
    }

    /// The point of it nearest a view point (a leader's arrow).
    pub fn nearest(&self, q: P2) -> P2 {
        match *self {
            Self::Point(p) => p,
            Self::Line(a, b) => segment_foot(q, a, b),
            Self::Circle {
                center,
                radius,
                end_on,
                across,
                arc,
            } => {
                if end_on {
                    let d = sub2(q, center);
                    let angle = d[1].atan2(d[0]);
                    let on = |a: f64| add2(center, mul2([a.cos(), a.sin()], radius));
                    match arc {
                        Some((start, end)) if !in_sweep(angle, start, end) => {
                            // Off the arc: its nearer end.
                            let (s, e) = (on(start), on(end));
                            if len2(sub2(q, s)) <= len2(sub2(q, e)) {
                                s
                            } else {
                                e
                            }
                        }
                        _ => on(angle),
                    }
                } else {
                    let (low, high) = arc.unwrap_or((-radius, radius));
                    segment_foot(
                        q,
                        add2(center, mul2(across, low)),
                        add2(center, mul2(across, high)),
                    )
                }
            }
            Self::Axis { a, b, radius } => {
                let n = perp(unit2(sub2(b, a)));
                let foot = segment_foot(q, a, b);
                let side = if dot2(sub2(q, foot), n) >= 0.0 {
                    1.0
                } else {
                    -1.0
                };
                add2(foot, mul2(n, side * radius))
            }
        }
    }
}

fn mid(a: P2, b: P2) -> P2 {
    mul2(add2(a, b), 0.5)
}

/// Whether an angle lies in the counter-clockwise range from `start` to
/// `end` (radians).
fn in_sweep(angle: f64, start: f64, end: f64) -> bool {
    (angle - start).rem_euclid(TAU) <= end - start + 1e-12
}

/// How far an arc of a circle (`start` to `end` counter-clockwise about
/// `normal`, the same point for a whole circle) turns, radians: a whole
/// circle's 2 pi.
pub fn arc_sweep(center: Vec3, normal: Vec3, radius: f64, start: Vec3, end: Vec3) -> f64 {
    if mitcad_vec3::distance(start, end) < 1e-6 * radius.max(1.0) {
        return TAU;
    }
    let n = mitcad_vec3::unit(normal).unwrap_or([0.0, 0.0, 1.0]);
    let u = mitcad_vec3::sub(start, center);
    let w = mitcad_vec3::sub(end, center);
    let turn = mitcad_vec3::dot(mitcad_vec3::cross(u, w), n).atan2(mitcad_vec3::dot(u, w));
    turn.rem_euclid(TAU)
}

/// Points along an arc of a circle in 3D (as [`arc_sweep`] has it), its
/// ends among them.
pub fn arc_points3(center: Vec3, normal: Vec3, radius: f64, start: Vec3, end: Vec3) -> Vec<Vec3> {
    let sweep = arc_sweep(center, normal, radius, start, end);
    let n = mitcad_vec3::unit(normal).unwrap_or([0.0, 0.0, 1.0]);
    let u = mitcad_vec3::unit(mitcad_vec3::sub(start, center)).unwrap_or([1.0, 0.0, 0.0]);
    let v = mitcad_vec3::cross(n, u);
    (0..=32)
        .map(|i| {
            let a = sweep * f64::from(i) / 32.0;
            let (s, c) = a.sin_cos();
            mitcad_vec3::add(
                center,
                mitcad_vec3::add(
                    mitcad_vec3::scale(u, c * radius),
                    mitcad_vec3::scale(v, s * radius),
                ),
            )
        })
        .collect()
}

/// The unit vector along `a`, or the x axis when `a` has no length.
fn unit2(a: P2) -> P2 {
    vec2::unit(a).unwrap_or([1.0, 0.0])
}

fn segment_foot(q: P2, a: P2, b: P2) -> P2 {
    let ab = sub2(b, a);
    let length = dot2(ab, ab);
    if length < 1e-24 {
        return a;
    }
    let t = (dot2(sub2(q, a), ab) / length).clamp(0.0, 1.0);
    add2(a, mul2(ab, t))
}

/// How far a point is from the segment from `a` to `b`.
pub fn segment_distance(q: P2, a: P2, b: P2) -> f64 {
    len2(sub2(q, segment_foot(q, a, b)))
}

/// How a view sees a curve (in the drawing's component coordinates): the
/// view's projection `project`, the direction to its eye `eye`; a point
/// of it when `point` is given.
pub fn seen_curve(
    curve: &CurveGeometry,
    point: Option<PickPoint>,
    project: impl Fn(Vec3) -> P2,
    eye: Vec3,
) -> Result<Seen, String> {
    let (start, end) = match curve {
        CurveGeometry::Line { start, end } => (*start, *end),
        CurveGeometry::Circle { start, end, .. } => (*start, *end),
        CurveGeometry::Other { start, end, .. } => (*start, *end),
    };
    match point {
        Some(PickPoint::Start) => return Ok(Seen::Point(project(start))),
        Some(PickPoint::End) => return Ok(Seen::Point(project(end))),
        Some(PickPoint::Center) => {
            return match curve {
                CurveGeometry::Circle { center, .. } => Ok(Seen::Point(project(*center))),
                CurveGeometry::Line { start, end } => {
                    Ok(Seen::Point(mid(project(*start), project(*end))))
                }
                CurveGeometry::Other { kind, .. } => {
                    Err(format!("a {kind} edge has no centre to pick"))
                }
            };
        }
        None => {}
    }
    match curve {
        CurveGeometry::Circle {
            center,
            normal,
            radius,
            start,
            end,
        } => {
            let n = mitcad_vec3::unit(*normal).unwrap_or([0.0, 0.0, 1.0]);
            let facing = mitcad_vec3::dot(n, eye);
            let end_on = facing.abs() > 1.0 - 1e-6;
            // The diameter square to both the axis and the eye is seen at
            // full length.
            let across = if end_on {
                [1.0, 0.0]
            } else {
                let d = mitcad_vec3::cross(n, eye);
                let o = project([0.0; 3]);
                unit2(sub2(project(d), o))
            };
            let c = project(*center);
            let sweep = arc_sweep(*center, n, *radius, *start, *end);
            let arc = if sweep >= TAU - 1e-9 {
                None
            } else if end_on {
                // Counter-clockwise in the view when its normal faces the
                // eye, else from its end to its start.
                let angle = |p: Vec3| {
                    let d = sub2(project(p), c);
                    d[1].atan2(d[0])
                };
                let from = if facing > 0.0 {
                    angle(*start)
                } else {
                    angle(*end)
                };
                Some((from, from + sweep))
            } else {
                let reach: Vec<f64> = arc_points3(*center, n, *radius, *start, *end)
                    .into_iter()
                    .map(|p| dot2(sub2(project(p), c), across))
                    .collect();
                Some((
                    reach.iter().copied().fold(f64::INFINITY, f64::min),
                    reach.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                ))
            };
            Ok(Seen::Circle {
                center: c,
                radius: *radius,
                end_on,
                across,
                arc,
            })
        }
        _ => {
            let (a, b) = (project(start), project(end));
            if len2(sub2(b, a)) < 1e-9 {
                Ok(Seen::Point(a))
            } else {
                Ok(Seen::Line(a, b))
            }
        }
    }
}

// --- Words ---------------------------------------------------------------

/// A dimension's value in words as #702's engine writes it: the main text
/// (own text before, prefix, value, a ± tolerance or a fit code), a stack
/// of `upper` over `lower`, and text `after`; `text` all on one line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ValueText {
    pub text: String,
    pub main: String,
    pub upper: Option<String>,
    pub lower: Option<String>,
    pub brackets: bool,
    pub after: String,
}

/// The size of a stacked tolerance's text against the value's.
const STACK: f64 = 0.7;

impl ValueText {
    fn parts(&self, h: f64) -> Vec<(f64, f64, String, f64)> {
        // (along, up, text, height) from the start of the baseline.
        let mut out = Vec::new();
        let mut x = 0.0;
        if !self.main.is_empty() {
            out.push((x, 0.0, self.main.clone(), h));
            x += text_width(&self.main, h) + 0.6;
        }
        if let (Some(upper), Some(lower)) = (&self.upper, &self.lower) {
            let small = h * STACK;
            let (open, close) = if self.brackets { ("(", ")") } else { ("", "") };
            if !open.is_empty() {
                out.push((x, 0.0, open.to_owned(), h));
                x += text_width(open, h);
            }
            out.push((x, h - small, upper.clone(), small));
            out.push((x, -0.25 * small, lower.clone(), small));
            x += text_width(upper, small).max(text_width(lower, small)) + 0.3;
            if !close.is_empty() {
                out.push((x, 0.0, close.to_owned(), h));
                x += text_width(close, h);
            }
            x += 0.3;
        }
        if !self.after.is_empty() {
            out.push((x, 0.0, self.after.clone(), h));
        }
        out
    }

    /// Its width at this height, mm.
    pub fn width(&self, h: f64) -> f64 {
        self.parts(h)
            .iter()
            .map(|(x, _, t, size)| x + text_width(t, *size))
            .fold(0.0, f64::max)
    }

    /// Writes it on a baseline: `at` its start (`Anchor::Start`), its
    /// middle or its end, along `angle`.
    fn write(&self, group: &mut Group, at: P2, angle: f64, anchor: Anchor) {
        let h = DIMENSION_TEXT;
        let along = [angle.cos(), angle.sin()];
        let up = [-angle.sin(), angle.cos()];
        let shift = match anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => self.width(h) / 2.0,
            Anchor::End => self.width(h),
        };
        let start = sub2(at, mul2(along, shift));
        for (x, y, text, size) in self.parts(h) {
            group.text(
                add2(add2(start, mul2(along, x)), mul2(up, y)),
                size,
                angle,
                Anchor::Start,
                text,
            );
        }
    }
}

// --- On the sheet ----------------------------------------------------------

/// What lays an annotation out: its view's map onto the sheet and the
/// view's box on the sheet.
#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    pub map: &'a ViewMap,
    pub view_box: Bounds,
}

impl Frame<'_> {
    fn at(&self, p: P2) -> P2 {
        self.map.at(p)
    }
}

/// An annotation laid out: its group and its box on the sheet.
#[derive(Debug, Clone)]
pub struct Laid {
    pub group: Group,
    pub bounds: Bounds,
}

/// The value a dimension measures in its view: a length (view mm, the
/// model's) or an angle (radians); None for what is not a dimension.
pub fn measure(annotation: &ViewAnnotation, seen: &[Seen]) -> Result<Option<f64>, String> {
    match annotation.kind {
        AnnotationType::Linear => {
            let (_, d) = linear_axes(annotation.along, seen)?;
            Ok(Some(dot2(sub2(seen[1].point(), seen[0].point()), d).abs()))
        }
        AnnotationType::Ordinate => {
            let d = match annotation.along {
                Some(Along::Vertical) => [0.0, 1.0],
                _ => [1.0, 0.0],
            };
            Ok(Some(dot2(sub2(seen[1].point(), seen[0].point()), d)))
        }
        AnnotationType::Angle => {
            let (_, r1, r2) = corner(seen)?;
            Ok(Some(dot2(r1, r2).clamp(-1.0, 1.0).acos()))
        }
        AnnotationType::Radius | AnnotationType::Diameter => {
            let r = round_radius(&seen[0])?;
            Ok(Some(if annotation.kind == AnnotationType::Diameter {
                2.0 * r
            } else {
                r
            }))
        }
        _ => Ok(None),
    }
}

fn round_radius(seen: &Seen) -> Result<f64, String> {
    match *seen {
        Seen::Circle { radius, .. } | Seen::Axis { radius, .. } => Ok(radius),
        _ => Err("the pick is not a circle or a round face".to_owned()),
    }
}

/// The direction a linear dimension of these picks measures along when
/// none is given: the view's x or y, whichever its picks lie further apart
/// along; a straight edge with a point or a parallel edge, square to the
/// edge when aligned.
pub fn default_along(seen: &[Seen]) -> Along {
    let d = sub2(seen[1].point(), seen[0].point());
    if d[0].abs() >= d[1].abs() {
        Along::Horizontal
    } else {
        Along::Vertical
    }
}

/// The direction (unit, view) a linear dimension measures along, and
/// whether its picks are parallel lines.
fn linear_axes(along: Option<Along>, seen: &[Seen]) -> Result<(bool, P2), String> {
    let line_of = |s: &Seen| match *s {
        Seen::Line(a, b) => Some(unit2(sub2(b, a))),
        _ => None,
    };
    match along {
        Some(Along::Horizontal) | None => Ok((false, [1.0, 0.0])),
        Some(Along::Vertical) => Ok((false, [0.0, 1.0])),
        Some(Along::Aligned) => {
            let lines: Vec<P2> = seen.iter().filter_map(line_of).collect();
            if let Some(first) = lines.first() {
                // Square to the edge: the distance from it.
                return Ok((lines.len() == 2, perp(*first)));
            }
            vec2::unit(sub2(seen[1].point(), seen[0].point()))
                .map(|d| (false, d))
                .ok_or_else(|| "the picks lie on one point".to_owned())
        }
    }
}

/// The corner of two straight edges and the unit rays from it to their
/// far ends.
fn corner(seen: &[Seen]) -> Result<(P2, P2, P2), String> {
    let (Seen::Line(a1, b1), Seen::Line(a2, b2)) = (seen[0], seen[1]) else {
        return Err("an angle is between two straight edges".to_owned());
    };
    let d1 = sub2(b1, a1);
    let d2 = sub2(b2, a2);
    let det = vec2::cross(d1, d2);
    if det.abs() < 1e-9 * len2(d1).max(1e-9) * len2(d2).max(1e-9) {
        return Err("the edges are parallel: they make no angle".to_owned());
    }
    let t = vec2::cross(sub2(a2, a1), d2) / det;
    let c = add2(a1, mul2(d1, t));
    let far = |a: P2, b: P2| {
        if len2(sub2(a, c)) > len2(sub2(b, c)) {
            a
        } else {
            b
        }
    };
    let r1 = unit2(sub2(far(a1, b1), c));
    let r2 = unit2(sub2(far(a2, b2), c));
    Ok((c, r1, r2))
}

/// Where a dimension's text goes with its value: what the caller worked
/// out with #702's engine.
pub fn lay_out(
    annotation: &ViewAnnotation,
    seen: &[Seen],
    frame: &Frame<'_>,
    value: Option<&ValueText>,
    frame_cells: &[String],
) -> Result<Laid, String> {
    let mut g = Group::new(match annotation.kind {
        AnnotationType::CenterMark | AnnotationType::Centerline => "center-mark",
        AnnotationType::Note => "note",
        AnnotationType::Datum => "datum",
        AnnotationType::Frame => "feature-control-frame",
        AnnotationType::Finish => "surface-finish",
        AnnotationType::Balloon => "balloon",
        AnnotationType::HoleTable => "hole-table",
        AnnotationType::PartsList => "parts-list",
        _ => "dimension",
    });
    g.id = Some(format!("A{}", annotation.id));
    let words = || value.cloned().unwrap_or_default();
    match annotation.kind {
        AnnotationType::Linear => {
            let (_, d) = linear_axes(annotation.along, seen)?;
            linear(&mut g, annotation, seen, d, frame, &words());
        }
        AnnotationType::Ordinate => ordinate(&mut g, annotation, seen, frame, &words()),
        AnnotationType::Angle => angle(&mut g, annotation, seen, frame, &words())?,
        AnnotationType::Radius | AnnotationType::Diameter => {
            round(&mut g, annotation, &seen[0], frame, &words())?
        }
        AnnotationType::CenterMark => match seen[0] {
            Seen::Circle {
                center,
                radius,
                end_on: true,
                ..
            } => center_mark(&mut g, frame.at(center), radius * frame.map.scale),
            Seen::Axis { a, b, .. } => centerline(&mut g, frame.at(a), frame.at(b), 0.0, 0.0),
            _ => {
                return Err(
                    "a centre mark is of a circle seen end on, or of a round face seen from \
                     the side"
                        .to_owned(),
                );
            }
        },
        AnnotationType::Centerline => centerline_of(&mut g, seen, frame)?,
        AnnotationType::Note => {
            let tip = leader_tip(annotation, &seen[0], frame);
            let shift = annotation.text_offset.unwrap_or(NOTE_SHIFT);
            let lines: Vec<String> = annotation
                .text
                .as_deref()
                .unwrap_or("")
                .lines()
                .map(|l| l.trim_end().to_owned())
                .collect();
            leader_text(&mut g, tip, add2(tip, shift), &lines);
        }
        AnnotationType::Datum => {
            let tip = leader_tip(annotation, &seen[0], frame);
            let shift = annotation.text_offset.unwrap_or(DATUM_SHIFT);
            datum(
                &mut g,
                tip,
                add2(tip, shift),
                annotation.letter.as_deref().unwrap_or(""),
            );
        }
        AnnotationType::Frame => {
            let tip = leader_tip(annotation, &seen[0], frame);
            let shift = annotation.text_offset.unwrap_or(NOTE_SHIFT);
            let characteristic = annotation
                .frame
                .as_ref()
                .map_or(Characteristic::Flatness, |f| f.characteristic);
            control_frame(&mut g, tip, add2(tip, shift), characteristic, frame_cells);
        }
        AnnotationType::Finish => {
            let tip = leader_tip(annotation, &seen[0], frame);
            let shift = annotation.text_offset.unwrap_or([0.0, 0.0]);
            let finish = annotation.finish.clone().unwrap_or_default();
            surface_finish(&mut g, tip, add2(tip, shift), &finish);
        }
        AnnotationType::Balloon => {
            // Its number in `value` (its row's in the parts list).
            let tip = leader_tip(annotation, &seen[0], frame);
            let shift = annotation.text_offset.unwrap_or(BALLOON_SHIFT);
            balloon(&mut g, tip, add2(tip, shift), &words().main);
        }
        AnnotationType::HoleTable | AnnotationType::PartsList => {
            return Err("a table is laid out with its rows (tables::lay_out_table)".to_owned());
        }
    }
    let bounds = group_bounds(&g);
    Ok(Laid { group: g, bounds })
}

/// A balloon: a circle with the number at `at` and a leader from its rim
/// to an arrow at `tip`.
fn balloon(g: &mut Group, tip: P2, at: P2, number: &str) {
    let r = BALLOON_RADIUS.max(text_width(number, DIMENSION_TEXT) / 2.0 + 1.2);
    g.items.push(Item::Arc {
        pen: Pen::Thin,
        center: at,
        radius: r,
        start: 0.0,
        end: TAU,
    });
    g.text(
        [at[0], at[1] - DIMENSION_TEXT / 2.0],
        DIMENSION_TEXT,
        0.0,
        Anchor::Middle,
        number,
    );
    let towards = sub2(tip, at);
    if len2(towards) > r + 0.5 {
        let from = add2(at, mul2(unit2(towards), r));
        g.line(Pen::Thin, from, tip);
        arrow(g, tip, unit2(towards));
    }
}

/// A text note on the sheet.
pub fn lay_out_note(note: &SheetNote) -> Laid {
    let mut g = Group::new("note");
    g.id = Some(format!("A{}", note.id));
    let h = note.height.unwrap_or(DIMENSION_TEXT);
    for (i, line) in note.text.lines().enumerate() {
        g.text(
            [note.at[0], note.at[1] - i as f64 * h * 1.6],
            h,
            0.0,
            Anchor::Start,
            line.trim_end(),
        );
    }
    let bounds = group_bounds(&g);
    Laid { group: g, bounds }
}

/// Where the dimension line of a linear dimension lies: across it from the
/// middle of its picks (sheet), paper mm. When placed for itself, beyond
/// the view's box on the side of the picks.
fn linear_offset(annotation: &ViewAnnotation, middle: P2, n: P2, view_box: &Bounds) -> f64 {
    if let Some(offset) = annotation.line_offset {
        return offset;
    }
    if view_box.is_empty() {
        return LINE_GAP;
    }
    let corners = [
        view_box.min,
        [view_box.max[0], view_box.min[1]],
        view_box.max,
        [view_box.min[0], view_box.max[1]],
    ];
    let across: Vec<f64> = corners.iter().map(|c| dot2(sub2(*c, middle), n)).collect();
    let high = across.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let low = across.iter().copied().fold(f64::INFINITY, f64::min);
    // On the side of the box the picks are nearer to.
    if dot2(sub2(view_box.center(), middle), n) <= 0.0 {
        high + LINE_GAP
    } else {
        low - LINE_GAP
    }
}

/// Where a linear dimension's line lies on the sheet: its distance across
/// the line (along the line's normal) from the sheet's origin, and its
/// offset from the middle of its picks, as it is drawn. None when its
/// picks give it no direction.
pub fn line_position(
    annotation: &ViewAnnotation,
    seen: &[Seen],
    frame: &Frame<'_>,
) -> Option<(f64, f64)> {
    let (_, d) = linear_axes(annotation.along, seen).ok()?;
    let n = perp(d);
    let middle = mid(frame.at(seen[0].point()), frame.at(seen[1].point()));
    let c = linear_offset(annotation, middle, n, &frame.view_box);
    Some((dot2(middle, n) + c, c))
}

/// The offset from the middle of its picks that puts a linear
/// dimension's line at `position` across the sheet (as
/// [`line_position`] gives it): the dimensions of a series in a row or
/// stacked, whatever their picks.
pub fn offset_for(
    annotation: &ViewAnnotation,
    seen: &[Seen],
    frame: &Frame<'_>,
    position: f64,
) -> Option<f64> {
    let (_, d) = linear_axes(annotation.along, seen).ok()?;
    let n = perp(d);
    let middle = mid(frame.at(seen[0].point()), frame.at(seen[1].point()));
    Some(position - dot2(middle, n))
}

/// The point of a pick an extension line starts from: a straight edge's
/// end nearer the dimension line (sheet), else the pick's point.
fn anchor_of(seen: &Seen, frame: &Frame<'_>, toward: impl Fn(P2) -> f64) -> P2 {
    match *seen {
        Seen::Line(a, b) => {
            let (sa, sb) = (frame.at(a), frame.at(b));
            if toward(sa) >= toward(sb) { sa } else { sb }
        }
        _ => frame.at(seen.point()),
    }
}

/// A linear dimension between two picks along `d` (unit, the view's and
/// the sheet's axes agree).
fn linear(
    g: &mut Group,
    annotation: &ViewAnnotation,
    seen: &[Seen],
    d: P2,
    frame: &Frame<'_>,
    words: &ValueText,
) {
    let n = perp(d);
    let (pa, pb) = (frame.at(seen[0].point()), frame.at(seen[1].point()));
    let middle = mid(pa, pb);
    let c = linear_offset(annotation, middle, n, &frame.view_box);
    let side = if c >= 0.0 { 1.0 } else { -1.0 };
    let toward = |p: P2| side * dot2(sub2(p, middle), n);
    let a = anchor_of(&seen[0], frame, toward);
    let b = anchor_of(&seen[1], frame, toward);
    let foot = |p: P2| add2(p, mul2(n, c - dot2(sub2(p, middle), n)));
    dimension_line(g, a, b, foot(a), foot(b), n, annotation, words);
}

/// Extension lines from `a` and `b` to the feet `fa` and `fb` on the
/// dimension line, the line with its arrows and the text over its middle
/// (moved along it by the text offset; the line runs on under a text
/// moved past an end).
#[allow(clippy::too_many_arguments)]
fn dimension_line(
    g: &mut Group,
    a: P2,
    b: P2,
    fa: P2,
    fb: P2,
    n: P2,
    annotation: &ViewAnnotation,
    words: &ValueText,
) {
    for (from, foot) in [(a, fa), (b, fb)] {
        let reach = len2(sub2(foot, from));
        if reach > EXTENSION_GAP {
            let out = unit2(sub2(foot, from));
            g.line(
                Pen::Thin,
                add2(from, mul2(out, EXTENSION_GAP)),
                add2(foot, mul2(out, EXTENSION_BEYOND)),
            );
        }
    }
    let length = len2(sub2(fb, fa));
    let towards = if length > 1e-9 {
        unit2(sub2(fb, fa))
    } else {
        perp(n).map(|v| -v)
    };
    g.line(Pen::Thin, fa, fb);
    if length >= 2.0 * ARROW_LENGTH + 1.0 {
        arrow(g, fa, mul2(towards, -1.0));
        arrow(g, fb, towards);
    } else {
        // Too short: the arrows outside, pointing in.
        g.line(Pen::Thin, fa, sub2(fa, mul2(towards, ARROW_LENGTH + 2.0)));
        g.line(Pen::Thin, fb, add2(fb, mul2(towards, ARROW_LENGTH + 2.0)));
        arrow(g, fa, towards);
        arrow(g, fb, mul2(towards, -1.0));
    }
    // The text moves along the way it reads (as a drag and a place keep
    // it), whichever way the picks run.
    let [u, v] = annotation.text_offset.unwrap_or([0.0, 0.0]);
    let along = upright_along(towards);
    let middle = add2(mid(fa, fb), mul2(along, u));
    if u.abs() > length / 2.0 {
        let fb_ahead = dot2(sub2(fb, fa), along) >= 0.0;
        let end = if fb_ahead == (u > 0.0) { fb } else { fa };
        g.line(Pen::Thin, end, middle);
    }
    let angle = upright(towards);
    let up = [-angle.sin(), angle.cos()];
    let at = add2(middle, mul2(up, 1.0 + v));
    words.write(g, at, angle, Anchor::Middle);
}

/// An ordinate dimension: a thin line from the point across to the level
/// of its origin's dimension line, the value along it there.
fn ordinate(
    g: &mut Group,
    annotation: &ViewAnnotation,
    seen: &[Seen],
    frame: &Frame<'_>,
    words: &ValueText,
) {
    // Measured along d, the leader runs along n.
    let n = match annotation.along {
        Some(Along::Vertical) => [1.0, 0.0],
        _ => [0.0, 1.0],
    };
    let origin = frame.at(seen[0].point());
    let p = frame.at(seen[1].point());
    let level = match annotation.line_offset {
        Some(offset) => offset,
        None => {
            let b = &frame.view_box;
            if b.is_empty() {
                LINE_GAP
            } else {
                let top = [b.max[0], b.max[1]];
                dot2(sub2(top, origin), n) + LINE_GAP
            }
        }
    };
    let across = level - dot2(sub2(p, origin), n);
    let end = add2(p, mul2(n, across));
    let out = if across >= 0.0 { n } else { mul2(n, -1.0) };
    if across.abs() > EXTENSION_GAP {
        g.line(Pen::Thin, add2(p, mul2(out, EXTENSION_GAP)), end);
    }
    let shift = annotation.text_offset.unwrap_or([0.0, 0.0]);
    let at = add2(add2(end, mul2(out, 1.0)), mul2(perp(n), -shift[0]));
    let angle = upright(out);
    let anchor = if dot2([angle.cos(), angle.sin()], out) > 0.0 {
        Anchor::Start
    } else {
        Anchor::End
    };
    // The text's baseline beside the line, its middle on it.
    let up = [-angle.sin(), angle.cos()];
    words.write(g, sub2(at, mul2(up, DIMENSION_TEXT / 2.0)), angle, anchor);
}

/// How far a straight edge's ends are from a corner on the sheet: the
/// nearer and the farther.
fn arm(seen: &Seen, center: P2, frame: &Frame<'_>) -> (f64, f64) {
    match *seen {
        Seen::Line(a, b) => {
            let (da, db) = (
                len2(sub2(frame.at(a), center)),
                len2(sub2(frame.at(b), center)),
            );
            (da.min(db), da.max(db))
        }
        _ => (0.0, 0.0),
    }
}

/// An angle's arc radius on the sheet: its own, else three fifths of the
/// shorter edge's reach from the corner, 8 mm at least.
fn arc_radius(annotation: &ViewAnnotation, seen: &[Seen], frame: &Frame<'_>) -> f64 {
    if let Some(radius) = annotation.line_offset {
        return radius;
    }
    let Ok((c, _, _)) = corner(seen) else {
        return 8.0;
    };
    let center = frame.at(c);
    let (_, f1) = arm(&seen[0], center, frame);
    let (_, f2) = arm(&seen[1], center, frame);
    (0.6 * f1.min(f2)).max(8.0)
}

/// An angle between two straight edges: an arc about their corner with an
/// arrow at each end, extension lines along the edges to it where it lies
/// beyond them, the value on the arc's middle.
fn angle(
    g: &mut Group,
    annotation: &ViewAnnotation,
    seen: &[Seen],
    frame: &Frame<'_>,
    words: &ValueText,
) -> Result<(), String> {
    let (c, r1, r2) = corner(seen)?;
    let center = frame.at(c);
    let (n1, f1) = arm(&seen[0], center, frame);
    let (n2, f2) = arm(&seen[1], center, frame);
    let radius = arc_radius(annotation, seen, frame);
    let (t1, t2) = (r1[1].atan2(r1[0]), r2[1].atan2(r2[0]));
    let sweep = dot2(r1, r2).clamp(-1.0, 1.0).acos();
    let (start, from, to) = if vec2::cross(r1, r2) >= 0.0 {
        (t1, r1, r2)
    } else {
        (t2, r2, r1)
    };
    g.items.push(Item::Arc {
        pen: Pen::Thin,
        center,
        radius,
        start,
        end: start + sweep,
    });
    // The rays' extension lines where the arc lies beyond an edge, or
    // short of it.
    for (ray, near, far) in [(r1, n1, f1), (r2, n2, f2)] {
        if radius > far + EXTENSION_GAP {
            g.line(
                Pen::Thin,
                add2(center, mul2(ray, far + EXTENSION_GAP)),
                add2(center, mul2(ray, radius + EXTENSION_BEYOND)),
            );
        } else if radius < near - EXTENSION_GAP {
            g.line(
                Pen::Thin,
                add2(center, mul2(ray, radius - EXTENSION_BEYOND)),
                add2(center, mul2(ray, near - EXTENSION_GAP)),
            );
        }
    }
    let tangent = |r: P2| perp(r);
    arrow(
        g,
        add2(center, mul2(from, radius)),
        mul2(tangent(from), -1.0),
    );
    arrow(g, add2(center, mul2(to, radius)), tangent(to));
    let half = start + sweep / 2.0;
    let middle = [half.cos(), half.sin()];
    let [u, v] = annotation.text_offset.unwrap_or([0.0, 0.0]);
    let along = tangent(middle);
    let at = add2(add2(center, mul2(middle, radius + 1.0 + v)), mul2(along, u));
    let angle = upright(along);
    words.write(g, at, angle, Anchor::Middle);
    Ok(())
}

/// A radius or a diameter: of a circle seen end on, a line from its
/// centre (across it for a diameter) to the rim with an arrow, run on to
/// the text on a shoulder outside; of a circle or round face seen from the
/// side, a linear dimension across it.
fn round(
    g: &mut Group,
    annotation: &ViewAnnotation,
    seen: &Seen,
    frame: &Frame<'_>,
    words: &ValueText,
) -> Result<(), String> {
    let diameter = annotation.kind == AnnotationType::Diameter;
    let scale = frame.map.scale;
    let (center, radius, across, axis) = match *seen {
        Seen::Circle {
            center,
            radius,
            end_on: true,
            arc,
            ..
        } => {
            let c = frame.at(center);
            let r = radius * scale;
            let shift = annotation.text_offset.unwrap_or_else(|| {
                let out = r + 10.0;
                match arc {
                    // Out from the middle of an arc.
                    Some((start, end)) => {
                        let half = (start + end) / 2.0;
                        [out * half.cos(), out * half.sin()]
                    }
                    None => [out * FRAC_1_SQRT_2, out * FRAC_1_SQRT_2],
                }
            });
            let text_at = add2(c, shift);
            let mut u = vec2::unit(shift).unwrap_or([1.0, 0.0]);
            // The arrow on the arc where it is: its nearer end when the
            // text lies beyond it.
            if let Some((start, end)) = arc
                && !in_sweep(u[1].atan2(u[0]), start, end)
            {
                let angle = u[1].atan2(u[0]);
                let off = |a: f64| {
                    let d = (angle - a).rem_euclid(TAU);
                    d.min(TAU - d)
                };
                let a = if off(start) <= off(end) { start } else { end };
                u = [a.cos(), a.sin()];
            }
            let rim = add2(c, mul2(u, r));
            if diameter {
                let opposite = sub2(c, mul2(u, r));
                g.line(Pen::Thin, opposite, rim);
                arrow(g, opposite, mul2(u, -1.0));
            } else {
                g.line(Pen::Thin, c, rim);
            }
            arrow(g, rim, u);
            let side = if shift[0] >= 0.0 { 1.0 } else { -1.0 };
            let width = words.width(DIMENSION_TEXT) + 1.0;
            if len2(shift) > r {
                g.line(Pen::Thin, rim, text_at);
                g.line(Pen::Thin, text_at, add2(text_at, [side * width, 0.0]));
            }
            let (x, anchor) = if side > 0.0 {
                (text_at[0] + 0.5, Anchor::Start)
            } else {
                (text_at[0] - 0.5, Anchor::End)
            };
            words.write(g, [x, text_at[1] + 1.0], 0.0, anchor);
            return Ok(());
        }
        Seen::Circle {
            center,
            radius,
            across,
            ..
        } => (center, radius, across, None),
        Seen::Axis { a, b, radius } => (mid(a, b), radius, perp(unit2(sub2(b, a))), Some((a, b))),
        _ => return Err("the pick is not a circle or a round face".to_owned()),
    };
    // Seen from the side: across the circle or face, along `across`.
    let d = across;
    let n = perp(d);
    let from = if diameter {
        sub2(center, mul2(d, radius))
    } else {
        center
    };
    let to = add2(center, mul2(d, radius));
    let (a, b) = (frame.at(from), frame.at(to));
    let middle = mid(a, b);
    let c = match annotation.line_offset {
        Some(offset) => offset,
        None => match axis {
            // Beyond the face's end on the side its axis runs to.
            Some((p, q)) => {
                let (sp, sq) = (frame.at(p), frame.at(q));
                let reach = dot2(sub2(sp, middle), n).max(dot2(sub2(sq, middle), n));
                reach + LINE_GAP
            }
            None => linear_offset(annotation, middle, n, &frame.view_box),
        },
    };
    let foot = |p: P2| add2(p, mul2(n, c - dot2(sub2(p, middle), n)));
    // The extension lines start at the face's end nearer the line.
    let start = |p: P2| match axis {
        Some((s, e)) => {
            let (ss, se) = (frame.at(s), frame.at(e));
            let end =
                if (dot2(sub2(ss, middle), n) - c).abs() <= (dot2(sub2(se, middle), n) - c).abs() {
                    ss
                } else {
                    se
                };
            add2(
                p,
                mul2(n, dot2(sub2(end, middle), n) - dot2(sub2(p, middle), n)),
            )
        }
        None => p,
    };
    let (sa, sb) = (start(a), start(b));
    dimension_line(g, sa, sb, foot(a), foot(b), n, annotation, words);
    Ok(())
}

/// Crossed centre lines through a circle's centre (sheet), reaching
/// beyond its rim: long-dash dotted for a larger circle, thin for a small
/// one, each from the centre out so that they cross on a dash.
pub fn center_mark(g: &mut Group, center: P2, radius: f64) {
    let pen = if radius >= 4.0 { Pen::Chain } else { Pen::Thin };
    let reach = radius + CENTER_BEYOND;
    for d in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
        g.line(pen, center, add2(center, mul2(d, reach)));
    }
}

/// A centreline from `a` to `b` (sheet), run on beyond each end by the
/// circle's radius there (`ra`, `rb`) and a little more.
pub fn centerline(g: &mut Group, a: P2, b: P2, ra: f64, rb: f64) {
    let d = unit2(sub2(b, a));
    let from = sub2(a, mul2(d, ra + CENTER_BEYOND));
    let to = add2(b, mul2(d, rb + CENTER_BEYOND));
    // From the middle out: the lines cross the middle on a dash.
    let middle = mid(from, to);
    g.line(Pen::Chain, middle, from);
    g.line(Pen::Chain, middle, to);
}

fn centerline_of(g: &mut Group, seen: &[Seen], frame: &Frame<'_>) -> Result<(), String> {
    let scale = frame.map.scale;
    let rim = |s: &Seen| match *s {
        Seen::Circle {
            radius,
            end_on: true,
            ..
        } => radius * scale,
        _ => 0.0,
    };
    match seen {
        [Seen::Axis { a, b, .. }] => {
            centerline(g, frame.at(*a), frame.at(*b), 0.0, 0.0);
            Ok(())
        }
        [one] => Err(format!(
            "a centreline of one pick is a round face's axis, not a {}",
            match one {
                Seen::Point(_) => "point",
                Seen::Line(..) => "straight edge",
                _ => "circle",
            }
        )),
        [Seen::Line(a1, b1), Seen::Line(a2, b2)] => {
            // Halfway between two edges, over both.
            let d = unit2(sub2(*b1, *a1));
            let d2 = unit2(sub2(*b2, *a2));
            if vec2::cross(d, d2).abs() > 1e-6 {
                return Err("a centreline between edges needs parallel edges".to_owned());
            }
            let o = mid(mid(*a1, *b1), mid(*a2, *b2));
            let ts: Vec<f64> = [a1, b1, a2, b2]
                .iter()
                .map(|p| dot2(sub2(**p, o), d))
                .collect();
            let low = ts.iter().copied().fold(f64::INFINITY, f64::min);
            let high = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            centerline(
                g,
                frame.at(add2(o, mul2(d, low))),
                frame.at(add2(o, mul2(d, high))),
                0.0,
                0.0,
            );
            Ok(())
        }
        [p, q] => {
            let (a, b) = (frame.at(p.point()), frame.at(q.point()));
            if len2(sub2(b, a)) < 1e-9 {
                return Err("the picks share their centre".to_owned());
            }
            centerline(g, a, b, rim(p), rim(q));
            Ok(())
        }
        _ => Err("a centreline takes one or two picks".to_owned()),
    }
}

/// Where a leader's arrow points (sheet): the pick's point nearest where
/// it was clicked (its point, where it is one).
fn leader_tip(annotation: &ViewAnnotation, seen: &Seen, frame: &Frame<'_>) -> P2 {
    let near = annotation.at.unwrap_or_else(|| seen.point());
    frame.at(seen.nearest(near))
}

/// A leader from `tip` (its arrow there) to text lines on a shoulder at
/// `knee`, the shoulder and the text running away from the tip.
fn leader_text(g: &mut Group, tip: P2, knee: P2, lines: &[String]) {
    let side = if knee[0] >= tip[0] { 1.0 } else { -1.0 };
    let width = lines
        .iter()
        .map(|l| text_width(l, DIMENSION_TEXT))
        .fold(0.0, f64::max);
    g.line(Pen::Thin, tip, knee);
    g.line(Pen::Thin, knee, add2(knee, [side * (width + 2.0), 0.0]));
    arrow(g, tip, unit2(sub2(tip, knee)));
    let (x, anchor) = if side > 0.0 {
        (knee[0] + 1.0, Anchor::Start)
    } else {
        (knee[0] - 1.0, Anchor::End)
    };
    for (i, line) in lines.iter().rev().enumerate() {
        let y = knee[1] + 1.0 + i as f64 * DIMENSION_TEXT * 1.6;
        g.text([x, y], DIMENSION_TEXT, 0.0, anchor, line.clone());
    }
}

/// A datum label: a filled triangle on the feature at `tip`, a leader to
/// a square at `at` (its lower middle) with the letter.
fn datum(g: &mut Group, tip: P2, at: P2, letter: &str) {
    let v = unit2(sub2(at, tip));
    let base = mul2(perp(v), 1.6);
    g.items.push(Item::Fill {
        points: vec![add2(tip, base), sub2(tip, base), add2(tip, mul2(v, 2.8))],
    });
    g.line(Pen::Thin, tip, at);
    let half = CELL / 2.0;
    g.rect(
        Pen::Thin,
        [at[0] - half, at[1]],
        [at[0] + half, at[1] + CELL],
    );
    g.text(
        [at[0], at[1] + half - DIMENSION_TEXT / 2.0],
        DIMENSION_TEXT,
        0.0,
        Anchor::Middle,
        letter,
    );
}

/// A feature control frame at `at` (its left middle), a leader from it to
/// an arrow at `tip`: the characteristic's symbol drawn, then the other
/// cells (`cells`: the symbol's text first, which is not written; the
/// tolerance; the datums).
fn control_frame(g: &mut Group, tip: P2, at: P2, characteristic: Characteristic, cells: &[String]) {
    let widths: Vec<f64> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                CELL
            } else {
                (text_width(c, DIMENSION_TEXT) + 3.0).max(CELL)
            }
        })
        .collect();
    let total: f64 = widths.iter().sum::<f64>().max(CELL);
    let bottom = at[1] - CELL / 2.0;
    g.rect(Pen::Thin, [at[0], bottom], [at[0] + total, bottom + CELL]);
    let mut x = at[0];
    for (i, w) in widths.iter().enumerate() {
        if i > 0 {
            g.line(Pen::Thin, [x, bottom], [x, bottom + CELL]);
            g.text(
                [x + w / 2.0, at[1] - DIMENSION_TEXT / 2.0],
                DIMENSION_TEXT,
                0.0,
                Anchor::Middle,
                cells[i].clone(),
            );
        } else {
            symbol(g, characteristic, [x + w / 2.0, at[1]], CELL * 0.6);
        }
        x += w;
    }
    // The leader from the frame's nearer end.
    let from = if tip[0] <= at[0] + total / 2.0 {
        at
    } else {
        [at[0] + total, at[1]]
    };
    g.line(Pen::Thin, from, tip);
    arrow(g, tip, unit2(sub2(tip, from)));
}

/// A geometric characteristic's symbol (ISO 1101) drawn about `c`, `s`
/// mm across: as lines, so that every writer shows it.
fn symbol(g: &mut Group, characteristic: Characteristic, c: P2, s: f64) {
    let h = s / 2.0;
    let p = |x: f64, y: f64| [c[0] + x * h, c[1] + y * h];
    let circle = |g: &mut Group, r: f64| {
        g.items.push(Item::Arc {
            pen: Pen::Thin,
            center: c,
            radius: r * h,
            start: 0.0,
            end: TAU,
        });
    };
    match characteristic {
        Characteristic::Straightness => g.line(Pen::Thin, p(-1.0, 0.0), p(1.0, 0.0)),
        Characteristic::Flatness => g.items.push(Item::Polyline {
            pen: Pen::Thin,
            points: vec![p(-1.0, -0.5), p(0.6, -0.5), p(1.0, 0.5), p(-0.6, 0.5)],
            closed: true,
        }),
        Characteristic::Circularity => circle(g, 0.8),
        Characteristic::Cylindricity => {
            circle(g, 0.6);
            g.line(Pen::Thin, p(-0.9, -0.9), p(-0.2, 0.9));
            g.line(Pen::Thin, p(0.2, -0.9), p(0.9, 0.9));
        }
        Characteristic::LineProfile => g.items.push(Item::Arc {
            pen: Pen::Thin,
            center: p(0.0, -0.4),
            radius: h,
            start: 0.0,
            end: PI,
        }),
        Characteristic::SurfaceProfile => {
            g.items.push(Item::Arc {
                pen: Pen::Thin,
                center: p(0.0, -0.4),
                radius: h,
                start: 0.0,
                end: PI,
            });
            g.line(Pen::Thin, p(-1.0, -0.4), p(1.0, -0.4));
        }
        Characteristic::Parallelism => {
            g.line(Pen::Thin, p(-0.8, -0.9), p(-0.1, 0.9));
            g.line(Pen::Thin, p(0.1, -0.9), p(0.8, 0.9));
        }
        Characteristic::Perpendicularity => {
            g.line(Pen::Thin, p(-0.9, -0.9), p(0.9, -0.9));
            g.line(Pen::Thin, p(0.0, -0.9), p(0.0, 0.9));
        }
        Characteristic::Angularity => {
            g.line(Pen::Thin, p(-0.9, -0.9), p(0.9, -0.9));
            g.line(Pen::Thin, p(-0.9, -0.9), p(0.9, 0.6));
        }
        Characteristic::Position => {
            circle(g, 0.6);
            g.line(Pen::Thin, p(-1.0, 0.0), p(1.0, 0.0));
            g.line(Pen::Thin, p(0.0, -1.0), p(0.0, 1.0));
        }
        Characteristic::Concentricity => {
            circle(g, 0.4);
            circle(g, 0.85);
        }
        Characteristic::Symmetry => {
            g.line(Pen::Thin, p(-0.6, 0.6), p(0.6, 0.6));
            g.line(Pen::Thin, p(-1.0, 0.0), p(1.0, 0.0));
            g.line(Pen::Thin, p(-0.6, -0.6), p(0.6, -0.6));
        }
        Characteristic::CircularRunout | Characteristic::TotalRunout => {
            let mut runout = |x: f64| {
                g.line(Pen::Thin, p(x - 0.5, -0.9), p(x + 0.3, 0.7));
                arrow_small(g, p(x + 0.3, 0.7), [0.8, 1.6]);
            };
            if characteristic == Characteristic::TotalRunout {
                runout(-0.4);
                runout(0.4);
                g.line(Pen::Thin, p(-0.9, -0.9), p(0.9, -0.9));
            } else {
                runout(0.0);
            }
        }
    }
}

/// A small open arrowhead for the runout symbols.
fn arrow_small(g: &mut Group, tip: P2, direction: P2) {
    let d = unit2(direction);
    let back = sub2(tip, mul2(d, 1.0));
    let side = mul2(perp(d), 0.5);
    g.line(Pen::Thin, tip, add2(back, side));
    g.line(Pen::Thin, tip, sub2(back, side));
}

/// A surface finish symbol (ISO 1302) on the surface at `root` (sheet; a
/// leader from `tip` to it when moved off): a short leg and a long one at
/// 60 degrees to each other, a bar closing them for material removal, a
/// circle in them for none, the requirement over the long leg's line.
fn surface_finish(g: &mut Group, tip: P2, root: P2, finish: &SurfaceFinish) {
    if len2(sub2(root, tip)) > 0.5 {
        g.line(Pen::Thin, root, tip);
        arrow(g, tip, unit2(sub2(tip, root)));
    }
    let short = 1.4 * DIMENSION_TEXT;
    let long = 2.0 * short;
    let (c60, s60) = (0.5, 3f64.sqrt() / 2.0);
    let left = [root[0] - short * c60, root[1] + short * s60];
    let right = [root[0] + long * c60, root[1] + long * s60];
    g.line(Pen::Thin, root, left);
    g.line(Pen::Thin, root, right);
    match finish.process {
        FinishProcess::Removal => {
            // At the short leg's height across to the long leg.
            g.line(Pen::Thin, left, [root[0] + short * c60, left[1]]);
        }
        FinishProcess::NoRemoval => {
            let r = short * 0.35;
            g.items.push(Item::Arc {
                pen: Pen::Thin,
                center: [root[0], root[1] + 2.0 * r],
                radius: r,
                start: 0.0,
                end: TAU,
            });
        }
        FinishProcess::Any => {}
    }
    if let Some(text) = finish.text.as_deref().filter(|t| !t.trim().is_empty()) {
        let width = text_width(text, DIMENSION_TEXT) + 1.0;
        g.line(Pen::Thin, right, [right[0] + width, right[1]]);
        g.text(
            [right[0] + 0.5, right[1] - DIMENSION_TEXT - 0.8],
            DIMENSION_TEXT,
            0.0,
            Anchor::Start,
            text.trim(),
        );
    }
}

/// A group's box on the sheet: its lines' and arcs' ends and circles,
/// its texts by their width and height.
pub fn group_bounds(g: &Group) -> Bounds {
    let mut b = Bounds::empty();
    for item in &g.items {
        match item {
            Item::Line { a, b: q, .. } => {
                b.add(*a);
                b.add(*q);
            }
            Item::Arc { center, radius, .. } => {
                b.add([center[0] - radius, center[1] - radius]);
                b.add([center[0] + radius, center[1] + radius]);
            }
            Item::Polyline { points, .. } | Item::Fill { points } | Item::Shade { points, .. } => {
                for p in points {
                    b.add(*p);
                }
            }
            Item::Text {
                at,
                height,
                angle,
                anchor,
                text,
            } => {
                let w = text_width(text, *height);
                let along = [angle.cos(), angle.sin()];
                let up = [-angle.sin(), angle.cos()];
                let start = match anchor {
                    Anchor::Start => *at,
                    Anchor::Middle => sub2(*at, mul2(along, w / 2.0)),
                    Anchor::End => sub2(*at, mul2(along, w)),
                };
                for p in [
                    start,
                    add2(start, mul2(along, w)),
                    add2(start, mul2(up, *height)),
                    add2(add2(start, mul2(along, w)), mul2(up, *height)),
                ] {
                    b.add(p);
                }
            }
        }
    }
    b
}

/// The place a drag of an annotation by `delta` (sheet) gives it: a
/// dimension line moves across, its text along it; a radius' or a
/// leader's text moves; an angle's arc grows or shrinks.
pub fn moved(annotation: &mut ViewAnnotation, seen: &[Seen], frame: &Frame<'_>, delta: P2) {
    let shift = |offset: Option<P2>, default: P2| {
        let [u, v] = offset.unwrap_or(default);
        Some([u + delta[0], v + delta[1]])
    };
    match annotation.kind {
        AnnotationType::Linear => {
            let Ok((_, d)) = linear_axes(annotation.along, seen) else {
                return;
            };
            let n = perp(d);
            let (pa, pb) = (frame.at(seen[0].point()), frame.at(seen[1].point()));
            let c = linear_offset(annotation, mid(pa, pb), n, &frame.view_box);
            annotation.line_offset = Some(c + dot2(delta, n));
            let [u, v] = annotation.text_offset.unwrap_or([0.0, 0.0]);
            let along = upright_along(d);
            annotation.text_offset = Some([u + dot2(delta, along), v]);
        }
        AnnotationType::Ordinate => {
            let n = match annotation.along {
                Some(Along::Vertical) => [1.0, 0.0],
                _ => [0.0, 1.0],
            };
            let origin = frame.at(seen[0].point());
            let level = annotation.line_offset.unwrap_or_else(|| {
                let b = &frame.view_box;
                if b.is_empty() {
                    LINE_GAP
                } else {
                    dot2(sub2(b.max, origin), n) + LINE_GAP
                }
            });
            annotation.line_offset = Some(level + dot2(delta, n));
        }
        AnnotationType::Angle => {
            if let Ok((_, r1, r2)) = corner(seen) {
                let middle = unit2(add2(r1, r2));
                let radius = arc_radius(annotation, seen, frame);
                annotation.line_offset = Some((radius + dot2(delta, middle)).max(2.0));
            }
        }
        AnnotationType::Radius | AnnotationType::Diameter => match seen.first() {
            Some(Seen::Circle {
                radius,
                end_on: true,
                ..
            }) => {
                let out = radius * frame.map.scale + 10.0;
                annotation.text_offset = shift(
                    annotation.text_offset,
                    [out * FRAC_1_SQRT_2, out * FRAC_1_SQRT_2],
                );
            }
            Some(seen) => {
                let (center, radius, across) = match *seen {
                    Seen::Circle {
                        center,
                        radius,
                        across,
                        ..
                    } => (center, radius, across),
                    Seen::Axis { a, b, radius } => (mid(a, b), radius, perp(unit2(sub2(b, a)))),
                    _ => return,
                };
                let n = perp(across);
                let (a, b) = (
                    frame.at(sub2(center, mul2(across, radius))),
                    frame.at(add2(center, mul2(across, radius))),
                );
                let c = annotation
                    .line_offset
                    .unwrap_or_else(|| linear_offset(annotation, mid(a, b), n, &frame.view_box));
                annotation.line_offset = Some(c + dot2(delta, n));
            }
            None => {}
        },
        AnnotationType::Note | AnnotationType::Frame => {
            annotation.text_offset = shift(annotation.text_offset, NOTE_SHIFT);
        }
        AnnotationType::Datum => {
            annotation.text_offset = shift(annotation.text_offset, DATUM_SHIFT);
        }
        AnnotationType::Finish => {
            annotation.text_offset = shift(annotation.text_offset, [0.0, 0.0]);
        }
        AnnotationType::Balloon => {
            annotation.text_offset = shift(annotation.text_offset, BALLOON_SHIFT);
        }
        AnnotationType::HoleTable | AnnotationType::PartsList => {
            if let Some(table) = &mut annotation.table {
                table.place = add2(table.place, delta);
            }
        }
        AnnotationType::CenterMark | AnnotationType::Centerline => {}
    }
}

/// The direction a dimension's text runs along a line along `d`.
fn upright_along(d: P2) -> P2 {
    let angle = upright(d);
    [angle.cos(), angle.sin()]
}

/// The place `at` (sheet) gives a new annotation: its dimension line or
/// arc through it with its text there, a leader's text there.
pub fn placed_at(annotation: &mut ViewAnnotation, seen: &[Seen], frame: &Frame<'_>, at: P2) {
    match annotation.kind {
        AnnotationType::Linear => {
            if let Ok((_, d)) = linear_axes(annotation.along, seen) {
                let n = perp(d);
                let (pa, pb) = (frame.at(seen[0].point()), frame.at(seen[1].point()));
                let middle = mid(pa, pb);
                annotation.line_offset = Some(dot2(sub2(at, middle), n));
                let u = dot2(sub2(at, middle), upright_along(d));
                // Left in the middle unless clearly off it.
                let half = len2(sub2(pb, pa)) / 2.0;
                annotation.text_offset = (u.abs() > half).then_some([u, 0.0]);
            }
        }
        AnnotationType::Ordinate => {
            let n = match annotation.along {
                Some(Along::Vertical) => [1.0, 0.0],
                _ => [0.0, 1.0],
            };
            let origin = frame.at(seen[0].point());
            annotation.line_offset = Some(dot2(sub2(at, origin), n));
        }
        AnnotationType::Angle => {
            if let Ok((c, _, _)) = corner(seen) {
                annotation.line_offset = Some(len2(sub2(at, frame.at(c))).max(2.0));
            }
        }
        AnnotationType::Radius | AnnotationType::Diameter => match seen.first() {
            Some(Seen::Circle {
                center,
                end_on: true,
                ..
            }) => {
                annotation.text_offset = Some(sub2(at, frame.at(*center)));
            }
            Some(seen) => {
                let (center, radius, across) = match *seen {
                    Seen::Circle {
                        center,
                        radius,
                        across,
                        ..
                    } => (center, radius, across),
                    Seen::Axis { a, b, radius } => (mid(a, b), radius, perp(unit2(sub2(b, a)))),
                    _ => return,
                };
                let n = perp(across);
                let middle = mid(
                    frame.at(sub2(center, mul2(across, radius))),
                    frame.at(add2(center, mul2(across, radius))),
                );
                annotation.line_offset = Some(dot2(sub2(at, middle), n));
            }
            None => {}
        },
        AnnotationType::Note
        | AnnotationType::Datum
        | AnnotationType::Frame
        | AnnotationType::Finish
        | AnnotationType::Balloon => {
            let tip = leader_tip(annotation, &seen[0], frame);
            annotation.text_offset = Some(sub2(at, tip));
        }
        AnnotationType::HoleTable | AnnotationType::PartsList => {
            if let Some(table) = &mut annotation.table {
                table.place = at;
            }
        }
        AnnotationType::CenterMark | AnnotationType::Centerline => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> ViewMap {
        ViewMap::new(1.0, [0.0, 0.0], [100.0, 100.0])
    }

    fn view_box() -> Bounds {
        let mut b = Bounds::empty();
        b.add([100.0, 100.0]);
        b.add([140.0, 120.0]);
        b
    }

    fn words(text: &str) -> ValueText {
        ValueText {
            text: text.to_owned(),
            main: text.to_owned(),
            ..ValueText::default()
        }
    }

    fn texts(laid: &Laid) -> Vec<String> {
        laid.group
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_linear_dimension_measures_along_the_view_and_lies_beyond_it() {
        let map = map();
        let frame = Frame {
            map: &map,
            view_box: view_box(),
        };
        // The two upright sides of a 40 x 20 box at the view's origin.
        let seen = [
            Seen::Line([0.0, 0.0], [0.0, 20.0]),
            Seen::Line([40.0, 0.0], [40.0, 20.0]),
        ];
        let mut d = ViewAnnotation::new(1, AnnotationType::Linear, Vec::new());
        assert_eq!(default_along(&seen), Along::Horizontal);
        d.along = Some(Along::Horizontal);
        assert_eq!(measure(&d, &seen).unwrap(), Some(40.0));
        d.along = Some(Along::Aligned);
        assert_eq!(measure(&d, &seen).unwrap(), Some(40.0));
        d.along = Some(Along::Vertical);
        assert_eq!(measure(&d, &seen).unwrap(), Some(0.0));
        d.along = Some(Along::Horizontal);
        let laid = lay_out(&d, &seen, &frame, Some(&words("40")), &[]).unwrap();
        assert_eq!(texts(&laid), ["40"]);
        // The picks lie in the lower half of the box... at its middle: the
        // line goes below or above it, beyond its edge.
        assert!(
            laid.bounds.max[1] > 120.0 + LINE_GAP - 1e-9 || laid.bounds.min[1] < 100.0 - 9.0,
            "{:?}",
            laid.bounds
        );
        // Placed through a point above: the line there.
        placed_at(&mut d, &seen, &frame, [120.0, 140.0]);
        assert_eq!(d.line_offset, Some(30.0));
        assert_eq!(d.text_offset, None);
        // Dragged down by 5: 5 nearer.
        moved(&mut d, &seen, &frame, [3.0, -5.0]);
        assert_eq!(d.line_offset, Some(25.0));
        assert_eq!(d.text_offset, Some([3.0, 0.0]));
    }

    #[test]
    fn a_dragged_text_moves_the_way_the_drag_goes_whatever_the_picks_order() {
        let map = map();
        let frame = Frame {
            map: &map,
            view_box: view_box(),
        };
        let left = Seen::Line([0.0, 0.0], [0.0, 20.0]);
        let right = Seen::Line([40.0, 0.0], [40.0, 20.0]);
        let text_x = |laid: &Laid| {
            laid.group
                .items
                .iter()
                .find_map(|i| match i {
                    Item::Text { at, .. } => Some(at[0]),
                    _ => None,
                })
                .unwrap()
        };
        for seen in [[left, right], [right, left]] {
            let mut d = ViewAnnotation::new(1, AnnotationType::Linear, Vec::new());
            d.along = Some(Along::Horizontal);
            let before = text_x(&lay_out(&d, &seen, &frame, Some(&words("40")), &[]).unwrap());
            // Dragged 30 mm right: past the right end, the line run on under it.
            moved(&mut d, &seen, &frame, [30.0, 0.0]);
            let laid = lay_out(&d, &seen, &frame, Some(&words("40")), &[]).unwrap();
            assert!(
                (text_x(&laid) - before - 30.0).abs() < 1e-9,
                "{before} -> {}",
                text_x(&laid)
            );
            let run_on = laid.group.items.iter().any(|i| {
                matches!(i, Item::Line { a, b, .. }
                    if (a[0] - 140.0).abs() < 1e-9 && (b[0] - 150.0).abs() < 1e-9)
            });
            assert!(run_on, "{:?}", laid.group.items);
            // Placed through a point left of the middle: the text there.
            placed_at(&mut d, &seen, &frame, [110.0, 140.0]);
            assert_eq!(d.text_offset, None, "inside the line: left in its middle");
        }
    }

    #[test]
    fn circles_angles_and_ordinates_are_measured() {
        let circle = Seen::Circle {
            center: [10.0, 10.0],
            radius: 5.0,
            end_on: true,
            across: [1.0, 0.0],
            arc: None,
        };
        let r = ViewAnnotation::new(1, AnnotationType::Radius, Vec::new());
        let dia = ViewAnnotation::new(2, AnnotationType::Diameter, Vec::new());
        assert_eq!(measure(&r, &[circle]).unwrap(), Some(5.0));
        assert_eq!(measure(&dia, &[circle]).unwrap(), Some(10.0));
        let side = Seen::Axis {
            a: [0.0, 0.0],
            b: [0.0, 30.0],
            radius: 4.0,
        };
        assert_eq!(measure(&dia, &[side]).unwrap(), Some(8.0));
        assert!(measure(&dia, &[Seen::Point([0.0, 0.0])]).is_err());
        let map = map();
        let frame = Frame {
            map: &map,
            view_box: view_box(),
        };
        let laid = lay_out(&dia, &[circle], &frame, Some(&words("Ø10")), &[]).unwrap();
        assert_eq!(texts(&laid), ["Ø10"]);
        assert!(lay_out(&dia, &[side], &frame, Some(&words("Ø8")), &[]).is_ok());

        // 90 degrees between X and Y.
        let a = ViewAnnotation::new(3, AnnotationType::Angle, Vec::new());
        let lines = [
            Seen::Line([0.0, 0.0], [10.0, 0.0]),
            Seen::Line([0.0, 0.0], [0.0, 10.0]),
        ];
        let value = measure(&a, &lines).unwrap().unwrap();
        assert!((value - PI / 2.0).abs() < 1e-12);
        let parallel = [
            Seen::Line([0.0, 0.0], [10.0, 0.0]),
            Seen::Line([0.0, 5.0], [10.0, 5.0]),
        ];
        assert!(measure(&a, &parallel).unwrap_err().contains("parallel"));

        let mut o = ViewAnnotation::new(4, AnnotationType::Ordinate, Vec::new());
        o.along = Some(Along::Horizontal);
        let points = [Seen::Point([0.0, 0.0]), Seen::Point([25.0, 5.0])];
        assert_eq!(measure(&o, &points).unwrap(), Some(25.0));
        o.along = Some(Along::Vertical);
        assert_eq!(measure(&o, &points).unwrap(), Some(5.0));
    }

    #[test]
    fn annotations_are_checked() {
        let pick = ViewPick {
            body: "F2.b0".parse().unwrap(),
            occurrence: None,
            edge: Some("E{F2:start|F2:side(c1[c4,c2])}".parse().unwrap()),
            face: None,
            point: None,
        };
        let mut d = ViewAnnotation::new(1, AnnotationType::Linear, vec![pick.clone()]);
        assert!(d.check().unwrap_err().contains("takes 2 picks"));
        d.picks.push(pick.clone());
        assert!(d.check().unwrap_err().contains("along a direction"));
        d.along = Some(Along::Aligned);
        d.check().unwrap();
        d.text = Some("x".to_owned());
        assert!(d.check().unwrap_err().contains("no text of its own"));
        d.text = None;
        d.tolerance = Some(Tolerance::Symmetric { value: 0.1 });
        d.check().unwrap();
        let mut note = ViewAnnotation::new(2, AnnotationType::Note, vec![pick.clone()]);
        assert!(note.check().unwrap_err().contains("needs a text"));
        note.text = Some("DEBURR".to_owned());
        note.check().unwrap();
        note.tolerance = Some(Tolerance::Symmetric { value: 0.1 });
        assert!(note.check().unwrap_err().contains("no value's format"));
        let mut datum = ViewAnnotation::new(3, AnnotationType::Datum, vec![pick.clone()]);
        assert!(datum.check().unwrap_err().contains("letter"));
        datum.letter = Some("I".to_owned());
        assert!(datum.check().is_err());
        datum.letter = Some("A".to_owned());
        datum.check().unwrap();
        let mut o = ViewAnnotation::new(4, AnnotationType::Ordinate, vec![pick.clone(), pick]);
        o.along = Some(Along::Aligned);
        assert!(o.check().unwrap_err().contains("x or y"));
    }

    #[test]
    fn marks_notes_and_symbols_are_drawn() {
        let mut g = Group::new("center-mark");
        center_mark(&mut g, [50.0, 50.0], 10.0);
        let b = group_bounds(&g);
        assert!((b.width() - 2.0 * (10.0 + CENTER_BEYOND)).abs() < 1e-9);
        let note = SheetNote {
            id: 5,
            text: "ALL EDGES\nDEBURRED".to_owned(),
            at: [20.0, 40.0],
            height: None,
        };
        note.check().unwrap();
        let laid = lay_out_note(&note);
        assert_eq!(laid.group.items.len(), 2);
        for characteristic in Characteristic::ALL {
            let mut g = Group::new("feature-control-frame");
            symbol(&mut g, characteristic, [0.0, 0.0], 4.0);
            assert!(!g.items.is_empty(), "{characteristic:?}");
        }
        let mut g = Group::new("surface-finish");
        surface_finish(
            &mut g,
            [0.0, 0.0],
            [0.0, 0.0],
            &SurfaceFinish {
                process: FinishProcess::Removal,
                text: Some("Ra 1.6".to_owned()),
            },
        );
        assert!(
            g.items
                .iter()
                .any(|i| matches!(i, Item::Text { text, .. } if text == "Ra 1.6"))
        );
    }
}
