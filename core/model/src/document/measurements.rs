// SPDX-License-Identifier: MIT
//! Measurement objects (mitcad#649): measurements kept in the document,
//! listed in the browser's Measurements folder, drawn over the view as
//! dimensions and saved in the design file. Each has a name (`Distance1`,
//! `Angle1`, ...), a colour, a light bulb, the kind of value it shows and
//! one or two picks.
//!
//! A pick refers to geometry as the `measure` query's selections do: a
//! body with a face, edge or vertex by its topological name, or a datum,
//! each with the occurrence that places it. The value is worked out at the
//! timeline marker whenever it is asked for (`api/measurements.rs`), so it
//! follows the model; a pick that is gone makes the object broken, and the
//! `measurements` query says why instead of giving a value.
//!
//! They change no geometry: adding, renaming, recolouring, showing, hiding,
//! changing the kind and deleting one is an undo step that recomputes
//! nothing.

use serde::{Deserialize, Serialize};

use super::{Document, ModelError, invalid};
use crate::features::OccurrencePath;
use crate::ids::BodyUid;
use crate::kernel::Kernel;
use crate::topo::{EdgeName, FaceName, VertexName};

/// The colours new measurements take in turn (sRGB), well apart from each
/// other and from the bodies' default grey.
pub const MEASUREMENT_COLORS: [&str; 6] = [
    "#d9480f", // orange
    "#1971c2", // blue
    "#2f9e44", // green
    "#c2255c", // magenta
    "#7048e8", // violet
    "#e67700", // amber
];

/// A measurement kept in the document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub name: String,
    /// The value it shows.
    pub kind: MeasureKind,
    /// `#rrggbb`, lower case.
    pub color: String,
    /// Drawn over the view; left out of files when true.
    #[serde(default = "shown", skip_serializing_if = "is_shown")]
    pub visible: bool,
    pub a: MeasurePick,
    /// The second pick: every kind but a radius or a diameter has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<MeasurePick>,
}

fn shown() -> bool {
    true
}

fn is_shown(visible: &bool) -> bool {
    *visible
}

/// What a measurement shows: of two picks their closest distance, the
/// angle between them, the offset of two parallel planes, the distance of
/// axes, or of two circles' centres (mitcad#646, mitcad#647); of one pick
/// with a circle (a round edge or face) its radius or diameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasureKind {
    Distance,
    Angle,
    Offset,
    AxisDistance,
    CenterDistance,
    Radius,
    Diameter,
}

impl MeasureKind {
    pub const ALL: [Self; 7] = [
        Self::Distance,
        Self::Angle,
        Self::Offset,
        Self::AxisDistance,
        Self::CenterDistance,
        Self::Radius,
        Self::Diameter,
    ];

    /// Its name in commands and files (`axis_distance`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Distance => "distance",
            Self::Angle => "angle",
            Self::Offset => "offset",
            Self::AxisDistance => "axis_distance",
            Self::CenterDistance => "center_distance",
            Self::Radius => "radius",
            Self::Diameter => "diameter",
        }
    }

    /// The first part of default names (`Distance1`, `AxisDistance1`).
    pub fn name_stem(self) -> &'static str {
        match self {
            Self::Distance => "Distance",
            Self::Angle => "Angle",
            Self::Offset => "Offset",
            Self::AxisDistance => "AxisDistance",
            Self::CenterDistance => "CenterDistance",
            Self::Radius => "Radius",
            Self::Diameter => "Diameter",
        }
    }

    /// Whether it is measured on one pick (a radius or a diameter) rather
    /// than between two.
    pub fn single(self) -> bool {
        matches!(self, Self::Radius | Self::Diameter)
    }

    /// Whether its value is an angle (radians) rather than a length (mm).
    pub fn is_angle(self) -> bool {
        self == Self::Angle
    }
}

/// A pick of a measurement, as the `measure` query takes a selection given
/// as fields: a body with an optional face, edge or vertex, or a datum
/// (`F5`, `xy`), each with the occurrence that places it in the design
/// (a path from the root, `"O1/O4"`; left out in the root, or for the
/// owner's only placement).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurePick {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyUid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<EdgeName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex: Option<VertexName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<OccurrencePath>,
}

impl MeasurePick {
    /// Why the pick is no pick, if it is not one (not its geometry).
    fn check(&self) -> Result<(), String> {
        let names = [
            self.face.is_some(),
            self.edge.is_some(),
            self.vertex.is_some(),
        ]
        .iter()
        .filter(|named| **named)
        .count();
        match (&self.body, &self.datum) {
            (Some(_), None) if names <= 1 => Ok(()),
            (None, Some(datum)) if names == 0 && !datum.trim().is_empty() => Ok(()),
            _ => {
                Err("a pick is a body with an optional face, edge or vertex, or a datum".to_owned())
            }
        }
    }
}

/// A colour as `#rrggbb` in lower case, or why the text is not one.
pub(crate) fn normalized_color(text: &str) -> Result<String, String> {
    let text = text.trim();
    let hex = text.strip_prefix('#').unwrap_or("");
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(format!("#{}", hex.to_ascii_lowercase()))
    } else {
        Err(format!("a colour is \"#rrggbb\", got '{text}'"))
    }
}

impl Measurement {
    /// Why the measurement cannot be kept, if it cannot (not its geometry).
    pub(crate) fn check(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("the measurement needs a name".to_owned());
        }
        let color = normalized_color(&self.color).map_err(|e| format!("{}: {e}", self.name))?;
        if color != self.color {
            return Err(format!("{}: the colour is written \"{color}\"", self.name));
        }
        self.a.check().map_err(|e| format!("{}: {e}", self.name))?;
        match (&self.b, self.kind.single()) {
            (Some(b), false) => b.check().map_err(|e| format!("{}: {e}", self.name)),
            (None, true) => Ok(()),
            (Some(_), true) => Err(format!(
                "{}: a {} is measured on one pick",
                self.name,
                self.kind.key()
            )),
            (None, false) => Err(format!(
                "{}: a {} is measured between two picks",
                self.name,
                self.kind.key().replace('_', " ")
            )),
        }
    }
}

/// A measurement to add: what [`Document::add_measurement`] takes once the
/// API has measured it (`api/measurements.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct NewMeasurement {
    pub kind: MeasureKind,
    pub a: MeasurePick,
    pub b: Option<MeasurePick>,
    /// The next free `<Kind><n>` when none.
    pub name: Option<String>,
    /// The next of [`MEASUREMENT_COLORS`] when none.
    pub color: Option<String>,
}

impl<K: Kernel> Document<K> {
    /// The measurements in the order they were added.
    pub fn measurements(&self) -> &[Measurement] {
        &self.state.measurements
    }

    /// The measurement of this name.
    pub fn measurement(&self, name: &str) -> Result<&Measurement, ModelError> {
        Ok(&self.state.measurements[self.measurement_index(name)?])
    }

    /// The next free default name of a kind: `Distance1`, `Distance2`, ...
    pub fn next_measurement_name(&self, kind: MeasureKind) -> String {
        let stem = kind.name_stem();
        (1..)
            .map(|n| format!("{stem}{n}"))
            .find(|name| self.measurement_index(name).is_err())
            .expect("a free name")
    }

    /// Adds a measurement, shown; an undo step `Add Distance1` that
    /// recomputes nothing. Returns its name. Its picks are not checked
    /// against the model here: the API measures them first.
    pub fn add_measurement(&mut self, new: NewMeasurement) -> Result<String, ModelError> {
        let name = match new.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(name) => name.to_owned(),
            None => self.next_measurement_name(new.kind),
        };
        if self.measurement_index(&name).is_ok() {
            return Err(invalid(format!("a measurement '{name}' exists")));
        }
        let color = match new.color.as_deref() {
            Some(color) => normalized_color(color).map_err(invalid)?,
            None => MEASUREMENT_COLORS[self.state.measurements.len() % MEASUREMENT_COLORS.len()]
                .to_owned(),
        };
        let measurement = Measurement {
            name: name.clone(),
            kind: new.kind,
            color,
            visible: true,
            a: new.a,
            b: new.b,
        };
        measurement.check().map_err(invalid)?;
        let label = format!("Add {name}");
        self.apply_with(|state| {
            state.measurements.push(measurement);
            Ok((label, (), false))
        })?;
        Ok(name)
    }

    pub fn rename_measurement(&mut self, name: &str, new_name: &str) -> Result<(), ModelError> {
        let i = self.measurement_index(name)?;
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err(invalid("the measurement needs a name"));
        }
        if new_name == name {
            return Ok(());
        }
        if self.measurement_index(new_name).is_ok() {
            return Err(invalid(format!("a measurement '{new_name}' exists")));
        }
        let label = format!("Rename {name}");
        let new_name = new_name.to_owned();
        self.apply_with(|state| {
            state.measurements[i].name = new_name;
            Ok((label, (), false))
        })
    }

    /// Gives a measurement another colour (`#rrggbb`); an undo step
    /// `Colour of Distance1` unless nothing changes.
    pub fn set_measurement_color(&mut self, name: &str, color: &str) -> Result<(), ModelError> {
        let i = self.measurement_index(name)?;
        let color = normalized_color(color).map_err(invalid)?;
        if self.state.measurements[i].color == color {
            return Ok(());
        }
        let label = format!("Colour of {name}");
        self.apply_with(|state| {
            state.measurements[i].color = color;
            Ok((label, (), false))
        })
    }

    /// Shows or hides a measurement; an undo step `Show Distance1` /
    /// `Hide Distance1` unless nothing changes.
    pub fn set_measurement_visible(&mut self, name: &str, visible: bool) -> Result<(), ModelError> {
        let i = self.measurement_index(name)?;
        if self.state.measurements[i].visible == visible {
            return Ok(());
        }
        let label = format!("{} {name}", if visible { "Show" } else { "Hide" });
        self.apply_with(|state| {
            state.measurements[i].visible = visible;
            Ok((label, (), false))
        })
    }

    /// Changes the kind of value a measurement shows; an undo step `Kind of
    /// Distance1` unless nothing changes. The API checks first that its
    /// picks have that value.
    pub fn set_measurement_kind(
        &mut self,
        name: &str,
        kind: MeasureKind,
    ) -> Result<(), ModelError> {
        let i = self.measurement_index(name)?;
        let old = &self.state.measurements[i];
        if old.kind == kind {
            return Ok(());
        }
        let mut changed = old.clone();
        changed.kind = kind;
        changed.check().map_err(invalid)?;
        let label = format!("Kind of {name}");
        self.apply_with(|state| {
            state.measurements[i] = changed;
            Ok((label, (), false))
        })
    }

    pub fn delete_measurement(&mut self, name: &str) -> Result<(), ModelError> {
        let i = self.measurement_index(name)?;
        let label = format!("Delete {name}");
        self.apply_with(|state| {
            state.measurements.remove(i);
            Ok((label, (), false))
        })
    }

    fn measurement_index(&self, name: &str) -> Result<usize, ModelError> {
        self.state
            .measurements
            .iter()
            .position(|m| m.name == name)
            .ok_or_else(|| invalid(format!("there is no measurement '{name}'")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_six_hex_digits_in_lower_case() {
        assert_eq!(normalized_color(" #A1b2C3 ").unwrap(), "#a1b2c3");
        for wrong in ["a1b2c3", "#a1b2c", "#a1b2c3d", "#g1b2c3", "red", ""] {
            assert!(normalized_color(wrong).is_err(), "{wrong}");
        }
        for color in MEASUREMENT_COLORS {
            assert_eq!(normalized_color(color).unwrap(), color);
        }
    }

    #[test]
    fn kinds_have_their_keys_and_stems() {
        for kind in MeasureKind::ALL {
            let json = serde_json::to_value(kind).unwrap();
            assert_eq!(json, serde_json::json!(kind.key()));
            assert_eq!(serde_json::from_value::<MeasureKind>(json).unwrap(), kind);
        }
        assert_eq!(MeasureKind::AxisDistance.name_stem(), "AxisDistance");
        assert!(MeasureKind::Radius.single() && MeasureKind::Diameter.single());
        assert!(!MeasureKind::Offset.single());
    }

    #[test]
    fn a_measurement_has_the_picks_its_kind_needs() {
        let pick = MeasurePick {
            body: Some("F2.b0".parse().unwrap()),
            ..MeasurePick::default()
        };
        let mut m = Measurement {
            name: "Distance1".to_owned(),
            kind: MeasureKind::Distance,
            color: "#d9480f".to_owned(),
            visible: true,
            a: pick.clone(),
            b: None,
        };
        assert!(m.check().unwrap_err().contains("between two picks"));
        m.b = Some(MeasurePick {
            datum: Some("xy".to_owned()),
            ..MeasurePick::default()
        });
        m.check().unwrap();
        m.kind = MeasureKind::Radius;
        assert!(m.check().unwrap_err().contains("on one pick"));
        m.b = None;
        m.check().unwrap();
        m.a = MeasurePick::default();
        assert!(m.check().unwrap_err().contains("a pick is a body"));
        m.a = pick;
        m.color = "#D9480F".to_owned();
        assert!(m.check().unwrap_err().contains("written"));
    }
}
