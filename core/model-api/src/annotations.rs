// SPDX-License-Identifier: MIT
//! Annotations (mitcad#702, `document/annotations.rs`): the commands
//! `add_annotation`, `edit_annotation` and `delete_annotation`, and the
//! `annotations` query (the `measurements` query is the same), which
//! measures each one at the timeline marker (`measurements.rs`), writes its
//! value ISO 129 style with its prefix, decimals, own text and tolerance,
//! and lays it out in its plane: the dimension and extension lines, an
//! angle's arc, the arrows, and where its text stands and which way it
//! runs. Hidden ones are measured only when asked for, and picks are
//! measured again only when their bodies or datums changed (mitcad#716,
//! `Document::measure_picks`). See `commands.md`, "Annotations".
//!
//! Everything is in the design's coordinates and millimetres; the
//! application projects it, keeps the text readable within its plane and
//! sizes arrows and text in the view's pixels.

use mitcad_vec3::{add, add_scaled, cross, distance, dot, norm, scale, sub, unit, unit_above};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value, json};

use super::ApiError;
use super::analysis::{MeasureCache, MeasuredPicks};
use super::measure_axes::Element;
use super::measurements::{
    Drawn, Found, across, add_drawn, check_kind, default_kind, no_single_value, values_of,
};
use super::prelude::*;
use mitcad_model::datum::Vec3;
use mitcad_model::internal::document::normalized_color;
use mitcad_model::kernel::Kernel;
use mitcad_model::transform::Transform;
use mitcad_model::{
    Annotation, AnnotationFormat, AnnotationPlane, Characteristic, DatumNeed, Document,
    GeometricTolerance, MeasureKind, MeasurePick, Tolerance,
};

const X: Vec3 = [1.0, 0.0, 0.0];
const Y: Vec3 = [0.0, 1.0, 0.0];
const Z: Vec3 = [0.0, 0.0, 1.0];

/// A length's dimension line from its ends when the offset is not set:
/// this share of the length, at least 1 mm.
const LINE_SHARE: f64 = 0.25;
/// An angle's arc radius (mm) when neither it nor the picks' size is known.
const ARC_MM: f64 = 10.0;
/// A note's text from its arrow's point (mm, along and across) when not
/// set.
const NOTE_MM: f64 = 10.0;
/// A circle's text from its centre when not moved: radii.
const CIRCLE_TEXT: f64 = 1.5;
/// The places a view suggests when an annotation is added, in the view's
/// pixels: a length's dimension line from its ends, an angle's arc radius,
/// a circle's text beyond its rim, a note's text from its point.
const LINE_PIXELS: f64 = 22.0;
const ARC_PIXELS: f64 = 42.0;
const TEXT_PIXELS: f64 = 30.0;
const NOTE_PIXELS: [f64; 2] = [40.0, 30.0];
/// Deviations without set decimals show up to this many.
const DEVIATION_DECIMALS: usize = 4;

/// A field that may be given as `null`: `Some(None)` then, `None` when
/// left out.
pub(super) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

/// What the view looked like when an annotation is added: the direction it
/// looks along (from the eye), its up direction and the size of a pixel
/// (mm) near the picks. It suggests the plane (square to the view) and the
/// places of the line and the text, which are then kept.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ViewHint {
    direction: Vec3,
    #[serde(default)]
    up: Option<Vec3>,
    #[serde(default)]
    pixel: Option<f64>,
}

/// `add_annotation`: the fields of [`Annotation`], each optional but the
/// first pick, and the view it is added in.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NewAnnotation {
    a: MeasurePick,
    #[serde(default)]
    b: Option<MeasurePick>,
    /// What the picks give first when left out; `note` when a `text` is
    /// given.
    #[serde(default)]
    kind: Option<MeasureKind>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    visible: Option<bool>,
    #[serde(default)]
    plane: Option<AnnotationPlane>,
    #[serde(default)]
    line_offset: Option<f64>,
    #[serde(default)]
    text_offset: Option<[f64; 2]>,
    #[serde(default)]
    format: Option<AnnotationFormat>,
    #[serde(default)]
    tolerance: Option<Tolerance>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    at: Option<Vec3>,
    #[serde(default)]
    view: Option<ViewHint>,
    /// A datum's letter (the next free one when left out).
    #[serde(default)]
    letter: Option<String>,
    /// A feature control frame's tolerance, and the dimension it stands
    /// under instead of a leader.
    #[serde(default)]
    frame: Option<GeometricTolerance>,
    #[serde(default)]
    on: Option<String>,
}

/// `edit_annotation`: the annotation's `name` (or `names`, several at
/// once) and what changes; `null` sets a field back to its default.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AnnotationEdit {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    names: Option<Vec<String>>,
    #[serde(default)]
    new_name: Option<String>,
    #[serde(default)]
    kind: Option<MeasureKind>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default, deserialize_with = "present")]
    group: Option<Option<String>>,
    #[serde(default)]
    visible: Option<bool>,
    #[serde(default)]
    a: Option<MeasurePick>,
    #[serde(default, deserialize_with = "present")]
    b: Option<Option<MeasurePick>>,
    #[serde(default, deserialize_with = "present")]
    plane: Option<Option<AnnotationPlane>>,
    #[serde(default, deserialize_with = "present")]
    line_offset: Option<Option<f64>>,
    #[serde(default, deserialize_with = "present")]
    text_offset: Option<Option<[f64; 2]>>,
    #[serde(default)]
    format: Option<FormatEdit>,
    #[serde(default, deserialize_with = "present")]
    tolerance: Option<Option<Tolerance>>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default, deserialize_with = "present")]
    at: Option<Option<Vec3>>,
    #[serde(default)]
    letter: Option<String>,
    #[serde(default)]
    frame: Option<GeometricTolerance>,
    #[serde(default, deserialize_with = "present")]
    on: Option<Option<String>>,
}

/// The format's fields that change, the others kept.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FormatEdit {
    #[serde(default, deserialize_with = "present")]
    decimals: Option<Option<u8>>,
    #[serde(default, deserialize_with = "present")]
    prefix: Option<Option<String>>,
    #[serde(default)]
    before: Option<String>,
    #[serde(default)]
    after: Option<String>,
    #[serde(default, deserialize_with = "present")]
    replace: Option<Option<String>>,
}

impl FormatEdit {
    pub(super) fn apply(&self, format: &mut AnnotationFormat) {
        if let Some(decimals) = self.decimals {
            format.decimals = decimals;
        }
        if let Some(prefix) = &self.prefix {
            format.prefix.clone_from(prefix);
        }
        if let Some(before) = &self.before {
            before.trim().clone_into(&mut format.before);
        }
        if let Some(after) = &self.after {
            after.trim().clone_into(&mut format.after);
        }
        if let Some(replace) = &self.replace {
            format.replace = replace
                .as_deref()
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_owned);
        }
    }
}

/// The names a command gives: `name`, or `names` of several.
pub(super) fn names_of(
    name: Option<String>,
    names: Option<Vec<String>>,
) -> Result<Vec<String>, ApiError> {
    match (name, names) {
        (Some(name), None) => Ok(vec![name]),
        (None, Some(names)) if !names.is_empty() => Ok(names),
        _ => Err(ApiError(
            "give the annotation's `name`, or the `names` of several".to_owned(),
        )),
    }
}

/// What an annotation is drawn from: a length's two ends, a circle, an
/// angle's corner and rays, or a note's point (with its pick's plane's
/// normal).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Ends([Vec3; 2]),
    Circle {
        center: Vec3,
        axis: Vec3,
        radius: f64,
    },
    Arc {
        center: Vec3,
        from: Vec3,
        to: Vec3,
        reach: Option<f64>,
    },
    Note {
        anchor: Vec3,
        normal: Option<Vec3>,
    },
}

impl Shape {
    fn of(found: &Found) -> Self {
        match found.drawn {
            Drawn::Ends(ends) => Self::Ends(ends),
            Drawn::Circle {
                center,
                axis,
                radius,
                ..
            } => Self::Circle {
                center,
                axis,
                radius,
            },
            Drawn::Arc {
                center,
                from,
                to,
                reach,
            } => Self::Arc {
                center,
                from,
                to,
                reach,
            },
        }
    }
}

/// An angle's arc: its centre, radius, the unit rays it runs between (from
/// `from` towards `to`) and its plane's normal.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ArcDrawn {
    center: Vec3,
    radius: f64,
    from: Vec3,
    to: Vec3,
    normal: Vec3,
}

/// Where an annotation is drawn, in the design's coordinates.
#[derive(Debug, Clone, PartialEq)]
struct Drawing {
    /// Its plane: a point of it, the unit axes `text_offset` is along —
    /// `x` (a length's dimension line, the tangent at an angle's arc's
    /// middle, a circle's `across` of its axis, a note's text) and `y`
    /// (across it: the side a length's line is offset to) — and the
    /// normal, a right-handed frame.
    origin: Vec3,
    x: Vec3,
    y: Vec3,
    normal: Vec3,
    /// The dimension line's offset or the arc's radius in use.
    line_offset: f64,
    /// The text's offset in use, along `x` and `y`.
    text_offset: [f64; 2],
    /// From a measured point to the dimension line (the application leaves
    /// a small gap at the point and runs a little past the line).
    extension_lines: Vec<[Vec3; 2]>,
    /// Dimension lines and a note's leader.
    dimension_lines: Vec<[Vec3; 2]>,
    arcs: Vec<ArcDrawn>,
    /// Each arrow's tip and the unit direction it points along.
    arrows: Vec<[Vec3; 2]>,
    /// A circle's centre, for its centre mark.
    center: Option<Vec3>,
    /// Where the text stands (its middle on the line, or a note's start)
    /// and the unit directions it runs along and up.
    text: [Vec3; 3],
}

impl Drawing {
    fn to_json(&self) -> Value {
        // + 0.0 turns -0.0 into 0.0.
        let v = |v: Vec3| v.map(|x| x + 0.0);
        let pairs = |lines: &[[Vec3; 2]]| -> Vec<[Vec3; 2]> {
            lines.iter().map(|[p, q]| [v(*p), v(*q)]).collect()
        };
        let mut value = json!({
            "plane": {"origin": v(self.origin), "x": v(self.x), "y": v(self.y),
                      "normal": v(self.normal)},
            "line_offset": self.line_offset + 0.0,
            "text_offset": [self.text_offset[0] + 0.0, self.text_offset[1] + 0.0],
            "extension_lines": pairs(&self.extension_lines),
            "dimension_lines": pairs(&self.dimension_lines),
            "arcs": self.arcs.iter().map(|arc| json!({
                "center": v(arc.center), "radius": arc.radius, "from": v(arc.from),
                "to": v(arc.to), "normal": v(arc.normal)})).collect::<Vec<_>>(),
            "arrows": self.arrows.iter().map(|[tip, direction]| json!({
                "tip": v(*tip), "direction": v(*direction)})).collect::<Vec<_>>(),
            "text": {"at": v(self.text[0]), "direction": v(self.text[1]),
                     "up": v(self.text[2])},
        });
        if let Some(center) = self.center {
            value["center"] = json!(v(center));
        }
        value
    }
}

/// The document's length unit for values in words.
#[derive(Debug, Clone, Copy)]
pub(super) struct Units {
    symbol: &'static str,
    millimetres: f64,
}

impl Units {
    #[cfg(test)]
    pub(super) const MM: Self = Self {
        symbol: "mm",
        millimetres: 1.0,
    };

    /// A unit by its symbol and its millimetres (a drawing sheet's).
    pub(super) fn new(symbol: &'static str, millimetres: f64) -> Self {
        Self {
            symbol,
            millimetres,
        }
    }
}

/// A value of a dimension on a drawing's view in words (mitcad#707 phase
/// 2): the words of a dimension of the model of this kind, format and
/// tolerance, one engine for both; with why a fit code's deviations are
/// not shown, if they are not.
pub(super) fn value_text(
    kind: MeasureKind,
    format: &AnnotationFormat,
    tolerance: Option<&Tolerance>,
    value: f64,
    units: Units,
) -> (mitcad_model_drawing::annotate::ValueText, Option<String>) {
    let mut annotation = Annotation::new("", kind, "#000000", MeasurePick::default());
    annotation.format = format.clone();
    annotation.tolerance = tolerance.cloned();
    let label = dimension_label(&annotation, value, units);
    (
        mitcad_model_drawing::annotate::ValueText {
            text: label.text,
            main: label.main,
            upper: label.upper,
            lower: label.lower,
            brackets: label.brackets,
            after: label.after,
        },
        label.warning,
    )
}

/// A feature control frame's cells (the symbol's text first) and all of
/// them on one line, as the model's frames have them, with the warning for
/// a datum referred to that no datum label has (`datums`: those there are).
pub(super) fn frame_words(
    frame: &GeometricTolerance,
    units: Units,
    datums: &[&str],
) -> (Vec<String>, String, Option<String>) {
    let label = frame_label(frame, units, datums);
    (label.frame, label.text, label.warning)
}

/// A dimension's value in words, ISO 129 style: `main` (its own text
/// before, the prefix, the value, a symmetric tolerance or a fit code),
/// a stack of `upper` over `lower` (deviations, limits, a fit's
/// deviations in `brackets`), then the text `after`; `text` all on one
/// line. A note's `lines`.
#[derive(Debug, Clone, Default, PartialEq)]
struct Label {
    text: String,
    main: String,
    upper: Option<String>,
    lower: Option<String>,
    brackets: bool,
    after: String,
    /// The tolerance alone in words (`±0.1`, `+0.2/-0.1`, `H7`).
    tolerance: String,
    /// The tolerance's deviations (mm or degrees), upper and lower.
    deviations: Option<(f64, f64)>,
    /// Why a fit code's deviations are not shown.
    warning: Option<String>,
    lines: Vec<String>,
    /// A feature control frame's cells: the symbol, the tolerance, the
    /// datums referred to.
    frame: Vec<String>,
    /// A datum label's letter.
    datum: Option<String>,
}

impl Label {
    fn to_json(&self) -> Value {
        let mut value = json!({"text": self.text});
        if let Some(warning) = &self.warning {
            value["warning"] = json!(warning);
        }
        if !self.lines.is_empty() {
            value["lines"] = json!(self.lines);
            return value;
        }
        if !self.frame.is_empty() {
            value["frame"] = json!(self.frame);
            value["tolerance"] = json!(self.tolerance);
            return value;
        }
        if let Some(datum) = &self.datum {
            value["datum"] = json!(datum);
            return value;
        }
        value["main"] = json!(self.main);
        if let (Some(upper), Some(lower)) = (&self.upper, &self.lower) {
            value["upper"] = json!(upper);
            value["lower"] = json!(lower);
            value["brackets"] = json!(self.brackets);
        }
        if !self.after.is_empty() {
            value["after"] = json!(self.after);
        }
        if !self.tolerance.is_empty() {
            value["tolerance"] = json!(self.tolerance);
        }
        value
    }
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait AnnotationsOps<K: Kernel> {
    fn add_annotation_command(&mut self, new: NewAnnotation) -> Result<Value, ApiError>;

    fn edit_annotation_command(&mut self, edit: AnnotationEdit) -> Result<Value, ApiError>;

    /// The `annotations` query (and `measurements`): each annotation in its
    /// file form, with what its picks give at the marker, its value in
    /// words and where it is drawn, or why it cannot be measured there. A
    /// hidden one is measured only with `measure_hidden` (mitcad#716);
    /// otherwise it is marked `"measured": false`, with an `error` only
    /// when a pick's body or datum is not there.
    fn annotations_json(&self, measure_hidden: bool) -> Value;
}

impl<K: Kernel> AnnotationsOps<K> for Document<K> {
    fn add_annotation_command(&mut self, new: NewAnnotation) -> Result<Value, ApiError> {
        let picks = self
            .measure_picks(&new.a, new.b.as_ref())
            .map_err(|e| ApiError(format!("the annotation's picks: {e}")))?;
        // A text makes a note, a letter a datum, a tolerance a frame.
        let given = new.kind.or_else(|| {
            if new.text.is_some() {
                Some(MeasureKind::Note)
            } else if new.letter.is_some() {
                Some(MeasureKind::Datum)
            } else if new.frame.is_some() {
                Some(MeasureKind::Frame)
            } else {
                None
            }
        });
        let kind = match given {
            Some(kind) if kind.is_leader() => kind,
            kind => {
                let found = values_of(&picks);
                let kind = match kind {
                    Some(kind) => kind,
                    None => default_kind(&picks, &found).ok_or_else(no_single_value)?,
                };
                check_kind(kind, &found)?;
                kind
            }
        };
        let mut annotation = Annotation::new(
            new.name.as_deref().unwrap_or(""),
            kind,
            new.color.as_deref().unwrap_or(""),
            new.a,
        );
        annotation.b = new.b;
        annotation.visible = new.visible.unwrap_or(true);
        annotation.group = new.group;
        annotation.plane = new.plane;
        annotation.line_offset = new.line_offset;
        annotation.text_offset = new.text_offset;
        annotation.format = new.format.unwrap_or_default();
        annotation.tolerance = new.tolerance;
        annotation.text = new.text;
        annotation.at = new.at;
        annotation.frame = new.frame;
        annotation.on = new.on;
        // A datum takes the first free letter unless given one.
        if kind == MeasureKind::Datum {
            annotation.letter = Some(match new.letter {
                Some(letter) => letter.trim().to_owned(),
                None => self.next_datum_letter().map_err(ApiError)?,
            });
        } else {
            annotation.letter = new.letter;
        }
        // A note, a datum or a frame needs a point to point at: a body picked
        // without one has none. A datum or a frame stands on a feature, not
        // on a point (under a dimension the pick is only an anchor); a
        // frame under a dimension needs that dimension.
        if kind.is_leader() && annotation.on.is_none() {
            self.shape_of(&annotation, &picks).map_err(ApiError)?;
            feature_pick(kind, &annotation.a, &picks).map_err(ApiError)?;
        }
        if let Some(on) = &annotation.on {
            self.dimension_to_stand_under(on.trim()).map_err(ApiError)?;
        }
        if let Some(view) = &new.view {
            self.place_for_view(&mut annotation, &picks, view)?;
        }
        let name = self.add_annotation(annotation)?;
        Ok(json!({"name": name, "kind": kind}))
    }

    fn edit_annotation_command(&mut self, edit: AnnotationEdit) -> Result<Value, ApiError> {
        let AnnotationEdit {
            name,
            names,
            new_name,
            kind,
            color,
            group,
            visible,
            a,
            b,
            plane,
            line_offset,
            text_offset,
            format,
            tolerance,
            text,
            at,
            letter,
            frame,
            on,
        } = edit;
        let names = names_of(name, names)?;
        if names.len() > 1 && (new_name.is_some() || a.is_some() || b.is_some()) {
            return Err(ApiError(
                "several annotations change together without a new name or picks".to_owned(),
            ));
        }
        let color = color
            .map(|c| normalized_color(&c))
            .transpose()
            .map_err(ApiError)?;
        let picks_change = a.is_some() || b.is_some();
        // The kind each shows afterwards, checked against its picks: one
        // they do not have any more is replaced by their first.
        let mut kinds = Vec::with_capacity(names.len());
        for name in &names {
            let current = self.annotation(name)?;
            let new_kind = kind.unwrap_or(current.kind);
            if new_kind != current.kind && (new_kind.is_leader() || current.kind.is_leader()) {
                return Err(ApiError(format!(
                    "{name}: a {0} stays a {0}, a dimension a dimension",
                    current.kind.key()
                )));
            }
            // Under a dimension afterwards, or on a leader again.
            let on_after = match &on {
                Some(on) => on.clone(),
                None => current.on.clone(),
            };
            let leader_again = current.on.is_some() && on_after.is_none();
            // A note's pick or point changed: it must still give a point,
            // as when it was added (another pick drops the point kept in
            // the old pick's coordinates unless a new one is given).
            // So must a frame taken from under its dimension; under one, its
            // pick is only an anchor and nothing is checked.
            if current.kind.is_leader()
                && on_after.is_none()
                && (a.is_some() || at.is_some() || leader_again)
            {
                let mut probe = current.clone();
                if let Some(a) = &a {
                    probe.a = a.clone();
                    probe.at = None;
                }
                if let Some(at) = at {
                    probe.at = at;
                }
                let picks = self
                    .measure_picks(&probe.a, None)
                    .map_err(|e| ApiError(format!("{name}: the picks: {e}")))?;
                self.shape_of(&probe, &picks)
                    .and_then(|_| feature_pick(current.kind, &probe.a, &picks))
                    .map_err(|e| ApiError(format!("{name}: {e}")))?;
            }
            // Under a dimension there is (not itself, not a leader).
            if let Some(Some(under)) = &on {
                self.dimension_to_stand_under(under.trim())
                    .map_err(|e| ApiError(format!("{name}: {e}")))?;
            }
            if !(picks_change || kind.is_some()) {
                kinds.push(new_kind);
                continue;
            }
            let new_a = a.as_ref().unwrap_or(&current.a);
            let new_b = match &b {
                Some(b) => b.as_ref(),
                None => current.b.as_ref(),
            };
            let picks = self
                .measure_picks(new_a, new_b)
                .map_err(|e| ApiError(format!("{name}: the picks: {e}")))?;
            if new_kind.is_leader() {
                kinds.push(new_kind);
                continue;
            }
            let found = values_of(&picks);
            let resolved = if kind.is_some() || found.iter().any(|f| f.kind == new_kind) {
                new_kind
            } else {
                default_kind(&picks, &found).ok_or_else(no_single_value)?
            };
            check_kind(resolved, &found).map_err(|e| ApiError(format!("{name}: {e}")))?;
            kinds.push(resolved);
        }
        let mut index = 0;
        let changed = self.edit_annotations(&names, None, |annotation| {
            let kind = kinds[index];
            index += 1;
            // Another kind or other picks are drawn afresh.
            if kind != annotation.kind || picks_change {
                annotation.line_offset = None;
                annotation.text_offset = None;
                if matches!(
                    kind,
                    MeasureKind::Radius | MeasureKind::Diameter | MeasureKind::Angle
                ) {
                    annotation.plane = None;
                }
                // Between an angle and a length a tolerance means another
                // thing (degrees, millimetres, a fit).
                if kind.is_angle() != annotation.kind.is_angle() {
                    annotation.tolerance = None;
                }
            }
            annotation.kind = kind;
            if let Some(new_name) = &new_name {
                annotation.name.clone_from(new_name);
            }
            if let Some(color) = &color {
                annotation.color.clone_from(color);
            }
            if let Some(group) = &group {
                annotation.group.clone_from(group);
            }
            if let Some(visible) = visible {
                annotation.visible = visible;
            }
            if let Some(a) = &a {
                annotation.a = a.clone();
                // A direction and a point kept in the old pick's
                // coordinates mean nothing in the new one's: dropped unless
                // given again (below).
                if annotation.plane.as_ref().is_some_and(|p| p.pick.is_none()) {
                    annotation.plane = None;
                }
                annotation.at = None;
            }
            if let Some(b) = &b {
                annotation.b.clone_from(b);
            }
            if let Some(plane) = &plane {
                annotation.plane.clone_from(plane);
            }
            if let Some(offset) = line_offset {
                annotation.line_offset = offset;
            }
            if let Some(offset) = text_offset {
                annotation.text_offset = offset;
            }
            if let Some(format) = &format {
                format.apply(&mut annotation.format);
            }
            if let Some(tolerance) = &tolerance {
                annotation.tolerance.clone_from(tolerance);
            }
            if let Some(text) = &text {
                annotation.text = Some(text.clone());
            }
            if let Some(letter) = &letter {
                annotation.letter = Some(letter.trim().to_owned());
            }
            if let Some(frame) = &frame {
                annotation.frame = Some(frame.clone());
            }
            if let Some(on) = &on {
                annotation.on.clone_from(on);
            }
            if let Some(at) = at {
                annotation.at = at;
            }
            Ok(())
        })?;
        Ok(json!({"changed": changed}))
    }

    fn annotations_json(&self, measure_hidden: bool) -> Value {
        let unit = self.parameters().context().default_length_unit;
        let units = Units {
            symbol: unit.symbol(),
            millimetres: unit.millimetres(),
        };
        // Each uses at most two keys, its picks' and its plane's pick's
        // (mitcad#758); all of them stay measured.
        self.side_cache::<MeasureCache>()
            .hold(2 * self.annotations().len());
        Value::Array(
            self.annotations()
                .iter()
                .map(|annotation| self.annotation_json(annotation, units, measure_hidden))
                .collect(),
        )
    }
}

/// The document's methods used only in this module, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
trait AnnotationsLocal<K: Kernel> {
    /// The plane and the places a view suggests for what is not given: a
    /// length's or a note's plane square to the view (a length's turned to
    /// hold its ends; seen along them, square to the view's up; a note's
    /// text along the view's right), kept in the first pick's own
    /// coordinates; a length's dimension line on the view's upper side of
    /// its ends, and sizes of so many pixels.
    fn place_for_view(
        &self,
        annotation: &mut Annotation,
        picks: &MeasuredPicks,
        view: &ViewHint,
    ) -> Result<(), ApiError>;

    /// What a dimension or a leader (a note, a datum, a frame) of these
    /// picks is drawn from; a leader's point as picked is in its pick's own
    /// coordinates.
    fn shape_of(&self, annotation: &Annotation, picks: &MeasuredPicks) -> Result<Shape, String>;

    fn annotation_json(&self, annotation: &Annotation, units: Units, measure_hidden: bool)
    -> Value;

    /// Why `on` names no dimension a frame can stand under: none of that
    /// name, or a note, a datum or a frame (the frame itself among them).
    fn dimension_to_stand_under(&self, on: &str) -> Result<(), String>;

    /// Where a frame under a dimension stands: at that dimension's text, in
    /// its text's frame (`x` along it, `y` up), marked `attached` with the
    /// dimension's name; the application sets the frame just below the
    /// dimension line there; none while that dimension is hidden. Why
    /// not, when the dimension is not there.
    fn attached_drawing(&self, on: &str) -> Result<Option<Value>, String>;

    /// The normal and the text's direction its plane gives, where it has
    /// one: a direction kept, or the normal of the planar face or datum
    /// plane picked for it, as that pick is now.
    fn plane_hint(&self, annotation: &Annotation) -> Result<(Option<Vec3>, Option<Vec3>), String>;
}

impl<K: Kernel> AnnotationsLocal<K> for Document<K> {
    fn place_for_view(
        &self,
        annotation: &mut Annotation,
        picks: &MeasuredPicks,
        view: &ViewHint,
    ) -> Result<(), ApiError> {
        let direction = unit(view.direction)
            .ok_or_else(|| ApiError("a view's direction is not zero".to_owned()))?;
        let up = view.up.and_then(unit);
        let pixel = view.pixel.filter(|p| p.is_finite() && *p > 0.0);
        let kind = annotation.kind;
        let planar = matches!(
            kind,
            MeasureKind::Distance
                | MeasureKind::Offset
                | MeasureKind::AxisDistance
                | MeasureKind::CenterDistance
                | MeasureKind::Note
                | MeasureKind::Datum
                | MeasureKind::Frame
        );
        let transform = self.pick_transform(&annotation.a)?;
        let shape = self.shape_of(annotation, picks).ok();
        if annotation.plane.is_none() && planar {
            let toward = scale(direction, -1.0);
            let normal = match shape {
                Some(Shape::Ends([p, q])) => unit(sub(q, p))
                    .and_then(|d| {
                        square_to(toward, d).or_else(|| up.and_then(|up| square_to(up, d)))
                    })
                    .unwrap_or(toward),
                _ => toward,
            };
            let x = if kind.is_leader() {
                up.and_then(|up| unit(cross(direction, up)))
            } else {
                None
            };
            annotation.plane = Some(AnnotationPlane {
                normal: Some(back(&transform, normal)),
                x: x.map(|x| back(&transform, x)),
                pick: None,
            });
        }
        let (Some(shape), Ok((normal, x))) = (shape, self.plane_hint(annotation)) else {
            return Ok(());
        };
        let drawing = layout(annotation, &shape, normal, x);
        match shape {
            Shape::Ends(_) if annotation.line_offset.is_none() => {
                let size = pixel.map_or(drawing.line_offset, |p| LINE_PIXELS * p);
                let side = match up {
                    Some(up) if dot(drawing.y, up) < 0.0 => -1.0,
                    _ => 1.0,
                };
                annotation.line_offset = Some(side * size);
            }
            Shape::Arc { .. } if annotation.line_offset.is_none() => {
                if let Some(p) = pixel {
                    annotation.line_offset = Some(ARC_PIXELS * p);
                }
            }
            Shape::Circle { radius, .. } if annotation.text_offset.is_none() => {
                if let Some(p) = pixel {
                    let extra = TEXT_PIXELS * p - (CIRCLE_TEXT - 1.0) * radius;
                    if extra > 0.0 {
                        annotation.text_offset = Some([extra, 0.0]);
                    }
                }
            }
            Shape::Note { .. } if annotation.text_offset.is_none() => {
                if let Some(p) = pixel {
                    annotation.text_offset = Some([NOTE_PIXELS[0] * p, NOTE_PIXELS[1] * p]);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn shape_of(&self, annotation: &Annotation, picks: &MeasuredPicks) -> Result<Shape, String> {
        if annotation.kind.is_leader() {
            let at = match annotation.at {
                Some(at) => Some(
                    self.pick_transform(&annotation.a)
                        .map_err(|e| e.to_string())?
                        .apply_point(at),
                ),
                None => None,
            };
            return note_shape(picks, at);
        }
        values_of(picks)
            .iter()
            .find(|f| f.kind == annotation.kind)
            .map(Shape::of)
            .ok_or_else(|| format!("the picks have no {}", annotation.kind.key()))
    }

    fn annotation_json(
        &self,
        annotation: &Annotation,
        units: Units,
        measure_hidden: bool,
    ) -> Value {
        let mut value = serde_json::to_value(annotation).expect("annotations serialize");
        value["visible"] = json!(annotation.visible);
        if annotation.kind.is_leader() {
            // A frame may refer to the drawings' datum labels too: the
            // model's and the drawings' letters are one set (mitcad#745).
            let letters = self.all_datum_letters();
            let datums: Vec<&str> = letters.iter().map(String::as_str).collect();
            value["label"] = leader_label(annotation, units, &datums).to_json();
        }
        // Hidden: not drawn, so not measured unless asked for (mitcad#716);
        // whether its picks are there is known without the kernel. A frame
        // under a dimension is broken only by that dimension.
        if !annotation.visible && !measure_hidden {
            value["measured"] = json!(false);
            let under = annotation.on.is_some() && annotation.kind == MeasureKind::Frame;
            if !under && let Err(error) = self.picks_key(&annotation.a, annotation.b.as_ref()) {
                value["error"] = json!(format!("a pick is not there: {error}"));
            }
            return value;
        }
        // A frame under a dimension: drawn, or broken, by that dimension
        // alone; its own pick (the dimension's first when it was put there)
        // may be stale.
        if let Some(on) = annotation
            .on
            .as_deref()
            .filter(|_| annotation.kind == MeasureKind::Frame)
        {
            match self.attached_drawing(on) {
                Ok(Some(attached)) => value["drawing"] = attached,
                Ok(None) => {} // under a hidden dimension: not drawn either
                Err(error) => value["error"] = json!(error),
            }
            return value;
        }
        let picks = match self.measure_picks(&annotation.a, annotation.b.as_ref()) {
            Ok(picks) => picks,
            Err(error) => {
                value["error"] = json!(format!("a pick is not there: {error}"));
                return value;
            }
        };
        // A datum label or a frame on its leader that stands on a point (a
        // frame whose dimension was deleted, a file): broken, with why.
        if let Err(error) = feature_pick(annotation.kind, &annotation.a, &picks) {
            value["error"] = json!(error);
            return value;
        }
        let mut measured = None;
        if !annotation.kind.is_leader() {
            let found = values_of(&picks);
            let mut values = Map::new();
            for f in &found {
                values.insert(f.kind.key().to_owned(), json!(f.value));
            }
            value["values"] = Value::Object(values);
            match found.iter().find(|f| f.kind == annotation.kind) {
                Some(f) => {
                    value["value"] = json!(f.value);
                    add_drawn(&mut value, &f.drawn);
                    measured = Some(*f);
                }
                None => {
                    value["error"] = json!(format!(
                        "the picks have no {} now",
                        annotation.kind.key().replace('_', " ")
                    ));
                    return value;
                }
            }
        }
        let (normal, x) = match self.plane_hint(annotation) {
            Ok(hint) => hint,
            Err(error) => {
                value["error"] = json!(format!("the plane: {error}"));
                return value;
            }
        };
        let shape = match measured {
            Some(found) => Shape::of(&found),
            None => match self.shape_of(annotation, &picks) {
                Ok(shape) => shape,
                Err(error) => {
                    value["error"] = json!(error);
                    return value;
                }
            },
        };
        let mut drawing = layout(annotation, &shape, normal, x).to_json();
        // A datum label's leader ends in a filled triangle on its feature
        // (ISO 5459), not an arrow.
        if annotation.kind == MeasureKind::Datum {
            drawing["terminator"] = json!("datum");
        }
        value["drawing"] = drawing;
        if let Some(found) = measured {
            let label = dimension_label(annotation, found.value, units);
            if let Some((upper, lower)) = label.deviations {
                value["deviations"] = json!({"upper": upper + 0.0, "lower": lower + 0.0});
            }
            value["label"] = label.to_json();
        }
        value
    }

    fn dimension_to_stand_under(&self, on: &str) -> Result<(), String> {
        match self.annotation(on) {
            Err(_) => Err(format!("there is no dimension {on} to stand under")),
            Ok(dimension) if dimension.kind.is_leader() => Err(format!("{on} is no dimension")),
            Ok(_) => Ok(()),
        }
    }

    fn attached_drawing(&self, on: &str) -> Result<Option<Value>, String> {
        let dimension = self
            .annotation(on)
            .map_err(|_| format!("the dimension {on} is not there"))?;
        if dimension.kind.is_leader() {
            return Err(format!("{on} is no dimension"));
        }
        if !dimension.visible {
            return Ok(None);
        }
        let picks = self
            .measure_picks(&dimension.a, dimension.b.as_ref())
            .map_err(|e| format!("{on}: a pick is not there: {e}"))?;
        let shape = self
            .shape_of(dimension, &picks)
            .map_err(|e| format!("{on}: {e}"))?;
        let (normal, x) = self
            .plane_hint(dimension)
            .map_err(|e| format!("{on}: the plane: {e}"))?;
        let [at, direction, up] = layout(dimension, &shape, normal, x).text;
        let v = |v: Vec3| v.map(|x| x + 0.0);
        Ok(Some(json!({
            "plane": {"origin": v(at), "x": v(direction), "y": v(up),
                      "normal": v(cross(direction, up))},
            "line_offset": 0.0,
            "text_offset": [0.0, 0.0],
            "extension_lines": [],
            "dimension_lines": [],
            "arcs": [],
            "arrows": [],
            "text": {"at": v(at), "direction": v(direction), "up": v(up)},
            "attached": on,
        })))
    }

    fn plane_hint(&self, annotation: &Annotation) -> Result<(Option<Vec3>, Option<Vec3>), String> {
        let Some(plane) = &annotation.plane else {
            return Ok((None, None));
        };
        if let Some(pick) = &plane.pick {
            let picks = self
                .measure_picks(pick, None)
                .map_err(|e| format!("its pick is not there: {e}"))?;
            return match picks.elements[0] {
                Some(Element::Plane { normal, .. }) => Ok((unit(normal), None)),
                _ => Err("its pick is not a planar face or a datum plane".to_owned()),
            };
        }
        // A direction kept in the first pick's own coordinates, which
        // follows it.
        let transform = self
            .pick_transform(&annotation.a)
            .map_err(|e| format!("its pick is not placed: {e}"))?;
        let placed = |v: Option<Vec3>| v.map(|v| transform.apply_vector(v)).and_then(unit);
        Ok((placed(plane.normal), placed(plane.x)))
    }
}

/// A direction of the design in a rigid transform's own coordinates (its
/// inverse is its transpose).
fn back(transform: &Transform, v: Vec3) -> Vec3 {
    std::array::from_fn(|c| (0..3).map(|r| transform.linear[r][c] * v[r]).sum::<f64>() + 0.0)
}

/// Why a datum label or a feature control frame cannot stand on this pick:
/// they mark a feature (a face, an edge, an axis, a plane), not a vertex
/// or a datum point. A note may point anywhere.
fn feature_pick(
    kind: MeasureKind,
    pick: &MeasurePick,
    picks: &MeasuredPicks,
) -> Result<(), String> {
    let what = match kind {
        MeasureKind::Datum => "a datum label",
        MeasureKind::Frame => "a feature control frame",
        _ => return Ok(()),
    };
    let point = pick.vertex.is_some()
        || (pick.datum.is_some() && matches!(picks.elements[0], Some(Element::Point(_))));
    if point {
        return Err(format!(
            "{what} stands on a face, an edge, an axis or a plane, not on a point"
        ));
    }
    Ok(())
}

/// A note's arrow point: a vertex; on a circle or a round face, the point
/// of it nearest where the note was picked (`at`, in the design's
/// coordinates); that point brought onto a planar face's plane or a
/// straight edge or an axis; else where it was picked.
fn note_shape(picks: &MeasuredPicks, at: Option<Vec3>) -> Result<Shape, String> {
    let element = picks.elements[0];
    let single = picks.single.as_ref();
    let normal = match element {
        Some(Element::Plane { normal, .. }) => unit(normal),
        _ => None,
    };
    let anchor = if let Some(point) = single.and_then(|m| m.point) {
        point
    } else if let Some(circle) = single.and_then(|m| m.circle).filter(|c| c.radius > 0.0) {
        let center = circle.center;
        match unit(circle.axis) {
            Some(n) => {
                let foot = at.map_or(center, |p| add_scaled(center, n, dot(sub(p, center), n)));
                let radial = at
                    .and_then(|p| unit(sub(p, foot)))
                    .unwrap_or_else(|| across(n));
                add_scaled(foot, radial, circle.radius)
            }
            // A sphere.
            None => {
                let radial = at.and_then(|p| unit(sub(p, center))).unwrap_or(Z);
                add_scaled(center, radial, circle.radius)
            }
        }
    } else {
        match element {
            Some(Element::Plane { origin, normal: n }) => match (at, unit(n)) {
                (Some(p), Some(n)) => add_scaled(p, n, -dot(sub(p, origin), n)),
                _ => origin,
            },
            Some(Element::Line { start, end }) => {
                let along = sub(end, start);
                let length = dot(along, along);
                match at {
                    Some(p) if length > 0.0 => {
                        let t = (dot(sub(p, start), along) / length).clamp(0.0, 1.0);
                        add_scaled(start, along, t)
                    }
                    _ => scale(add(start, end), 0.5),
                }
            }
            Some(Element::Axis {
                origin, direction, ..
            }) => match (at, unit(direction)) {
                (Some(p), Some(d)) => add_scaled(origin, d, dot(sub(p, origin), d)),
                _ => origin,
            },
            Some(Element::Point(point)) => point,
            None => at.ok_or_else(|| {
                "the note's pick has no point to point at: pick it where the arrow goes".to_owned()
            })?,
        }
    };
    Ok(Shape::Note { anchor, normal })
}

/// `v` without its part along the unit `along`, as a unit vector; none
/// when less than 1e-6 of it is left (it lies nearly along `along`).
fn square_to(v: Vec3, along: Vec3) -> Option<Vec3> {
    unit_above(sub(v, scale(along, dot(v, along))), 1e-6)
}

/// The coordinate axis a length's plane is square to when nothing else
/// says: the one least along the length (Z, then Y, then X when even).
fn default_normal(along: Vec3) -> Vec3 {
    let mut best = Z;
    let mut least = f64::INFINITY;
    for axis in [Z, Y, X] {
        let part = dot(along, axis).abs();
        if part < least - 1e-9 {
            least = part;
            best = axis;
        }
    }
    best
}

/// Lays an annotation out in its plane: `normal` and `x` are what its
/// plane gives (a direction kept or a pick's normal).
fn layout(
    annotation: &Annotation,
    shape: &Shape,
    normal: Option<Vec3>,
    x: Option<Vec3>,
) -> Drawing {
    match *shape {
        Shape::Ends(ends) => layout_length(annotation, ends, normal),
        Shape::Circle {
            center,
            axis,
            radius,
        } => layout_circle(annotation, center, axis, radius),
        Shape::Arc {
            center,
            from,
            to,
            reach,
        } => layout_angle(annotation, center, from, to, reach),
        Shape::Note {
            anchor,
            normal: own,
        } => layout_note(annotation, anchor, normal.or(own), x),
    }
}

/// A length from `p` to `q`: in the plane through them square to the
/// normal (turned square to them), the dimension line parallel to them
/// `line_offset` across (a quarter of the length, at least 1 mm, when not
/// set), extension lines from the ends to it, an arrow at each end pointing
/// out, the text at its middle; text moved past an end has the line run on
/// under it.
fn layout_length(annotation: &Annotation, [p, q]: [Vec3; 2], hint: Option<Vec3>) -> Drawing {
    let length = distance(p, q);
    let along = unit(sub(q, p));
    let normal = match along {
        Some(d) => hint
            .and_then(|n| square_to(n, d))
            .or_else(|| square_to(default_normal(d), d))
            .unwrap_or(Z),
        None => hint.unwrap_or(Z),
    };
    let x = along.unwrap_or_else(|| across(normal));
    let y = cross(normal, x);
    let offset = annotation
        .line_offset
        .unwrap_or((LINE_SHARE * length).max(1.0));
    let a = add_scaled(p, y, offset);
    let b = add_scaled(q, y, offset);
    let middle = scale(add(a, b), 0.5);
    let [u, v] = annotation.text_offset.unwrap_or([0.0, 0.0]);
    let foot = add_scaled(middle, x, u);
    let mut dimension_lines = vec![[a, b]];
    if u.abs() > length / 2.0 {
        dimension_lines.push([if u > 0.0 { b } else { a }, foot]);
    }
    Drawing {
        origin: p,
        x,
        y,
        normal,
        line_offset: offset,
        text_offset: [u, v],
        extension_lines: vec![[p, a], [q, b]],
        dimension_lines,
        arcs: Vec::new(),
        arrows: vec![[a, scale(x, -1.0)], [b, x]],
        center: None,
        text: [add_scaled(foot, y, v), x, y],
    }
}

/// A radius or a diameter in its circle's plane: the line from the centre
/// (a radius) or across the circle (a diameter) towards the text, which
/// stands one and a half radii out unless moved, with an arrow on the rim
/// pointing out (both rims for a diameter) and the line run on to the text
/// outside the circle.
fn layout_circle(annotation: &Annotation, center: Vec3, axis: Vec3, radius: f64) -> Drawing {
    let normal = unit(axis).unwrap_or(Z);
    let u0 = across(normal);
    let w = cross(normal, u0);
    let [u, v] = annotation.text_offset.unwrap_or([0.0, 0.0]);
    let text_at = add_scaled(
        add_scaled(add_scaled(center, u0, CIRCLE_TEXT * radius), u0, u),
        w,
        v,
    );
    let toward = sub(text_at, center);
    let x = square_to(toward, normal).unwrap_or(u0);
    let rim = add_scaled(center, x, radius);
    let mut dimension_lines = Vec::new();
    let mut arrows = vec![[rim, x]];
    if annotation.kind == MeasureKind::Diameter {
        let opposite = add_scaled(center, x, -radius);
        dimension_lines.push([opposite, rim]);
        arrows.push([opposite, scale(x, -1.0)]);
    } else {
        dimension_lines.push([center, rim]);
    }
    if norm(toward) > radius {
        dimension_lines.push([rim, text_at]);
    }
    let y = cross(normal, x);
    Drawing {
        origin: center,
        x: u0,
        y: w,
        normal,
        line_offset: radius,
        text_offset: [u, v],
        extension_lines: Vec::new(),
        dimension_lines,
        arcs: Vec::new(),
        arrows,
        center: Some(center),
        text: [text_at, x, y],
    }
}

/// An angle in the plane of its rays: an arc about the corner from the
/// first ray to the second, `line_offset` out (half the distance to the
/// nearer pick, else 10 mm, when not set), the rays from the corner to it
/// as extension lines, an arrow at each end of the arc pointing out along
/// it, the text on the arc's middle running along it. Parallel picks have
/// no sector: the text at the corner.
fn layout_angle(
    annotation: &Annotation,
    center: Vec3,
    from: Vec3,
    to: Vec3,
    reach: Option<f64>,
) -> Drawing {
    let radius = annotation.line_offset.unwrap_or(reach.unwrap_or(ARC_MM));
    let [u, v] = annotation.text_offset.unwrap_or([0.0, 0.0]);
    let sector = unit(from).zip(unit(to)).and_then(|(f, t)| {
        let angle = dot(f, t).clamp(-1.0, 1.0).acos();
        (angle > 1e-6).then(|| {
            let w = square_to(t, f).unwrap_or_else(|| across(f));
            (f, t, w, angle)
        })
    });
    let Some((f, t, w, angle)) = sector else {
        let x = unit(from).unwrap_or(X);
        let normal = square_to(default_normal(x), x).unwrap_or(Z);
        let y = cross(normal, x);
        return Drawing {
            origin: center,
            x,
            y,
            normal,
            line_offset: radius,
            text_offset: [u, v],
            extension_lines: Vec::new(),
            dimension_lines: Vec::new(),
            arcs: Vec::new(),
            arrows: Vec::new(),
            center: None,
            text: [add_scaled(add_scaled(center, x, u), y, v), x, y],
        };
    };
    let normal = cross(f, w);
    let half = angle / 2.0;
    let middle = add(scale(f, half.cos()), scale(w, half.sin()));
    let along = add(scale(f, -half.sin()), scale(w, half.cos()));
    let start = add_scaled(center, f, radius);
    let end = add_scaled(
        center,
        add(scale(f, angle.cos()), scale(w, angle.sin())),
        radius,
    );
    let end_tangent = add(scale(f, -angle.sin()), scale(w, angle.cos()));
    let text_at = add_scaled(
        add_scaled(add_scaled(center, middle, radius), along, u),
        middle,
        v,
    );
    Drawing {
        origin: center,
        x: along,
        y: middle,
        normal,
        line_offset: radius,
        text_offset: [u, v],
        extension_lines: vec![[center, start], [center, end]],
        dimension_lines: Vec::new(),
        arcs: vec![ArcDrawn {
            center,
            radius,
            from: f,
            to: t,
            normal,
        }],
        arrows: vec![[start, scale(w, -1.0)], [end, end_tangent]],
        center: None,
        text: [text_at, along, middle],
    }
}

/// A note: the leader from the text to its point with an arrow there, in
/// the plane square to the normal (the plane picked or kept, else its
/// pick's face, else XY), the text `text_offset` from the point along and
/// across (10 mm each when not set), running along `x` (the direction
/// kept, else X or Y in the plane).
fn layout_note(
    annotation: &Annotation,
    anchor: Vec3,
    normal: Option<Vec3>,
    x: Option<Vec3>,
) -> Drawing {
    let normal = normal.unwrap_or(Z);
    let x = x
        .and_then(|x| square_to(x, normal))
        .or_else(|| square_to(X, normal))
        .or_else(|| square_to(Y, normal))
        .unwrap_or(X);
    let y = cross(normal, x);
    let [u, v] = annotation.text_offset.unwrap_or([NOTE_MM, NOTE_MM]);
    let text_at = add_scaled(add_scaled(anchor, x, u), y, v);
    let pointing = unit(sub(anchor, text_at)).unwrap_or_else(|| scale(y, -1.0));
    Drawing {
        origin: anchor,
        x,
        y,
        normal,
        line_offset: 0.0,
        text_offset: [u, v],
        extension_lines: Vec::new(),
        dimension_lines: vec![[text_at, anchor]],
        arcs: Vec::new(),
        arrows: vec![[anchor, pointing]],
        center: None,
        text: [text_at, x, y],
    }
}

/// A number with `decimals` decimals, or up to `most` without trailing
/// zeros; never `-0`.
fn fixed(value: f64, decimals: Option<u8>, most: usize) -> String {
    let mut text = match decimals {
        Some(d) => format!("{:.*}", usize::from(d), value),
        None => {
            let mut text = format!("{value:.most$}");
            if text.contains('.') {
                while text.ends_with('0') {
                    text.pop();
                }
                if text.ends_with('.') {
                    text.pop();
                }
            }
            text
        }
    };
    if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
        text.remove(0);
    }
    text
}

/// A deviation with its sign; `0` alone.
fn signed(value: f64, decimals: Option<u8>) -> String {
    let text = fixed(value.abs(), decimals, DEVIATION_DECIMALS);
    if text.chars().all(|c| c == '0' || c == '.') {
        "0".to_owned()
    } else if value < 0.0 {
        format!("-{text}")
    } else {
        format!("+{text}")
    }
}

/// `before` and `rest` with a space between, either may be empty.
fn joined(before: &str, rest: &str) -> String {
    match (before.trim(), rest) {
        ("", rest) => rest.to_owned(),
        (before, "") => before.to_owned(),
        (before, rest) => format!("{before} {rest}"),
    }
}

/// A dimension's value (mm or radians) in words, in the document's length
/// unit (none written for millimetres) or degrees.
fn dimension_label(annotation: &Annotation, value: f64, units: Units) -> Label {
    let format = &annotation.format;
    let angle = annotation.kind.is_angle();
    let millimetres = if units.millimetres > 0.0 {
        units.millimetres
    } else {
        1.0
    };
    // The value shown, a tolerance (mm or degrees) in the unit shown, and
    // the unit written after each.
    let shown = if angle {
        value.to_degrees()
    } else {
        value / millimetres
    };
    let tolerance_scale = if angle { 1.0 } else { 1.0 / millimetres };
    let unit_text = if angle {
        "°".to_owned()
    } else if units.symbol == "mm" {
        String::new()
    } else {
        format!(" {}", units.symbol)
    };
    let deviation_unit = if angle { "°" } else { "" };
    let number = |v: f64| fixed(v, format.decimals, 3);
    // Two deviations with the decimals they need, the same for both (ISO
    // 129), not the value's.
    let pair = |upper: f64, lower: f64, most: usize| {
        let (upper, lower) = (upper * tolerance_scale, lower * tolerance_scale);
        let decimals = needed(upper, most).max(needed(lower, most));
        (
            format!("{}{deviation_unit}", signed(upper, Some(decimals))),
            format!("{}{deviation_unit}", signed(lower, Some(decimals))),
        )
    };
    let prefix = format
        .prefix
        .clone()
        .unwrap_or_else(|| annotation.kind.default_prefix().to_owned());
    let nominal = format!("{prefix}{}{unit_text}", number(shown));
    let mut label = Label::default();
    match (&format.replace, &annotation.tolerance) {
        (Some(replace), _) => replace.clone_into(&mut label.main),
        (None, None) => label.main = nominal,
        (None, Some(Tolerance::Symmetric { value })) => {
            let plus_minus = format!(
                "±{}{deviation_unit}",
                fixed(value * tolerance_scale, None, DEVIATION_DECIMALS)
            );
            label.main = format!("{nominal} {plus_minus}");
            label.tolerance = plus_minus;
            label.deviations = Some((*value, -*value));
        }
        (None, Some(Tolerance::Deviations { upper, lower })) => {
            let (up, down) = pair(*upper, *lower, DEVIATION_DECIMALS);
            label.main = nominal;
            label.upper = Some(up);
            label.lower = Some(down);
            label.deviations = Some((*upper, *lower));
        }
        (None, Some(Tolerance::Limits { upper, lower })) => {
            // The largest and the smallest size with as many decimals as
            // the value and the deviations need.
            let (largest, smallest) = (
                shown + upper * tolerance_scale,
                shown + lower * tolerance_scale,
            );
            let decimals = format
                .decimals
                .unwrap_or_else(|| needed(shown, 3))
                .max(needed(upper * tolerance_scale, DEVIATION_DECIMALS))
                .max(needed(lower * tolerance_scale, DEVIATION_DECIMALS));
            label.main = prefix;
            label.upper = Some(format!(
                "{}{unit_text}",
                fixed(largest, Some(decimals), DEVIATION_DECIMALS)
            ));
            label.lower = Some(format!(
                "{}{unit_text}",
                fixed(smallest, Some(decimals), DEVIATION_DECIMALS)
            ));
            label.deviations = Some((*upper, *lower));
        }
        (
            None,
            Some(Tolerance::Fit {
                code,
                deviations: shown_too,
            }),
        ) => {
            let code = code.trim();
            label.main = format!("{nominal} {code}");
            code.clone_into(&mut label.tolerance);
            // A fit is of a size as written (its range by the value shown,
            // not a hair over it); a radius' is its diameter's, and its
            // deviations are half the diameter's, as a radius.
            // (In millimetres only: rounded in another unit the size would
            // move by up to half its last decimal, a range off.)
            let written = if units.symbol == "mm" {
                fixed(shown, format.decimals, 3)
                    .parse::<f64>()
                    .map_or(value, |v| v * millimetres)
            } else {
                value
            };
            let radius = annotation.kind == MeasureKind::Radius;
            let size = if radius { 2.0 * written } else { written };
            match mitcad_model::iso286::deviations(code, size) {
                Ok((upper, lower)) => {
                    let (upper, lower) = if radius {
                        (upper / 2.0, lower / 2.0)
                    } else {
                        (upper, lower)
                    };
                    label.deviations = Some((upper, lower));
                    if *shown_too {
                        // A radius' halved deviations may need a fifth decimal.
                        let most = DEVIATION_DECIMALS + usize::from(radius);
                        let (up, down) = pair(upper, lower, most);
                        label.upper = Some(up);
                        label.lower = Some(down);
                        label.brackets = true;
                    }
                }
                Err(error) => label.warning = Some(error),
            }
        }
    }
    label.main = joined(&format.before, &label.main);
    format.after.trim().clone_into(&mut label.after);
    let stack = match (&label.upper, &label.lower) {
        (Some(upper), Some(lower)) => {
            let pair = format!("{upper}/{lower}");
            if label.tolerance.is_empty() || label.brackets {
                if label.brackets {
                    label.tolerance = format!("{} ({pair})", label.tolerance);
                } else {
                    label.tolerance.clone_from(&pair);
                }
            }
            if label.brackets {
                format!("({pair})")
            } else {
                pair
            }
        }
        _ => String::new(),
    };
    label.text = joined(&joined(&label.main, &stack), &label.after);
    label
}

/// The decimals a number needs, up to `most`: none for a whole one.
fn needed(value: f64, most: usize) -> u8 {
    let text = fixed(value.abs(), None, most);
    text.split_once('.')
        .map_or(0, |(_, decimals)| u8::try_from(decimals.len()).unwrap_or(0))
}

/// A leader's words: a note's lines, a datum's letter, a feature control
/// frame's cells (`datums`: the letters the design's datum labels have,
/// for a warning on one referred to that is not there).
fn leader_label(annotation: &Annotation, units: Units, datums: &[&str]) -> Label {
    match (annotation.kind, &annotation.frame) {
        (MeasureKind::Datum, _) => {
            let letter = annotation.letter.clone().unwrap_or_default();
            Label {
                text: letter.clone(),
                datum: Some(letter),
                ..Label::default()
            }
        }
        (MeasureKind::Frame, Some(frame)) => frame_label(frame, units, datums),
        _ => note_label(annotation),
    }
}

/// A feature control frame's cells (ISO 1101): the characteristic's
/// symbol; the tolerance, Ø before a cylindrical zone's, the material
/// condition after it, in the document's length unit (none written); each
/// datum referred to, in order. On one line, `|` between them.
fn frame_label(frame: &GeometricTolerance, units: Units, datums: &[&str]) -> Label {
    let per = if units.millimetres > 0.0 {
        units.millimetres
    } else {
        1.0
    };
    let mut tolerance = format!(
        "{}{}",
        if frame.diameter { "Ø" } else { "" },
        fixed(frame.value / per, None, DEVIATION_DECIMALS)
    );
    if let Some(modifier) = frame.modifier {
        tolerance.push(' ');
        tolerance.push_str(modifier.symbol());
    }
    let mut cells = vec![frame.characteristic.symbol().to_owned(), tolerance.clone()];
    cells.extend(frame.datums.iter().cloned());
    let missing: Vec<&str> = frame
        .datums
        .iter()
        .map(String::as_str)
        .filter(|d| !datums.contains(d))
        .collect();
    Label {
        text: cells.join(" | "),
        tolerance,
        warning: (!missing.is_empty()).then(|| format!("no datum label {}", missing.join(", "))),
        frame: cells,
        ..Label::default()
    }
}

/// The `geometric_characteristics` query: each characteristic a feature
/// control frame can have, in ISO 1101's order, with its symbol and its
/// rules: whether it refers to datums (`none`, `optional`, `required`),
/// takes a cylindrical zone (Ø) and a material condition (Ⓜ, Ⓛ). An editor
/// of frames offers them so; `add_annotation` and `edit_annotation` check
/// them.
pub(super) fn geometric_characteristics_json() -> Value {
    Value::Array(
        Characteristic::ALL
            .iter()
            .map(|c| {
                let datums = match c.datums() {
                    DatumNeed::None => "none",
                    DatumNeed::Optional => "optional",
                    DatumNeed::Required => "required",
                };
                json!({"key": c.key(), "symbol": c.symbol(), "datums": datums,
                       "diameter": c.takes_diameter(), "modifier": c.takes_modifier()})
            })
            .collect(),
    )
}

/// A note's text: its lines, and all of them on one line.
fn note_label(annotation: &Annotation) -> Label {
    let lines: Vec<String> = annotation
        .text
        .as_deref()
        .unwrap_or("")
        .lines()
        .map(|line| line.trim_end().to_owned())
        .collect();
    Label {
        text: lines.join(" / "),
        lines,
        ..Label::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} instead of {b}");
    }

    fn near_point(a: Vec3, b: Vec3) {
        for (x, y) in a.into_iter().zip(b) {
            near(x, y);
        }
    }

    fn dimension(kind: MeasureKind) -> Annotation {
        let pick = MeasurePick {
            datum: Some("xy".to_owned()),
            ..MeasurePick::default()
        };
        let mut annotation = Annotation::new("D", kind, "#d9480f", pick.clone());
        if !kind.single() {
            annotation.b = Some(pick);
        }
        annotation
    }

    fn text_of(annotation: &Annotation, value: f64) -> String {
        dimension_label(annotation, value, Units::MM).text
    }

    #[test]
    fn values_are_written_with_their_prefix_decimals_and_own_text() {
        let mut d = dimension(MeasureKind::Diameter);
        assert_eq!(text_of(&d, 20.0), "Ø20");
        assert_eq!(text_of(&d, 12.34567), "Ø12.346");
        d.format.decimals = Some(2);
        assert_eq!(text_of(&d, 20.0), "Ø20.00");
        d.format.prefix = Some(String::new());
        d.format.before = "4 x".to_owned();
        d.format.after = "THRU".to_owned();
        assert_eq!(text_of(&d, 20.0), "4 x 20.00 THRU");
        d.format.replace = Some("SEE NOTE".to_owned());
        d.tolerance = Some(Tolerance::Symmetric { value: 0.1 });
        assert_eq!(text_of(&d, 20.0), "4 x SEE NOTE THRU");
        let r = dimension(MeasureKind::Radius);
        assert_eq!(text_of(&r, 5.0), "R5");
        // Another length unit is written after the value.
        let cm = Units {
            symbol: "cm",
            millimetres: 10.0,
        };
        assert_eq!(dimension_label(&r, 25.0, cm).text, "R2.5 cm");
        // Angles in degrees.
        let mut a = dimension(MeasureKind::Angle);
        assert_eq!(text_of(&a, std::f64::consts::FRAC_PI_2), "90°");
        a.tolerance = Some(Tolerance::Symmetric { value: 0.5 });
        assert_eq!(text_of(&a, std::f64::consts::FRAC_PI_6), "30° ±0.5°");
    }

    #[test]
    fn tolerances_are_written_iso_129_style() {
        let mut d = dimension(MeasureKind::Distance);
        d.tolerance = Some(Tolerance::Symmetric { value: 0.1 });
        let label = dimension_label(&d, 40.0, Units::MM);
        assert_eq!(label.text, "40 ±0.1");
        assert_eq!(label.tolerance, "±0.1");
        assert_eq!(label.deviations, Some((0.1, -0.1)));

        d.tolerance = Some(Tolerance::Deviations {
            upper: 0.2,
            lower: -0.1,
        });
        let label = dimension_label(&d, 40.0, Units::MM);
        assert_eq!(label.main, "40");
        assert_eq!(label.upper.as_deref(), Some("+0.2"));
        assert_eq!(label.lower.as_deref(), Some("-0.1"));
        assert!(!label.brackets);
        assert_eq!(label.text, "40 +0.2/-0.1");
        d.tolerance = Some(Tolerance::Deviations {
            upper: 0.0,
            lower: -0.05,
        });
        assert_eq!(text_of(&d, 40.0), "40 0/-0.05");

        d.tolerance = Some(Tolerance::Limits {
            upper: 0.1,
            lower: -0.1,
        });
        let label = dimension_label(&d, 10.0, Units::MM);
        assert_eq!(label.main, "");
        assert_eq!(label.text, "10.1/9.9");
        assert_eq!(label.tolerance, "10.1/9.9");

        let mut hole = dimension(MeasureKind::Diameter);
        hole.tolerance = Some(Tolerance::Fit {
            code: "H7".to_owned(),
            deviations: false,
        });
        let label = dimension_label(&hole, 20.0, Units::MM);
        assert_eq!(label.text, "Ø20 H7");
        assert_eq!(label.tolerance, "H7");
        let (upper, lower) = label.deviations.unwrap();
        near(upper, 0.021);
        near(lower, 0.0);
        hole.tolerance = Some(Tolerance::Fit {
            code: "g6".to_owned(),
            deviations: true,
        });
        let label = dimension_label(&hole, 20.0, Units::MM);
        assert_eq!(label.text, "Ø20 g6 (-0.007/-0.020)");
        assert!(label.brackets);
        assert_eq!(label.tolerance, "g6 (-0.007/-0.020)");
        // A size the fit codes do not cover: the code with a warning.
        let label = dimension_label(&hole, 600.0, Units::MM);
        assert_eq!(label.text, "Ø600 g6");
        assert!(label.warning.unwrap().contains("500 mm"));
        // A radius' fit is of its diameter's size, half its deviations: R10 H7
        // is Ø20 H7's, as a radius.
        let mut radius = dimension(MeasureKind::Radius);
        radius.tolerance = Some(Tolerance::Fit {
            code: "H7".to_owned(),
            deviations: true,
        });
        assert_eq!(text_of(&radius, 10.0), "R10 H7 (+0.0105/0)");
        // Halved, a radius' deviations may need a fifth decimal.
        radius.tolerance = Some(Tolerance::Fit {
            code: "js6".to_owned(),
            deviations: true,
        });
        assert_eq!(text_of(&radius, 10.0), "R10 js6 (+0.00325/-0.00325)");
        // In inches the range is the measured size's: Ø10 mm (0.394 in as
        // written, 10.0076 mm) stays in the range up to 10 mm.
        let mut inch_hole = dimension(MeasureKind::Diameter);
        inch_hole.tolerance = Some(Tolerance::Fit {
            code: "H7".to_owned(),
            deviations: false,
        });
        let inches = Units {
            symbol: "in",
            millimetres: 25.4,
        };
        let label = dimension_label(&inch_hole, 10.0, inches);
        assert_eq!(label.text, "Ø0.394 in H7");
        let (upper, _) = label.deviations.unwrap();
        near(upper, 0.015);
        // The range by the size as written: a measured 10.0004 shown as 10
        // takes the range up to 10.
        hole.tolerance = Some(Tolerance::Fit {
            code: "H7".to_owned(),
            deviations: true,
        });
        assert_eq!(text_of(&hole, 10.0004), "Ø10 H7 (+0.015/0)");
        hole.format.decimals = Some(4);
        assert_eq!(text_of(&hole, 10.0004), "Ø10.0004 H7 (+0.018/0)");
        hole.format.decimals = None;
        // Tolerances keep their own decimals: none set or none on the value.
        let mut d = dimension(MeasureKind::Distance);
        d.format.decimals = Some(0);
        d.tolerance = Some(Tolerance::Symmetric { value: 0.1 });
        assert_eq!(text_of(&d, 40.0), "40 ±0.1");
        d.tolerance = Some(Tolerance::Limits {
            upper: 0.05,
            lower: -0.1,
        });
        assert_eq!(text_of(&d, 40.0), "40.05/39.90");
        d.format.decimals = Some(2);
        d.tolerance = Some(Tolerance::Deviations {
            upper: 0.1,
            lower: -0.25,
        });
        assert_eq!(text_of(&d, 40.0), "40.00 +0.10/-0.25");
    }

    #[test]
    fn a_frame_writes_its_zone_in_the_document_unit() {
        let frame = GeometricTolerance {
            characteristic: mitcad_model::Characteristic::Position,
            value: 0.254,
            diameter: true,
            modifier: Some(mitcad_model::MaterialCondition::M),
            datums: vec!["A".to_owned(), "C".to_owned()],
        };
        let inches = Units {
            symbol: "in",
            millimetres: 25.4,
        };
        let label = frame_label(&frame, inches, &["A"]);
        assert_eq!(label.text, "⌖ | Ø0.01 Ⓜ | A | C");
        assert_eq!(label.tolerance, "Ø0.01 Ⓜ");
        assert_eq!(label.warning.as_deref(), Some("no datum label C"));
        let millimetres = Units {
            symbol: "mm",
            millimetres: 1.0,
        };
        assert_eq!(
            frame_label(&frame, millimetres, &["A", "C"]).tolerance,
            "Ø0.254 Ⓜ"
        );
    }

    #[test]
    fn a_length_lies_in_its_plane_with_its_line_offset_and_text() {
        let mut d = dimension(MeasureKind::Distance);
        let ends = [[0.0, 0.0, 0.0], [40.0, 0.0, 0.0]];
        // No plane: square to Z (the axis least along X), the line a
        // quarter of the length off along +Y.
        let drawing = layout(&d, &Shape::Ends(ends), None, None);
        near_point(drawing.normal, Z);
        near_point(drawing.x, X);
        near_point(drawing.y, Y);
        near(drawing.line_offset, 10.0);
        assert_eq!(
            drawing.extension_lines,
            vec![
                [[0.0, 0.0, 0.0], [0.0, 10.0, 0.0]],
                [[40.0, 0.0, 0.0], [40.0, 10.0, 0.0]]
            ]
        );
        assert_eq!(
            drawing.dimension_lines,
            vec![[[0.0, 10.0, 0.0], [40.0, 10.0, 0.0]]]
        );
        assert_eq!(
            drawing.arrows,
            vec![
                [[0.0, 10.0, 0.0], [-1.0, 0.0, 0.0]],
                [[40.0, 10.0, 0.0], [1.0, 0.0, 0.0]]
            ]
        );
        near_point(drawing.text[0], [20.0, 10.0, 0.0]);
        // A plane square to Y (the front), the line below, the text moved
        // past the second end: the line runs on under it.
        d.line_offset = Some(-6.0);
        d.text_offset = Some([30.0, 2.0]);
        let drawing = layout(&d, &Shape::Ends(ends), Some([0.0, -1.0, 0.0]), None);
        near_point(drawing.normal, [0.0, -1.0, 0.0]);
        near_point(drawing.y, [0.0, 0.0, 1.0]);
        near_point(drawing.dimension_lines[0][0], [0.0, 0.0, -6.0]);
        assert_eq!(drawing.dimension_lines.len(), 2);
        near_point(drawing.dimension_lines[1][1], [50.0, 0.0, -6.0]);
        near_point(drawing.text[0], [50.0, 0.0, -4.0]);
        // A normal that is not square to the ends is turned square.
        let drawing = layout(&d, &Shape::Ends(ends), Some([1.0, 0.0, 1.0]), None);
        near_point(drawing.normal, Z);
    }

    #[test]
    fn circles_angles_and_notes_are_laid_out_in_their_planes() {
        // A diameter of 10 about Z through (5, 5, 0): across the circle
        // along X (`across` of Z), the text one and a half radii out.
        let d = dimension(MeasureKind::Diameter);
        let circle = Shape::Circle {
            center: [5.0, 5.0, 0.0],
            axis: [0.0, 0.0, 2.0],
            radius: 5.0,
        };
        let drawing = layout(&d, &circle, None, None);
        let x = across(Z);
        near_point(drawing.x, x);
        near_point(drawing.text[0], add_scaled([5.0, 5.0, 0.0], x, 7.5));
        near_point(
            drawing.dimension_lines[0][0],
            add_scaled([5.0, 5.0, 0.0], x, -5.0),
        );
        assert_eq!(drawing.arrows.len(), 2);
        assert_eq!(drawing.center, Some([5.0, 5.0, 0.0]));
        assert_eq!(drawing.dimension_lines.len(), 2, "run on to the text");
        // A radius with its text dragged inside: one arrow, no run-on.
        let mut r = dimension(MeasureKind::Radius);
        r.text_offset = Some([-5.0, 0.0]);
        let drawing = layout(&r, &circle, None, None);
        assert_eq!(drawing.arrows.len(), 1);
        assert_eq!(drawing.dimension_lines.len(), 1);

        // 90 degrees between X and Y at the origin, the arc 8 out.
        let mut a = dimension(MeasureKind::Angle);
        a.line_offset = Some(8.0);
        let arc = Shape::Arc {
            center: [0.0, 0.0, 0.0],
            from: X,
            to: Y,
            reach: Some(3.0),
        };
        let drawing = layout(&a, &arc, None, None);
        near_point(drawing.normal, Z);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        near_point(drawing.text[0], [8.0 * s, 8.0 * s, 0.0]);
        near_point(drawing.arrows[0][0], [8.0, 0.0, 0.0]);
        near_point(drawing.arrows[0][1], [0.0, -1.0, 0.0]);
        near_point(drawing.arrows[1][0], [0.0, 8.0, 0.0]);
        near_point(drawing.arrows[1][1], [-1.0, 0.0, 0.0]);
        near(drawing.arcs[0].radius, 8.0);
        // Without its own radius: the picks' reach.
        a.line_offset = None;
        near(layout(&a, &arc, None, None).arcs[0].radius, 3.0);

        // A note on a point, in the plane square to Y, its text along X.
        let mut note = dimension(MeasureKind::Note);
        note.text = Some("Deburr".to_owned());
        let shape = Shape::Note {
            anchor: [1.0, 2.0, 3.0],
            normal: Some(Z),
        };
        let drawing = layout(&note, &shape, Some([0.0, -1.0, 0.0]), None);
        near_point(drawing.x, X);
        near_point(drawing.y, Z);
        near_point(drawing.text[0], [11.0, 2.0, 13.0]);
        near_point(drawing.arrows[0][0], [1.0, 2.0, 3.0]);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        near_point(drawing.arrows[0][1], [-s, 0.0, -s]);
        // Without a plane of its own: its face's.
        let drawing = layout(&note, &shape, None, None);
        near_point(drawing.normal, Z);
        near_point(drawing.y, Y);
        assert_eq!(note_label(&note).lines, ["Deburr"]);
    }

    #[test]
    fn numbers_are_written_without_minus_zero() {
        assert_eq!(fixed(-0.0001, None, 3), "0");
        assert_eq!(fixed(-0.0001, Some(2), 3), "0.00");
        assert_eq!(fixed(2.5, None, 3), "2.5");
        assert_eq!(fixed(2.0, None, 3), "2");
        assert_eq!(signed(0.0, Some(3)), "0");
        assert_eq!(signed(-0.021, None), "-0.021");
        assert_eq!(signed(0.021, Some(2)), "+0.02");
        assert_eq!(joined(" ", "x"), "x");
        assert_eq!(joined("a", ""), "a");
    }
}
