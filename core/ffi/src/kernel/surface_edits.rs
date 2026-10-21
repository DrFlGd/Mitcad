// SPDX-License-Identifier: MIT
//! Surface modelling: Trim, Untrim, Unstitch and Reverse Normal
//! (`geometry/include/mitcad/geometry/surface_edits.hpp`, mitcad#455).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// A trim's input besides its tool: with `by_curves` the chains
    /// (`chains[i]` curves each) swept along `direction` instead of the
    /// tool; the keep points x0, y0, z0, x1, ...
    struct TrimData {
        by_curves: bool,
        chains: Vec<usize>,
        direction: [f64; 3],
        extend: bool,
        keep: Vec<f64>,
    }

    unsafe extern "C++" {
        include!("bridge/surface_edits.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type ShapeList = crate::kernel::shape::ffi::ShapeList;
        type Frame = crate::kernel::profile::ffi::Frame;
        type Region = crate::kernel::profile::ffi::Region;
        type ToolSpec = crate::kernel::faceops::ffi::ToolSpec;
        type SweepCurve = crate::kernel::sweeps::ffi::SweepCurve;

        /// The surface body cut by the tool (or the curves), the pieces
        /// the points pick kept.
        fn trim_surface(
            body: &Shape,
            tool: &ToolSpec,
            tool_shapes: &ShapeList,
            frame: &Frame,
            regions: &[Region],
            curves: &[SweepCurve],
            data: &TrimData,
        ) -> Result<SharedPtr<Shape>>;
        /// The face restored towards its surface.
        fn untrim_surface(
            feature: &str,
            body: &Shape,
            face: &str,
            holes: bool,
            outer: bool,
            in_place: bool,
        ) -> Result<SharedPtr<Shape>>;
        /// What is left (first; empty when nothing is) and each face taken
        /// out.
        fn unstitch(body: &Shape, faces: &[String]) -> Result<UniquePtr<ShapeList>>;
        /// The faces turned over.
        fn reverse_faces(body: &Shape, faces: &[String]) -> Result<SharedPtr<Shape>>;
    }
}
