// SPDX-License-Identifier: MIT
//! A sketch's place on its plane (mitcad#787, Redefine Sketch Plane): an
//! offset along the plane's normal, a move in the plane, a rotation about
//! the normal and a flip, kept with the sketch (`placement` in its
//! definition) so that they can be changed again. Each value is a
//! parameter of the sketch (its slots `placement.offset`, `placement.x`,
//! `placement.y` and `placement.angle`), so expressions and other
//! parameters drive them; a value left out is zero.
//!
//! The placement acts on the sketch's base frame, the plane's frame with
//! the sketch's own frame ([`crate::def::FrameDef`]) applied:
//! the origin moves by `x` and `y` along the base's axes and by `offset`
//! along its normal, then the axes turn by `angle` counter-clockwise about
//! the normal, and a flip turns the sketch over (its y axis, and with it
//! the normal, reversed; its x axis kept). The geometry keeps its own 2D
//! coordinates: only the frame moves.

use serde::{Deserialize, Serialize};

use crate::parameters::ParamId;
use crate::profile::SketchFrame;

/// The placement of a sketch on its plane; see the module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SketchPlacement<P = ParamId> {
    /// Along the base frame's normal.
    #[serde(default = "none", skip_serializing_if = "Option::is_none")]
    pub offset: Option<P>,
    /// Along the base frame's x axis.
    #[serde(default = "none", skip_serializing_if = "Option::is_none")]
    pub x: Option<P>,
    /// Along the base frame's y axis.
    #[serde(default = "none", skip_serializing_if = "Option::is_none")]
    pub y: Option<P>,
    /// Counter-clockwise about the base frame's normal.
    #[serde(default = "none", skip_serializing_if = "Option::is_none")]
    pub angle: Option<P>,
    /// The sketch turned over: its normal reversed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip: bool,
}

fn none<P>() -> Option<P> {
    None
}

impl<P> Default for SketchPlacement<P> {
    fn default() -> Self {
        Self {
            offset: None,
            x: None,
            y: None,
            angle: None,
            flip: false,
        }
    }
}

/// The slots of the values, in the order [`SketchPlacement::map_value`]
/// gives them.
pub const SLOTS: [&str; 4] = [
    "placement.offset",
    "placement.x",
    "placement.y",
    "placement.angle",
];

impl<P> SketchPlacement<P> {
    /// Whether it places nothing: no value and no flip.
    pub fn is_empty(&self) -> bool {
        !self.flip && self.values().into_iter().all(Option::is_none)
    }

    /// The values in the order of [`SLOTS`].
    pub fn values(&self) -> [&Option<P>; 4] {
        [&self.offset, &self.x, &self.y, &self.angle]
    }

    pub fn map_value<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<SketchPlacement<Q>, E> {
        let mut map = |slot: &str, value: &Option<P>| -> Result<Option<Q>, E> {
            value.as_ref().map(|v| f(slot, v)).transpose()
        };
        Ok(SketchPlacement {
            offset: map(SLOTS[0], &self.offset)?,
            x: map(SLOTS[1], &self.x)?,
            y: map(SLOTS[2], &self.y)?,
            angle: map(SLOTS[3], &self.angle)?,
            flip: self.flip,
        })
    }
}

impl SketchPlacement {
    /// The values with `value`'s numbers (mm, rad), each checked to be
    /// finite.
    pub fn resolve(
        &self,
        value: &mut dyn FnMut(ParamId) -> Result<f64, String>,
    ) -> Result<Placed, String> {
        let mut number = |what: &str, id: Option<ParamId>| -> Result<f64, String> {
            let Some(id) = id else {
                return Ok(0.0);
            };
            let v = value(id)?;
            if v.is_finite() {
                Ok(v)
            } else {
                Err(format!(
                    "the sketch's {what} on its plane must be a finite number, got {v}"
                ))
            }
        };
        Ok(Placed {
            offset: number("offset", self.offset)?,
            x: number("move along x", self.x)?,
            y: number("move along y", self.y)?,
            angle: number("rotation", self.angle)?,
            flip: self.flip,
        })
    }
}

/// A placement's values in millimetres and radians.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Placed {
    pub offset: f64,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub flip: bool,
}

impl Placed {
    /// The sketch's frame: `base` placed by these values (see the module).
    pub fn apply(&self, base: &SketchFrame) -> SketchFrame {
        let n = base.normal();
        let (bx, by) = (base.x_axis, base.y_axis);
        let origin = std::array::from_fn(|i| {
            base.origin[i] + self.x * bx[i] + self.y * by[i] + self.offset * n[i]
        });
        let (s, c) = self.angle.sin_cos();
        let x_axis = std::array::from_fn(|i| c * bx[i] + s * by[i]);
        let turn = if self.flip { -1.0 } else { 1.0 };
        let y_axis = std::array::from_fn(|i| turn * (c * by[i] - s * bx[i]));
        SketchFrame {
            origin,
            x_axis,
            y_axis,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12)
    }

    /// The XZ frame: x = +X, y = -Z, normal +Y.
    fn xz() -> SketchFrame {
        SketchFrame {
            origin: [1.0, 2.0, 3.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 0.0, -1.0],
        }
    }

    #[test]
    fn nothing_placed_keeps_the_base() {
        assert_eq!(Placed::default().apply(&xz()), xz());
    }

    #[test]
    fn offset_and_move_go_along_the_base_axes() {
        let placed = Placed {
            offset: 5.0,
            x: 10.0,
            y: 20.0,
            ..Placed::default()
        }
        .apply(&xz());
        // 10 along +X, 20 along -Z, 5 along the normal +Y.
        assert!(near(placed.origin, [11.0, 7.0, -17.0]), "{placed:?}");
        assert!(near(placed.x_axis, [1.0, 0.0, 0.0]));
        assert!(near(placed.y_axis, [0.0, 0.0, -1.0]));
    }

    #[test]
    fn a_rotation_turns_x_towards_y_about_the_normal() {
        let placed = Placed {
            angle: std::f64::consts::FRAC_PI_2,
            ..Placed::default()
        }
        .apply(&SketchFrame::XY);
        assert!(near(placed.x_axis, [0.0, 1.0, 0.0]), "{placed:?}");
        assert!(near(placed.y_axis, [-1.0, 0.0, 0.0]), "{placed:?}");
        assert!(near(placed.normal(), [0.0, 0.0, 1.0]));
    }

    #[test]
    fn a_flip_reverses_the_normal_and_keeps_x() {
        let placed = Placed {
            flip: true,
            offset: 2.0,
            ..Placed::default()
        }
        .apply(&SketchFrame::XY);
        assert!(
            near(placed.origin, [0.0, 0.0, 2.0]),
            "the offset before the flip"
        );
        assert!(near(placed.x_axis, [1.0, 0.0, 0.0]));
        assert!(near(placed.y_axis, [0.0, -1.0, 0.0]));
        assert!(near(placed.normal(), [0.0, 0.0, -1.0]));
    }

    #[test]
    fn placements_read_and_write_only_what_they_have() {
        let empty: SketchPlacement<String> = serde_json::from_str("{}").unwrap();
        assert!(empty.is_empty());
        assert_eq!(serde_json::to_string(&empty).unwrap(), "{}");
        let some: SketchPlacement<String> =
            serde_json::from_str(r#"{"offset": "d4", "angle": "d5", "flip": true}"#).unwrap();
        assert_eq!(some.offset.as_deref(), Some("d4"));
        assert_eq!(some.x, None);
        assert!(some.flip);
        let mut slots = Vec::new();
        let _ = some.map_value(&mut |slot, v: &String| {
            slots.push(format!("{slot}={v}"));
            Ok::<_, ()>(v.clone())
        });
        assert_eq!(slots, ["placement.offset=d4", "placement.angle=d5"]);
        assert!(serde_json::from_str::<SketchPlacement<String>>(r#"{"z": 1}"#).is_err());
    }
}
