// SPDX-License-Identifier: MIT
//! A body's display triangulations as bytes for the result store
//! (`geometry/io/include/mitcad/io/mesh_store.hpp`, mitcad#858): what the
//! view's meshing left in a shape, given back to the same shape when the
//! design is opened again.

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/display_mesh.hpp");
        include!("bridge/trycatch.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;

        /// io::triangulation_bytes of a shape meshed for display to the
        /// end (`geometry::Shape::display_meshed`, not a stand-in of the
        /// interactive tier); an error otherwise (a string, so that
        /// megabytes are not pushed byte by byte).
        fn display_mesh_bytes(shape: &Shape) -> Result<UniquePtr<CxxString>>;
        /// io::restore_triangulations, and the shape marked as meshed for
        /// display to the end; the faces given a triangulation (0 when the
        /// shape had them all, or is meshed already). Bytes of another
        /// shape write nothing: an error.
        fn restore_display_mesh(shape: &Shape, data: &[u8]) -> Result<u64>;
    }
}
