// SPDX-License-Identifier: MIT
//! Thin extrusions along chains of sketch curves (mitcad#463;
//! `ExtrudeFeatureSpec::chains` in
//! `geometry/include/mitcad/geometry/extrude.hpp`).

#[cxx::bridge(namespace = "mitcad::bridge")]
pub mod ffi {
    unsafe extern "C++" {
        include!("bridge/extrude_curves.hpp");
        include!("bridge/trycatch.hpp");

        #[namespace = "mitcad::geometry"]
        type Shape = crate::kernel::shape::ffi::Shape;
        type ShapeList = crate::kernel::shape::ffi::ShapeList;
        type Frame = crate::kernel::profile::ffi::Frame;
        type ExtrudeFeatureInput = crate::kernel::profile_feature::ffi::ExtrudeFeatureInput;
        type SweepCurve = crate::kernel::sweeps::ffi::SweepCurve;

        fn extrude_curves(
            feature: &str,
            frame: &Frame,
            input: &ExtrudeFeatureInput,
            curves: &[SweepCurve],
            chains: &[usize],
            keys: &[String],
            bodies: &ShapeList,
        ) -> Result<SharedPtr<Shape>>;
    }
}

use cxx::SharedPtr;
use mitcad_model::{EntityUid, ExtrudeFeatureSpec, KernelError, PathCurve, RegionKey, SegmentKey};

use super::{Shape, failed, profile, profile_feature, sweeps};

/// The key of a chain's caps: its curves as a region key (`r{c1,c2}`).
fn key(chain: &[PathCurve]) -> Result<String, KernelError> {
    let segments = chain
        .iter()
        .map(|p| {
            EntityUid::parse_curve(&p.name)
                .map(SegmentKey::closed)
                .map_err(|e| KernelError::failed(format!("thin wall curve {}: {e}", p.name)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    RegionKey::new(segments)
        .map(|k| k.to_string())
        .ok_or_else(|| KernelError::failed("a chain of thin wall curves is empty"))
}

pub fn extrude_curves(
    spec: &ExtrudeFeatureSpec<'_, SharedPtr<Shape>>,
    chains: &[Vec<PathCurve>],
) -> Result<SharedPtr<Shape>, KernelError> {
    let mut bodies = profile_feature::Bodies::default();
    let input = profile_feature::extrude_input(spec, &mut bodies)?;
    let curves: Vec<sweeps::ffi::SweepCurve> = chains.iter().flatten().map(sweeps::curve).collect();
    let counts: Vec<usize> = chains.iter().map(Vec::len).collect();
    let keys = chains
        .iter()
        .map(|c| key(c))
        .collect::<Result<Vec<_>, _>>()?;
    ffi::extrude_curves(
        &spec.feature.to_string(),
        &profile::frame(&spec.frame),
        &input,
        &curves,
        &counts,
        &keys,
        bodies.list(),
    )
    .map_err(failed)
}
