// SPDX-License-Identifier: MIT
//! The `threads` query: the threads on the bodies at the timeline marker,
//! cosmetic or modelled, from thread features and tapped holes, with what a
//! view needs to draw them (cosmetic threads are drawn on their faces).
//! And the `thread_sizes` query: the size table of the panels (P9), and the
//! `thread_chamfer` query: a size's default end chamfer (mitcad#304).

use serde::Deserialize;
use serde_json::{Value, json};

use super::ApiError;
use mitcad_model::Document;
use mitcad_model::FeatureStatus;
use mitcad_model::features::hole::hole_face;
use mitcad_model::features::thread::{CustomSize, DEFAULT_CHAMFER_ANGLE, default_chamfer_diameter};
use mitcad_model::features::thread_table;
use mitcad_model::features::{
    FeatureDef, FeatureEntry, ThreadData, ThreadEnd, ThreadSize, ThreadStandard,
};
use mitcad_model::ids::BodyUid;
use mitcad_model::internal::features::CREST_ON_FACE;
use mitcad_model::kernel::{Cylinder, Kernel};
use mitcad_model::parameters::ParamId;
use mitcad_model::topo::{FaceName, RoleKey};

/// (length, offset, from the high end) of a partial thread.
type Part = Option<(f64, f64, bool)>;

/// What the query reads of a thread's definition.
struct ThreadOf<'a> {
    entry: &'a FeatureEntry,
    size: ThreadSize,
    /// The size's dimensions: the table's, or a custom thread's of its
    /// parameters' values (mitcad#893); None when there are none.
    data: Option<ThreadData>,
    /// The number of starts (mitcad#893).
    starts: u32,
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
    /// A taper pipe thread that tapers (mitcad#733).
    tapers: bool,
}

/// The document's methods of this module used across the api, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
pub(super) trait ThreadsOps<K: Kernel> {
    fn threads_json(&self) -> Value {
        self.threads_json_of(false)
    }

    /// The `threads` query; with `cosmetic`, only the threads a view draws
    /// (cosmetic ones): a modelled thread is in the geometry, and measuring
    /// its face (on the body before it, too) is what made the query cost
    /// the window 0.3-0.6 s each time it showed a design with modelled
    /// threads (mitcad#878).
    fn threads_json_of(&self, cosmetic: bool) -> Value;
}

impl<K: Kernel> ThreadsOps<K> for Document<K> {
    fn threads_json_of(&self, cosmetic: bool) -> Value {
        let params = self.parameters();
        let value = |id: Option<ParamId>| id.and_then(|id| params.value(id));
        // A size's dimensions and starts: a custom thread's of its values,
        // else the table's; the starts given, else the designation's
        // (mitcad#893).
        let sized = |size: &ThreadSize, custom: Option<&CustomSize>, starts: Option<ParamId>| {
            let data = match custom {
                Some(c) => value(Some(c.major))
                    .zip(value(Some(c.pitch)))
                    .and_then(|(major, pitch)| ThreadData::custom(major, pitch).ok()),
                None => size.data().ok(),
            };
            let starts = value(starts)
                .map(|n| n.round().max(1.0) as u32)
                .or(data.as_ref().map(|d| d.starts))
                .unwrap_or(1);
            (data, starts)
        };
        let mut threads = Vec::new();
        for entry in self.features() {
            if self.status(entry.uid) != Some(&FeatureStatus::Ok) {
                continue;
            }
            match &entry.def {
                FeatureDef::Thread(def) => {
                    if cosmetic && def.modeled {
                        continue;
                    }
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
                    let (data, starts) = sized(&def.thread, def.custom.as_ref(), def.starts);
                    let of = ThreadOf {
                        entry,
                        size: def.thread.clone(),
                        data,
                        starts,
                        modeled: def.modeled,
                        part,
                        chamfer,
                        profile: (diameters, value(def.radial_clearance).unwrap_or(0.0)),
                        on_input: true,
                        tapers: def.tapers(),
                    };
                    for face in &def.faces {
                        threads.extend(self.thread_json(&of, face.body, &face.face));
                    }
                }
                FeatureDef::Hole(def) => {
                    let Some(thread) = def.thread.as_ref().filter(|t| !(cosmetic && t.modeled))
                    else {
                        continue;
                    };
                    // Measured from the hole's start: the low end of the wall's
                    // axis, which points into the material.
                    let part = value(thread.length)
                        .map(|length| (length, value(thread.offset).unwrap_or(0.0), false));
                    let size = thread.size();
                    let (data, starts) = sized(&size, thread.custom.as_ref(), thread.starts);
                    let of = ThreadOf {
                        entry,
                        size,
                        data,
                        starts,
                        modeled: thread.modeled,
                        part,
                        chamfer: None,
                        profile: (None, value(thread.radial_clearance).unwrap_or(0.0)),
                        on_input: false,
                        tapers: thread.tapers(),
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
                    let Some(mitcad_model::features::gear::Bore::Thread { thread }) = &def.bore
                    else {
                        continue;
                    };
                    let of = ThreadOf {
                        entry,
                        size: thread.clone(),
                        data: thread.data().ok(),
                        starts: 1,
                        modeled: false,
                        part: None,
                        chamfer: None,
                        profile: (None, 0.0),
                        on_input: false,
                        tapers: false,
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
                    if let Some(mitcad_model::features::gear::Bore::Thread { thread }) = &def.bore {
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
                        let data = size.data().ok();
                        let of = ThreadOf {
                            entry,
                            size,
                            data,
                            starts: 1,
                            modeled: false,
                            part: None,
                            chamfer: None,
                            profile: (None, 0.0),
                            on_input: false,
                            tapers: false,
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
}

/// The document's methods used only in this module, as an
/// extension trait (mitcad#777): the api can then live in a crate above
/// the model.
trait ThreadsLocal<K: Kernel> {
    /// A thread on a face of a body at the marker, if the face is there or
    /// the thread took its place (mitcad#433, a tapped hole's wall
    /// mitcad#436).
    fn thread_json(&self, of: &ThreadOf<'_>, body: BodyUid, face: &FaceName) -> Option<Value>;

    /// The face a modelled thread is on as it was before the thread, with
    /// its cylinder, while the faces the thread made from it are on the
    /// body at the marker.
    fn face_before(
        &self,
        of: &ThreadOf<'_>,
        body: BodyUid,
        face: &FaceName,
    ) -> Option<(FaceName, Option<Cylinder>)>;
}

impl<K: Kernel> ThreadsLocal<K> for Document<K> {
    fn thread_json(&self, of: &ThreadOf<'_>, body: BodyUid, face: &FaceName) -> Option<Value> {
        let shape = self.body_shape(body)?;
        let kernel = self.kernel();
        let entry = of.entry;
        let size = &of.size;
        let data = of.data.clone()?;
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
        // A taper pipe thread's taper on the diameter (mitcad#733).
        if let Some(taper) = data.taper.filter(|_| of.tapers) {
            thread["taper"] = json!(taper.per_length);
        }
        // A multi-start thread's starts and lead (mitcad#893).
        if of.starts > 1 {
            thread["starts"] = json!(of.starts);
            thread["lead"] = json!(data.pitch * f64::from(of.starts));
        }
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
/// external): the size's major and minor diameters (a trapezoidal thread's
/// with its crest clearance, `ThreadData::profile`) or the class's given
/// ones, moved by the radial clearance.
fn profile_diameters(of: &ThreadOf<'_>, data: &ThreadData, internal: bool) -> (f64, f64) {
    let (diameters, radial) = of.profile;
    let (major, minor) = diameters.unwrap_or_else(|| {
        let (major, minor, _) = data.profile(internal);
        (major, minor)
    });
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
    mitcad_model::internal::features::find_face(kernel, shape, face).ok()?
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
    let mut standards: Vec<Value> = ThreadStandard::ALL
        .into_iter()
        .filter(|s| standard.is_none_or(|wanted| wanted == *s))
        .map(|s| {
            let sizes: Vec<Value> = thread_table::sizes(s)
                .into_iter()
                .map(|size| {
                    // A taper pipe thread's tap drill (mitcad#733).
                    let drill = size
                        .designations
                        .last()
                        .and_then(|d| thread_table::lookup(s, d).ok())
                        .and_then(|data| data.tap_drill());
                    let mut entry = json!({"size": size.size, "major_diameter": size.major,
                           "designations": size.designations});
                    if let Some(drill) = drill {
                        entry["tap_drill"] = json!(drill);
                    }
                    entry
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
    // A custom thread (mitcad#893) last: no sizes and no classes, its major
    // diameter and pitch given.
    if standard.is_none_or(|wanted| wanted == ThreadStandard::Custom) {
        let custom = ThreadStandard::Custom;
        standards.push(json!({
            "standard": custom,
            "title": custom.title(),
            "default": false,
            "custom": true,
            "sizes": [],
            "classes_external": [],
            "classes_internal": [],
            "default_class_external": custom.default_class(false),
            "default_class_internal": custom.default_class(true),
        }));
    }
    json!({"standards": standards})
}

/// The `thread_chamfer` query (mitcad#304): a thread size's default end
/// chamfer, for the Thread panel's fields.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ThreadChamferQuery {
    #[serde(default)]
    standard: ThreadStandard,
    #[serde(default)]
    designation: String,
    #[serde(default)]
    internal: bool,
    /// A custom thread's major diameter and pitch (mitcad#893).
    #[serde(default)]
    major: Option<f64>,
    #[serde(default)]
    pitch: Option<f64>,
}

pub(super) fn thread_chamfer_json(query: &ThreadChamferQuery) -> Result<Value, ApiError> {
    let data = if query.standard == ThreadStandard::Custom {
        let (Some(major), Some(pitch)) = (query.major, query.pitch) else {
            return Err(ApiError(
                "a custom thread's chamfer needs its `major` diameter and `pitch`".to_owned(),
            ));
        };
        ThreadData::custom(major, pitch).map_err(ApiError)?
    } else {
        thread_table::lookup(query.standard, &query.designation).map_err(ApiError)?
    };
    Ok(json!({
        "diameter": default_chamfer_diameter(&data, query.internal, None),
        "angle": DEFAULT_CHAMFER_ANGLE.to_degrees(),
    }))
}
