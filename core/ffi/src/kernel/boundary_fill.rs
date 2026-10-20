// SPDX-License-Identifier: MIT
//! Boundary Fill: the cells tool bodies enclose
//! (`geometry/include/mitcad/geometry/boundary_fill.hpp`, mitcad#457).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/boundary_fill.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type ShapeList = crate::kernel::shape::ffi::ShapeList;

        /// The cells the points pick, in the order of the points.
        type BoundaryCells;

        /// The cell each point (x, y, z in turn) picks among the tools.
        fn boundary_cells(tools: &ShapeList, points: &[f64]) -> Result<UniquePtr<BoundaryCells>>;
        fn cell_count(self: &BoundaryCells) -> usize;
        fn cell(self: &BoundaryCells, index: usize) -> SharedPtr<Shape>;
        /// The tools that hold the cell's point, in increasing order.
        fn cell_holders(self: &BoundaryCells, index: usize) -> Vec<usize>;
    }
}
