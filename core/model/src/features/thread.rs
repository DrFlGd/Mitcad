// SPDX-License-Identifier: MIT
//! Thread: a screw thread on cylindrical faces.
//! A cosmetic thread changes no geometry; it is data on the faces that the
//! `threads` query lists for display. A modelled thread builds the
//! 60-degree profile on the faces (ISO metric and Unified threads;
//! Whitworth and NPT threads are cosmetic only): the basic profile of the
//! size, or the profile of given diameters (a tolerance class's, as .f3d
//! designs store them), turned about its axis by an angle, with optional
//! extra radial and flank clearance for a looser fit (mitcad#195). Either
//! may have a chamfer cut at its free ends (mitcad#304), a cosmetic one too.

use serde::{Deserialize, Serialize};

use super::reference::FaceRef;
use super::thread_table::{self, ThreadData, ThreadStandard};
use super::{
    BodyChange, CheckContext, EvalContext, Evaluate, FeatureInfo, FeatureOutput, References,
    is_false,
};
use crate::ids::BodyUid;
use crate::kernel::{Kernel, ThreadChamfer, ThreadSpec};
use crate::parameters::ParamId;
use crate::topo::FaceName;

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn is_iso(standard: &ThreadStandard) -> bool {
    *standard == ThreadStandard::IsoMetric
}

/// A thread size of a standard (`ThreadInfo` in .f3d designs): `M10x1.5` and `6g`,
/// or `1/4-20 UNC` and `2A`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadSize {
    #[serde(default, skip_serializing_if = "is_iso")]
    pub standard: ThreadStandard,
    pub designation: String,
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub right_handed: bool,
}

impl ThreadSize {
    pub fn data(&self) -> Result<ThreadData, String> {
        thread_table::lookup(self.standard, &self.designation)
    }

    /// Checks the designation, and the class for an internal or external
    /// thread (either when unknown).
    pub fn check(&self, internal: Option<bool>) -> Result<ThreadData, String> {
        let data = self.data()?;
        if let Some(class) = &self.class {
            thread_table::check_class(self.standard, class, internal)?;
        }
        Ok(data)
    }
}

/// The end of the cylinder a partial thread is measured from: where the
/// axis of the face's cylinder points (`HighEnd` in .f3d designs, the default), or
/// the other one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadEnd {
    #[default]
    HighEnd,
    LowEnd,
}

impl ThreadEnd {
    pub fn is_high(&self) -> bool {
        *self == Self::HighEnd
    }
}

/// The ends of a thread that get a chamfer (mitcad#304), measured like
/// `location`: the start is the end `location` names, the end the other
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadChamferEnds {
    Start,
    End,
    Both,
}

impl ThreadChamferEnds {
    /// Whether the low and the high end along the face's cylinder's axis
    /// are asked for, with the thread measured from `location`.
    pub fn low_high(self, location: ThreadEnd) -> (bool, bool) {
        let (start, end) = match self {
            Self::Start => (true, false),
            Self::End => (false, true),
            Self::Both => (true, true),
        };
        if location.is_high() {
            (end, start)
        } else {
            (start, end)
        }
    }
}

/// The default angle of a thread's end chamfer to the axis: 45 degrees
/// (an internal thread's countersink of 90 degrees).
pub const DEFAULT_CHAMFER_ANGLE: f64 = std::f64::consts::FRAC_PI_4;

/// How far (mm) a modelled thread's crest may lie from its face and still
/// be on it, as the OCCT facade tells them apart (`kSame` in
/// `geometry/src/thread.cpp`). A crest off its face takes or covers all of
/// the face along the thread's part (mitcad#628).
pub(crate) const CREST_ON_FACE: f64 = 1e-6;

/// The default diameter of a thread's end chamfer at the end (mitcad#304):
/// a tenth of a pitch below the minor diameter of the size for an external
/// thread, above its major diameter for an internal one, and past the
/// profile's `root` diameter (a class's, moved by a radial clearance) when
/// that lies further out, so that no groove reaches the end's plane.
pub fn default_chamfer_diameter(data: &ThreadData, internal: bool, root: Option<f64>) -> f64 {
    let margin = data.pitch / 10.0;
    if internal {
        data.major.max(root.unwrap_or(data.major)) + margin
    } else {
        data.minor.min(root.unwrap_or(data.minor)) - margin
    }
}

/// Whether a thread part (`thread_part`) reaches the low and the high end
/// of a face of `length` (within a micrometre).
pub(crate) fn part_reaches(part: Option<(f64, f64, bool)>, length: f64) -> (bool, bool) {
    const NEAR: f64 = 1e-6;
    match part {
        None => (true, true),
        Some((l, offset, true)) => (offset + l >= length - NEAR, offset <= NEAR),
        Some((l, offset, false)) => (offset <= NEAR, offset + l >= length - NEAR),
    }
}

/// The diameters of a modelled thread's profile: its crests and roots lie
/// on the major and minor diameters, and it is half a pitch wide at the
/// pitch diameter (`majorDiameter`, `minorDiameter` and `pitchDiameter` of
/// a .f3d design's `ThreadInfo`: the middle of the class's tolerances).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadDiameters<P = ParamId> {
    pub major: P,
    pub minor: P,
    pub pitch: P,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadDef<P = ParamId> {
    /// Cylindrical faces, all internal (hole walls) or all external.
    pub faces: Vec<FaceRef>,
    pub thread: ThreadSize,
    /// Cut the thread into the geometry; cosmetic otherwise.
    #[serde(default, skip_serializing_if = "is_false")]
    pub modeled: bool,
    /// A part of each face, `length` long from `offset`, measured from
    /// `location`; the whole face (`isFullLength` in .f3d designs) without.
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub length: Option<P>,
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub offset: Option<P>,
    #[serde(default, skip_serializing_if = "ThreadEnd::is_high")]
    pub location: ThreadEnd,
    /// A modelled thread's diameters; the size's basic ones (D, D1, D2)
    /// without.
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub diameters: Option<ThreadDiameters<P>>,
    /// A modelled thread turned about its axis (see `ThreadSpec`); 0
    /// without.
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub angle: Option<P>,
    /// A modelled thread's extra radial clearance per side (see
    /// `ThreadSpec`); 0 without.
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub radial_clearance: Option<P>,
    /// A modelled thread's extra clearance perpendicular to each flank (see
    /// `ThreadSpec`); 0 without.
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub flank_clearance: Option<P>,
    /// A chamfer at these ends of each face where the thread runs out
    /// through a free end (mitcad#304); none without.
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub chamfer: Option<ThreadChamferEnds>,
    /// The chamfer's angle to the axis; `DEFAULT_CHAMFER_ANGLE` without.
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub chamfer_angle: Option<P>,
    /// The chamfer's diameter at the end; `default_chamfer_diameter` of the
    /// size without (it follows the size).
    #[serde(default = "Default::default", skip_serializing_if = "Option::is_none")]
    pub chamfer_diameter: Option<P>,
}

impl<P> ThreadDef<P> {
    pub const TYPE: &'static str = "thread";
    pub const BASE_NAME: &'static str = "Thread";

    // Written by hand: the diameters map under nested slots (`diameters.major`).
    pub fn map_params<Q, E>(
        &self,
        f: &mut dyn FnMut(&str, &P) -> Result<Q, E>,
    ) -> Result<ThreadDef<Q>, E> {
        Ok(ThreadDef {
            faces: self.faces.clone(),
            thread: self.thread.clone(),
            modeled: self.modeled,
            length: self.length.as_ref().map(|v| f("length", v)).transpose()?,
            offset: self.offset.as_ref().map(|v| f("offset", v)).transpose()?,
            location: self.location,
            diameters: match &self.diameters {
                Some(d) => Some(ThreadDiameters {
                    major: f("diameters.major", &d.major)?,
                    minor: f("diameters.minor", &d.minor)?,
                    pitch: f("diameters.pitch", &d.pitch)?,
                }),
                None => None,
            },
            angle: self.angle.as_ref().map(|v| f("angle", v)).transpose()?,
            radial_clearance: self
                .radial_clearance
                .as_ref()
                .map(|v| f("radial_clearance", v))
                .transpose()?,
            flank_clearance: self
                .flank_clearance
                .as_ref()
                .map(|v| f("flank_clearance", v))
                .transpose()?,
            chamfer: self.chamfer,
            chamfer_angle: self
                .chamfer_angle
                .as_ref()
                .map(|v| f("chamfer_angle", v))
                .transpose()?,
            chamfer_diameter: self
                .chamfer_diameter
                .as_ref()
                .map(|v| f("chamfer_diameter", v))
                .transpose()?,
        })
    }
}

/// A body's threaded faces with the ends (low, high) to chamfer of each.
type ThreadedBody<S> = (BodyUid, S, Vec<FaceName>, Vec<(bool, bool)>);

/// A chamfer angle's check: between 0 and 90 degrees to the axis.
fn check_chamfer_angle(angle: f64) -> Result<(), String> {
    if angle > 0.0 && angle < std::f64::consts::FRAC_PI_2 {
        Ok(())
    } else {
        Err(format!(
            "the thread's chamfer angle must be between 0 and 90 degrees to the axis, got {} deg",
            angle.to_degrees()
        ))
    }
}

/// The narrowest crest a flank clearance may leave, in pitches.
const MIN_CREST: f64 = 0.02;

/// A clearance's value: a finite length of 0 or more (`what`: "radial
/// clearance"); 0 without.
pub(crate) fn clearance_value(what: &str, value: Option<f64>) -> Result<f64, String> {
    match value {
        None => Ok(0.0),
        Some(v) if v.is_finite() && v >= 0.0 => Ok(v),
        Some(v) => Err(format!(
            "the thread's {what} must be a finite length of 0 or more, got {v}"
        )),
    }
}

/// A modelled thread's extra clearance (radial, flank), read and checked
/// for its own values.
pub(crate) fn thread_clearance<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    radial: Option<ParamId>,
    flank: Option<ParamId>,
) -> Result<(f64, f64), String> {
    let radial = radial.map(|id| ctx.param(id)).transpose()?;
    let flank = flank.map(|id| ctx.param(id)).transpose()?;
    Ok((
        clearance_value("radial clearance", radial)?,
        clearance_value("flank clearance", flank)?,
    ))
}

/// Whether extra clearance leaves a thread of the profile (`pitch` and the
/// `major`, `minor` and pitch diameters) on an internal or external face:
/// the flank clearance moves each flank `flank / cos 30°` into the tooth
/// along the axis, the crest keeping its radius, and must leave the crest
/// at least P/50 wide; the radial clearance moves the whole profile away
/// from the material and must leave an external thread a core. The
/// geometry checks the same (`geometry/src/thread.cpp`).
pub(crate) fn check_clearance(
    pitch: f64,
    (major, minor, pitch_diameter): (f64, f64, f64),
    internal: bool,
    (radial, flank): (f64, f64),
) -> Result<(), String> {
    let tan30 = 30f64.to_radians().tan();
    let cos30 = 30f64.to_radians().cos();
    if flank > 0.0 {
        // Half a pitch at the pitch diameter, narrowing along the flanks.
        let depth = if internal {
            pitch_diameter - minor
        } else {
            major - pitch_diameter
        };
        let crest = pitch / 2.0 - depth * tan30 - 2.0 * flank / cos30;
        let needed = MIN_CREST * pitch;
        if crest < needed {
            return Err(format!(
                "the flank clearance of {flank} mm leaves no usable tooth: the crest would be \
                 {crest:.4} mm wide, at least {needed:.4} mm is needed"
            ));
        }
    }
    if !internal && minor - 2.0 * radial <= 0.0 {
        return Err(format!(
            "the radial clearance of {radial} mm leaves the external thread no core \
             (minor diameter {minor} mm)"
        ));
    }
    Ok(())
}

/// The part of a cylinder a thread covers, for the kernel: (length,
/// offset, from the high end), None for the whole face.
pub(crate) fn thread_part<K: Kernel>(
    ctx: &mut EvalContext<'_, K>,
    length: Option<ParamId>,
    offset: Option<ParamId>,
    high_end: bool,
) -> Result<Option<(f64, f64, bool)>, String> {
    let Some(length) = length else {
        return Ok(None);
    };
    let length = ctx.positive(length)?;
    let offset = match offset {
        Some(id) => {
            let value = ctx.param(id)?;
            if value < 0.0 {
                return Err(format!(
                    "{} must not be negative, got {value}",
                    ctx.param_name(id)
                ));
            }
            value
        }
        None => 0.0,
    };
    Ok(Some((length, offset, high_end)))
}

impl FeatureInfo for ThreadDef {
    fn references(&self) -> References {
        let mut references = References::default();
        for face in &self.faces {
            face.add_references(&mut references);
        }
        references
    }

    fn check(&self, ctx: &CheckContext<'_>) -> Result<(), String> {
        if self.faces.is_empty() {
            return Err("no faces selected".to_owned());
        }
        for (i, face) in self.faces.iter().enumerate() {
            if self.faces[..i].contains(face) {
                return Err(format!("face {} is listed more than once", face.face));
            }
            face.check(ctx)?;
        }
        if self.offset.is_some() && self.length.is_none() {
            return Err("a thread over the whole face has no offset".to_owned());
        }
        if self.modeled {
            self.thread.standard.check_modeled()?;
        } else if self.diameters.is_some() || self.angle.is_some() {
            return Err("only a modelled thread has diameters and an angle".to_owned());
        } else if self.radial_clearance.is_some() || self.flank_clearance.is_some() {
            return Err("only a modelled thread has extra clearance".to_owned());
        }
        if self.chamfer.is_none()
            && (self.chamfer_angle.is_some() || self.chamfer_diameter.is_some())
        {
            return Err("a thread without a chamfer has no chamfer angle or diameter".to_owned());
        }
        self.thread.check(None).map(|_| ())
    }

    fn check_values(&self, value: &dyn Fn(ParamId) -> f64) -> Result<(), String> {
        if let Some(length) = self.length
            && (value(length).is_nan() || value(length) <= 0.0)
        {
            return Err(format!(
                "the thread length must be greater than zero, got {}",
                value(length)
            ));
        }
        if let Some(offset) = self.offset
            && value(offset) < 0.0
        {
            return Err(format!(
                "the thread offset must not be negative, got {}",
                value(offset)
            ));
        }
        if let Some(d) = &self.diameters {
            let (major, minor, pitch) = (value(d.major), value(d.minor), value(d.pitch));
            if !(minor > 0.0 && minor < pitch && pitch < major) {
                return Err(format!(
                    "the thread's diameters must grow from the minor ({minor}) to the pitch \
                     ({pitch}) to the major one ({major})"
                ));
            }
        }
        clearance_value("radial clearance", self.radial_clearance.map(value))?;
        clearance_value("flank clearance", self.flank_clearance.map(value))?;
        if let Some(angle) = self.chamfer_angle {
            check_chamfer_angle(value(angle))?;
        }
        if let Some(diameter) = self.chamfer_diameter
            && (value(diameter).is_nan() || value(diameter) <= 0.0)
        {
            return Err(format!(
                "the thread's chamfer diameter must be greater than zero, got {}",
                value(diameter)
            ));
        }
        Ok(())
    }
}

impl<K: Kernel> Evaluate<K> for ThreadDef {
    fn evaluate(&self, ctx: &mut EvalContext<'_, K>) -> Result<FeatureOutput<K::Shape>, String> {
        let part = thread_part(
            ctx,
            self.length,
            self.offset,
            self.location == ThreadEnd::HighEnd,
        )?;
        // The profile's diameters: given, or the size's basic ones.
        let diameters = match &self.diameters {
            Some(d) => {
                let (major, minor, pitch) = (
                    ctx.positive(d.major)?,
                    ctx.positive(d.minor)?,
                    ctx.positive(d.pitch)?,
                );
                if !(minor < pitch && pitch < major) {
                    return Err(format!(
                        "the thread's diameters must grow from the minor ({minor}) to the pitch \
                         ({pitch}) to the major one ({major})"
                    ));
                }
                Some((major, minor, pitch))
            }
            None => None,
        };
        let angle = match self.angle {
            Some(id) => ctx.param(id)?,
            None => 0.0,
        };
        let clearance = thread_clearance(ctx, self.radial_clearance, self.flank_clearance)?;
        let (radial, _) = clearance;
        // The chamfer's angle (checked before the faces).
        let chamfer_angle = match (self.chamfer, self.chamfer_angle) {
            (Some(_), Some(id)) => {
                let angle = ctx.param(id)?;
                check_chamfer_angle(angle)?;
                Some(angle)
            }
            (Some(_), None) => Some(DEFAULT_CHAMFER_ANGLE),
            (None, _) => None,
        };
        let chamfer_diameter = self
            .chamfer_diameter
            .filter(|_| self.chamfer.is_some())
            .map(|id| ctx.positive(id))
            .transpose()?;
        // The faces by body, each a cylinder, all on one side of their
        // material, with the ends to chamfer (low, high) of each.
        let mut bodies: Vec<ThreadedBody<K::Shape>> = Vec::new();
        // The thinnest external and the widest internal face, for the
        // chamfer's check.
        let mut extreme_diameter: Option<f64> = None;
        let mut internal = None;
        for face in &self.faces {
            let shape = ctx.surface_body(face.body)?;
            // A face a sketch change renamed is found again (mitcad#272).
            let found = ctx.face_or(&shape, &face.face, || {
                format!("body {} has no face {}", face.body, face.face)
            })?;
            let cylinder = ctx
                .kernel
                .face_cylinder(&shape, &found)
                .map_err(|e| e.to_string())?;
            if internal.is_some_and(|i| i != cylinder.internal) {
                return Err("the faces of a thread must all be internal or all external".to_owned());
            }
            internal = Some(cylinder.internal);
            let data = self.thread.check(internal)?;
            // The size must fit the cylinder: an external thread's root lies
            // inside it, an internal thread's root outside the bore (each
            // moved by the radial clearance).
            let diameter = 2.0 * cylinder.radius;
            let (major, minor, _) =
                diameters.unwrap_or((data.major, data.minor, data.pitch_diameter));
            let fits = if cylinder.internal {
                diameter < major + 2.0 * radial
            } else {
                diameter > minor - 2.0 * radial
            };
            if self.modeled && !fits {
                return Err(format!(
                    "thread {} does not fit face {} of diameter {diameter}",
                    data.designation, face.face
                ));
            }
            // The ends asked for that the thread reaches; the kernel
            // chamfers those with nothing beyond them.
            let ends = match self.chamfer {
                Some(chamfer) => {
                    let (low, high) = chamfer.low_high(self.location);
                    let (reaches_low, reaches_high) = part_reaches(part, cylinder.length);
                    (low && reaches_low, high && reaches_high)
                }
                None => (false, false),
            };
            // The material at the face: the face, or the crest of a modelled
            // thread where that lies further into the material.
            let material = if !self.modeled {
                diameter
            } else if cylinder.internal {
                diameter.min(minor + 2.0 * radial)
            } else {
                diameter.max(major - 2.0 * radial)
            };
            extreme_diameter = Some(match extreme_diameter {
                None => material,
                Some(d) if cylinder.internal => d.max(material),
                Some(d) => d.min(material),
            });
            match bodies.iter_mut().find(|(uid, _, _, _)| *uid == face.body) {
                Some((_, _, faces, all_ends)) => {
                    faces.push(found);
                    all_ends.push(ends);
                }
                None => bodies.push((face.body, shape, vec![found], vec![ends])),
            }
        }
        let mut output = FeatureOutput::default();
        let data = self.thread.data()?;
        let (major, minor, pitch_diameter) =
            diameters.unwrap_or((data.major, data.minor, data.pitch_diameter));
        let is_internal = internal.unwrap_or(false);
        let (radial_clearance, flank_clearance) = clearance;
        // The end chamfers, when an end is to be chamfered.
        let chamfer = match chamfer_angle {
            Some(angle)
                if bodies
                    .iter()
                    .any(|(_, _, _, ends)| ends.iter().any(|&(l, h)| l || h)) =>
            {
                // The profile's root diameter, moved by the radial clearance.
                let root = self.modeled.then_some(if is_internal {
                    major + 2.0 * radial_clearance
                } else {
                    minor - 2.0 * radial_clearance
                });
                let diameter = chamfer_diameter
                    .unwrap_or_else(|| default_chamfer_diameter(&data, is_internal, root));
                if let Some(material) = extreme_diameter {
                    if !is_internal && diameter >= material {
                        return Err(format!(
                            "the thread's chamfer diameter {diameter} must be smaller than the \
                             thread's diameter {material}"
                        ));
                    }
                    if is_internal && diameter <= material {
                        return Err(format!(
                            "the thread's chamfer diameter {diameter} must be larger than the \
                             thread's diameter {material}"
                        ));
                    }
                }
                Some((angle, diameter))
            }
            _ => None,
        };
        if !self.modeled && chamfer.is_none() {
            return Ok(output);
        }
        if self.modeled {
            check_clearance(
                data.pitch,
                (major, minor, pitch_diameter),
                is_internal,
                clearance,
            )?;
        }
        for (uid, shape, faces, ends) in bodies {
            let spec = ThreadSpec {
                feature: ctx.uid,
                faces: &faces,
                pitch: data.pitch,
                major,
                minor,
                pitch_diameter,
                right_handed: self.thread.right_handed,
                angle,
                part,
                radial_clearance,
                flank_clearance,
                groove: self.modeled,
                chamfer: chamfer.map(|(angle, diameter)| ThreadChamfer {
                    angle,
                    diameter,
                    ends,
                }),
            };
            if !self.modeled
                && spec
                    .chamfer
                    .as_ref()
                    .is_none_or(|c| c.ends.iter().all(|&(l, h)| !l && !h))
            {
                continue;
            }
            let threaded = ctx
                .kernel
                .modeled_thread(&shape, &spec)
                .map_err(|e| e.to_string())?;
            output.changes.push(BodyChange::Set(uid, threaded));
        }
        Ok(output)
    }
}
