// SPDX-License-Identifier: MIT
//! How a new feature's tool lies against bodies: Extrude's automatic
//! operation (`geometry/include/mitcad/geometry/contact.hpp`, mitcad#289).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/contact.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type ShapeList = crate::kernel::shape::ffi::ShapeList;

        /// One per body: 0 apart, 1 touching or overlapping, 2 the tool's
        /// inside in the body.
        fn tool_contacts(tool: &Shape, bodies: &ShapeList) -> Result<Vec<u8>>;
    }
}
