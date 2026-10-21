// SPDX-License-Identifier: MIT
//! Surface modelling: Offset Surface, Ruled Surface and Extend
//! (`geometry/include/mitcad/geometry/surface_offsets.hpp`, mitcad#455).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// Free edges of an opening: its index among the body's open
    /// boundaries, the faces its wanted edges bound (none: all) and points
    /// x0, y0, z0, x1, ... that pick the nearest of those (none: all).
    struct OpeningData {
        index: i32,
        faces: Vec<String>,
        at: Vec<f64>,
    }

    /// A ruled strip's input: `mode` 0 normal, 1 tangent, 2 angle, 3
    /// direction; `corners` 0 mitre, 1 round; `face` empty for none;
    /// `direction` used when `has_direction`.
    struct RuledData {
        edges: Vec<String>,
        openings: Vec<OpeningData>,
        face: String,
        distance: f64,
        mode: u8,
        angle: f64,
        has_direction: bool,
        direction: [f64; 3],
        flip: bool,
        corners: u8,
    }

    unsafe extern "C++" {
        include!("bridge/surface_offsets.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;

        /// The faces (all when none) offset into a new surface body.
        fn offset_surface(
            feature: &str,
            body: &Shape,
            faces: &[String],
            distance: f64,
            chain: bool,
        ) -> Result<SharedPtr<Shape>>;
        /// A ruled strip off a chain of the body's edges.
        fn ruled_surface(feature: &str, body: &Shape, spec: &RuledData)
        -> Result<SharedPtr<Shape>>;
        /// The body's faces extended past the edges; `mode` 0 natural, 1
        /// tangent, 2 curvature.
        fn extend_surface(
            body: &Shape,
            edges: &[String],
            openings: &[OpeningData],
            distance: f64,
            mode: u8,
        ) -> Result<SharedPtr<Shape>>;
    }
}

use mitcad_model::{ExtendMode, OpeningEdges, RuledCorners, RuledMode, RuledSpec};

/// The bridge's form of free edges of openings.
pub(crate) fn openings(openings: &[OpeningEdges<'_>]) -> Vec<ffi::OpeningData> {
    openings
        .iter()
        .map(|o| ffi::OpeningData {
            index: i32::try_from(o.index).unwrap_or(i32::MAX),
            faces: o.faces.iter().map(ToString::to_string).collect(),
            at: o.at.iter().flatten().copied().collect(),
        })
        .collect()
}

/// The bridge's form of a ruled strip's input.
pub(crate) fn ruled<S>(spec: &RuledSpec<'_, S>) -> ffi::RuledData {
    ffi::RuledData {
        edges: spec.edges.iter().map(ToString::to_string).collect(),
        openings: openings(spec.openings),
        face: spec.face.map(ToString::to_string).unwrap_or_default(),
        distance: spec.distance,
        mode: match spec.mode {
            RuledMode::Normal => 0,
            RuledMode::Tangent => 1,
            RuledMode::Angle => 2,
            RuledMode::Direction => 3,
        },
        angle: spec.angle,
        has_direction: spec.direction.is_some(),
        direction: spec.direction.unwrap_or([0.0, 0.0, 1.0]),
        flip: spec.flip,
        corners: match spec.corners {
            RuledCorners::Mitre => 0,
            RuledCorners::Round => 1,
        },
    }
}

pub(crate) fn extend_mode(mode: ExtendMode) -> u8 {
    match mode {
        ExtendMode::Natural => 0,
        ExtendMode::Tangent => 1,
        ExtendMode::Curvature => 2,
    }
}
