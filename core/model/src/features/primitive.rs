// SPDX-License-Identifier: MIT
//! Primitives (Box, Cylinder, Sphere and Torus): a solid placed
//! on a plane at a point in the plane's coordinates (the frame a sketch on
//! the plane has, see [`super::geom_ref`]), sized by parameters, that
//! becomes a new body or joins, cuts or intersects bodies like an
//! extrusion. Faces are `<feature>:bottom` (on the plane), `top` and
//! `side<i>`: a box's sides follow its base rectangle counter-clockwise
//! from the side along the plane's x axis (side0 at the corner's y, side1
//! at x + length, …), a cylinder's, sphere's and torus's curved face is
//! `side0`. The height of a box or a cylinder goes along the plane's normal
//! (against it when negative), or both ways with `symmetric` (experiment
//! K59 asks the defaults of .f3d designs). `at` places a primitive by
//! picked geometry instead (mitcad#656, `place_at.rs`): its frame is the
//! plane, its origin the position (a box's corner).

use serde::{Deserialize, Serialize};

use super::bodies::{BodySet, apply_tool, boolean_op, verb};
use super::geom_ref::{GeomRef, Want};
use super::mapping::map_params;
use super::place_at::{PlaceAt, map_at};
use super::{
    CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, Operation, References,
    ToolUse, is_false,
};
use mitcad_vec3::{add, scale as scaled};

use crate::ids::{BodyUid, FeatureUid};
use crate::kernel::Kernel;
use crate::parameters::ParamId;
use crate::transform::{PrimitiveShape, PrimitiveSpec, Transform};

/// Where a torus lies relative to its diameter (Position).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TorusPosition {
    /// The ring inside the diameter.
    Inside,
    /// The ring's centre line on the diameter.
    #[default]
    OnCenter,
    /// The ring outside the diameter.
    Outside,
}

fn is_on_center(position: &TorusPosition) -> bool {
    *position == TorusPosition::OnCenter
}

/// A coordinate of a primitive's position on its plane (mitcad#689): a
/// plain number, as documents and imports have always written it, or a
/// value like the sizes: an expression or a parameter (`"-L/2"`, `"W"`),
/// which becomes a parameter of the feature and follows its changes. A
/// number stays a number, so stored documents load as they were.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged, expecting = "a number or an expression")]
pub enum Coord<P = ParamId> {
    Number(f64),
    Value(P),
}

impl<P> Default for Coord<P> {
    fn default() -> Self {
        Self::Number(0.0)
    }
}

/// A position left out: the origin. (A named default, so that serde asks no
/// `Default` of the parameter type, as `#[serde(default)]` would.)
fn origin<P>() -> [Coord<P>; 2] {
    [Coord::Number(0.0), Coord::Number(0.0)]
}

impl Coord {
    /// The coordinate in millimetres, reading its parameter.
    fn read<K: Kernel>(&self, ctx: &mut EvalContext<'_, K>) -> Result<f64, String> {
        match self {
            Self::Number(value) => Ok(*value),
            Self::Value(id) => ctx.param(*id),
        }
    }

    /// The coordinate by `value` (a parameter's value).
    fn value(&self, value: &dyn Fn(ParamId) -> f64) -> f64 {
        match self {
            Self::Number(number) => *number,
            Self::Value(id) => value(*id),
        }
    }
}

/// Maps a position's values: slots `<field>.x` and `<field>.y`; a plain
/// number has no slot.
fn map_point<P, Q, E>(
    field: &str,
    point: &[Coord<P>; 2],
    f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
) -> Result<[Coord<Q>; 2], E> {
    let mut one = |axis: &str, coord: &Coord<P>| -> Result<Coord<Q>, E> {
        Ok(match coord {
            Coord::Number(number) => Coord::Number(*number),
            Coord::Value(value) => Coord::Value(f(&format!("{field}.{axis}"), value)?),
        })
    };
    let x = one("x", &point[0])?;
    let y = one("y", &point[1])?;
    Ok([x, y])
}

fn map_corner<P, Q, E>(
    corner: &[Coord<P>; 2],
    f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
) -> Result<[Coord<Q>; 2], E> {
    map_point("corner", corner, f)
}

fn map_center<P, Q, E>(
    center: &[Coord<P>; 2],
    f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
) -> Result<[Coord<Q>; 2], E> {
    map_point("center", center, f)
}

/// Checks a position's values: finite.
fn check_point(
    what: &str,
    point: &[Coord; 2],
    value: &dyn Fn(ParamId) -> f64,
) -> Result<(), String> {
    if point.iter().all(|coord| coord.value(value).is_finite()) {
        Ok(())
    } else {
        Err(format!("the {what}'s position must be finite"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoxDef<P = ParamId> {
    /// None with `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<GeomRef>,
    /// The first corner, in plane coordinates.
    #[serde(default = "origin")]
    pub corner: [Coord<P>; 2],
    /// Along the plane's x axis.
    pub length: P,
    /// Along the plane's y axis.
    pub width: P,
    pub height: P,
    #[serde(default, skip_serializing_if = "is_false")]
    pub symmetric: bool,
    pub operation: Operation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
    /// Placed by picked geometry instead of `plane` and the position
    /// (mitcad#656, `place_at.rs`).
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub at: Option<PlaceAt<P>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CylinderDef<P = ParamId> {
    /// None with `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<GeomRef>,
    /// The centre of the base, in plane coordinates.
    #[serde(default = "origin")]
    pub center: [Coord<P>; 2],
    pub diameter: P,
    pub height: P,
    #[serde(default, skip_serializing_if = "is_false")]
    pub symmetric: bool,
    pub operation: Operation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
    /// Placed by picked geometry instead of `plane` and the position
    /// (mitcad#656, `place_at.rs`).
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub at: Option<PlaceAt<P>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SphereDef<P = ParamId> {
    /// None with `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<GeomRef>,
    #[serde(default = "origin")]
    pub center: [Coord<P>; 2],
    pub diameter: P,
    pub operation: Operation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
    /// Placed by picked geometry instead of `plane` and the position
    /// (mitcad#656, `place_at.rs`).
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub at: Option<PlaceAt<P>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TorusDef<P = ParamId> {
    /// None with `at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<GeomRef>,
    /// The centre, in plane coordinates; the axis is the plane's normal.
    #[serde(default = "origin")]
    pub center: [Coord<P>; 2],
    /// The diameter the ring lies on (see `position`).
    pub diameter: P,
    /// The diameter of the ring's section.
    pub section_diameter: P,
    #[serde(default, skip_serializing_if = "is_on_center")]
    pub position: TorusPosition,
    pub operation: Operation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<BodyUid>,
    /// Placed by picked geometry instead of `plane` and the position
    /// (mitcad#656, `place_at.rs`).
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub at: Option<PlaceAt<P>>,
}

impl<P> BoxDef<P> {
    pub const TYPE: &'static str = "box";
    pub const BASE_NAME: &'static str = "Box";

    map_params! {
        BoxDef {
            plane: clone,
            corner: with(map_corner),
            length: param,
            width: param,
            height: param,
            symmetric: copy,
            operation: copy,
            participants: clone,
            at: with(map_at),
        }
    }
}

impl<P> CylinderDef<P> {
    pub const TYPE: &'static str = "cylinder";
    pub const BASE_NAME: &'static str = "Cylinder";

    map_params! {
        CylinderDef {
            plane: clone,
            center: with(map_center),
            diameter: param,
            height: param,
            symmetric: copy,
            operation: copy,
            participants: clone,
            at: with(map_at),
        }
    }
}

impl<P> SphereDef<P> {
    pub const TYPE: &'static str = "sphere";
    pub const BASE_NAME: &'static str = "Sphere";

    map_params! {
        SphereDef {
            plane: clone,
            center: with(map_center),
            diameter: param,
            operation: copy,
            participants: clone,
            at: with(map_at),
        }
    }
}

impl<P> TorusDef<P> {
    pub const TYPE: &'static str = "torus";
    pub const BASE_NAME: &'static str = "Torus";

    map_params! {
        TorusDef {
            plane: clone,
            center: with(map_center),
            diameter: param,
            section_diameter: param,
            position: copy,
            operation: copy,
            participants: clone,
            at: with(map_at),
        }
    }
}

/// What the four primitives share: placement, operation and participants.
struct Common<'a> {
    what: &'static str,
    plane: Option<&'a GeomRef>,
    /// Placed by picked geometry (mitcad#656).
    at: Option<&'a PlaceAt>,
    point: &'a [Coord; 2],
    operation: Operation,
    participants: &'a [BodyUid],
}

impl Common<'_> {
    fn references(&self) -> References {
        let mut references = References::default();
        if let Some(plane) = self.plane {
            plane.add_to(&mut references);
        }
        if let Some(at) = self.at {
            at.add_to(&mut references);
        }
        for body in self.participants {
            references.body(*body);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        // On a plane, or placed by picked geometry (mitcad#656).
        match (self.plane, self.at) {
            (Some(plane), None) => plane.check(ctx, Want::Plane)?,
            (None, Some(at)) => {
                if self.point.iter().any(|coord| *coord != Coord::Number(0.0)) {
                    return Err(format!(
                        "a {} placed by at takes its position from the placement",
                        self.what
                    ));
                }
                at.check(ctx)?;
            }
            (None, None) => {
                return Err(format!("a {} needs a plane or a placement (at)", self.what));
            }
            (Some(_), Some(_)) => {
                return Err(format!("a {} placed by at takes no plane", self.what));
            }
        }
        // A value's own number is checked by check_values (mitcad#689).
        if !self.point.iter().all(|coord| match coord {
            Coord::Number(number) => number.is_finite(),
            Coord::Value(_) => true,
        }) {
            return Err(format!("the {}'s position must be finite", self.what));
        }
        if self.operation.makes_bodies() && !self.participants.is_empty() {
            return Err(format!(
                "a new body {} has no participant bodies",
                self.what
            ));
        }
        for (i, body) in self.participants.iter().enumerate() {
            if self.participants[..i].contains(body) {
                return Err(format!("body {body} is listed more than once"));
            }
            ctx.body(*body)?;
        }
        Ok(())
    }

    /// The solid, named after `feature`, on the plane moved by `placement`;
    /// `lift` moves it along the plane's normal.
    fn tool<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
        feature: FeatureUid,
        placement: &Transform,
        shape: PrimitiveShape,
        lift: f64,
    ) -> Result<K::Shape, String> {
        let mut frame = match (self.plane, self.at) {
            // Placed by picked geometry (mitcad#656).
            (_, Some(at)) => ctx.place_def(at)?.frame,
            (Some(plane), None) => {
                let point = [self.point[0].read(ctx)?, self.point[1].read(ctx)?];
                let mut frame = ctx.plane(plane)?.frame();
                frame.origin = frame.point(point);
                frame
            }
            (None, None) => return Err(format!("a {} needs a plane or a placement", self.what)),
        };
        frame.origin = add(frame.origin, scaled(frame.normal(), lift));
        let spec = PrimitiveSpec {
            feature,
            frame: placement.apply_frame(&frame),
            shape,
        };
        ctx.kernel.primitive(&spec).map_err(|e| e.to_string())
    }

    fn evaluate<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
        tool: K::Shape,
    ) -> Result<FeatureOutput<K::Shape>, String> {
        let mut bodies = BodySet::new(if self.operation.makes_bodies() {
            Vec::new()
        } else {
            ctx.bodies()
        });
        if !apply_tool(ctx, &mut bodies, self.operation, self.participants, &tool)? {
            let op = boolean_op(self.operation).expect("new bodies always apply");
            return Err(format!(
                "the {} does not {} any participant body",
                self.what,
                match verb(op) {
                    "join" => "touch",
                    "cut" => "cut into",
                    other => other,
                }
            ));
        }
        Ok(FeatureOutput {
            changes: bodies.changes(),
            tool: Some(tool),
            ..FeatureOutput::default()
        })
    }

    fn tool_use(&self) -> ToolUse {
        ToolUse {
            operation: self.operation,
            participants: self.participants.to_vec(),
        }
    }
}

fn positive(what: &str, value: f64) -> Result<f64, String> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(format!("the {what} must be greater than zero, got {value}"))
    }
}

fn nonzero(what: &str, value: f64) -> Result<f64, String> {
    if value.is_finite() && value != 0.0 {
        Ok(value)
    } else {
        Err(format!("the {what} must not be zero, got {value}"))
    }
}

/// The height along the normal and the lift of the base.
fn height_and_lift(height: f64, symmetric: bool) -> (f64, f64) {
    if symmetric {
        (height.abs(), -height.abs() / 2.0)
    } else if height < 0.0 {
        (-height, height)
    } else {
        (height, 0.0)
    }
}

/// The ring radius of a torus.
fn torus_radii(diameter: f64, section: f64, position: TorusPosition) -> Result<(f64, f64), String> {
    let minor = section / 2.0;
    let major = match position {
        TorusPosition::Inside => diameter / 2.0 - minor,
        TorusPosition::OnCenter => diameter / 2.0,
        TorusPosition::Outside => diameter / 2.0 + minor,
    };
    if major > minor {
        Ok((major, minor))
    } else {
        Err("the torus section is too large for its diameter".to_owned())
    }
}

/// Reads the values of a primitive: its shape and the lift of its base.
trait Primitive {
    fn common(&self) -> Common<'_>;
    fn shape<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
    ) -> Result<(PrimitiveShape, f64), String>;
}

impl Primitive for BoxDef {
    fn common(&self) -> Common<'_> {
        Common {
            what: "box",
            plane: self.plane.as_ref(),
            at: self.at.as_ref(),
            point: &self.corner,
            operation: self.operation,
            participants: &self.participants,
        }
    }

    fn shape<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
    ) -> Result<(PrimitiveShape, f64), String> {
        let length = ctx.positive(self.length)?;
        let width = ctx.positive(self.width)?;
        let height = ctx.param(self.height)?;
        nonzero(&ctx.param_name(self.height), height)?;
        let (height, lift) = height_and_lift(height, self.symmetric);
        Ok((
            PrimitiveShape::Box {
                length,
                width,
                height,
            },
            lift,
        ))
    }
}

impl Primitive for CylinderDef {
    fn common(&self) -> Common<'_> {
        Common {
            what: "cylinder",
            plane: self.plane.as_ref(),
            at: self.at.as_ref(),
            point: &self.center,
            operation: self.operation,
            participants: &self.participants,
        }
    }

    fn shape<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
    ) -> Result<(PrimitiveShape, f64), String> {
        let radius = ctx.positive(self.diameter)? / 2.0;
        let height = ctx.param(self.height)?;
        nonzero(&ctx.param_name(self.height), height)?;
        let (height, lift) = height_and_lift(height, self.symmetric);
        Ok((PrimitiveShape::Cylinder { radius, height }, lift))
    }
}

impl Primitive for SphereDef {
    fn common(&self) -> Common<'_> {
        Common {
            what: "sphere",
            plane: self.plane.as_ref(),
            at: self.at.as_ref(),
            point: &self.center,
            operation: self.operation,
            participants: &self.participants,
        }
    }

    fn shape<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
    ) -> Result<(PrimitiveShape, f64), String> {
        let radius = ctx.positive(self.diameter)? / 2.0;
        Ok((PrimitiveShape::Sphere { radius }, 0.0))
    }
}

impl Primitive for TorusDef {
    fn common(&self) -> Common<'_> {
        Common {
            what: "torus",
            plane: self.plane.as_ref(),
            at: self.at.as_ref(),
            point: &self.center,
            operation: self.operation,
            participants: &self.participants,
        }
    }

    fn shape<K: Kernel>(
        &self,
        ctx: &mut EvalContext<'_, K>,
    ) -> Result<(PrimitiveShape, f64), String> {
        let diameter = ctx.positive(self.diameter)?;
        let section = ctx.positive(self.section_diameter)?;
        let (major_radius, minor_radius) = torus_radii(diameter, section, self.position)?;
        Ok((
            PrimitiveShape::Torus {
                major_radius,
                minor_radius,
            },
            0.0,
        ))
    }
}

/// `FeatureInfo` and `Evaluate` of a primitive type.
macro_rules! primitive_feature {
    ($def:ident, |$self:ident, $value:ident| $check_values:expr) => {
        impl FeatureInfo for $def {
            fn references(&self) -> References {
                self.common().references()
            }

            fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
                self.common().check(ctx)
            }

            fn check_values(&$self, $value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
                $check_values
            }

            fn creates_bodies(&self) -> bool {
                true
            }

            fn tool_use(&self) -> Option<ToolUse> {
                Some(self.common().tool_use())
            }

            fn new_component(&self) -> bool {
                self.common().tool_use().operation == Operation::NewComponent
            }
        }

        impl<K: Kernel> Evaluate<K> for $def {
            fn evaluate(
                &self,
                ctx: &mut EvalContext<'_, K>,
            ) -> Result<FeatureOutput<K::Shape>, String> {
                let (shape, lift) = self.shape(ctx)?;
                let common = self.common();
                let uid = ctx.uid;
                let tool = common.tool(ctx, uid, &Transform::IDENTITY, shape, lift)?;
                common.evaluate(ctx, tool)
            }

            fn placed_tool(
                &self,
                ctx: &mut EvalContext<'_, K>,
                feature: FeatureUid,
                placement: &Transform,
            ) -> Option<Result<K::Shape, String>> {
                // A face found again on the later bodies could lie elsewhere;
                // the pattern moves the finished tool instead.
                // So could a placement's references (mitcad#656).
                let common = self.common();
                if matches!(common.plane, Some(GeomRef::Face { .. })) || common.at.is_some() {
                    return None;
                }
                Some(self.shape(ctx).and_then(|(shape, lift)| {
                    self.common().tool(ctx, feature, placement, shape, lift)
                }))
            }
        }
    };
}

primitive_feature!(BoxDef, |self, value| {
    check_point("box", &self.corner, value)?;
    positive("box length", value(self.length))?;
    positive("box width", value(self.width))?;
    nonzero("box height", value(self.height)).map(|_| ())
});

primitive_feature!(CylinderDef, |self, value| {
    check_point("cylinder", &self.center, value)?;
    positive("cylinder diameter", value(self.diameter))?;
    nonzero("cylinder height", value(self.height)).map(|_| ())
});

primitive_feature!(SphereDef, |self, value| {
    check_point("sphere", &self.center, value)?;
    positive("sphere diameter", value(self.diameter)).map(|_| ())
});

primitive_feature!(TorusDef, |self, value| {
    check_point("torus", &self.center, value)?;
    let diameter = positive("torus diameter", value(self.diameter))?;
    let section = positive("torus section diameter", value(self.section_diameter))?;
    torus_radii(diameter, section, self.position).map(|_| ())
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torus_positions_place_the_ring() {
        assert_eq!(
            torus_radii(40.0, 10.0, TorusPosition::OnCenter),
            Ok((20.0, 5.0))
        );
        assert_eq!(
            torus_radii(40.0, 10.0, TorusPosition::Inside),
            Ok((15.0, 5.0))
        );
        assert_eq!(
            torus_radii(40.0, 10.0, TorusPosition::Outside),
            Ok((25.0, 5.0))
        );
        assert!(torus_radii(10.0, 10.0, TorusPosition::Inside).is_err());
    }

    #[test]
    fn heights_go_both_ways() {
        assert_eq!(height_and_lift(10.0, false), (10.0, 0.0));
        assert_eq!(height_and_lift(-10.0, false), (10.0, -10.0));
        assert_eq!(height_and_lift(10.0, true), (10.0, -5.0));
    }
}
