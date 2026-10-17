// SPDX-License-Identifier: MIT
//! What a library's `build` block says about a part (`docs/libraries.md`,
//! "BUILD parts"), read leniently: unknown fields are left out, so newer
//! libraries stay readable, and a value is a number of millimetres or a
//! text with its unit (`"30 mm"`, `"0.9 kg"`).

use serde::Deserialize;
use serde_json::Value;

use super::BuildRole;

/// A part's BUILD metadata.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct BuildMeta {
    pub role: BuildRole,
    /// A profile's system.
    pub system: Option<System>,
    /// The systems a connector, nut or cap fits.
    pub systems: Vec<System>,
    /// A cut-to-length part's length.
    pub length: Option<LengthMeta>,
    /// The hole spacing kept from end A (rails, racks).
    pub pitch: Option<Pitch>,
    /// A profile's outer size.
    pub section: Option<Section>,
    pub slot: Option<SlotMeta>,
    pub mass_per_metre: Option<MetaValue>,
    pub frames: Vec<FrameMeta>,
    pub connector: Option<ConnectorMeta>,
    pub panel: Option<PanelMeta>,
    pub linear: Option<LinearMeta>,
}

impl BuildMeta {
    /// The metadata of a source's `build` block.
    pub fn read(build: &Value) -> Result<Self, String> {
        serde_json::from_value(build.clone()).map_err(|e| format!("its build block: {e}"))
    }

    /// The frame of this name, if the part lists it.
    pub fn frame(&self, name: &str) -> Option<&FrameMeta> {
        self.frames.iter().find(|f| f.name == name)
    }

    /// The part's mass per metre in kilograms, if it has one.
    pub fn kilograms_per_metre(&self) -> Option<f64> {
        self.mass_per_metre.as_ref().and_then(MetaValue::kilograms)
    }

    /// Whether a part that lists `systems` fits a profile of `system`
    /// (one without a list fits any).
    pub fn fits(&self, system: Option<&System>) -> bool {
        if self.systems.is_empty() {
            return true;
        }
        match system {
            Some(system) => self.systems.iter().any(|s| s.fits(system)),
            None => false,
        }
    }

    /// The slot's depth less the clearance a panel leaves in it.
    pub fn slot_depth(&self) -> f64 {
        self.slot
            .as_ref()
            .and_then(|s| s.depth.as_ref())
            .and_then(MetaValue::millimetres)
            .unwrap_or(0.0)
    }
}

/// A value of the metadata: a number of millimetres, or a text with its
/// unit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum MetaValue {
    Number(f64),
    Text(String),
}

impl MetaValue {
    /// The value in millimetres (a length); None for a text that is not
    /// one.
    pub fn millimetres(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(text) => crate::expr::evaluate(
                text,
                &(),
                &crate::parameters::default_context(),
                crate::expr::Unit::MM,
            )
            .ok()
            .map(|q| q.value),
        }
    }

    /// A mass in kilograms: a number is kilograms, a text `0.9 kg` or
    /// `900 g`.
    pub fn kilograms(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Text(text) => {
                let text = text.trim();
                let split = text.find(|c: char| c.is_alphabetic()).unwrap_or(text.len());
                let number: f64 = text[..split].trim().replace(',', ".").parse().ok()?;
                match text[split..].trim() {
                    "" | "kg" => Some(number),
                    "g" => Some(number / 1000.0),
                    "t" => Some(number * 1000.0),
                    _ => None,
                }
            }
        }
    }

    /// The text of the value as an expression (`30 mm`, a parameter
    /// name).
    pub fn text(&self) -> String {
        match self {
            Self::Number(value) => format!("{value} mm"),
            Self::Text(text) => text.clone(),
        }
    }
}

/// A slot system: the type (`I`, `B`), the slot width (mm) and the
/// series (the grid, mm).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct System {
    #[serde(rename = "type")]
    pub kind: String,
    pub slot: Option<f64>,
    pub grid: Option<f64>,
}

impl System {
    /// Whether a part made for this system fits a profile of `profile`:
    /// the same type and slot, and the same grid when this one names it.
    pub fn fits(&self, profile: &System) -> bool {
        let same = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            (None, _) => true,
            (Some(_), None) => false,
        };
        self.kind.eq_ignore_ascii_case(&profile.kind)
            && same(self.slot, profile.slot)
            && same(self.grid, profile.grid)
    }

    /// `B8` (`B8/30` with the grid), for messages and answers.
    pub fn label(&self) -> String {
        let number = |v: f64| {
            if v.fract() == 0.0 {
                format!("{v:.0}")
            } else {
                format!("{v}")
            }
        };
        let mut text = self.kind.clone();
        if let Some(slot) = self.slot {
            text.push_str(&number(slot));
        }
        if let Some(grid) = self.grid {
            text.push('/');
            text.push_str(&number(grid));
        }
        text
    }
}

/// A cut-to-length part's length: the source's parameter, its limits and
/// the stock lengths it is sold in.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LengthMeta {
    pub parameter: String,
    pub min: Option<MetaValue>,
    pub max: Option<MetaValue>,
    pub stock: Vec<MetaValue>,
    /// The length the design has, when the library says.
    pub default: Option<MetaValue>,
}

impl LengthMeta {
    /// Why a length is outside the limits, if it is.
    pub fn outside(&self, length: f64) -> Option<String> {
        if !length.is_finite() || length <= 0.0 {
            return Some(format!(
                "the length must be greater than zero, got {length} mm"
            ));
        }
        if let Some(min) = self.min.as_ref().and_then(MetaValue::millimetres)
            && length < min - 1e-9
        {
            return Some(format!(
                "the length {} mm is shorter than the part's shortest, {} mm",
                round(length),
                round(min)
            ));
        }
        if let Some(max) = self.max.as_ref().and_then(MetaValue::millimetres)
            && length > max + 1e-9
        {
            return Some(format!(
                "the length {} mm is longer than the part's longest, {} mm",
                round(length),
                round(max)
            ));
        }
        None
    }

    /// The stock lengths in millimetres, longest first.
    pub fn stock_lengths(&self) -> Vec<f64> {
        let mut out: Vec<f64> = self
            .stock
            .iter()
            .filter_map(MetaValue::millimetres)
            .filter(|l| *l > 0.0)
            .collect();
        out.sort_by(|a, b| b.total_cmp(a));
        out
    }
}

/// A number for messages: at most three decimals, no trailing zeros.
pub(crate) fn round(value: f64) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Pitch {
    pub distance: Option<MetaValue>,
    pub first: Option<MetaValue>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Section {
    pub width: Option<MetaValue>,
    pub height: Option<MetaValue>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SlotMeta {
    pub depth: Option<MetaValue>,
    pub opening: Option<MetaValue>,
    pub inner_width: Option<MetaValue>,
}

/// A mount frame the part lists: a `mount:<name>` joint origin of its
/// design.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FrameMeta {
    pub name: String,
    /// `end`, `slot`, `bore`, `face`, `rail`, `shaft`, `hole`.
    pub kind: String,
    /// What may mate there; anything that fits the system when empty.
    pub accepts: Vec<BuildRole>,
    pub diameter: Option<MetaValue>,
}

impl FrameMeta {
    pub fn accepts(&self, role: BuildRole) -> bool {
        self.accepts.is_empty() || self.accepts.contains(&role)
    }
}

/// A library part named by a connector (a nut, a screw, a panel's clamp).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PartRef {
    pub library: String,
    pub url: String,
    pub rev: String,
    pub component: String,
    pub config: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ConnectorMeta {
    /// `L`, `T`, `cross`, `inline`, `surface`.
    pub junctions: Vec<String>,
    /// `inner_corner`, `outer`, `end`, `through`.
    pub space: String,
    pub mates: Vec<Mate>,
    pub fasteners: Vec<FastenerMeta>,
    pub machining: Vec<Machining>,
    /// The room an inner-corner connector takes.
    pub envelope: Option<Envelope>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Envelope {
    pub x: Option<MetaValue>,
    pub y: Option<MetaValue>,
    pub z: Option<MetaValue>,
}

/// Which frame of the connector mates with which frame kind of the
/// junction's profiles.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Mate {
    pub frame: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct FastenerMeta {
    pub frame: String,
    pub nut: Option<PartRef>,
    pub screw: Option<PartRef>,
    pub washer: Option<PartRef>,
}

/// What a connector needs cut into a profile.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Machining {
    /// `a` (the profile whose end meets) or `b`.
    pub on: String,
    /// `end` or `slot`.
    pub frame: String,
    pub hole: HoleMeta,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct HoleMeta {
    pub diameter: Option<MetaValue>,
    pub depth: Option<MetaValue>,
    pub thread: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PanelMeta {
    /// `slot` or `surface`.
    pub mount: String,
    /// A value, or the source's parameter that sets it.
    pub thickness: Option<MetaValue>,
    /// The parameters that size it; none for a fixed width or height.
    pub width: Option<String>,
    pub height: Option<String>,
    /// Left in the slot's depth, 0.5 mm when left out.
    pub clearance: Option<MetaValue>,
    pub clamp: Option<PartRef>,
}

impl PanelMeta {
    pub fn clearance(&self) -> f64 {
        self.clearance
            .as_ref()
            .and_then(MetaValue::millimetres)
            .unwrap_or(0.5)
    }

    pub fn on_surface(&self) -> bool {
        self.mount == "surface"
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LinearMeta {
    /// `rail`, `carriage`, `shaft`, `bearing`, ...
    pub kind: String,
    pub mate: Option<LinearMate>,
    /// What parts that go together share (a carriage and its rail: `HGW15`).
    pub size: Option<MetaValue>,
}

/// Whether two linear parts' sizes match: the same length, or the same
/// text without case; a part without a size fits any.
pub fn same_size(a: Option<&MetaValue>, b: Option<&MetaValue>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return true;
    };
    match (a.millimetres(), b.millimetres()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a.text().trim().eq_ignore_ascii_case(b.text().trim()),
    }
}

/// How a part moves on another: its frame, the frame kind of the other
/// part, the joint kind and its axis.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct LinearMate {
    pub frame: String,
    pub on: String,
    pub joint: String,
    pub axis: String,
}
