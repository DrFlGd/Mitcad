// SPDX-License-Identifier: MIT
//! Convert Mesh (mitcad#462): a mesh body of one profile extruded straight
//! (a constant-section part from an STL or OBJ file) rebuilt as a solid of
//! planar and cylindrical faces. The section is found within a tolerance by
//! the conversion adapted from SolidCraft
//! (`third_party/solidcraft/mesh_convert.rs`): the extrusion direction among
//! the triangles' normals, the two caps' boundary loops (which must be the
//! same), every side vertex on them; a loop of corners on one circle with
//! its chords within the tolerance is that circle, other loops polygons. A
//! mesh of any other shape is refused and nothing is converted.
//!
//! The solid is the kernel's extrusion of the section, so its faces are
//! named as an extrusion's: `<feature>:side(<segment>)`, `start(<region>)`
//! at the low cap and `end(<region>)` at the high one, the section's edges
//! numbered as curves `c1`, `c2`, ... loop by loop (an outer loop
//! counter-clockwise from its lowest corner, a hole clockwise, a circle one
//! curve). The body keeps its id, name and appearance; the mesh it was
//! stays as the features before this one left it.

#[path = "../../../../third_party/solidcraft/mesh_convert.rs"]
mod prism;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

use super::mapping::map_params;
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
};
use crate::exchange::MeshTolerance;
use crate::ids::{BodyUid, EntityUid};
use crate::kernel::{ExtrudeSpec, Kernel};
use crate::parameters::ParamId;
use crate::profile::{ProfileLoop, ProfileRegion, ProfileSegment, SegmentGeometry};
use crate::topo::{RegionKey, SegmentKey};

pub use prism::{
    MAX_TOLERANCE, MIN_TOLERANCE, Prism, SectionLoop, SectionRegion, convert_prismatic_mesh,
};

/// A mesh body (`body`) rebuilt as planes and cylinders within `tolerance`
/// (mm, 0.000001 to 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeshConvertDef<P = ParamId> {
    pub body: BodyUid,
    pub tolerance: P,
}

impl<P> MeshConvertDef<P> {
    pub const TYPE: &'static str = "mesh_convert";
    pub const BASE_NAME: &'static str = "ConvertMesh";

    map_params! {
        MeshConvertDef {
            body: copy,
            tolerance: param,
        }
    }
}

fn check_tolerance(value: f64) -> Result<(), String> {
    if value.is_finite() && (MIN_TOLERANCE..=MAX_TOLERANCE).contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "the tolerance must be between 0.000001 and 1 mm, got {value}"
        ))
    }
}

impl FeatureInfo for MeshConvertDef {
    fn references(&self) -> References {
        let mut references = References::default();
        references.body(self.body);
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        ctx.body(self.body)
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        check_tolerance(value(self.tolerance))
    }
}

/// A mesh body's triangles come as they are; the meshing tolerance is not
/// used for them.
const AS_STORED: MeshTolerance = MeshTolerance {
    deviation: 0.01,
    angle: 0.1,
    face_triangles: 0,
};

impl<K: Kernel> Evaluate<K> for MeshConvertDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let tolerance = ctx.param(self.tolerance)?;
        check_tolerance(tolerance)?;
        let shape = ctx.body(self.body)?;
        if !ctx.is_mesh(&shape) {
            return Err(format!(
                "body {} (from {}) is not a mesh body; Convert Mesh rebuilds the triangles of a \
                 mesh body (an STL or OBJ file's) as planes and cylinders",
                self.body,
                ctx.feature_name(self.body.feature)
            ));
        }
        let mesh = ctx
            .kernel
            .triangle_mesh(&shape, &AS_STORED)
            .map_err(|e| format!("the mesh's triangles: {e}"))?;
        let prism = convert_prismatic_mesh(&mesh, tolerance)?;
        let regions = profile_regions(&prism);
        let solid = ctx
            .kernel
            .extrude(&ExtrudeSpec {
                feature: ctx.uid,
                frame: prism.frame,
                regions: &regions,
                direction: prism.frame.normal(),
                start: 0.0,
                end: prism.height,
            })
            .map_err(|e| e.to_string())?;
        if matches!(ctx.kernel.is_valid(&solid), Ok(false)) {
            return Err("the rebuilt solid is not valid; no geometry was converted".to_owned());
        }
        // The kernel's solid holds the section's volume (a kernel that does
        // not measure is trusted).
        if let Ok(properties) = ctx.kernel.mass_properties(&solid) {
            let expected = prism.volume();
            if (properties.volume - expected).abs() > 1e-5 * expected.abs() + 1e-9 {
                return Err(format!(
                    "the rebuilt solid's volume {} mm³ is not its section's {expected} mm³; no \
                     geometry was converted",
                    properties.volume
                ));
            }
        }
        Ok(FeatureOutput {
            changes: vec![BodyChange::Set(self.body, solid)],
            ..FeatureOutput::default()
        })
    }
}

/// The section as the kernel's profile regions, its edges numbered as
/// curves `c1`, `c2`, ... loop by loop.
pub(crate) fn profile_regions(prism: &Prism) -> Vec<ProfileRegion> {
    let mut next = 1u32;
    let mut regions = Vec::with_capacity(prism.regions.len());
    for region in &prism.regions {
        let outer = loop_segments(&region.outer, &mut next);
        let key = RegionKey::new(outer.iter().map(|s| s.key.clone())).expect("a loop has segments");
        let mut loops = vec![ProfileLoop { segments: outer }];
        for hole in &region.holes {
            loops.push(ProfileLoop {
                segments: loop_segments(hole, &mut next),
            });
        }
        regions.push(ProfileRegion { key, loops });
    }
    regions
}

/// A loop's segments: a polygon's edges between their neighbours
/// (`c2[c1,c3]`), a circle one closed curve (`c5`).
fn loop_segments(section: &SectionLoop, next: &mut u32) -> Vec<ProfileSegment> {
    match section {
        SectionLoop::Circle { center, radius } => {
            let curve = EntityUid(*next);
            *next += 1;
            vec![ProfileSegment {
                key: SegmentKey::closed(curve),
                geometry: SegmentGeometry::Circle {
                    center: *center,
                    radius: *radius,
                    start: 0.0,
                },
            }]
        }
        SectionLoop::Polygon(points) => {
            let n = points.len();
            let first = *next;
            *next += n as u32;
            let curve = |i: usize| EntityUid(first + (i % n) as u32);
            (0..n)
                .map(|i| ProfileSegment {
                    key: SegmentKey::between(curve(i), curve(i + n - 1), curve(i + 1)),
                    geometry: SegmentGeometry::Line {
                        start: points[i],
                        end: points[(i + 1) % n],
                    },
                })
                .collect()
        }
    }
}
