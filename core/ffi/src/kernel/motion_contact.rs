// SPDX-License-Identifier: MIT
//! Contact sets: how far bodies placed by rigid transforms pass through
//! each other (`geometry/include/mitcad/geometry/motion_contact.hpp`,
//! mitcad#454).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    /// Two shapes of the list, each placed by a rigid transform (rows of
    /// a rotation and a translation, row by row).
    struct OverlapPair {
        a: usize,
        a_placement: [f64; 12],
        b: usize,
        b_placement: [f64; 12],
    }

    unsafe extern "C++" {
        include!("bridge/motion_contact.hpp");

        type ShapeList = crate::kernel::shape::ffi::ShapeList;

        /// The common volume of each pair, mm³ (0 apart or touching within
        /// `tolerance`, mm).
        fn overlap_volumes(
            shapes: &ShapeList,
            pairs: &[OverlapPair],
            tolerance: f64,
        ) -> Result<Vec<f64>>;
    }
}

/// A transform as the bridge's rows.
pub(crate) fn rows(t: &mitcad_model::Transform) -> [f64; 12] {
    std::array::from_fn(|i| {
        let (row, column) = (i / 4, i % 4);
        if column == 3 {
            t.translation[row]
        } else {
            t.linear[row][column]
        }
    })
}
