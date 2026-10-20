// SPDX-License-Identifier: MIT
//! Patch, Stitch and Thicken: surface bodies filled, sewn and made solid
//! (`geometry/include/mitcad/geometry/surfaces.hpp`, mitcad#456).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// An open boundary of a body: the names of the faces its edges bound,
    /// points along it (x0, y0, z0, x1, ...) and whether it closes up.
    struct OpenBoundaryData {
        faces: Vec<String>,
        points: Vec<f64>,
        closed: bool,
    }

    /// The loop a patch fills: the named `edges`, or the body's open
    /// boundary at `opening` when it is not negative. Continuities are 0,
    /// 1 or 2 (G0, G1, G2); `edge_continuity[i]` is that of
    /// `edge_continuity_edges[i]`. Points x0, y0, z0, x1, ...
    struct PatchLoopData {
        edges: Vec<String>,
        opening: i32,
        continuity: u8,
        edge_continuity_edges: Vec<String>,
        edge_continuity: Vec<u8>,
        points: Vec<f64>,
    }

    unsafe extern "C++" {
        include!("bridge/surfaces.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type ShapeList = crate::kernel::shape::ffi::ShapeList;
        type Frame = crate::kernel::profile::ffi::Frame;
        type Region = crate::kernel::profile::ffi::Region;

        /// A surface body per region, in order.
        fn patch_regions(
            feature: &str,
            frame: &Frame,
            regions: &[Region],
        ) -> Result<UniquePtr<ShapeList>>;
        /// The body's open boundaries in their geometric order.
        fn open_boundaries(body: &Shape) -> Result<Vec<OpenBoundaryData>>;
        /// A surface body of one face filling the loop.
        fn patch_loop(
            feature: &str,
            body: &Shape,
            spec: &PatchLoopData,
        ) -> Result<SharedPtr<Shape>>;
        /// The shapes' faces sewn together within `tolerance` (0: the
        /// kernel's own).
        fn stitch(shapes: &ShapeList, tolerance: f64) -> Result<SharedPtr<Shape>>;
        /// The surface body made solid.
        fn thicken(
            feature: &str,
            sheet: &Shape,
            thickness: f64,
            symmetric: bool,
        ) -> Result<SharedPtr<Shape>>;
    }
}

use mitcad_model::{FaceName, KernelError, OpenBoundary, PatchBoundary, PatchContinuity};

pub(crate) fn continuity(g: PatchContinuity) -> u8 {
    match g {
        PatchContinuity::G0 => 0,
        PatchContinuity::G1 => 1,
        PatchContinuity::G2 => 2,
    }
}

/// The bridge's form of a patch's loop.
pub(crate) fn patch_loop<S>(spec: &mitcad_model::PatchLoopSpec<'_, S>) -> ffi::PatchLoopData {
    let (edges, opening) = match spec.boundary {
        PatchBoundary::Edges(edges) => (edges.iter().map(ToString::to_string).collect(), -1),
        PatchBoundary::Opening(index) => (Vec::new(), i32::try_from(index).unwrap_or(i32::MAX)),
    };
    ffi::PatchLoopData {
        edges,
        opening,
        continuity: continuity(spec.continuity),
        edge_continuity_edges: spec
            .edge_continuity
            .iter()
            .map(|(edge, _)| edge.to_string())
            .collect(),
        edge_continuity: spec
            .edge_continuity
            .iter()
            .map(|(_, g)| continuity(*g))
            .collect(),
        points: spec.points.iter().flatten().copied().collect(),
    }
}

/// An open boundary as the model has it.
pub(crate) fn open_boundary(data: ffi::OpenBoundaryData) -> Result<OpenBoundary, KernelError> {
    let faces = data
        .faces
        .iter()
        .map(|name| {
            name.parse::<FaceName>()
                .map_err(|_| KernelError::failed(format!("invalid face name {name}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(OpenBoundary {
        faces,
        points: data.points.as_chunks::<3>().0.to_vec(),
        closed: data.closed,
    })
}
