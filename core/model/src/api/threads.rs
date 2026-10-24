// SPDX-License-Identifier: MIT
//! The `threads` query: the threads on the bodies at the timeline marker,
//! cosmetic or modelled, from thread features and tapped holes, with what a
//! view needs to draw them (cosmetic threads are drawn on their faces).
//! And the `thread_sizes` query: the size table of the panels (P9), and the
//! `thread_chamfer` query: a size's default end chamfer (mitcad#304).

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use crate::document::Document;
use crate::features::hole::hole_face;
use crate::features::thread::{CREST_ON_FACE, DEFAULT_CHAMFER_ANGLE, default_chamfer_diameter};
use crate::features::thread_table;
use crate::features::{
    FeatureDef, FeatureEntry, ThreadData, ThreadEnd, ThreadSize, ThreadStandard,
};
use crate::ids::BodyUid;
use crate::kernel::{Cylinder, Kernel};
use crate::parameters::ParamId;
use crate::recompute::FeatureStatus;
use crate::topo::{FaceName, RoleKey};

/// (length, offset, from the high end) of a partial thread.
type Part = Option<(f64, f64, bool)>;

/// What the query reads of a thread's definition.
struct ThreadOf<'a> {
    entry: &'a FeatureEntry,
    size: ThreadSize,
    modeled: bool,
    part: Part,
    /// The end chamfers asked for (mitcad#304): the angle and the diameter
    /// (the size's default without).
    chamfer: Option<(f64, Option<f64>)>,
    /// A modelled thread's major and minor diameters when given, and its
    /// radial clearance: where its crest and root lie.
    profile: (Option<(f64, f64)>, f64),
    /// The feature threads faces of the bodies before it (a thread
    /// feature), not faces it makes itself (a hole's walls).
    on_input: bool,
}

impl<K: Kernel> Document<K> {
    pub(super) fn threads_json(&self) -> Value {
        let params = self.parameters();
        let value = |id: Option<ParamId>| id.and_then(|id| params.value(id));
        let mut threads = Vec::new();
        for entry in self.features() {
            if self.status(entry.uid) != Some(&FeatureStatus::Ok) {
                continue;
            }
            match &entry.def {
                FeatureDef::Thread(def) => {
                    let part = value(def.length).map(|length| {
                        (
                            length,
                            value(def.offset).unwrap_or(0.0),
                            def.location == ThreadEnd::HighEnd,
                        )
                    });
                    let chamfer = def.chamfer.map(|_| {
                        (
                            value(def.chamfer_angle).unwrap_or(DEFAULT_CHAMFER_ANGLE),
                            value(def.chamfer_diameter),
                        )
                    });
                    let diameters = def
                        .diameters
                        .as_ref()
                        .and_then(|d| Some((value(Some(d.major))?, value(Some(d.minor))?)));
                    let of = ThreadOf {
                        entry,
                        size: def.thread.clone(),
                        modeled: def.modeled,
                        part,
                        chamfer,
                        profile: (diameters, value(def.radial_clearance).unwrap_or(0.0)),
                        on_input: true,
                    };
                    for face in &def.faces {
                        threads.extend(self.thread_json(&of, face.body, &face.face));
                    }
                }
                FeatureDef::Hole(def) => {
                    let Some(thread) = &def.thread else {
                        continue;
                    };
                    // Measured from the hole's start: the low end of the wall's
                    // axis, which points into the material.
                    let part = value(thread.length)
                        .map(|length| (length, value(thread.offset).unwrap_or(0.0), false));
                    let of = ThreadOf {
                        entry,
                        size: thread.size(),
                        modeled: thread.modeled,
                        part,
                        chamfer: None,
                        profile: (None, value(thread.radial_clearance).unwrap_or(0.0)),
                        on_input: false,
                    };
                    let walls: Vec<FaceName> = (0..def.count())
                        .map(|i| hole_face(entry.uid, i, "wall"))
                        .collect();
                    for body in self.bodies() {
                        for wall in &walls {
                            threads.extend(self.thread_json(&of, body.uid, wall));
                        }
                    }
                }
                // A gear's threaded bore, cosmetic (mitcad#472).
                FeatureDef::Gear(def) => {
                    let Some(crate::features::gear::Bore::Thread { thread }) = &def.bore else {
                        continue;
                    };
                    let of = ThreadOf {
                        entry,
                        size: thread.clone(),
                        modeled: false,
                        part: None,
                        chamfer: None,
                        profile: (None, 0.0),
                        on_input: false,
                    };
                    let bore = FaceName::new(entry.uid, "bore", None);
                    for body in self.bodies() {
                        threads.extend(self.thread_json(&of, body.uid, &bore));
                    }
                }
                // A pulley's threaded bore and its set screws' tap drills,
                // cosmetic (mitcad#562).
                FeatureDef::Pulley(def) => {
                    let mut faces: Vec<(ThreadSize, FaceName)> = Vec::new();
                    if let Some(crate::features::gear::Bore::Thread { thread }) = &def.bore {
                        faces.push((thread.clone(), FaceName::new(entry.uid, "bore", None)));
                    }
                    if let Some(screws) = &def.set_screws {
                        let count = value(Some(screws.count)).unwrap_or(0.0).round() as u32;
                        for k in 1..=count {
                            faces.push((
                                screws.thread.clone(),
                                FaceName::new(entry.uid, "set_screw", Some(RoleKey::Index(k))),
                            ));
                        }
                    }
                    for (size, face) in faces {
                        let of = ThreadOf {
                            entry,
                            size,
                            modeled: false,
                            part: None,
                            chamfer: None,
                            profile: (None, 0.0),
                            on_input: false,
                        };
                        for body in self.bodies() {
                            threads.extend(self.thread_json(&of, body.uid, &face));
                        }
                    }
                }
                _ => {}
            }
        }
        Value::Array(threads)
    }

    /// A thread on a face of a body at the marker, if the face is there or
    /// the thread took its place (mitcad#433, a tapped hole's wall
    /// mitcad#436).
    fn thread_json(&self, of: &ThreadOf<'_>, body: BodyUid, face: &FaceName) -> Option<Value> {
        let shape = self.body_shape(body)?;
        let kernel = self.kernel();
        let entry = of.entry;
        let size = &of.size;
        let data = size.data().ok()?;
        // The face on the body at the marker. It shows where the thread
        // runs (less the cones of its end chamfers) when pieces of it stay
        // between the grooves: a cosmetic thread's face, or a modelled
        // thread's whose crest lies on it. A modelled thread whose crest
        // lies off its face (a class's diameters, a radial clearance) cuts
        // away, or covers, all of the face along the thread: what is left
        // of it lies outside the thread's part only (nothing for a thread
        // over the whole face), so it is measured as it was before the
        // thread instead (mitcad#628), as is a modelled thread's face whose
        // pieces at the marker are no one cylinder.
        let at_marker = find_face(kernel, shape, face).map(|found| {
            let cylinder = kernel.face_cylinder(shape, &found).ok();
            (found, cylinder)
        });
        let spans = at_marker.as_ref().is_some_and(|(_, cylinder)| {
            !of.modeled || cylinder.as_ref().is_some_and(|c| !crest_off(of, &data, c))
        });
        // The face the thread is on, its cylinder, and whether that face
        // lost the ends the chamfers took (measured on the body at the
        // marker) or not (as it was before the thread). Without the face as
        // it was (a result stored before mitcad#628 has no wall of a part),
        // the face at the marker is measured.
        let before = if spans {
            None
        } else {
            self.face_before(of, body, face)
        };
        let (face, cylinder, trimmed) = match (before, at_marker) {
            (Some((found, cylinder)), _) => (found, cylinder, false),
            (None, Some((found, cylinder))) => (found, cylinder, true),
            (None, None) => return None,
        };
        let face = &face;
        let mut thread = json!({
            "feature": entry.uid,
            "name": entry.name,
            "body": body,
            "face": face,
            "standard": size.standard,
            "designation": data.designation,
            "class": size.class,
            "right_handed": size.right_handed,
            "modeled": of.modeled,
            "major_diameter": data.major,
            "minor_diameter": data.minor,
            "pitch": data.pitch,
        });
        if let Some(cylinder) = cylinder {
            // The cones of end chamfers take `low` and `high` off the face's
            // ends, and the thread starts where a cone meets the cylinder.
            // On the face at the marker that is a cosmetic thread's face
            // (what a modelled thread whose crest lies on its face leaves of
            // it ends at the cones already); the part is measured on the
            // face as it was. On the face as it was before a modelled
            // thread, the cones are measured to its crest (mitcad#433,
            // mitcad#628).
            let chamfer = of.chamfer.filter(|_| !trimmed || !of.modeled);
            let (low, high) = match chamfer {
                Some((angle, diameter)) => {
                    let depth = if of.modeled {
                        // From the chamfer's diameter at the end to the
                        // crest, which the cut profile leaves.
                        let (crest, root) = profile_diameters(of, &data, cylinder.internal);
                        let diameter = diameter.unwrap_or_else(|| {
                            default_chamfer_diameter(&data, cylinder.internal, Some(root))
                        });
                        (crest - diameter).abs() / 2.0 / angle.tan()
                    } else {
                        let diameter = diameter.unwrap_or_else(|| {
                            default_chamfer_diameter(&data, cylinder.internal, None)
                        });
                        (cylinder.radius - diameter / 2.0).abs() / angle.tan()
                    };
                    let cut = |role: &str| {
                        if has_face(kernel, shape, &derived(entry, role, face)) {
                            depth
                        } else {
                            0.0
                        }
                    };
                    (cut("thread_chamfer_low"), cut("thread_chamfer_high"))
                }
                None => (0.0, 0.0),
            };
            // The face as it was, and where the face measured starts on it.
            let (whole, base) = if trimmed {
                (cylinder.length + low + high, low)
            } else {
                (cylinder.length, 0.0)
            };
            let (from, to) = match of.part {
                None => (0.0, whole),
                Some((length, offset, true)) => (whole - offset - length, whole - offset),
                Some((length, offset, false)) => (offset, offset + length),
            };
            let (from, to) = (from.max(low) - base, to.min(whole - high) - base);
            let at = |t: f64| -> [f64; 3] {
                std::array::from_fn(|i| cylinder.axis.origin[i] + t * cylinder.axis.direction[i])
            };
            thread["internal"] = json!(cylinder.internal);
            thread["radius"] = json!(cylinder.radius);
            thread["start"] = json!(at(from));
            thread["end"] = json!(at(to));
        }
        Some(thread)
    }

    /// The face a modelled thread is on as it was before the thread, with
    /// its cylinder, while the faces the thread made from it are on the
    /// body at the marker.
    fn face_before(
        &self,
        of: &ThreadOf<'_>,
        body: BodyUid,
        face: &FaceName,
    ) -> Option<(FaceName, Option<Cylinder>)> {
        let shape = self.body_shape(body)?;
        let kernel = self.kernel();
        let entry = of.entry;
        if of.on_input {
            // A thread feature's face is on the body before the feature
            // (mitcad#433); the faces the feature made from it stand in its
            // place.
            let before = self.body_before(entry.uid, body)?;
            let found = find_face(kernel, before, face)?;
            let made = ["thread", "thread_chamfer_low", "thread_chamfer_high"]
                .into_iter()
                .any(|role| has_face(kernel, shape, &derived(entry, role, &found)));
            if !made {
                return None;
            }
            let cylinder = kernel.face_cylinder(before, &found).ok();
            Some((found, cylinder))
        } else {
            // A tapped hole makes its wall in the same step as the thread,
            // so no body before the hole has it: the hole measured it
            // before cutting the thread (mitcad#436, mitcad#628). Its
            // thread faces stand in its place.
            let cylinder = *self.threaded_wall(entry.uid, body, face)?;
            if !has_face(kernel, shape, &derived(entry, "thread", face)) {
                return None;
            }
            Some((face.clone(), Some(cylinder)))
        }
    }
}

/// A modelled thread's crest and root diameters on a face (internal or
/// external): the size's major and minor diameters or the class's given
/// ones, moved by the radial clearance.
fn profile_diameters(of: &ThreadOf<'_>, data: &ThreadData, internal: bool) -> (f64, f64) {
    let (diameters, radial) = of.profile;
    let (major, minor) = diameters.unwrap_or((data.major, data.minor));
    if internal {
        (minor + 2.0 * radial, major + 2.0 * radial)
    } else {
        (major - 2.0 * radial, minor - 2.0 * radial)
    }
}

/// Whether a modelled thread's crest lies off the face's cylinder (by more
/// than the kernel's 1e-6 mm), so that the thread takes or covers all of
/// the face along its part (mitcad#628).
fn crest_off(of: &ThreadOf<'_>, data: &ThreadData, cylinder: &Cylinder) -> bool {
    let (crest, _) = profile_diameters(of, data, cylinder.internal);
    (crest / 2.0 - cylinder.radius).abs() > CREST_ON_FACE
}

/// A face of a shape by its name, or the face a sketch change renamed, as
/// the thread feature finds it (mitcad#272).
fn find_face<K: Kernel>(kernel: &K, shape: &K::Shape, face: &FaceName) -> Option<FaceName> {
    if kernel.count_faces(shape, face).ok()? > 0 {
        return Some(face.clone());
    }
    crate::features::renamed::find_face(kernel, shape, face).ok()?
}

fn has_face<K: Kernel>(kernel: &K, shape: &K::Shape, face: &FaceName) -> bool {
    kernel.count_faces(shape, face).is_ok_and(|n| n > 0)
}

/// A face a thread feature made from a face it threads:
/// `<feature>:<role>(<face>)`.
fn derived(entry: &FeatureEntry, role: &str, face: &FaceName) -> FaceName {
    FaceName::new(entry.uid, role, Some(RoleKey::Face(Box::new(face.clone()))))
}

/// The `thread_sizes` query (P9, P11): the size table of the Hole and
/// Thread panels, every standard (or the one asked for), ISO metric first
/// and the default.
pub(super) fn thread_sizes_json(standard: Option<ThreadStandard>) -> Value {
    let standards: Vec<Value> = ThreadStandard::ALL
        .into_iter()
        .filter(|s| standard.is_none_or(|wanted| wanted == *s))
        .map(|s| {
            let sizes: Vec<Value> = thread_table::sizes(s)
                .into_iter()
                .map(|size| {
                    json!({"size": size.size, "major_diameter": size.major,
                           "designations": size.designations})
                })
                .collect();
            let (external, internal) = thread_table::classes(s);
            json!({
                "standard": s,
                "title": s.title(),
                "default": s == ThreadStandard::default(),
                "sizes": sizes,
                "classes_external": external,
                "classes_internal": internal,
                "default_class_external": s.default_class(false),
                "default_class_internal": s.default_class(true),
            })
        })
        .collect();
    json!({"standards": standards})
}

/// The `thread_chamfer` query (mitcad#304): a thread size's default end
/// chamfer, for the Thread panel's fields.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ThreadChamferQuery {
    #[serde(default)]
    standard: ThreadStandard,
    designation: String,
    #[serde(default)]
    internal: bool,
}

pub(super) fn thread_chamfer_json(query: &ThreadChamferQuery) -> Result<Value, ApiError> {
    let data = thread_table::lookup(query.standard, &query.designation).map_err(ApiError)?;
    Ok(json!({
        "diameter": default_chamfer_diameter(&data, query.internal, None),
        "angle": DEFAULT_CHAMFER_ANGLE.to_degrees(),
    }))
}
