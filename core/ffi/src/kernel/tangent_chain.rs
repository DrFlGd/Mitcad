// SPDX-License-Identifier: MIT
//! The tangent chains of a body's edges and faces, for the hover
//! (mitcad#290; `geometry/include/mitcad/geometry/tangent_chain.hpp`).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// The names of one chain's edges or faces (a face by its first name).
    struct TangentChain {
        names: Vec<String>,
    }

    unsafe extern "C++" {
        include!("bridge/tangent_chain.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;

        /// The chains of two or more named edges (`faces` false) or faces.
        fn tangent_chains(shape: &Shape, faces: bool) -> Result<Vec<TangentChain>>;
    }
}
