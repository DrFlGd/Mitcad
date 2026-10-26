// SPDX-License-Identifier: MIT
//! The bodies of a 3MF or STL file on the print bed (mitcad#731,
//! `ExportPlacement::on_bed`): the 3D print export turns them so that a
//! body's print bed face (`document/print_bed.rs`) lies on the bed, its
//! outward normal straight down, puts their lowest point on the bed and
//! centres them on the plate. The design does not move: the export moves
//! the meshes' placements after the design's (a 3MF part's components, an
//! STL file's vertices), and the meshes are the kernel's.
//!
//! The bodies written are one piece, as the 3MF file's one object: they
//! turn together onto the one bed face among them, also when the body is
//! placed several times alike (a component placed again, moved only: the
//! face applies to every occurrence). Bed faces of several bodies, or of
//! placements turned differently, keep the bodies as they are assembled,
//! and so do none. Bodies that are not printed (surface bodies) are left
//! out. A bed face that is gone or no longer planar is reported, and its
//! body is written as the design has it (−Z down) unless another body's
//! face turns the piece.

use crate::datum::Vec3;
use crate::document::{Document, PrintTarget};
use crate::ids::{BodyUid, ComponentUid};
use crate::kernel::Kernel;
use crate::print_keys;
use crate::transform::Transform;

use super::Exported;

/// The plate when the design's printer gives no bed shape, mm: the Bambu
/// family's 256 x 256 mm, the slicer the export writes for by default.
pub const DEFAULT_PLATE: [f64; 2] = [256.0, 256.0];

/// How the export put the bodies on the print bed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OnBed {
    /// The body whose print bed face lies on the bed, and the face (as it
    /// was assigned); None when the bodies are written as they are.
    pub face: Option<(BodyUid, String)>,
    /// What moved the bodies after their placements in the design: the
    /// turn of the face down, then the move of the lowest point onto the
    /// bed (z = 0) and of the middle of the box over the plate's centre.
    pub transform: Option<Transform>,
    /// The plate's centre (x, y), mm: of the design's printer's bed shape,
    /// else of [`DEFAULT_PLATE`].
    pub plate: [f64; 2],
    /// Bodies with bed faces, kept as they are assembled (several faces).
    pub kept: Vec<BodyUid>,
    /// Bed faces that cannot be used, and why.
    pub problems: Vec<(BodyUid, String)>,
}

/// The turn that takes the unit vector `down` straight down (−Z): about
/// the axis square to both, by the angle between them; half a turn about X
/// for straight up.
pub(super) fn turn_down(down: Vec3) -> Transform {
    let target = [0.0, 0.0, -1.0];
    let cos = mitcad_vec3::dot(down, target).clamp(-1.0, 1.0);
    let axis = mitcad_vec3::cross(down, target);
    match Transform::rotation([0.0; 3], axis, cos.acos()) {
        Some(turn) => turn,
        // Along Z already: down, or up.
        None if cos > 0.0 => Transform::IDENTITY,
        None => Transform {
            linear: [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
            translation: [0.0; 3],
        },
    }
}

/// `turn`, then the move that puts the lowest of the turned points on the
/// bed (z = 0) and the middle of their box over `plate`.
pub(super) fn settled(
    turn: &Transform,
    points: impl Iterator<Item = [f64; 3]>,
    plate: [f64; 2],
) -> Transform {
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for point in points {
        let p = turn.apply_point(point);
        min = std::array::from_fn(|i| min[i].min(p[i]));
        max = std::array::from_fn(|i| max[i].max(p[i]));
    }
    if !min.iter().chain(&max).all(|v| v.is_finite()) {
        return *turn;
    }
    let shift = [
        plate[0] - (min[0] + max[0]) / 2.0,
        plate[1] - (min[1] + max[1]) / 2.0,
        -min[2],
    ];
    Transform::translation(shift).after(turn)
}

impl<K: Kernel> Document<K> {
    /// The centre of the design's printer's plate (the box of its bed
    /// shape), else of [`DEFAULT_PLATE`].
    pub(crate) fn plate_centre(&self) -> [f64; 2] {
        let effective = self.effective_print(&PrintTarget::Component(ComponentUid::ROOT));
        match effective.value("bed_shape") {
            Some(print_keys::Value::Points(bed)) if !bed.is_empty() => {
                let (min, max) = print_keys::bounds(&bed);
                [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0]
            }
            _ => [DEFAULT_PLATE[0] / 2.0, DEFAULT_PLATE[1] / 2.0],
        }
    }

    /// Where the export puts the bodies on the bed: their print bed faces
    /// where `placements` place them, and the one body's face that every
    /// placement turns alike (its turn in `transform`, which the writer
    /// completes), or why they stay as they are.
    pub(super) fn bed_plan(
        &self,
        bodies: &[BodyUid],
        placements: &dyn Fn(BodyUid) -> Vec<Transform>,
    ) -> OnBed {
        let mut plan = OnBed {
            plate: self.plate_centre(),
            ..OnBed::default()
        };
        // Each bed face where a placement puts it, by its outward normal;
        // bodies that are not printed (surface bodies) are left out.
        let mut down: Vec<(BodyUid, String, Vec3)> = Vec::new();
        for &uid in bodies {
            if !self.printed_body(uid) {
                continue;
            }
            let Some(bed) = self.print_bed_face(uid) else {
                continue;
            };
            match bed.plane {
                Ok(plane) => {
                    let normal = plane.normal();
                    for placement in placements(uid) {
                        let turned = placement.apply_vector(normal);
                        if let Some(unit) = mitcad_vec3::unit(turned) {
                            down.push((uid, bed.face.clone(), unit));
                        }
                    }
                }
                Err(why) => plan.problems.push((uid, why)),
            }
        }
        // One face, or one body's face where every placement turns it alike
        // (a component placed again, moved only): the piece turns onto it.
        let Some((uid, face, normal)) = down.first() else {
            return plan;
        };
        let alike = |(other, _, n): &(BodyUid, String, Vec3)| {
            other == uid && mitcad_vec3::dot(*n, *normal) > 1.0 - 1e-9
        };
        if down.iter().all(alike) {
            plan.face = Some((*uid, face.clone()));
            plan.transform = Some(turn_down(*normal));
        } else {
            for (uid, ..) in &down {
                if !plan.kept.contains(uid) {
                    plan.kept.push(*uid);
                }
            }
        }
        plan
    }

    /// The export's account of the bed: the plan as written, and a warning
    /// per bed face that cannot be used.
    pub(super) fn report_bed(&self, exported: &mut Exported, on_bed: Option<OnBed>) {
        let Some(on_bed) = on_bed else {
            return;
        };
        for (uid, why) in &on_bed.problems {
            let name = self.body_name(*uid);
            let warning = if on_bed.face.is_some() {
                format!("{name}: {why}")
            } else {
                format!("{name}: {why}; printed as in the design")
            };
            exported.warnings.push(warning);
        }
        exported.on_bed = Some(on_bed);
    }
}
