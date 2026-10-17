// SPDX-License-Identifier: MIT
//! Placing and editing BUILD parts (`build_place`, `build_edit`): the
//! source a part is made from, where it goes (a profile's side or end, a
//! frame it mates with, the first part's plane, point or face), the joint
//! that places it, its bound length and its mitres. Everything is planned
//! against the document as it is, then made in one undo step; a refusal
//! changes nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;

use serde::Deserialize;

use super::geometry::{V3, angle_between, dot, inverse, parts, sub, unit, x_axis, z_axis};
use super::parts::{PartInfo, as_input, default_value, designation, source_state};
use super::{BuildMeta, BuildRecord, BuildRole, LibrarySourceDef, SourceId};
use crate::document::{DocState, Document, ModelError};
use crate::expr::Unit;
use crate::features::bound_length::{
    BoundFrom, BoundLengthDef, BoundMode, BoundTo, RayDirection, ShiftAxis,
};
use crate::features::derived_part::{DerivedPartDef, EndCut, Ends, Tilt, evaluate_source};
use crate::features::extrude::Operation;
use crate::features::{
    FeatureDef, FeatureEntry, FrameOverride, GeomRef, JointDef, JointOrigin, OccurrencePath,
    ValueInput,
};
use crate::ids::{ComponentUid, FeatureUid, OccurrenceUid};
use crate::joints::{JointKind, Motion, SlideAxis};
use crate::kernel::{BoundingBox, Kernel};
use crate::library::LibraryRequest;
use crate::parameters::ParamId;
use crate::transform::Transform;

pub(crate) fn refuse(message: impl Into<String>) -> ModelError {
    ModelError::Invalid(message.into())
}

// Inputs.

/// The part a command places: a library's component at a version, or a
/// source the design has.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartInput {
    #[serde(default)]
    pub library: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub component: Option<String>,
    #[serde(default)]
    pub config: Option<String>,
    #[serde(default)]
    pub source: Option<SourceId>,
}

/// Whether a pick on a profile goes on its side or on its end.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceKind {
    #[default]
    Auto,
    Side,
    End,
}

/// The end an offset is measured from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub enum EndFrom {
    #[default]
    #[serde(rename = "a")]
    A,
    #[serde(rename = "b")]
    B,
}

/// Where a part goes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceAt {
    /// A path from the active component (uids or names); empty for its own
    /// geometry.
    #[serde(default)]
    pub occurrence: String,
    #[serde(default)]
    pub geometry: Option<GeomRef>,
    /// Where it was picked, in the active component's coordinates.
    #[serde(default)]
    pub point: Option<V3>,
    #[serde(default)]
    pub place: PlaceKind,
    #[serde(default)]
    pub offset: Option<ValueInput>,
    #[serde(default)]
    pub from: EndFrom,
    /// On an end: the new part's axis, in the active component's
    /// coordinates; along the target's axis when left out.
    #[serde(default)]
    pub direction: Option<V3>,
}

/// What a length is bound to.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindInput {
    #[serde(default)]
    pub occurrence: String,
    pub geometry: GeomRef,
}

/// A length: a value, or bound to what the part's ray meets.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LengthInput {
    Value(ValueInput),
    Bound {
        bind: Box<BindInput>,
        #[serde(default)]
        mode: BoundMode,
        #[serde(default)]
        add: Option<ValueInput>,
    },
}

/// Which ends a mitre cuts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MitreCut {
    #[default]
    New,
    Existing,
    Shared,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MitreInput {
    #[serde(default)]
    pub angle: Option<ValueInput>,
    #[serde(default)]
    pub cut: MitreCut,
}

/// The fields of `build_place` (and of `build_preview` for one).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceInput {
    pub part: PartInput,
    #[serde(default)]
    pub at: Option<PlaceAt>,
    #[serde(default)]
    pub rotate: Option<ValueInput>,
    #[serde(default)]
    pub flip: bool,
    #[serde(default)]
    pub length: Option<LengthInput>,
    #[serde(default)]
    pub mitre: Option<MitreInput>,
    #[serde(default)]
    pub parameters: BTreeMap<String, ValueInput>,
    #[serde(default)]
    pub name: Option<String>,
}

/// An end's cut in `build_edit`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndCutInput {
    pub angle: ValueInput,
    #[serde(default)]
    pub tilt: Option<Tilt>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndsInput {
    /// null squares the end; left out keeps it.
    #[serde(default, deserialize_with = "some")]
    pub a: Option<Option<EndCutInput>>,
    #[serde(default, deserialize_with = "some")]
    pub b: Option<Option<EndCutInput>>,
}

fn some<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditAt {
    #[serde(default)]
    pub offset: Option<ValueInput>,
    #[serde(default)]
    pub from: Option<EndFrom>,
}

/// The fields of `build_edit`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditInput {
    pub component: String,
    #[serde(default)]
    pub at: Option<EditAt>,
    #[serde(default)]
    pub rotate: Option<ValueInput>,
    #[serde(default)]
    pub flip: Option<bool>,
    #[serde(default)]
    pub length: Option<LengthInput>,
    #[serde(default)]
    pub ends: Option<EndsInput>,
    #[serde(default)]
    pub config: Option<String>,
    #[serde(default)]
    pub parameters: BTreeMap<String, ValueInput>,
    /// The part's own end machining (not a connector's): replaces what it
    /// had; an empty list takes it away.
    #[serde(default)]
    pub machining: Option<Vec<OwnMachining>>,
}

/// A hole at one of a part's ends.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnMachining {
    pub end: EndFrom,
    pub hole: super::HoleMeta,
}

/// What a placing command made.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placed {
    pub component: Option<ComponentUid>,
    pub occurrence: Option<OccurrenceUid>,
    pub name: String,
    pub features: Vec<FeatureUid>,
    pub joint: Option<FeatureUid>,
    pub bound: Vec<FeatureUid>,
    pub adds: Vec<super::BuildItem>,
    pub warnings: Vec<String>,
    /// `side`, `end`, `frame`, `geometry` or `grounded`.
    pub place: &'static str,
    /// A panel's fit (`build_panel`): false with the reason.
    pub fits: Option<(bool, String)>,
}

// Planning.

/// The source a command uses: one of the design's, or one it adds.
#[derive(Debug, Clone)]
pub(crate) struct SourceUse {
    pub def: LibrarySourceDef,
    pub new: bool,
    pub meta: BuildMeta,
    pub config: Option<String>,
}

/// The part a pick is on.
#[derive(Debug, Clone)]
pub(crate) struct Target {
    pub path: Vec<OccurrenceUid>,
    pub info: PartInfo,
    /// From its coordinates into the active component's.
    pub placement: Transform,
}

impl Target {
    pub fn part(&self) -> Result<FeatureUid, ModelError> {
        self.info
            .part
            .ok_or_else(|| refuse(format!("{} has no derived part", self.info.name)))
    }

    /// A frame in the active component's coordinates.
    pub fn frame(&self, name: &str) -> Option<Transform> {
        Some(self.placement.after(self.info.mount(name)?))
    }

    pub fn origin(&self, name: &str) -> Result<JointOrigin, ModelError> {
        Ok(JointOrigin {
            occurrence: OccurrencePath(self.path.clone()),
            geometry: GeomRef::Mount {
                feature: self.part()?,
                name: name.to_owned(),
            },
            frame_override: None,
        })
    }
}

/// The ray of a bound length before the joint exists: its start, and
/// whether the joint's slide along the slot shifts it.
#[derive(Debug, Clone)]
pub(crate) struct RayPlan {
    pub from: JointOrigin,
    pub shift: [f64; 3],
    pub shift_by_joint: bool,
    pub direction: RayDirection,
    /// The start and direction now, in the active component's coordinates
    /// (for previews).
    #[allow(dead_code)]
    pub start: V3,
    #[allow(dead_code)]
    pub along: V3,
}

/// How the new part is placed.
#[derive(Debug, Clone)]
pub(crate) struct JointPlan {
    pub kind: JointKind,
    pub slide: SlideAxis,
    /// The new part's frame that mates.
    pub own: String,
    pub b: JointOrigin,
    /// b's frame in the active component's coordinates, at the slide.
    pub frame_b: Transform,
    pub flip: bool,
    pub slide_value: Option<(Motion, ValueInput, f64)>,
    pub ray: Option<RayPlan>,
}

#[derive(Debug, Clone)]
pub(crate) enum Spot {
    Grounded(Transform),
    Joint(Box<JointPlan>),
}

#[derive(Debug, Clone)]
pub(crate) enum LengthPlan {
    None,
    Value(ValueInput, f64),
    Bound {
        to: Box<BoundTo>,
        mode: BoundMode,
        add: Option<ValueInput>,
        first: f64,
    },
}

/// Everything a `build_place` makes, worked out first.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub source: SourceUse,
    pub values: BTreeMap<String, ValueInput>,
    pub length: LengthPlan,
    pub spot: Spot,
    pub own_ends: Ends<ValueInput>,
    /// The target's derived part with its end cut (`existing`, `shared`).
    pub target_ends: Option<(FeatureUid, bool, EndCut<ValueInput>)>,
    pub rotate: f64,
    pub name: Option<String>,
    pub designation: String,
    pub place: &'static str,
    pub warnings: Vec<String>,
    /// The new part's frames and box in its coordinates, at its length.
    pub mounts: BTreeMap<String, Transform>,
    pub bounds: Option<BoundingBox>,
    /// Where the new part goes in the active component's coordinates.
    pub placement: Transform,
    pub target: Option<Target>,
}

/// The value of a command's value in the document: a number as it is
/// (millimetres, radians), a text evaluated with the parameters.
pub(crate) fn value_in<K: Kernel>(
    doc: &Document<K>,
    value: &ValueInput,
    unit: Unit,
) -> Result<f64, ModelError> {
    match value {
        ValueInput::Number(n) if n.is_finite() => Ok(*n),
        ValueInput::Number(n) => Err(refuse(format!("{n} is not a finite number"))),
        ValueInput::Name(text) => {
            let expression = crate::document::split_named(text).map_or(text.as_str(), |(_, e)| e);
            let params = doc.parameters();
            crate::expr::evaluate(expression, params.table(), &params.context(), unit)
                .map(|q| q.value)
                .map_err(|e| refuse(format!("'{text}': {e}")))
        }
    }
}

/// The bounds of shapes.
pub(crate) fn bounds_of<K: Kernel>(kernel: &K, shapes: &[K::Shape]) -> Option<BoundingBox> {
    let mut all: Option<BoundingBox> = None;
    for shape in shapes {
        let b = kernel.bounding_box(shape).ok()??;
        all = Some(match all {
            Some(a) => a.union(&b),
            None => b,
        });
    }
    all
}

impl<K: Kernel> Document<K> {
    /// An occurrence path from the active component.
    pub(crate) fn path_from_active(&self, text: &str) -> Result<Vec<OccurrenceUid>, ModelError> {
        let a = self.assembly();
        crate::joints::find_path_from(a, a.active, text)
            .ok_or_else(|| refuse(format!("no occurrence {text} in {}", a.name(a.active))))
    }

    /// The source a part input names: the design's, else read through the
    /// library resolver (a source the design has at that version is used
    /// again). Refused for a part without `build`.
    pub(crate) fn source_use(&self, part: &PartInput) -> Result<SourceUse, ModelError> {
        let state = self.state();
        let (def, new) =
            match part.source {
                Some(id) => (
                    state.library_sources.get(id).cloned().ok_or_else(|| {
                        refuse(format!("library source {id} is not in the design"))
                    })?,
                    false,
                ),
                None => {
                    let (Some(library), Some(component)) = (&part.library, &part.component) else {
                        return Err(refuse(
                            "the part is a library's component (library, url, rev, component) or a \
                         source of the design (source)",
                        ));
                    };
                    let rev = part.rev.clone().unwrap_or_else(|| "HEAD".to_owned());
                    let resolver = self.link_resolver().ok_or_else(|| {
                        refuse(
                            "no component libraries are set up here (libraries are read by the \
                            application and mitcad-cli)",
                        )
                    })?;
                    let read = resolver
                        .library_source(&LibraryRequest {
                            library: library.clone(),
                            url: part.url.clone(),
                            rev,
                            component: component.clone(),
                        })
                        .map_err(refuse)?;
                    match state
                        .library_sources
                        .find(&read.library, &read.component, &read.rev)
                    {
                        Some(id) => (
                            state.library_sources.get(id).cloned().expect("found"),
                            false,
                        ),
                        None => (
                            LibrarySourceDef::from_library(state.library_sources.next_id(), &read)
                                .map_err(refuse)?,
                            true,
                        ),
                    }
                }
            };
        let Some(build) = &def.build else {
            return Err(refuse(format!(
                "{} has no build: it is no part of the BUILD tab",
                def.label()
            )));
        };
        let meta = BuildMeta::read(build).map_err(|e| refuse(format!("{}: {e}", def.label())))?;
        let design = def
            .design
            .state()
            .map_err(|e| refuse(format!("{}: {e}", def.label())))?;
        if let Some(config) = &part.config
            && design.configurations.row(config).is_none()
        {
            return Err(refuse(format!(
                "{} has no configuration {config}",
                def.label()
            )));
        }
        Ok(SourceUse {
            def,
            new,
            meta,
            config: part.config.clone(),
        })
    }

    /// The BUILD part a pick names, with its placement.
    pub(crate) fn target(&self, path: &[OccurrenceUid]) -> Option<Target> {
        let component =
            crate::joints::path_component(self.assembly(), self.assembly().active, path).ok()?;
        if path.is_empty() {
            return None;
        }
        Some(Target {
            path: path.to_vec(),
            info: self.part_info(component)?,
            placement: self.path_transform(path),
        })
    }

    /// Plans a `build_place`.
    pub(crate) fn plan_place(&self, input: &PlaceInput) -> Result<Plan, ModelError> {
        let source = self.source_use(&input.part)?;
        let label = source.def.label();
        match source.meta.role {
            BuildRole::Panel => {
                return Err(refuse(format!(
                    "{label} is a panel: it is placed in a frame with build_panel"
                )));
            }
            BuildRole::Connector => {
                return Err(refuse(format!(
                    "{label} is a connector: it is placed at a junction with build_connect"
                )));
            }
            BuildRole::Fastener => {
                return Err(refuse(format!(
                    "{label} is a fastener: it comes with its connector"
                )));
            }
            _ => {}
        }
        let mut warnings = Vec::new();
        // Values of the source's parameters, the length first.
        let mut values = input.parameters.clone();
        let mut numbers = Vec::new();
        for (name, value) in &input.parameters {
            numbers.push((name.clone(), value_in(self, value, Unit::MM)?));
        }
        let config = source.config.as_deref();
        let length = match (&source.meta.length, &input.length) {
            (None, None) => LengthPlan::None,
            (None, Some(_)) => {
                return Err(refuse(format!("{label} is not cut to length")));
            }
            (Some(meta), input_length) => {
                let default =
                    default_value(&source.def, config, &meta.parameter).ok_or_else(|| {
                        refuse(format!(
                            "{label}: its length parameter {} does not exist",
                            meta.parameter
                        ))
                    })?;
                match input_length {
                    None => LengthPlan::Value(ValueInput::Number(default), default),
                    Some(LengthInput::Value(value)) => {
                        let length = value_in(self, value, Unit::MM)?;
                        if let Some(problem) = meta.outside(length) {
                            return Err(refuse(problem));
                        }
                        LengthPlan::Value(value.clone(), length)
                    }
                    Some(LengthInput::Bound { bind, mode, add }) => LengthPlan::Bound {
                        to: Box::new(BoundTo {
                            occurrence: OccurrencePath(self.path_from_active(&bind.occurrence)?),
                            geometry: bind.geometry.clone(),
                        }),
                        mode: *mode,
                        add: add.clone(),
                        first: default,
                    },
                }
            }
        };
        let length_value = match &length {
            LengthPlan::Value(_, v) => Some(*v),
            LengthPlan::Bound { first, .. } => Some(*first),
            LengthPlan::None => None,
        };
        if let (Some(meta), LengthPlan::Value(value, _)) = (&source.meta.length, &length) {
            values.insert(meta.parameter.clone(), value.clone());
        }
        if let (Some(meta), Some(v)) = (&source.meta.length, length_value) {
            numbers.retain(|(n, _)| *n != meta.parameter);
            numbers.push((meta.parameter.clone(), v));
        }
        // The new part's frames and box.
        let state = source_state(&source.def, config, &numbers)
            .map_err(|e| refuse(format!("{label}: {e}")))?;
        let evaluated =
            evaluate_source(self.kernel(), &state).map_err(|e| refuse(format!("{label}: {e}")))?;
        let bounds = bounds_of(self.kernel(), &evaluated.bodies);
        let mounts = evaluated.mounts;
        let rotate = match &input.rotate {
            Some(value) => value_in(self, value, Unit::DEG)?,
            None => 0.0,
        };
        let quarter = rotate / FRAC_PI_2;
        if (quarter - quarter.round()).abs() > 1e-6 {
            return Err(refuse("the rotation is 0, 90, 180 or 270 degrees"));
        }
        let touch = if input.flip { "end_b" } else { "end_a" };
        let at = input.at.clone().unwrap_or_default();
        let mut target = None;
        let (spot, place) = match &at.geometry {
            None => {
                let point = at.point.unwrap_or([0.0; 3]);
                (Spot::Grounded(Transform::translation(point)), "grounded")
            }
            Some(geometry) => {
                let path = self.path_from_active(&at.occurrence)?;
                match self.target(&path) {
                    Some(t) => {
                        let (spot, place) =
                            self.plan_on_part(&source, &t, geometry, &at, touch, &mounts, &length)?;
                        target = Some(t);
                        (spot, place)
                    }
                    None => (
                        self.plan_on_geometry(&source, &path, geometry, &at, touch, &mounts)?,
                        "geometry",
                    ),
                }
            }
        };
        // Where the new part goes.
        let placement = match &spot {
            Spot::Grounded(t) => *t,
            Spot::Joint(j) => {
                let own = mounts.get(&j.own).copied().unwrap_or(Transform::IDENTITY);
                j.frame_b
                    .after(&crate::joints::alignment(rotate, 0.0, j.flip))
                    .after(&inverse(&own))
            }
        };
        // Mitres.
        let mut own_ends = Ends { a: None, b: None };
        let mut target_ends = None;
        if let Some(mitre) = &input.mitre {
            let Some(target) = &target else {
                return Err(refuse("a mitre needs a profile's side or end to meet"));
            };
            if source.meta.length.is_none() {
                return Err(refuse(format!(
                    "{label} is not cut to length: it has no mitres"
                )));
            }
            let Spot::Joint(joint) = &spot else {
                return Err(refuse("a mitre needs a profile's side or end to meet"));
            };
            let own_end_b = joint.own == "end_b";
            if joint.own != "end_a" && joint.own != "end_b" {
                return Err(refuse("a mitre is cut at the end that meets the profile"));
            }
            // The new part's lean and the target's, in their coordinates.
            let to_new = |v: V3| inverse(&placement).apply_vector(v);
            let angle = |what: &str| -> Result<(ValueInput, f64), ModelError> {
                let value = mitre
                    .angle
                    .clone()
                    .ok_or_else(|| refuse(format!("the {what} mitre needs its angle")))?;
                let radians = value_in(self, &value, Unit::DEG)?;
                Ok((value, radians))
            };
            match place {
                "side" => {
                    if mitre.cut != MitreCut::New {
                        return Err(refuse(
                            "on a side only the new part's end is mitred (cut \"new\")",
                        ));
                    }
                    let (value, _) = angle("new")?;
                    let tilt = Tilt::towards(to_new(x_axis(&joint.frame_b)));
                    let cut = EndCut { angle: value, tilt };
                    if own_end_b {
                        own_ends.b = Some(cut);
                    } else {
                        own_ends.a = Some(cut);
                    }
                }
                "end" => {
                    let end_name = target_end_name(&joint.b);
                    let end_frame = target.frame(&end_name).ok_or_else(|| {
                        refuse(format!("{} has no frame {end_name}", target.info.name))
                    })?;
                    let u = z_axis(&end_frame);
                    let v = z_axis(&joint.frame_b);
                    let target_lean = inverse(&target.placement).apply_vector(v);
                    let own_lean = to_new(super::geometry::scale(u, -1.0));
                    let target_part = target.part()?;
                    let target_b = end_name == "end_b";
                    let (own_angle, target_angle) = match mitre.cut {
                        MitreCut::New => (Some(angle("new")?.0), None),
                        MitreCut::Existing => (None, Some(angle("existing")?.0)),
                        MitreCut::Shared => {
                            let half = angle_between(u, v) / 2.0;
                            if half < 1e-9 {
                                warnings.push(
                                    "the parts are on one axis: a shared mitre cuts nothing"
                                        .to_owned(),
                                );
                                (None, None)
                            } else {
                                (
                                    Some(ValueInput::Number(half)),
                                    Some(ValueInput::Number(half)),
                                )
                            }
                        }
                    };
                    if let Some(value) = own_angle {
                        let cut = EndCut {
                            angle: value,
                            tilt: Tilt::towards(own_lean),
                        };
                        if own_end_b {
                            own_ends.b = Some(cut);
                        } else {
                            own_ends.a = Some(cut);
                        }
                    }
                    if let Some(value) = target_angle {
                        if !target.info.is_cut_to_length() {
                            return Err(refuse(format!(
                                "{} is not cut to length: it has no mitres",
                                target.info.name
                            )));
                        }
                        target_ends = Some((
                            target_part,
                            target_b,
                            EndCut {
                                angle: value,
                                tilt: Tilt::towards(target_lean),
                            },
                        ));
                    }
                }
                _ => return Err(refuse("a mitre needs a profile's side or end to meet")),
            }
        }
        let mut shown = BTreeMap::new();
        for (name, v) in &numbers {
            shown.insert(name.clone(), *v);
        }
        let designation = designation(&source.def, config, &shown);
        Ok(Plan {
            source,
            values,
            length,
            spot,
            own_ends,
            target_ends,
            rotate,
            name: input.name.clone(),
            designation,
            place,
            warnings,
            mounts,
            bounds,
            placement,
            target,
        })
    }

    /// Plans a part on a BUILD part: on a side's slot, on an end, or on a
    /// frame it mates with.
    #[allow(clippy::too_many_arguments)]
    fn plan_on_part(
        &self,
        source: &SourceUse,
        target: &Target,
        geometry: &GeomRef,
        at: &PlaceAt,
        touch: &str,
        mounts: &BTreeMap<String, Transform>,
        length: &LengthPlan,
    ) -> Result<(Spot, &'static str), ModelError> {
        let label = source.def.label();
        let target_part = target.part()?;
        let point_local = at.point.map(|p| inverse(&target.placement).apply_point(p));
        // What was picked: a frame or a face.
        enum Pick {
            End(String),
            Slot(String),
            Frame(String),
            Face(Option<(crate::ids::BodyUid, crate::topo::FaceName)>),
        }
        let pick = match geometry {
            GeomRef::Mount { feature, name } => {
                if *feature != target_part {
                    return Err(refuse(format!(
                        "{geometry} is not a frame of {}",
                        target.info.name
                    )));
                }
                match target.info.frame_kind(name).as_str() {
                    "end" => Pick::End(name.clone()),
                    "slot" => Pick::Slot(name.clone()),
                    _ => Pick::Frame(name.clone()),
                }
            }
            GeomRef::Face { body, face } => {
                if face.feature == target_part && matches!(face.role.as_str(), "end_a" | "end_b") {
                    Pick::End(face.role.clone())
                } else {
                    Pick::Face(Some((*body, face.clone())))
                }
            }
            _ => Pick::Face(None),
        };
        let pick = match (at.place, pick) {
            (PlaceKind::Side, Pick::End(_)) => Pick::Face(None),
            (PlaceKind::End, Pick::Slot(_) | Pick::Face(_)) => {
                Pick::End(self.nearest_end(target, point_local)?)
            }
            (_, pick) => pick,
        };
        // A part that moves on another's frame (a carriage on its rail).
        let mate = source.meta.linear.as_ref().and_then(|l| l.mate.clone());
        if let Some(mate) = &mate {
            let on = match &pick {
                Pick::Frame(name) | Pick::Slot(name) | Pick::End(name)
                    if target.info.frame_kind(name) == mate.on =>
                {
                    Some(name.clone())
                }
                _ => target
                    .info
                    .mounts
                    .keys()
                    .find(|name| target.info.frame_kind(name) == mate.on)
                    .cloned(),
            };
            if let Some(on) = on {
                return self
                    .plan_mate(source, target, &on, &mate.frame, Some(mate), at, mounts)
                    .map(|spot| (spot, "frame"));
            }
        }
        match pick {
            Pick::End(end) => {
                if source.meta.role != BuildRole::Profile
                    && source.meta.role != BuildRole::Cap
                    && source.meta.length.is_none()
                {
                    return Err(refuse(format!(
                        "{label} cannot go on an end ({} is no profile)",
                        source.meta.role
                    )));
                }
                self.check_fits(source, target, &end)?;
                let own = if source.meta.length.is_some() {
                    touch.to_owned()
                } else {
                    own_frame(&source.meta, mounts, &["end", "face"]).ok_or_else(|| {
                        refuse(format!("{label} has no frame that goes on an end"))
                    })?
                };
                let end_frame_local = *target
                    .info
                    .mount(&end)
                    .ok_or_else(|| refuse(format!("{} has no frame {end}", target.info.name)))?;
                let bend = at
                    .direction
                    .and_then(unit)
                    .map(|d| unit(inverse(&target.placement).apply_vector(d)).unwrap_or(d));
                let bend = bend.filter(|d| dot(*d, z_axis(&end_frame_local)) < 1.0 - 1e-9);
                let mut b = target.origin(&end)?;
                let mut local = end_frame_local;
                if let Some(d) = bend {
                    // The end frame's first axis that is not along the new
                    // direction keeps the turned frame's roll.
                    let [_, ex, ey, ez] = parts(&end_frame_local);
                    let hint = [ex, ey, ez]
                        .into_iter()
                        .find(|a| dot(*a, d).abs() < 1.0 - 1e-6)
                        .unwrap_or(ex);
                    let o = FrameOverride {
                        origin: None,
                        z_axis: Some(d),
                        x_axis: Some(hint),
                    };
                    local = crate::joints::apply_override(&end_frame_local, &o).map_err(refuse)?;
                    b.frame_override = Some(o);
                }
                let frame_b = target.placement.after(&local);
                // Mating frames face each other: z against z.
                let flip = true;
                let ray = match length {
                    LengthPlan::Bound { .. } => {
                        let direction = match bend {
                            Some(d) => {
                                RayDirection::Vector(inverse(&end_frame_local).apply_vector(d))
                            }
                            None => RayDirection::default(),
                        };
                        Some(RayPlan {
                            from: target.origin(&end)?,
                            shift: [0.0; 3],
                            shift_by_joint: false,
                            direction,
                            start: frame_b.translation,
                            along: z_axis(&frame_b),
                        })
                    }
                    _ => None,
                };
                Ok((
                    Spot::Joint(Box::new(JointPlan {
                        kind: JointKind::Rigid,
                        slide: SlideAxis::Z,
                        own,
                        b,
                        frame_b,
                        flip,
                        slide_value: None,
                        ray,
                    })),
                    "end",
                ))
            }
            Pick::Frame(name) => self
                .plan_mate(source, target, &name, "", None, at, mounts)
                .map(|spot| (spot, "frame")),
            Pick::Slot(slot) => self.plan_side(source, target, &slot, at, touch, mounts, length),
            Pick::Face(face) => {
                let slot = self.pick_slot(target, face.as_ref(), point_local)?;
                self.plan_side(source, target, &slot, at, touch, mounts, length)
            }
        }
    }

    /// Refuses a part whose systems do not fit the target, or a frame that
    /// does not accept it.
    fn check_fits(
        &self,
        source: &SourceUse,
        target: &Target,
        frame: &str,
    ) -> Result<(), ModelError> {
        let label = source.def.label();
        if !source.meta.fits(target.info.meta.system.as_ref()) {
            let fits: Vec<String> = source.meta.systems.iter().map(|s| s.label()).collect();
            return Err(refuse(format!(
                "the systems do not fit: {label} fits {}, {} is {}",
                fits.join(", "),
                target.info.name,
                target
                    .info
                    .meta
                    .system
                    .as_ref()
                    .map_or_else(|| "of no system".to_owned(), |s| s.label())
            )));
        }
        if let Some(meta) = target.info.meta.frame(frame)
            && !meta.accepts(source.meta.role)
        {
            return Err(refuse(format!(
                "frame {frame} of {} does not take a {}",
                target.info.name, source.meta.role
            )));
        }
        Ok(())
    }

    /// The end frame of a target nearest to a point (its own coordinates).
    fn nearest_end(&self, target: &Target, point: Option<V3>) -> Result<String, ModelError> {
        let mut ends: Vec<(&String, &Transform)> = target
            .info
            .mounts
            .iter()
            .filter(|(n, _)| target.info.frame_kind(n) == "end")
            .collect();
        if ends.is_empty() {
            return Err(refuse(format!("{} has no end frames", target.info.name)));
        }
        if let Some(p) = point {
            ends.sort_by(|a, b| {
                let d = |t: &Transform| super::geometry::norm(sub(t.translation, p));
                d(a.1).total_cmp(&d(b.1))
            });
        }
        Ok(ends[0].0.clone())
    }

    /// The slot of a picked side face nearest to the point: of the slots
    /// on the face's plane when the face is planar (none: a closed side).
    fn pick_slot(
        &self,
        target: &Target,
        face: Option<&(crate::ids::BodyUid, crate::topo::FaceName)>,
        point: Option<V3>,
    ) -> Result<String, ModelError> {
        let mut slots = target.info.slots();
        if let Some((body, face)) = face
            && let Some(shape) = self.body_shape(*body)
            && let Ok(crate::datum::SurfaceGeometry::Plane { normal, .. }) =
                self.kernel().face_geometry(shape, face)
            && let Some(n) = unit(normal)
        {
            slots.retain(|(_, t)| dot(z_axis(t), n) > 1.0 - 1e-6);
            if slots.is_empty() {
                return Err(refuse(format!(
                    "the side has no slot frame: face {face} of {} is a closed side",
                    target.info.name
                )));
            }
        }
        if slots.is_empty() {
            return Err(refuse(format!(
                "{} has no slot frames to place on",
                target.info.name
            )));
        }
        if let Some(p) = point {
            let distance = |t: &Transform| {
                let [o, x, _, _] = parts(t);
                let along = dot(sub(p, o), x);
                super::geometry::norm(sub(sub(p, o), super::geometry::scale(x, along)))
            };
            slots.sort_by(|a, b| distance(a.1).total_cmp(&distance(b.1)));
        }
        Ok(slots[0].0.clone())
    }

    /// Plans a part on a slot of a profile's side, at the offset from an
    /// end along it.
    #[allow(clippy::too_many_arguments)]
    fn plan_side(
        &self,
        source: &SourceUse,
        target: &Target,
        slot: &str,
        at: &PlaceAt,
        touch: &str,
        mounts: &BTreeMap<String, Transform>,
        length: &LengthPlan,
    ) -> Result<(Spot, &'static str), ModelError> {
        let label = source.def.label();
        self.check_fits(source, target, slot)?;
        let slot_local = *target
            .info
            .mount(slot)
            .ok_or_else(|| refuse(format!("{} has no frame {slot}", target.info.name)))?;
        let target_length = target.info.length;
        let point_local = at.point.map(|p| inverse(&target.placement).apply_point(p));
        let (value, tx) = self.slide_value(at, &slot_local, point_local, target_length)?;
        // A profile stands on the slot with its end; another part with its
        // frame that mounts on a slot.
        let mounting = own_frame(&source.meta, mounts, &["slot", "face", "hole"]);
        let own = match mounting {
            Some(own) if source.meta.role != BuildRole::Profile => own,
            _ if source.meta.length.is_some() && mounts.contains_key(touch) => touch.to_owned(),
            Some(own) => own,
            None => {
                return Err(refuse(format!(
                    "{label} has no frame that mounts on a slot"
                )));
            }
        };
        // Mating frames face each other: z against z.
        let flip = true;
        let frame_b = target
            .placement
            .after(&slot_local)
            .after(&Transform::translation([tx, 0.0, 0.0]));
        let ray = match length {
            LengthPlan::Bound { .. } if own == touch => Some(RayPlan {
                from: target.origin(slot)?,
                shift: [tx, 0.0, 0.0],
                shift_by_joint: true,
                direction: RayDirection::default(),
                start: frame_b.translation,
                along: z_axis(&frame_b),
            }),
            LengthPlan::Bound { .. } => {
                return Err(refuse(format!(
                    "{label} stands on the slot with no end: its length cannot be bound"
                )));
            }
            _ => None,
        };
        Ok((
            Spot::Joint(Box::new(JointPlan {
                kind: JointKind::Slider,
                slide: SlideAxis::X,
                own,
                b: target.origin(slot)?,
                frame_b,
                flip,
                slide_value: Some((Motion::Tx, value, tx)),
                ray,
            })),
            "side",
        ))
    }

    /// The offset along a slot or rail: `at.offset` from end A or B, else
    /// the pick's distance along it, between the ends.
    fn slide_value(
        &self,
        at: &PlaceAt,
        frame_local: &Transform,
        point_local: Option<V3>,
        target_length: Option<(ParamId, f64)>,
    ) -> Result<(ValueInput, f64), ModelError> {
        let along = |p: V3| dot(sub(p, frame_local.translation), x_axis(frame_local));
        let clamp = |v: f64| match target_length {
            Some((_, l)) => v.clamp(0.0, l),
            None => v,
        };
        Ok(match (&at.offset, at.from) {
            (Some(offset), EndFrom::A) => (offset.clone(), value_in(self, offset, Unit::MM)?),
            (Some(offset), EndFrom::B) => {
                let Some((id, l)) = target_length else {
                    return Err(refuse("the part has no end B to measure from"));
                };
                let distance = value_in(self, offset, Unit::MM)?;
                let name = self.parameters().name(id);
                let text = match offset {
                    ValueInput::Number(n) => format!("{name} - {}", super::round(*n)),
                    ValueInput::Name(text) => format!("{name} - ({text})"),
                };
                (ValueInput::Name(text), l - distance)
            }
            (None, _) => {
                let v = clamp(point_local.map_or(0.0, along));
                (ValueInput::Number(v), v)
            }
        })
    }

    /// Plans a part whose frame mates with a frame of the target (a
    /// carriage on a rail, a cap on an end, a plate on a frame): the joint
    /// the part's `mate` says, else rigid.
    #[allow(clippy::too_many_arguments)]
    fn plan_mate(
        &self,
        source: &SourceUse,
        target: &Target,
        on: &str,
        own: &str,
        mate: Option<&super::LinearMate>,
        at: &PlaceAt,
        mounts: &BTreeMap<String, Transform>,
    ) -> Result<Spot, ModelError> {
        let label = source.def.label();
        self.check_fits(source, target, on)?;
        let own = if own.is_empty() || !mounts.contains_key(own) {
            let kind = target.info.frame_kind(on);
            own_frame(&source.meta, mounts, &[kind.as_str(), "face", "slot"])
                .or_else(|| mounts.keys().next().cloned())
                .ok_or_else(|| refuse(format!("{label} has no frames")))?
        } else {
            own.to_owned()
        };
        let (kind, slide) = match mate {
            Some(mate) => (
                serde_json::from_value::<JointKind>(serde_json::json!(mate.joint))
                    .unwrap_or(JointKind::Slider),
                serde_json::from_value::<SlideAxis>(serde_json::json!(mate.axis))
                    .unwrap_or(SlideAxis::X),
            ),
            None => (JointKind::Rigid, SlideAxis::Z),
        };
        let on_local = *target
            .info
            .mount(on)
            .ok_or_else(|| refuse(format!("{} has no frame {on}", target.info.name)))?;
        let motion = match kind {
            JointKind::Slider => Some(slide.motion()),
            JointKind::Cylindrical => Some(Motion::Tz),
            _ => None,
        };
        let point_local = at.point.map(|p| inverse(&target.placement).apply_point(p));
        let slide_value = match motion {
            Some(Motion::Tx) => {
                let (value, v) =
                    self.slide_value(at, &on_local, point_local, target.info.length)?;
                Some((Motion::Tx, value, v))
            }
            Some(m) => {
                let v = match &at.offset {
                    Some(offset) => (offset.clone(), value_in(self, offset, Unit::MM)?),
                    None => (ValueInput::Number(0.0), 0.0),
                };
                Some((m, v.0, v.1))
            }
            None => None,
        };
        let shift = match &slide_value {
            Some((m, _, v)) => {
                let axis = match m {
                    Motion::Tx => [*v, 0.0, 0.0],
                    Motion::Ty => [0.0, *v, 0.0],
                    _ => [0.0, 0.0, *v],
                };
                Transform::translation(axis)
            }
            None => Transform::IDENTITY,
        };
        // Mating frames face each other: z against z.
        let flip = true;
        Ok(Spot::Joint(Box::new(JointPlan {
            kind,
            slide,
            own,
            b: target.origin(on)?,
            frame_b: target.placement.after(&on_local).after(&shift),
            flip,
            slide_value,
            ray: None,
        })))
    }

    /// Plans the first part: on a plane, a point or a face (of the active
    /// component, or of an occurrence that is no BUILD part), at the
    /// picked point.
    fn plan_on_geometry(
        &self,
        source: &SourceUse,
        path: &[OccurrenceUid],
        geometry: &GeomRef,
        at: &PlaceAt,
        touch: &str,
        mounts: &BTreeMap<String, Transform>,
    ) -> Result<Spot, ModelError> {
        let label = source.def.label();
        let local = crate::joints::origin_frame(&mut self.resolver(), geometry)
            .map_err(|e| refuse(format!("{geometry}: {e}")))?;
        let placement = self.path_transform(path);
        let point_local = at.point.map(|p| inverse(&placement).apply_point(p));
        let own = if source.meta.length.is_some() {
            touch.to_owned()
        } else {
            own_frame(&source.meta, mounts, &["end", "face", "slot"])
                .or_else(|| mounts.keys().next().cloned())
                .ok_or_else(|| refuse(format!("{label} has no frames")))?
        };
        let override_frame = point_local.map(|p| FrameOverride {
            origin: Some(p),
            z_axis: None,
            x_axis: None,
        });
        let mut at_point = local;
        if let Some(o) = &override_frame {
            at_point = crate::joints::apply_override(&local, o).map_err(refuse)?;
        }
        let shift = point_local.map_or([0.0; 3], |p| inverse(&local).apply_point(p));
        let frame_b = placement.after(&at_point);
        Ok(Spot::Joint(Box::new(JointPlan {
            kind: JointKind::Rigid,
            slide: SlideAxis::Z,
            own,
            b: JointOrigin {
                occurrence: OccurrencePath(path.to_vec()),
                geometry: geometry.clone(),
                frame_override: override_frame,
            },
            frame_b,
            flip: true,
            slide_value: None,
            ray: Some(RayPlan {
                from: JointOrigin {
                    occurrence: OccurrencePath(path.to_vec()),
                    geometry: geometry.clone(),
                    frame_override: None,
                },
                shift: [shift[0], shift[1], 0.0],
                shift_by_joint: false,
                direction: RayDirection::default(),
                start: frame_b.translation,
                along: z_axis(&frame_b),
            }),
        })))
    }

    /// Places a part (`build_place`).
    pub fn build_place(&mut self, input: &PlaceInput) -> Result<Placed, ModelError> {
        let plan = self.plan_place(input)?;
        let active = self.active_component();
        self.state().editable(active)?;
        let label = format!(
            "Place {}",
            if plan.designation.is_empty() {
                plan.source.def.label()
            } else {
                plan.designation.clone()
            }
        );
        self.apply(|state| {
            let placed = make_part(state, &plan, active)?;
            Ok((label, placed))
        })
    }

    /// The `build_preview` of a placing: what `build_place` would do, the
    /// document left as it is.
    pub(crate) fn preview_place(&self, input: &PlaceInput) -> Result<Plan, ModelError> {
        self.plan_place(input)
    }
}

/// The name of the target's end a joint's b side names.
fn target_end_name(b: &JointOrigin) -> String {
    match &b.geometry {
        GeomRef::Mount { name, .. } => name.clone(),
        _ => "end_b".to_owned(),
    }
}

/// The first frame of the part of one of the kinds (in that order).
pub(crate) fn own_frame(
    meta: &BuildMeta,
    mounts: &BTreeMap<String, Transform>,
    kinds: &[&str],
) -> Option<String> {
    for kind in kinds {
        if let Some(frame) = meta
            .frames
            .iter()
            .find(|f| f.kind == *kind && mounts.contains_key(&f.name))
        {
            return Some(frame.name.clone());
        }
    }
    None
}

/// Adds a feature and returns its uid.
pub(crate) fn add_feature(
    state: &mut DocState,
    def: FeatureDef<ValueInput>,
    component: ComponentUid,
) -> Result<FeatureUid, ModelError> {
    Ok(state.add_feature_in(&def, None, component)?.uid)
}

/// Moves a feature to just before another one.
pub(crate) fn move_before(state: &mut DocState, uid: FeatureUid, before: FeatureUid) {
    let Some(from) = state.position(uid) else {
        return;
    };
    let entry = state.features.remove(from);
    let to = state.position(before).unwrap_or(state.features.len());
    state.features.insert(to, entry);
}

/// The parameter a joint slides by.
fn joint_slide_param(state: &DocState, joint: FeatureUid, motion: Motion) -> Option<ParamId> {
    match &state.entry(joint)?.def {
        FeatureDef::Joint(j) => j.position.get(&motion).copied(),
        _ => None,
    }
}

/// Edits a derived part's definition in place: its values and ends.
pub(crate) fn edit_part(
    state: &mut DocState,
    part: FeatureUid,
    edit: impl FnOnce(&mut DerivedPartDef<ValueInput>),
) -> Result<(), ModelError> {
    let entry = state
        .entry(part)
        .ok_or(ModelError::NoSuchFeature(part))?
        .clone();
    let FeatureDef::DerivedPart(_) = &entry.def else {
        return Err(refuse(format!("{} is no derived part", entry.name)));
    };
    let FeatureDef::DerivedPart(mut def) = as_input(&entry.def, &state.parameters) else {
        unreachable!("a derived part stays one")
    };
    edit(&mut def);
    state.edit_feature(part, &FeatureDef::DerivedPart(def))?;
    Ok(())
}

/// Sets a derived part's length to a parameter (a bound length's), the
/// one it had going when nothing else uses it.
pub(crate) fn set_part_length(
    state: &mut DocState,
    part: FeatureUid,
    name: &str,
    id: ParamId,
) -> Result<(), ModelError> {
    let position = state
        .position(part)
        .ok_or(ModelError::NoSuchFeature(part))?;
    let mut entry: FeatureEntry = (*state.features[position]).clone();
    let FeatureDef::DerivedPart(def) = &mut entry.def else {
        return Err(refuse(format!("{} is no derived part", entry.name)));
    };
    def.parameters.insert(name.to_owned(), id);
    state.features[position] = Arc::new(entry);
    state.release_params(part);
    Ok(())
}

/// Adds a bound length for a part before its derived part; returns it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_bound(
    state: &mut DocState,
    component: ComponentUid,
    part: FeatureUid,
    length_name: &str,
    ray: &RayPlan,
    shift_x: Option<ParamId>,
    to: &BoundTo,
    mode: BoundMode,
    add: Option<ValueInput>,
    first: f64,
) -> Result<FeatureUid, ModelError> {
    let mut shift = BTreeMap::new();
    match shift_x {
        Some(id) => {
            shift.insert(ShiftAxis::X, ValueInput::Name(state.parameters.name(id)));
        }
        None if ray.shift[0].abs() > 1e-12 => {
            shift.insert(ShiftAxis::X, ValueInput::Number(ray.shift[0]));
        }
        None => {}
    }
    if ray.shift[1].abs() > 1e-12 {
        shift.insert(ShiftAxis::Y, ValueInput::Number(ray.shift[1]));
    }
    if ray.shift[2].abs() > 1e-12 {
        shift.insert(ShiftAxis::Z, ValueInput::Number(ray.shift[2]));
    }
    let def = BoundLengthDef {
        parameter: ValueInput::Number(first),
        from: BoundFrom {
            occurrence: ray.from.occurrence.clone(),
            geometry: ray.from.geometry.clone(),
            shift,
        },
        direction: ray.direction,
        to: to.clone(),
        mode,
        add,
    };
    let bound = add_feature(state, FeatureDef::BoundLength(def), component)?;
    move_before(state, bound, part);
    let parameter = match &state.entry(bound).expect("added").def {
        FeatureDef::BoundLength(b) => b.parameter,
        _ => unreachable!("a bound length"),
    };
    set_part_length(state, part, length_name, parameter)?;
    let position = state.position(bound).expect("added");
    let entry = state.features[position].clone();
    state.check_feature(position, &entry).map_err(refuse)?;
    Ok(bound)
}

/// Makes what a plan says, in `state`: the source, the component, its
/// occurrence, the derived part, the joint, the bound length, the mitres
/// and the record.
pub(crate) fn make_part(
    state: &mut DocState,
    plan: &Plan,
    active: ComponentUid,
) -> Result<Placed, ModelError> {
    let source_id = plan.source.def.id;
    if plan.source.new {
        state.library_sources.0.push(plan.source.def.clone());
    }
    let wanted = plan.name.clone().unwrap_or_else(|| {
        if plan.designation.is_empty() {
            plan.source.def.label()
        } else {
            plan.designation.clone()
        }
    });
    let name = match &plan.name {
        Some(name) => {
            state.check_component_name(name, ComponentUid(u32::MAX))?;
            name.clone()
        }
        None => state.free_component_name(&wanted),
    };
    let made = state.assembly.create(Some(&name), None);
    let (transform, grounded) = match &plan.spot {
        Spot::Grounded(t) => (*t, true),
        Spot::Joint(_) => (Transform::IDENTITY, false),
    };
    let occurrence = state.assembly.place(made, active, transform);
    if grounded && let Some(o) = state.assembly.occurrence_mut(occurrence) {
        o.grounded = true;
    }
    let mut features = Vec::new();
    let part_def = DerivedPartDef {
        source: source_id,
        config: plan.source.config.clone(),
        parameters: plan.values.clone(),
        ends: plan.own_ends.clone(),
        operation: Operation::NewBody,
    };
    let part = add_feature(state, FeatureDef::DerivedPart(part_def), made)?;
    features.push(part);
    let mut joint = None;
    let mut slide_param = None;
    if let Spot::Joint(plan_joint) = &plan.spot {
        let mut position = BTreeMap::new();
        if let Some((motion, value, _)) = &plan_joint.slide_value {
            position.insert(*motion, value.clone());
        }
        let def = JointDef {
            kind: plan_joint.kind,
            slide_axis: plan_joint.slide,
            a: JointOrigin {
                occurrence: OccurrencePath(vec![occurrence]),
                geometry: GeomRef::Mount {
                    feature: part,
                    name: plan_joint.own.clone(),
                },
                frame_override: None,
            },
            b: plan_joint.b.clone(),
            offset: None,
            angle: (plan.rotate.abs() > 1e-12).then_some(ValueInput::Number(plan.rotate)),
            flip: plan_joint.flip,
            limits: BTreeMap::new(),
            position,
        };
        let uid = add_feature(state, FeatureDef::Joint(def), active)?;
        features.push(uid);
        joint = Some(uid);
        slide_param = plan_joint
            .slide_value
            .as_ref()
            .and_then(|(motion, _, _)| joint_slide_param(state, uid, *motion));
    }
    let mut bound = Vec::new();
    if let LengthPlan::Bound {
        to,
        mode,
        add,
        first,
    } = &plan.length
    {
        let ray = match &plan.spot {
            Spot::Joint(j) => j.ray.clone(),
            Spot::Grounded(t) => Some(RayPlan {
                from: JointOrigin {
                    occurrence: OccurrencePath::default(),
                    geometry: GeomRef::Origin(crate::datum::OriginDatum::Xy),
                    frame_override: None,
                },
                shift: t.translation,
                shift_by_joint: false,
                direction: RayDirection::default(),
                start: t.translation,
                along: super::geometry::Z,
            }),
        }
        .ok_or_else(|| refuse("the length cannot be bound here"))?;
        let length_name = plan
            .source
            .meta
            .length
            .as_ref()
            .map(|l| l.parameter.clone())
            .ok_or_else(|| refuse("the part is not cut to length"))?;
        let shift_x = if ray.shift_by_joint {
            slide_param
        } else {
            None
        };
        let uid = add_bound(
            state,
            active,
            part,
            &length_name,
            &ray,
            shift_x,
            to,
            *mode,
            add.clone(),
            *first,
        )?;
        features.insert(0, uid);
        bound.push(uid);
    }
    if let Some((target_part, b, cut)) = &plan.target_ends {
        let cut = cut.clone();
        edit_part(state, *target_part, |def| {
            if *b {
                def.ends.b = Some(cut);
            } else {
                def.ends.a = Some(cut);
            }
        })?;
    }
    let record = BuildRecord {
        role: plan.source.meta.role,
        source: Some(source_id),
        part: Some(part),
        joint,
        bound: bound.clone(),
        added_by: None,
        adds: Vec::new(),
        junction: None,
    };
    state.assembly.component_mut(made).expect("made").build = Some(record);
    Ok(Placed {
        component: Some(made),
        occurrence: Some(occurrence),
        name,
        features,
        joint,
        bound,
        adds: Vec::new(),
        warnings: plan.warnings.clone(),
        place: plan.place,
        fits: None,
    })
}

// Editing.

impl<K: Kernel> Document<K> {
    /// Edits a BUILD part (`build_edit`); returns what changed.
    pub fn build_edit(&mut self, input: &EditInput) -> Result<Vec<String>, ModelError> {
        let component = self.component_by_ref(&input.component)?;
        let info = self
            .part_info(component)
            .ok_or_else(|| refuse(format!("{} is no BUILD part", input.component)))?;
        let part = info
            .part
            .ok_or_else(|| refuse(format!("{} has no derived part", info.name)))?;
        let length_name = info.meta.length.as_ref().map(|l| l.parameter.clone());
        // Checks that need the document as it is.
        let mut bind_plan = None;
        if let Some(LengthInput::Bound { bind, mode, add }) = &input.length {
            let length_name = length_name
                .clone()
                .ok_or_else(|| refuse(format!("{} is not cut to length", info.name)))?;
            let to_path = self.path_from_active(&bind.occurrence)?;
            let to_component =
                crate::joints::path_component(self.assembly(), self.active_component(), &to_path)
                    .map_err(refuse)?;
            self.check_bind(component, to_component)?;
            let ray = self.part_ray(&info)?;
            bind_plan = Some((
                length_name,
                ray,
                BoundTo {
                    occurrence: OccurrencePath(to_path),
                    geometry: bind.geometry.clone(),
                },
                *mode,
                add.clone(),
            ));
        }
        if let Some(LengthInput::Value(value)) = &input.length {
            let length = value_in(self, value, Unit::MM)?;
            if let Some(problem) = info.meta.length.as_ref().and_then(|l| l.outside(length)) {
                return Err(refuse(problem));
            }
        }
        if let Some(config) = &input.config
            && let Some(source) = &info.source
            && source
                .design
                .state()
                .map_err(refuse)?
                .configurations
                .row(config)
                .is_none()
        {
            return Err(refuse(format!(
                "{} has no configuration {config}",
                source.label()
            )));
        }
        // The part's own end machining.
        let machining = match &input.machining {
            Some(list) => {
                let mut defs = Vec::new();
                for m in list {
                    let frame = if m.end == EndFrom::B {
                        "end_b"
                    } else {
                        "end_a"
                    };
                    let local = info
                        .mount(frame)
                        .ok_or_else(|| refuse(format!("{} has no frame {frame}", info.name)))?;
                    let def = super::connect::hole_def(&info, frame, local.translation, &m.hole)
                        .ok_or_else(|| refuse("a hole needs its diameter"))?;
                    let def: FeatureDef<ValueInput> = serde_json::from_value(def)
                        .map_err(|e| refuse(format!("the hole: {e}")))?;
                    defs.push(def);
                }
                Some(defs)
            }
            None => None,
        };
        let joint = info.record.joint;
        let label = format!("Edit {}", info.name);
        let rotate = match &input.rotate {
            Some(value) => {
                let angle = value_in(self, value, Unit::DEG)?;
                let quarter = angle / FRAC_PI_2;
                if (quarter - quarter.round()).abs() > 1e-6 {
                    return Err(refuse("the rotation is 0, 90, 180 or 270 degrees"));
                }
                Some(angle)
            }
            None => None,
        };
        let offset = match &input.at {
            Some(EditAt {
                offset: Some(offset),
                from,
            }) => {
                let joint = joint.ok_or_else(|| refuse(format!("{} has no joint", info.name)))?;
                let (target_length, target_name) = self.joint_target_length(joint);
                let value = match (from.unwrap_or_default(), offset) {
                    (EndFrom::A, value) => value.clone(),
                    (EndFrom::B, value) => {
                        let Some(name) = target_name else {
                            return Err(refuse("the part it is on has no end B to measure from"));
                        };
                        let _ = target_length;
                        match value {
                            ValueInput::Number(n) => {
                                ValueInput::Name(format!("{name} - {}", super::round(*n)))
                            }
                            ValueInput::Name(text) => {
                                ValueInput::Name(format!("{name} - ({text})"))
                            }
                        }
                    }
                };
                Some(value)
            }
            _ => None,
        };
        self.apply(|state| {
            let mut changed = Vec::new();
            if let Some(value) = offset {
                let joint = joint.expect("checked");
                edit_joint(state, joint, |j| {
                    let motion = match j.kind {
                        JointKind::Slider => j.slide_axis.motion(),
                        JointKind::Cylindrical => Motion::Tz,
                        _ => Motion::Tx,
                    };
                    j.position.insert(motion, value);
                })?;
                changed.push("offset".to_owned());
            }
            if let Some(angle) = rotate {
                let joint = joint.ok_or_else(|| refuse(format!("{} has no joint", info.name)))?;
                edit_joint(state, joint, |j| {
                    j.angle = Some(ValueInput::Number(angle));
                })?;
                changed.push("rotate".to_owned());
            }
            if let Some(flip) = input.flip {
                let joint = joint.ok_or_else(|| refuse(format!("{} has no joint", info.name)))?;
                let mut swapped = false;
                edit_joint(state, joint, |j| {
                    if let GeomRef::Mount { name, .. } = &mut j.a.geometry {
                        let wanted = if flip { "end_b" } else { "end_a" };
                        let current = name.as_str();
                        if (current == "end_a" || current == "end_b") && current != wanted {
                            *name = wanted.to_owned();
                            swapped = true;
                        }
                    }
                })?;
                if swapped {
                    // The cut goes with the end that meets.
                    edit_part(state, part, |def| {
                        std::mem::swap(&mut def.ends.a, &mut def.ends.b);
                    })?;
                    changed.push("flip".to_owned());
                }
            }
            match &input.length {
                Some(LengthInput::Value(value)) => {
                    let name = length_name
                        .clone()
                        .ok_or_else(|| refuse(format!("{} is not cut to length", info.name)))?;
                    for bound in current_bounds(state, component) {
                        detach(state, component, bound, part)?;
                        changed.push("detached".to_owned());
                    }
                    let value = value.clone();
                    edit_part(state, part, |def| {
                        def.parameters.insert(name, value);
                    })?;
                    changed.push("length".to_owned());
                }
                Some(LengthInput::Bound { .. }) => {
                    let (name, ray, to, mode, add) = bind_plan.take().expect("planned");
                    let current = current_bounds(state, component);
                    if let Some(bound) = current.first() {
                        edit_bound(state, *bound, |b| {
                            b.to = to;
                            b.mode = mode;
                            if let Some(add) = add {
                                b.add = Some(add);
                            }
                        })?;
                    } else {
                        let first = info.length.map_or(1.0, |(_, v)| v);
                        let shift_x = if ray.shift_by_joint {
                            joint.and_then(|j| {
                                joint_slide_param(state, j, Motion::Tx)
                                    .or_else(|| joint_slide_param(state, j, Motion::Tz))
                            })
                        } else {
                            None
                        };
                        let active = state.assembly.active;
                        let uid = add_bound(
                            state, active, part, &name, &ray, shift_x, &to, mode, add, first,
                        )?;
                        if let Some(record) = state
                            .assembly
                            .component_mut(component)
                            .and_then(|c| c.build.as_mut())
                        {
                            record.bound.push(uid);
                        }
                    }
                    changed.push("bound".to_owned());
                }
                None => {}
            }
            if let Some(ends) = &input.ends {
                let ends = ends.clone();
                edit_part(state, part, |def| {
                    let cut = |c: Option<EndCutInput>| {
                        c.map(|c| EndCut {
                            angle: c.angle,
                            tilt: c.tilt,
                        })
                    };
                    if let Some(a) = ends.a {
                        def.ends.a = cut(a);
                    }
                    if let Some(b) = ends.b {
                        def.ends.b = cut(b);
                    }
                })?;
                changed.push("ends".to_owned());
            }
            if let Some(config) = &input.config {
                let config = config.clone();
                edit_part(state, part, |def| def.config = Some(config))?;
                changed.push("config".to_owned());
            }
            if !input.parameters.is_empty() {
                let parameters = input.parameters.clone();
                edit_part(state, part, |def| {
                    for (name, value) in parameters {
                        def.parameters.insert(name, value);
                    }
                })?;
                changed.push("parameters".to_owned());
            }
            if let Some(defs) = machining {
                let own: BTreeSet<FeatureUid> = state
                    .assembly
                    .component(component)
                    .and_then(|c| c.build.as_ref())
                    .map(|r| {
                        r.adds
                            .iter()
                            .filter_map(|i| match i {
                                super::BuildItem::Feature(f) => Some(*f),
                                super::BuildItem::Component(_) => None,
                            })
                            .filter(|f| state.entry(*f).is_some_and(|e| e.component == component))
                            .collect()
                    })
                    .unwrap_or_default();
                if !own.is_empty() {
                    let mut all = own.clone();
                    all.extend(state.dependents(&own));
                    state.remove_features(&all);
                }
                let mut added = Vec::new();
                for def in defs {
                    added.push(super::BuildItem::Feature(add_feature(
                        state, def, component,
                    )?));
                }
                if let Some(record) = state
                    .assembly
                    .component_mut(component)
                    .and_then(|c| c.build.as_mut())
                {
                    record.adds.retain(|i| match i {
                        super::BuildItem::Feature(f) => !own.contains(f),
                        super::BuildItem::Component(_) => true,
                    });
                    record.adds.extend(added);
                }
                changed.push("machining".to_owned());
            }
            if changed.is_empty() {
                return Err(refuse("nothing to change"));
            }
            Ok((label, changed))
        })
    }

    /// A component by uid or name.
    pub(crate) fn component_by_ref(&self, text: &str) -> Result<ComponentUid, ModelError> {
        let a = self.assembly();
        if let Ok(uid) = text.parse::<ComponentUid>()
            && a.exists(uid)
        {
            return Ok(uid);
        }
        a.components
            .iter()
            .find(|c| c.name == text)
            .map(|c| c.uid)
            .ok_or_else(|| refuse(format!("no component {text}")))
    }

    /// The length parameter of the part a joint's b side is on.
    fn joint_target_length(&self, joint: FeatureUid) -> (Option<f64>, Option<String>) {
        let Some(FeatureDef::Joint(j)) = self.feature(joint).map(|e| &e.def) else {
            return (None, None);
        };
        let Ok(component) = crate::joints::path_component(
            self.assembly(),
            self.feature(joint)
                .map_or(ComponentUid::ROOT, |e| e.component),
            &j.b.occurrence.0,
        ) else {
            return (None, None);
        };
        match self.part_info(component).and_then(|i| i.length) {
            Some((id, v)) => (Some(v), Some(self.parameters().name(id))),
            None => (None, None),
        }
    }

    /// The ray a placed part's length is measured along: from the frame its
    /// joint places it on.
    pub(crate) fn part_ray(&self, info: &PartInfo) -> Result<RayPlan, ModelError> {
        let joint = info
            .record
            .joint
            .ok_or_else(|| refuse(format!("{} has no joint to measure from", info.name)))?;
        let Some(FeatureDef::Joint(j)) = self.feature(joint).map(|e| &e.def) else {
            return Err(refuse(format!(
                "{} has no joint to measure from",
                info.name
            )));
        };
        if !matches!(&j.a.geometry, GeomRef::Mount { name, .. } if name == "end_a" || name == "end_b")
        {
            return Err(refuse(format!(
                "{} is not placed with an end: its length cannot be bound",
                info.name
            )));
        }
        let mut from = j.b.clone();
        let direction = match from.frame_override.take() {
            Some(FrameOverride {
                z_axis: Some(z), ..
            }) => {
                let local = crate::joints::origin_frame(&mut self.resolver(), &from.geometry)
                    .map_err(refuse)?;
                RayDirection::Vector(inverse(&local).apply_vector(z))
            }
            _ => RayDirection::default(),
        };
        Ok(RayPlan {
            from,
            shift: [0.0; 3],
            shift_by_joint: j.kind == JointKind::Slider,
            direction,
            start: [0.0; 3],
            along: super::geometry::Z,
        })
    }

    /// Refuses a bound length that would make a cycle (the target's length
    /// bound, through others, to this part) or that ends on a part placed
    /// after this one.
    pub(crate) fn check_bind(
        &self,
        part: ComponentUid,
        target: ComponentUid,
    ) -> Result<(), ModelError> {
        if part == target {
            return Err(refuse("a part's length cannot be bound to itself"));
        }
        let names = |c: ComponentUid| self.assembly().name(c);
        // Is the target's length bound to the part, through others?
        let mut seen = BTreeSet::new();
        let mut todo = vec![target];
        while let Some(c) = todo.pop() {
            if !seen.insert(c) {
                continue;
            }
            for bound in self
                .part_info(c)
                .map(|i| i.record.bound)
                .unwrap_or_default()
            {
                if let Some(next) = self.bound_target(bound) {
                    if next == part {
                        return Err(refuse(format!(
                            "the lengths would be bound to each other: {} is bound to {}, so {} \
                             cannot be bound to it (a cycle)",
                            names(target),
                            names(part),
                            names(part)
                        )));
                    }
                    todo.push(next);
                }
            }
        }
        let position = |uid: Option<FeatureUid>| uid.and_then(|u| self.state().position(u));
        let info = self.part_info(part);
        let mine = position(info.as_ref().and_then(|i| i.part));
        let theirs = self
            .part_info(target)
            .map(|i| position(i.record.joint).or(position(i.part)));
        if let (Some(mine), Some(Some(theirs))) = (mine, theirs)
            && theirs > mine
        {
            return Err(refuse(format!(
                "{} is placed after {} in the timeline: a length is bound only to parts placed \
                 before it",
                names(target),
                names(part)
            )));
        }
        Ok(())
    }

    /// The component a bound length's ray ends on.
    pub(crate) fn bound_target(&self, bound: FeatureUid) -> Option<ComponentUid> {
        let entry = self.feature(bound)?;
        let FeatureDef::BoundLength(b) = &entry.def else {
            return None;
        };
        crate::joints::path_component(self.assembly(), entry.component, &b.to.occurrence.0).ok()
    }
}

/// The bound lengths a part's record lists that still exist.
pub(crate) fn current_bounds(state: &DocState, component: ComponentUid) -> Vec<FeatureUid> {
    state
        .assembly
        .component(component)
        .and_then(|c| c.build.as_ref())
        .map(|r| {
            r.bound
                .iter()
                .copied()
                .filter(|b| state.position(*b).is_some())
                .collect()
        })
        .unwrap_or_default()
}

/// Detaches a bound length: its reference becomes a dimension of the part
/// with the same name and its last value, and the feature goes.
pub(crate) fn detach(
    state: &mut DocState,
    component: ComponentUid,
    bound: FeatureUid,
    part: FeatureUid,
) -> Result<(), ModelError> {
    let Some(FeatureDef::BoundLength(b)) = state.entry(bound).map(|e| e.def.clone()) else {
        return Ok(());
    };
    let owner = state.entry(part).map(|_| part);
    state.parameters.set_owner(b.parameter, owner);
    state.remove_features(&BTreeSet::from([bound]));
    if let Some(record) = state
        .assembly
        .component_mut(component)
        .and_then(|c| c.build.as_mut())
    {
        record.bound.retain(|u| *u != bound);
    }
    Ok(())
}

/// Edits a joint's definition in command form.
pub(crate) fn edit_joint(
    state: &mut DocState,
    joint: FeatureUid,
    edit: impl FnOnce(&mut JointDef<ValueInput>),
) -> Result<(), ModelError> {
    let entry = state
        .entry(joint)
        .ok_or(ModelError::NoSuchFeature(joint))?
        .clone();
    let FeatureDef::Joint(mut def) = as_input(&entry.def, &state.parameters) else {
        return Err(refuse(format!("{} is no joint", entry.name)));
    };
    edit(&mut def);
    state.edit_feature(joint, &FeatureDef::Joint(def))?;
    Ok(())
}

/// Edits a bound length's definition in command form (its parameter kept).
pub(crate) fn edit_bound(
    state: &mut DocState,
    bound: FeatureUid,
    edit: impl FnOnce(&mut BoundLengthDef<ValueInput>),
) -> Result<(), ModelError> {
    let entry = state
        .entry(bound)
        .ok_or(ModelError::NoSuchFeature(bound))?
        .clone();
    let FeatureDef::BoundLength(mut def) = as_input(&entry.def, &state.parameters) else {
        return Err(refuse(format!("{} is no bound length", entry.name)));
    };
    edit(&mut def);
    state.edit_feature(bound, &FeatureDef::BoundLength(def))?;
    Ok(())
}
