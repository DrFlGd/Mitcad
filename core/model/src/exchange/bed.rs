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
//!
//! A 3MF file of chosen occurrences (mitcad#856, `ExportPlacement::
//! occurrences`) has objects of its own: two or more placements picked of
//! bodies with usable bed faces are each an object turned onto its face
//! (a nut placed many times, each turned differently), the bodies of an
//! occurrence sent whole (`ExportPlacement::whole`) one object as they are
//! assembled, and the rest the file's own object; each lies on the bed by
//! the rule above, and those turned are set side by side along X with a
//! gap of [`GAP`], the row centred on the plate (overlaps with objects
//! kept as designed are left to the slicer's arrange).
//!
//! A piece kept as assembled because several bed faces apply to it,
//! counted per placement, lists them as its choices (mitcad#857,
//! [`BedChoice`]); a face chosen (`ExportPlacement::bed_faces`: a body and
//! the occurrence of its placement) turns the piece it is in onto it,
//! whatever the rule above says, as one bed face would.

use std::collections::BTreeMap;

use crate::datum::Vec3;
use crate::document::{Document, ModelError, PrintTarget};
use crate::ids::{BodyUid, ComponentUid, OccurrenceUid};
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
    /// 3MF (mitcad#856): the file's other objects, each put on the bed by
    /// itself (the fields above are the file's own object's, the bodies
    /// not put apart; no face and no transform when there are none).
    pub objects: Vec<BedObject>,
    /// How many placements of bodies lie on their print bed faces, in
    /// every object.
    pub on_faces: usize,
    /// The bed faces the piece could be turned onto, one per placement,
    /// when several keep it as assembled (mitcad#857); empty otherwise.
    pub choices: Vec<BedChoice>,
}

/// An object of a 3MF file put on the bed by itself (mitcad#856): a
/// placement of a body with a print bed face, or the bodies of an
/// occurrence sent whole.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BedObject {
    /// "Nut (Nut:3)", or the occurrence's path name.
    pub name: String,
    /// As in [`OnBed`]: the body and face on the bed, what moved the
    /// object after its placements, the bodies kept as assembled, and the
    /// bed faces to choose from.
    pub face: Option<(BodyUid, String)>,
    pub transform: Option<Transform>,
    pub kept: Vec<BodyUid>,
    pub choices: Vec<BedChoice>,
}

/// A print bed face a piece kept as assembled could lie on (mitcad#857):
/// the body, the occurrence path of the placement (None for a body that is
/// not placed) and the face as it was assigned.
#[derive(Debug, Clone, PartialEq)]
pub struct BedChoice {
    pub body: BodyUid,
    pub path: Option<Vec<OccurrenceUid>>,
    pub face: String,
}

/// A bed face where a placement puts it: the body, the face as assigned,
/// its outward normal turned by the placement, and the placement's path.
type Down = (BodyUid, String, Vec3, Option<Vec<OccurrenceUid>>);

/// Whether a chosen bed face (a body and the path of its placement, empty
/// for the design itself or a body not placed) is that of this placement.
pub(super) fn is_chosen(
    chosen: &(BodyUid, Vec<OccurrenceUid>),
    body: BodyUid,
    path: Option<&[OccurrenceUid]>,
) -> bool {
    chosen.0 == body && path.unwrap_or(&[]) == chosen.1.as_slice()
}

/// An object of a 3MF file on the print bed (mitcad#856): its placements
/// of the bodies written, and how it lies.
#[derive(Debug, Clone)]
pub(super) struct Piece {
    /// None for the file's own object (the bodies not put apart).
    pub name: Option<String>,
    /// (index of the body among those written, index of its placement).
    pub members: Vec<(usize, usize)>,
    pub plan: OnBed,
}

/// The gap between the objects set side by side on the bed, mm.
pub(super) const GAP: f64 = 5.0;

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
    /// where `placements` place them (`paths`: the occurrence path of each
    /// placement, None for a body not placed), and the face `chosen` among
    /// them (mitcad#857) or else the one body's face that every placement
    /// turns alike (its turn in `transform`, which the writer completes),
    /// or why they stay as they are, with the faces to choose from. Two
    /// faces chosen among them are refused.
    pub(super) fn bed_plan(
        &self,
        bodies: &[BodyUid],
        placements: &dyn Fn(BodyUid) -> Vec<Transform>,
        paths: &dyn Fn(BodyUid) -> Vec<Option<Vec<OccurrenceUid>>>,
        chosen: &[(BodyUid, Vec<OccurrenceUid>)],
    ) -> Result<OnBed, ModelError> {
        let mut plan = OnBed {
            plate: self.plate_centre(),
            ..OnBed::default()
        };
        // Each bed face where a placement puts it, by its outward normal,
        // with the placement's path; bodies that are not printed (surface
        // bodies) are left out.
        let mut down: Vec<Down> = Vec::new();
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
                    let at = paths(uid);
                    for (k, placement) in placements(uid).into_iter().enumerate() {
                        let turned = placement.apply_vector(normal);
                        if let Some(unit) = mitcad_vec3::unit(turned) {
                            let path = at.get(k).cloned().flatten();
                            down.push((uid, bed.face.clone(), unit, path));
                        }
                    }
                }
                Err(why) => plan.problems.push((uid, why)),
            }
        }
        // A face chosen (mitcad#857): the piece turns onto it.
        let picked: Vec<&(BodyUid, Vec<OccurrenceUid>)> = chosen
            .iter()
            .filter(|c| {
                down.iter()
                    .any(|(uid, _, _, path)| is_chosen(c, *uid, path.as_deref()))
            })
            .collect();
        if picked.len() > 1 {
            let names: Vec<String> = picked
                .iter()
                .map(|(uid, path)| {
                    let at = self.assembly().path_name(path);
                    if at.is_empty() {
                        self.body_name(*uid)
                    } else {
                        format!("{} in {at}", self.body_name(*uid))
                    }
                })
                .collect();
            return Err(super::invalid(format!(
                "the print bed faces of {} are chosen for one piece; choose one",
                names.join(" and ")
            )));
        }
        let first = match picked.first() {
            Some(c) => down
                .iter()
                .find(|(uid, _, _, path)| is_chosen(c, *uid, path.as_deref())),
            None => down.first(),
        };
        // One face, or one body's face where every placement turns it alike
        // (a component placed again, moved only): the piece turns onto it.
        let Some((uid, face, normal, _)) = first else {
            return Ok(plan);
        };
        let alike =
            |(other, _, n, _): &Down| other == uid && mitcad_vec3::dot(*n, *normal) > 1.0 - 1e-9;
        if !picked.is_empty() || down.iter().all(alike) {
            plan.face = Some((*uid, face.clone()));
            plan.transform = Some(turn_down(*normal));
            plan.on_faces = down.iter().filter(|d| alike(d)).count();
        } else {
            for (uid, face, _, path) in &down {
                if !plan.kept.contains(uid) {
                    plan.kept.push(*uid);
                }
                plan.choices.push(BedChoice {
                    body: *uid,
                    path: path.clone(),
                    face: face.clone(),
                });
            }
        }
        Ok(plan)
    }

    /// The objects of a 3MF file on the print bed (mitcad#856), each with
    /// its plan ([`Document::bed_plan`] of its placements): the
    /// placements of `bodies` (`paths`, the occurrence paths of
    /// `placements`, None for a body not placed) in an occurrence of
    /// `whole` (the outermost) one object each, as assembled; each
    /// placement `picked` (bodies listed with their occurrences) of a body
    /// with a usable bed face an object of its own when there are two or
    /// more; the rest the file's own object (first, when it has any). A
    /// face `chosen` turns the piece it is in (mitcad#857).
    #[allow(clippy::type_complexity)]
    pub(super) fn bed_pieces(
        &self,
        bodies: &[BodyUid],
        placements: &dyn Fn(BodyUid) -> Vec<Transform>,
        paths: &dyn Fn(BodyUid) -> Vec<Option<Vec<OccurrenceUid>>>,
        picked: &BTreeMap<BodyUid, Vec<Vec<OccurrenceUid>>>,
        whole: &[Vec<OccurrenceUid>],
        chosen: &[(BodyUid, Vec<OccurrenceUid>)],
    ) -> Result<Vec<Piece>, ModelError> {
        let mut rest = Vec::new();
        let mut wholes: Vec<Vec<(usize, usize)>> = vec![Vec::new(); whole.len()];
        let mut apart: Vec<(usize, usize, Vec<OccurrenceUid>)> = Vec::new();
        for (b, &uid) in bodies.iter().enumerate() {
            let usable = picked.contains_key(&uid)
                && self.printed_body(uid)
                && self.print_bed_face(uid).is_some_and(|f| f.plane.is_ok());
            for (k, path) in paths(uid).into_iter().enumerate() {
                let Some(path) = path else {
                    rest.push((b, k));
                    continue;
                };
                let outer = whole
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| path.starts_with(w.as_slice()))
                    .min_by_key(|(_, w)| w.len());
                if let Some((w, _)) = outer {
                    wholes[w].push((b, k));
                } else if usable {
                    apart.push((b, k, path));
                } else {
                    rest.push((b, k));
                }
            }
        }
        // One placement on its face goes with the rest, as before: the
        // other bodies turn with it as one piece.
        if apart.len() < 2 {
            rest.extend(apart.drain(..).map(|(b, k, _)| (b, k)));
        }
        let mut pieces = Vec::new();
        if !rest.is_empty() {
            pieces.push((None, rest));
        }
        for (w, members) in wholes.into_iter().enumerate() {
            if !members.is_empty() {
                pieces.push((Some(self.assembly().path_name(&whole[w])), members));
            }
        }
        for (b, k, path) in apart {
            let body = self.body_name(bodies[b]);
            let name = match self.assembly().path_name(&path) {
                occurrence if occurrence.is_empty() => body,
                occurrence => format!("{body} ({occurrence})"),
            };
            pieces.push((Some(name), vec![(b, k)]));
        }
        pieces
            .into_iter()
            .map(|(name, members)| {
                let mine: Vec<BodyUid> = bodies
                    .iter()
                    .enumerate()
                    .filter(|(b, _)| members.iter().any(|(m, _)| m == b))
                    .map(|(_, uid)| *uid)
                    .collect();
                let of_piece = |uid: BodyUid| -> Vec<Transform> {
                    let all = placements(uid);
                    members
                        .iter()
                        .filter(|(b, _)| bodies[*b] == uid)
                        .filter_map(|(_, k)| all.get(*k).copied())
                        .collect()
                };
                let paths_of_piece = |uid: BodyUid| -> Vec<Option<Vec<OccurrenceUid>>> {
                    let all = paths(uid);
                    members
                        .iter()
                        .filter(|(b, _)| bodies[*b] == uid)
                        .map(|(_, k)| all.get(*k).cloned().flatten())
                        .collect()
                };
                let plan = self.bed_plan(&mine, &of_piece, &paths_of_piece, chosen)?;
                Ok(Piece {
                    name,
                    members,
                    plan,
                })
            })
            .collect()
    }

    /// The account of a 3MF file's objects on the bed (mitcad#856): the
    /// file's own object's plan (the one piece's when there is one) with
    /// the others as [`OnBed::objects`],
    /// every problem once and the placements on their faces counted.
    pub(super) fn merged_bed(&self, pieces: Vec<Piece>) -> OnBed {
        let mut merged = OnBed {
            plate: self.plate_centre(),
            ..OnBed::default()
        };
        // One piece is the file's own object, whatever it holds.
        let single = pieces.len() == 1;
        for piece in pieces {
            let plan = piece.plan;
            merged.on_faces += plan.on_faces;
            for problem in plan.problems {
                if !merged.problems.contains(&problem) {
                    merged.problems.push(problem);
                }
            }
            match piece.name.filter(|_| !single) {
                None => {
                    merged.face = plan.face;
                    merged.transform = plan.transform;
                    merged.kept = plan.kept;
                    merged.choices = plan.choices;
                }
                Some(name) => merged.objects.push(BedObject {
                    name,
                    face: plan.face,
                    transform: plan.transform,
                    kept: plan.kept,
                    choices: plan.choices,
                }),
            }
        }
        merged
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
