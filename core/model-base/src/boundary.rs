// SPDX-License-Identifier: MIT
//! Inputs of the kernel's bodies built from their boundaries and from
//! triangles (the IFC import's breps and face sets, mitcad#273;
//! [`crate::Kernel::boundary_body`], [`crate::Kernel::triangle_body`]).
//!
//! A boundary body is a neutral B-rep: vertices, curves, surfaces, edges
//! that run along increasing parameter of their curve from their first
//! vertex to their second, and faces whose loops of edges (each used
//! forward or reversed) have the face on their left seen from the side the
//! face's normal points to (the surface's own normal, or its opposite when
//! `reversed`). The first loop of a face is its outer one. Faces that share
//! an edge are joined there; a shell whose every edge two faces share is
//! closed and becomes a solid. Millimetres and radians.

use serde::{Deserialize, Serialize};

/// A curve. Lines are parameterised by length from `origin` along the
/// unit vector `dir`; ellipses (circles when `major == minor`) by the angle
/// from `major_dir` towards `normal × major_dir`; B-splines over their
/// knots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoundaryCurve {
    Line {
        origin: [f64; 3],
        dir: [f64; 3],
    },
    Ellipse {
        center: [f64; 3],
        normal: [f64; 3],
        major_dir: [f64; 3],
        major: f64,
        minor: f64,
    },
    /// Distinct knots with their multiplicities; empty weights when not
    /// rational.
    BSpline {
        degree: u32,
        knots: Vec<f64>,
        multiplicities: Vec<u32>,
        poles: Vec<[f64; 3]>,
        weights: Vec<f64>,
    },
}

/// A surface. Its own normal: the plane's `normal`; away from the axis for
/// cones (cylinders when `half_angle` is 0; the radius is the one at
/// `origin`, growing along `axis` for a positive angle); outwards for
/// spheres and tori; ∂u × ∂v for B-splines (poles in rows along u, each row
/// along v), d(curve) × `dir` for extrusions and the turned curve's
/// ∂u × ∂v for revolutions (u about the axis).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoundarySurface {
    Plane {
        origin: [f64; 3],
        normal: [f64; 3],
        u_dir: [f64; 3],
    },
    Cone {
        origin: [f64; 3],
        axis: [f64; 3],
        ref_dir: [f64; 3],
        radius: f64,
        half_angle: f64,
    },
    Sphere {
        center: [f64; 3],
        axis: [f64; 3],
        ref_dir: [f64; 3],
        radius: f64,
    },
    Torus {
        center: [f64; 3],
        axis: [f64; 3],
        ref_dir: [f64; 3],
        major: f64,
        minor: f64,
    },
    BSpline {
        u_degree: u32,
        v_degree: u32,
        u_knots: Vec<f64>,
        u_multiplicities: Vec<u32>,
        v_knots: Vec<f64>,
        v_multiplicities: Vec<u32>,
        poles: Vec<Vec<[f64; 3]>>,
        /// Empty when not rational, else one per pole.
        weights: Vec<Vec<f64>>,
    },
    Extrusion {
        curve: BoundaryCurve,
        dir: [f64; 3],
    },
    Revolution {
        curve: BoundaryCurve,
        origin: [f64; 3],
        axis: [f64; 3],
    },
}

/// An edge: the part of curve `curve` between the parameters `range`
/// (increasing), from vertex `vertices[0]` to `vertices[1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundaryEdge {
    pub curve: usize,
    pub range: [f64; 2],
    pub vertices: [usize; 2],
}

/// A face: its surface, whether its normal is the surface's opposite, and
/// its loops of edges (index, used forward), the outer one first. A face
/// without loops is its whole surface (a B-spline surface).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryFace {
    pub surface: usize,
    pub reversed: bool,
    pub loops: Vec<Vec<(usize, bool)>>,
}

/// A body's boundary (see the module docs).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BoundaryBody {
    pub vertices: Vec<[f64; 3]>,
    pub curves: Vec<BoundaryCurve>,
    pub surfaces: Vec<BoundarySurface>,
    pub edges: Vec<BoundaryEdge>,
    pub faces: Vec<BoundaryFace>,
    /// The faces of each shell.
    pub shells: Vec<Vec<usize>>,
}

/// What [`crate::Kernel::boundary_body`] made of a boundary.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BoundaryReport {
    /// Every shell became a solid.
    pub solid: bool,
    /// The kernel's checker accepts the body.
    pub valid: bool,
    pub faces: usize,
    /// Faces, edges or geometry the kernel could not build (left out).
    pub left_out: usize,
    pub messages: Vec<String>,
}

impl BoundaryBody {
    /// Checks the indices and the shapes of the parts: an error names the
    /// first that does not fit.
    pub fn check(&self) -> Result<(), String> {
        for (i, e) in self.edges.iter().enumerate() {
            if e.curve >= self.curves.len() || e.vertices.iter().any(|v| *v >= self.vertices.len())
            {
                return Err(format!("edge {i} refers to a missing curve or vertex"));
            }
            if e.range[0] > e.range[1] || e.range.iter().any(|t| !t.is_finite()) {
                return Err(format!("edge {i}: its range decreases or is not finite"));
            }
        }
        for (i, f) in self.faces.iter().enumerate() {
            if f.surface >= self.surfaces.len() {
                return Err(format!("face {i} refers to a missing surface"));
            }
            if f.loops
                .iter()
                .flatten()
                .any(|(e, _)| *e >= self.edges.len())
            {
                return Err(format!("face {i} refers to a missing edge"));
            }
        }
        if self.shells.iter().flatten().any(|f| *f >= self.faces.len()) {
            return Err("a shell refers to a missing face".to_owned());
        }
        Ok(())
    }
}
