// SPDX-License-Identifier: MIT
//! The inputs of a feature: the expressions that carry a parameter name
//! (`param_names` of the feature, in this order). This follows the order
//! of SolidCraft's feature walk (`crates/doc/src/walk.rs` and the
//! `#[walk]` attributes of `crates/doc/src/document.rs`; its golden record
//! `feature_walk_tests/golden_exprs.txt`): each kind's input fields in
//! declaration order, an optional field only when present, a plane's
//! offsets and angles outermost first. Placement coordinates (a
//! primitive's corner, a hole's position) are not inputs.

use serde_json::Value;

/// What an expression measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Length,
    Angle,
    Unitless,
}

/// One input: its label in SolidCraft, where it is in the feature's JSON
/// (a JSON pointer) and its expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub label: &'static str,
    pub pointer: String,
    pub expr: String,
    pub kind: Kind,
}

struct Walk<'a> {
    root: &'a Value,
    out: Vec<Input>,
}

impl Walk<'_> {
    fn expr(&mut self, pointer: &str, label: &'static str, kind: Kind) {
        if let Some(e) = self.root.pointer(pointer).and_then(expr_text) {
            self.out.push(Input {
                label,
                pointer: pointer.to_owned(),
                expr: e,
                kind,
            });
        }
    }

    /// A plane reference at `pointer`: its offsets and angles, outermost
    /// first.
    fn plane(&mut self, pointer: &str) {
        let mut at = pointer.to_owned();
        for _ in 0..64 {
            let Some(p) = self.root.pointer(&at) else {
                return;
            };
            match p.get("type").and_then(Value::as_str) {
                Some("offset") => {
                    self.expr(&format!("{at}/distance"), "Offset", Kind::Length);
                    at = format!("{at}/base");
                }
                Some("at_angle") => {
                    self.expr(&format!("{at}/angle"), "Angle", Kind::Angle);
                    at = format!("{at}/base");
                }
                Some("along_path") => {
                    return self.expr(&format!("{at}/t"), "Distance (0 to 1)", Kind::Unitless);
                }
                _ => return,
            }
        }
    }

    /// A construction reference: a plane reference inside it.
    fn geo(&mut self, pointer: &str) {
        if self
            .root
            .pointer(pointer)
            .and_then(|g| g.get("type"))
            .and_then(Value::as_str)
            == Some("plane")
        {
            self.plane(&format!("{pointer}/plane"));
        }
    }

    fn extent(&mut self, p: &str) {
        self.expr(&format!("{p}/distance"), "Distance", Kind::Length);
        self.expr(&format!("{p}/distance2"), "Distance 2", Kind::Length);
        self.expr(&format!("{p}/start_offset"), "Start offset", Kind::Length);
        self.expr(&format!("{p}/taper"), "Taper angle", Kind::Angle);
        self.expr(&format!("{p}/to_offset"), "To offset", Kind::Length);
    }

    fn count(&self, pointer: &str) -> usize {
        self.root
            .pointer(pointer)
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    }
}

/// An expression as SolidCraft stores it: a string, or a plain number.
pub fn expr_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// The inputs of a feature (its JSON, with `type`), in the order of its
/// `param_names`.
pub fn inputs(feature: &Value) -> Vec<Input> {
    use Kind::{Angle, Length, Unitless};
    let mut w = Walk {
        root: feature,
        out: Vec::new(),
    };
    let kind = feature.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "sketch" | "construction_plane" | "split" | "split_face" | "surface_trim" => {
            w.plane("/plane")
        }
        "mirror" => w.plane("/plane"),
        "replace_face" => w.plane("/target"),
        "extrude" => {
            w.extent("/extent");
            w.expr("/thin/thickness", "Thickness", Length);
            w.expr("/thin/thickness2", "Thickness 2", Length);
        }
        "surface_extrude" => w.extent("/extent"),
        "revolve" | "surface_revolve" => {
            w.expr("/angle", "Angle", Angle);
            w.expr("/angle2", "Angle 2", Angle);
        }
        "fillet" => {
            w.expr("/radius", "Radius", Length);
            match feature.pointer("/style/type").and_then(Value::as_str) {
                Some("variable") => {
                    w.expr("/style/radius2", "End radius", Length);
                    for k in 0..w.count("/style/mid") {
                        w.expr(&format!("/style/mid/{k}/1"), "Mid radius", Length);
                    }
                }
                Some("spline") => {
                    let n = w.count("/style/radii");
                    for k in 0..n {
                        let label = if k + 1 == n {
                            "End radius"
                        } else {
                            "Control radius"
                        };
                        w.expr(&format!("/style/radii/{k}"), label, Length);
                    }
                }
                _ => {}
            }
        }
        "chamfer" => {
            w.expr("/distance", "Distance", Length);
            w.expr("/distance2", "Distance 2", Length);
            w.expr("/angle", "Angle", Angle);
        }
        "box" => {
            w.expr("/length", "Length", Length);
            w.expr("/width", "Width", Length);
            w.expr("/height", "Height", Length);
        }
        "cylinder" => {
            w.expr("/radius", "Radius", Length);
            w.expr("/height", "Height", Length);
        }
        "sphere" => w.expr("/radius", "Radius", Length),
        "torus" => {
            w.expr("/major", "Major radius", Length);
            w.expr("/minor", "Minor radius", Length);
        }
        "pattern" => match feature.pointer("/pattern/type").and_then(Value::as_str) {
            Some("rectangular") => {
                w.expr("/pattern/count1", "Quantity", Unitless);
                w.expr("/pattern/spacing1", "Spacing", Length);
                w.expr("/pattern/count2", "Quantity 2", Unitless);
                w.expr("/pattern/spacing2", "Spacing 2", Length);
            }
            Some("circular") => {
                w.expr("/pattern/count", "Quantity", Unitless);
                w.expr("/pattern/angle", "Total angle", Angle);
            }
            Some("path") => {
                w.expr("/pattern/count", "Quantity", Unitless);
                w.expr("/pattern/spacing", "Distance", Length);
            }
            _ => {}
        },
        "hole" => {
            w.expr("/diameter", "Diameter", Length);
            w.expr("/depth", "Depth", Length);
            match feature.pointer("/hole/type").and_then(Value::as_str) {
                Some("drilled") => w.expr("/hole/tip_angle", "Tip angle", Angle),
                Some("counterbore") => {
                    w.expr("/hole/cb_diameter", "Counterbore diameter", Length);
                    w.expr("/hole/cb_depth", "Counterbore depth", Length);
                }
                Some("countersink") => {
                    w.expr("/hole/cs_diameter", "Countersink diameter", Length);
                    w.expr("/hole/cs_angle", "Countersink angle", Angle);
                }
                _ => {}
            }
        }
        "thread" => w.expr("/length", "Length", Length),
        "sweep" | "surface_sweep" => {
            w.expr("/twist", "Twist", Angle);
            w.expr("/scale", "Scale", Unitless);
        }
        "shell" => w.expr("/thickness", "Thickness", Length),
        "draft" => {
            w.expr("/angle", "Angle", Angle);
            w.plane("/neutral");
        }
        "construction_axis" => {
            for key in ["face", "point", "a", "b", "edge"] {
                w.geo(&format!("/def/{key}"));
            }
        }
        "construction_point" => {
            for key in ["point", "a", "b", "c", "of", "edge", "plane", "path"] {
                w.geo(&format!("/def/{key}"));
            }
            w.expr("/def/t", "Distance (0 to 1)", Unitless);
        }
        "scale" => {
            w.expr("/factor", "Scale factor", Unitless);
            w.expr("/factors/0", "X scale", Unitless);
            w.expr("/factors/1", "Y scale", Unitless);
            w.expr("/factors/2", "Z scale", Unitless);
        }
        "offset_face" => w.expr("/distance", "Distance", Length),
        "bounding_solid" => {
            if feature.get("margin").is_some() {
                w.expr("/margin", "Margin", Length);
            } else {
                // `margin` defaults to "0", which the walk visits.
                w.out.push(Input {
                    label: "Margin",
                    pointer: "/margin".to_owned(),
                    expr: "0".to_owned(),
                    kind: Length,
                });
            }
        }
        "pipe" => {
            w.expr("/diameter", "Diameter", Length);
            w.expr("/wall", "Wall", Length);
        }
        "emboss" => w.expr("/depth", "Depth", Length),
        "rib" => {
            w.expr("/thickness", "Thickness", Length);
            w.expr("/depth", "Depth", Length);
        }
        "coil" => {
            w.expr("/diameter", "Diameter", Length);
            w.expr("/pitch", "Pitch", Length);
            w.expr("/turns", "Revolutions", Unitless);
            w.expr("/section_size", "Section size", Length);
            w.expr("/start_angle", "Start angle", Angle);
        }
        "sheet_contour" => w.expr("/distance", "Distance", Length),
        "sheet_flange" => {
            w.expr("/height", "Height", Length);
            w.expr("/angle", "Angle", Angle);
            w.expr("/radius", "Bend radius", Length);
            for k in 0..w.count("/edges") {
                w.expr(&format!("/edges/{k}/angle"), "Edge angle", Angle);
                w.expr(&format!("/edges/{k}/height"), "Edge height", Length);
            }
            w.expr("/corner_relief", "Corner relief", Length);
        }
        "sheet_hem" => {
            w.expr("/length", "Length", Length);
            w.expr("/gap", "Gap", Length);
        }
        "sheet_perforate" => {
            w.expr("/diameter", "Diameter", Length);
            w.expr("/pitch", "Pitch", Length);
            w.expr("/margin", "Margin", Length);
        }
        "sheet_fold" => {
            w.expr("/angle", "Angle", Angle);
            w.expr("/radius", "Bend radius", Length);
        }
        "boss" => {
            w.expr("/diameter", "Diameter", Length);
            w.expr("/height", "Height", Length);
            w.expr("/hole_diameter", "Hole diameter", Length);
            w.expr("/hole_depth", "Hole depth", Length);
            w.expr("/draft", "Draft angle", Angle);
            w.expr("/fillet", "Root fillet", Length);
            w.expr("/ribs", "Ribs", Unitless);
            w.expr("/rib_thickness", "Rib thickness", Length);
            w.expr("/rib_length", "Rib length", Length);
            w.expr("/rib_offset", "Rib offset", Length);
        }
        "lip" => {
            w.expr("/width", "Width", Length);
            w.expr("/height", "Height", Length);
            w.expr("/gap", "Gap", Length);
        }
        "snap_fit" => {
            w.expr("/length", "Length", Length);
            w.expr("/thickness", "Thickness", Length);
            w.expr("/width", "Width", Length);
            w.expr("/catch_depth", "Catch depth", Length);
            w.expr("/catch_length", "Catch length", Length);
        }
        "rest" => {
            w.expr("/width", "Width", Length);
            w.expr("/height", "Height", Length);
            w.expr("/length", "Length", Length);
            w.expr("/draft", "Draft angle", Angle);
            w.expr("/thickness", "Wall thickness", Length);
        }
        "stitch" => w.expr("/tolerance", "Tolerance", Length),
        "thicken" => w.expr("/thickness", "Thickness", Length),
        "surface_offset" => {
            w.expr("/distance", "Distance", Length);
            w.expr("/tolerance", "Tolerance", Length);
        }
        "surface_fair" => w.expr("/tolerance", "Tolerance", Length),
        "surface_extend" => {
            w.expr("/distance", "Distance", Length);
            w.plane("/up_to/plane");
        }
        "surface_ruled" => {
            w.expr("/distance", "Distance", Length);
            w.expr("/angle", "Angle", Angle);
        }
        "move" => {
            w.expr("/translate/0", "X distance", Length);
            w.expr("/translate/1", "Y distance", Length);
            w.expr("/translate/2", "Z distance", Length);
            w.expr("/angle", "Angle", Angle);
        }
        "mesh_convert" => w.expr("/tolerance", "Tolerance", Length),
        _ => {}
    }
    w.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn labels(v: Value) -> Vec<&'static str> {
        inputs(&v).into_iter().map(|i| i.label).collect()
    }

    /// The orders of SolidCraft's golden record for a few kinds.
    #[test]
    fn inputs_follow_the_feature_walk() {
        let plane = json!({"type": "at_angle", "angle": "a", "axis_origin": [0, 0, 0], "axis_dir": [1, 0, 0],
            "base": {"type": "offset", "distance": "o", "base": {"type": "construction", "name": "CP"}}});
        assert_eq!(
            labels(json!({"type": "sketch", "plane": plane})),
            ["Angle", "Offset"]
        );
        assert_eq!(
            labels(
                json!({"type": "extrude", "extent": {"distance": "1", "taper": "2", "direction": "positive"},
                "thin": {"curves": [], "thickness": "3"}})
            ),
            ["Distance", "Taper angle", "Thickness"]
        );
        assert_eq!(
            labels(
                json!({"type": "sheet_flange", "height": "h", "angle": "a", "radius": "r",
                "edges": [{"point": [0, 0, 0], "angle": "e"}, {"point": [0, 0, 0], "height": "f"}, [0, 0, 0]],
                "corner_relief": "c"})
            ),
            [
                "Height",
                "Angle",
                "Bend radius",
                "Edge angle",
                "Edge height",
                "Corner relief"
            ]
        );
        assert_eq!(
            labels(
                json!({"type": "hole", "diameter": 5, "depth": "d", "hole": {"type": "countersink", "cs_diameter": "x", "cs_angle": "y"}})
            ),
            [
                "Diameter",
                "Depth",
                "Countersink diameter",
                "Countersink angle"
            ]
        );
        assert_eq!(
            labels(json!({"type": "bounding_solid", "bodies": []})),
            ["Margin"]
        );
        assert_eq!(labels(json!({"type": "combine"})), Vec::<&str>::new());
    }
}
