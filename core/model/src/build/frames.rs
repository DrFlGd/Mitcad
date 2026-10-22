// SPDX-License-Identifier: MIT
//! Timber frames (mitcad#479; `commands.md`, "BUILD: timber frames"): a
//! wall, floor or roof frame generated from a few values and kept as one,
//! its members ordinary BUILD parts made from the library's timber.
//!
//! A frame's definition ([`FrameInput`]) is kept on its lead member's
//! record (`framing`); every member's record names the lead (`frame`) and
//! its place in the frame (`member`: `stud.3`, `plate.top.0`). Laying a
//! frame out is geometry from definitions only: a wall reads the walls it
//! butts against and the walls that butt against it, a roof the two walls
//! it stands on, never the members as they are. Editing a frame lays it out
//! again with the frames that depend on it, keeping the components of the
//! members whose place stays (`spacing::replace_member`), in one undo step
//! with the frames' fasteners driven anew.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::connect::{RemovePlan, apply_remove};
use super::geometry::{V3, Z, add, cross, dot, inverse, norm, scale, sub, unit};
use super::place::{
    PartInput, PlaceAt, PlaceInput, Plan, SourceUse, Spot, edit_part, make_part, refuse, value_in,
};
use super::spacing::replace_member;
use super::timber::PointTo;
use super::{BuildRole, SourceId, round};
use crate::document::{DocState, Document, ModelError};
use crate::expr::Unit;
use crate::features::ValueInput;
use crate::features::derived_part::{EndCut, Ends, PartCut, Tilt};
use crate::ids::ComponentUid;
use crate::kernel::Kernel;
use crate::transform::Transform;

/// Lengths within this are the same, mm.
const EPS: f64 = 1e-6;

/// What a frame is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    #[default]
    Wall,
    Floor,
    Roof,
}

impl FrameKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wall => "wall",
            Self::Floor => "floor",
            Self::Roof => "roof",
        }
    }

    /// The frame's name's first word (`Wall 1`).
    fn title(self) -> &'static str {
        match self {
            Self::Wall => "Wall",
            Self::Floor => "Floor",
            Self::Roof => "Roof",
        }
    }
}

/// A roof's shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoofKind {
    /// Two slopes meeting at a ridge board halfway between the walls.
    #[default]
    Gable,
    /// One slope from the lower wall over the higher one.
    Mono,
}

/// A wall's plates: how many at its bottom and at its top (1 or 2 each).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plates {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<u32>,
}

/// The wall frames a wall's ends butt against (corners and T-junctions).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameJoins {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// Where its start meets the wall `start` names (mitcad#510): on its
    /// side (left out), or `end`: that wall's end face butts against this
    /// wall's side at its start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_on: Option<JoinOn>,
}

/// Where a wall's start meets the wall it joins (mitcad#510).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinOn {
    /// Its start butts against that wall's side.
    Side,
    /// That wall's end face butts against its side.
    End,
}

/// How a wall's ends meet the walls they join (mitcad#510).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallCorner {
    /// Its end studs and plates butt square against the other wall's side
    /// face; the other wall is neither run on, lapped nor notched, only
    /// given a backing stud each side of it where they fit.
    Butt,
    /// Near the other wall's end an L corner, the other wall run on to its
    /// outer face and the plates lapped; elsewhere a T, a stud each side of
    /// it in the other wall (mitcad#493). Left out: this, as walls saved
    /// before mitcad#510 keep their corners.
    Lapped,
}

impl WallCorner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Butt => "butt",
            Self::Lapped => "lapped",
        }
    }
}

/// Where an opening is: along a wall from its start, or a floor's
/// `[along the layout, along the joists]` from the outline's least corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpeningOffset {
    Along(f64),
    At([f64; 2]),
}

/// An opening in a frame, mm: a wall's door or window (`offset`, `width`,
/// `height`, `sill`: 0 or left out for a door) or a floor's stair opening
/// (`offset`, `size`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {
    pub offset: OpeningOffset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sill: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[f64; 2]>,
}

/// `build_frame`'s fields, and `build_edit`'s `frame` (every field
/// optional there, an edit's fields replacing the definition's). Points are
/// in the BUILD host's coordinates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<FrameKind>,
    /// The members' timber (studs, joists, rafters; the others' default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartInput>,
    /// A wall's plates and sills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate: Option<PartInput>,
    /// A wall's headers over its openings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<PartInput>,
    /// A floor's rim joists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rim: Option<PartInput>,
    /// A gable roof's ridge board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ridge: Option<PartInput>,
    /// The wood fastener driven at every junction of the frame's members
    /// (two, sized from the members, as `build_connect`'s default; its
    /// `config` a chosen size, mitcad#497).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fastener: Option<PartInput>,
    /// How many at each junction (default 2, mitcad#497).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fastener_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    // A wall.
    /// The wall's centre line at its base: from `start` to `end`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<V3>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<V3>,
    /// An edit's: the wall's end moved along it to this length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<ValueInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<ValueInput>,
    /// The members' centre distance (a wall's studs, a floor's joists, a
    /// roof's rafters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<ValueInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plates: Option<Plates>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openings: Option<Vec<Opening>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joins: Option<FrameJoins>,
    /// `build_frame`'s and its preview's only (mitcad#493): `start` is a
    /// point on the side (or an end) of the wall `joins.start` names, and
    /// the wall starts square to that wall as `corner` has it; not kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub square: Option<bool>,
    /// How its ends meet the walls they join (mitcad#510; left out
    /// `lapped`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner: Option<WallCorner>,
    // A floor.
    /// The closed outline the floor fills (its first point's height).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<Vec<V3>>,
    /// The joists' direction (square to the outline's first side when
    /// left out).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<V3>,
    /// Rows of blocking between the joists (0 none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocking: Option<u32>,
    /// Trimmer joists (and headers) on each side of a floor's opening.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trimmers: Option<u32>,
    // A roof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roof: Option<RoofKind>,
    /// The two wall frames it stands on (any member of each).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<ValueInput>,
    /// The rafters' run beyond the walls' outer faces, horizontally.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overhang: Option<ValueInput>,
}

impl FrameInput {
    /// This definition with an edit's fields over it.
    pub fn merged(&self, edit: &FrameInput) -> FrameInput {
        macro_rules! pick {
            ($field:ident) => {
                edit.$field.clone().or_else(|| self.$field.clone())
            };
        }
        FrameInput {
            kind: self.kind.or(edit.kind),
            part: pick!(part),
            plate: pick!(plate),
            header: pick!(header),
            rim: pick!(rim),
            ridge: pick!(ridge),
            fastener: pick!(fastener),
            fastener_count: pick!(fastener_count),
            name: pick!(name),
            start: pick!(start),
            end: pick!(end),
            length: pick!(length),
            height: pick!(height),
            spacing: pick!(spacing),
            plates: pick!(plates),
            openings: pick!(openings),
            joins: pick!(joins),
            square: None,
            corner: pick!(corner),
            outline: pick!(outline),
            direction: pick!(direction),
            blocking: pick!(blocking),
            trimmers: pick!(trimmers),
            roof: pick!(roof),
            walls: pick!(walls),
            pitch: pick!(pitch),
            overhang: pick!(overhang),
        }
    }

    fn kind(&self) -> FrameKind {
        self.kind.unwrap_or_default()
    }
}

/// What the lead member's record keeps of its frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FrameDef {
    pub frame: FrameInput,
    pub count: usize,
}

/// What `build_frame` and a frame's `build_edit` did.
#[derive(Debug, Clone, Default)]
pub struct Framed {
    /// The frame's lead member.
    pub lead: Option<ComponentUid>,
    pub name: String,
    pub kind: &'static str,
    /// Every member: its component and its place in the frame.
    pub members: Vec<(ComponentUid, String)>,
    /// The junctions given fasteners.
    pub fasteners: usize,
    pub warnings: Vec<String>,
    /// Other frames laid out again with it (their leads).
    pub relaid: Vec<ComponentUid>,
    pub changed: Vec<String>,
    /// A wall started square on another wall's side: where it meets it.
    pub junction: Option<StartJunction>,
}

/// Where a wall started on another wall's side (`square`, mitcad#493)
/// meets that wall.
#[derive(Debug, Clone, PartialEq)]
pub struct StartJunction {
    /// With lapped corners `L` (a corner at one of the wall's ends) or
    /// `T`; butting (mitcad#510) `side` (from its side at the click) or
    /// `end` (a corner at its end face).
    pub kind: &'static str,
    /// `butt` or `lapped` (mitcad#510).
    pub corner: &'static str,
    /// The wall's name and lead.
    pub wall: String,
    pub frame: ComponentUid,
    /// An L's or an end's end of that wall: `a` (its start) or `b`.
    pub end: Option<&'static str>,
    /// Along that wall from its start to the new wall's centre line, mm.
    pub at: f64,
    /// The new wall's length: from that wall's centre line, mm; from its
    /// far side at an `end`.
    pub length: f64,
}

impl StartJunction {
    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind,
            "corner": self.corner,
            "wall": self.wall,
            "frame": self.frame,
            "end": self.end,
            "at": (self.at * 1e6).round() / 1e6,
            "length": (self.length * 1e6).round() / 1e6,
        })
    }
}

/// A frame's member before it is planned: where its axis runs (in the
/// BUILD host's coordinates), the way its section's larger size points,
/// its end cuts and its notches.
#[derive(Debug, Clone)]
pub(crate) struct MemberSpec {
    pub key: String,
    pub role: &'static str,
    which: Which,
    pub start: V3,
    pub end: V3,
    /// Square to the axis: the section's larger size along it.
    wide: V3,
    /// Plumb cuts (a rafter's): the angle and the lean (in the host) of
    /// end A's and end B's cut.
    plumb: Option<(f64, [V3; 2])>,
    notches: Vec<Notch>,
}

/// A box a member is notched by, in the host: from `origin` along `x`, `y`
/// and their cross product by `size`; `with` the other wall's lead (None:
/// the frame being made); `kind` the cut's (`birdsmouth`, a lapped
/// corner's `half_lap`).
#[derive(Debug, Clone)]
struct Notch {
    kind: &'static str,
    origin: V3,
    x: V3,
    y: V3,
    size: [f64; 3],
    with: Option<ComponentUid>,
}

/// Which of a frame's parts a member is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Which {
    Part,
    Plate,
    Header,
    Rim,
    Ridge,
}

/// A part of a frame: its source and its section (W along X, T along Y).
#[derive(Debug, Clone)]
struct FramePart {
    input: PartInput,
    source: SourceUse,
    w: f64,
    t: f64,
}

impl FramePart {
    fn depth(&self) -> f64 {
        self.w.max(self.t)
    }

    fn thick(&self) -> f64 {
        self.w.min(self.t)
    }

    /// Its least length, mm (0 when the library gives none).
    fn least(&self) -> f64 {
        self.source
            .meta
            .length
            .as_ref()
            .and_then(|l| l.min.as_ref())
            .and_then(|m| m.millimetres())
            .unwrap_or(0.0)
    }
}

/// A frame of the design (or the one being made, `lead` None).
#[derive(Debug, Clone)]
struct Known {
    lead: Option<ComponentUid>,
    def: FrameInput,
}

/// A frame's members laid out.
#[derive(Debug, Default)]
struct Layout {
    members: Vec<MemberSpec>,
    warnings: Vec<String>,
}

impl Layout {
    fn add(
        &mut self,
        key: impl Into<String>,
        role: &'static str,
        which: Which,
        ends: [V3; 2],
        wide: V3,
    ) {
        self.members.push(MemberSpec {
            key: key.into(),
            role,
            which,
            start: ends[0],
            end: ends[1],
            wide,
            plumb: None,
            notches: Vec::new(),
        });
    }
}

/// A member planned: what `build_place` would make of it, and its notches
/// in its own coordinates.
#[derive(Debug, Clone)]
struct PlannedMember {
    spec: MemberSpec,
    plan: Plan,
    cuts: Vec<PartCut<ValueInput>>,
}

/// A frame laid out and planned, ready to be made.
#[derive(Debug, Clone)]
struct FramePlan {
    lead: Option<ComponentUid>,
    name: String,
    kind: FrameKind,
    def: FrameInput,
    parts: BTreeMap<Which, FramePart>,
    members: Vec<PlannedMember>,
    /// The members that stay, by their place.
    keep: BTreeMap<String, ComponentUid>,
    warnings: Vec<String>,
}

// --- Plane geometry.

fn cross2(a: V3, b: V3) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// A direction made horizontal.
fn level(v: V3) -> Option<V3> {
    unit([v[0], v[1], 0.0])
}

/// Where the line `p + t u` meets the line `q + s v` (in plan): `t`.
fn line_param(p: V3, u: V3, q: V3, v: V3) -> Option<f64> {
    let c = cross2(u, v);
    (c.abs() > 1e-6).then(|| cross2(sub(q, p), v) / c)
}

// --- Walls.

/// A wall frame's definition worked out.
#[derive(Debug, Clone)]
struct WallGeo {
    lead: Option<ComponentUid>,
    name: String,
    /// Its centre line's start at its base, the line's direction and the
    /// horizontal across it (to its left).
    start: V3,
    u: V3,
    n: V3,
    length: f64,
    height: f64,
    spacing: f64,
    bottom: usize,
    top: usize,
    stud: FramePart,
    plate: FramePart,
    header: FramePart,
    openings: Vec<Opening>,
    joins: [Option<ComponentUid>; 2],
    /// Its ends butt square against the walls they join, which get only
    /// backing studs (`corner: butt`, mitcad#510).
    butt: bool,
    /// The wall its start joins butts its end face against its side
    /// (`joins.start_on: end`, mitcad#510).
    start_on_end: bool,
    warnings: Vec<String>,
}

impl WallGeo {
    /// Its depth across: the studs' larger size.
    fn depth(&self) -> f64 {
        self.stud.depth()
    }

    /// A point `t` along it, `z` above its base and `a` across it.
    fn at(&self, t: f64, z: f64, a: f64) -> V3 {
        add(
            add(add(self.start, scale(self.u, t)), scale(Z, z)),
            scale(self.n, a),
        )
    }

    /// The top of its top plate.
    fn top_z(&self) -> f64 {
        self.start[2] + self.height
    }
}

/// Where a wall's plates run (`t0` to `t1` along it, after its corners)
/// and the extra studs where other walls butt against it.
#[derive(Debug, Clone, Default)]
struct WallSpan {
    t0: f64,
    t1: f64,
    extra: Vec<(f64, String)>,
    warnings: Vec<String>,
    /// The lapped corners at its start and its end (mitcad#493).
    laps: [Option<Lap>; 2],
}

/// A wall's plates at an L corner, lapped with the other wall's: `to`
/// along it, the lap's other end (beyond its end where it butts against
/// the other wall, `over` its plates; else the other wall's inner face,
/// under its plates); `with` the other wall's lead.
#[derive(Debug, Clone, Copy)]
struct Lap {
    to: f64,
    over: bool,
    with: Option<ComponentUid>,
}

/// A wall's span: its own ends butting against the walls they join, and
/// the corners and T-junctions of the walls that join it.
fn wall_span(w: &WallGeo, walls: &[WallGeo]) -> WallSpan {
    let mut span = WallSpan {
        t0: 0.0,
        t1: w.length,
        ..WallSpan::default()
    };
    for (k, end) in w.joins.iter().enumerate() {
        let Some(lead) = end else { continue };
        let which = if k == 0 { "start" } else { "end" };
        let Some(x) = walls.iter().find(|o| o.lead == Some(*lead)) else {
            span.warnings.push(format!(
                "{}: the wall frame its {which} joins is gone",
                w.name
            ));
            continue;
        };
        let along = dot(w.u, x.n);
        if along.abs() < 0.1 {
            span.warnings.push(format!(
                "{}: its {which} runs along {}, it does not butt against it",
                w.name, x.name
            ));
            continue;
        }
        if k == 0 && w.start_on_end {
            // That wall's end face against its side at its start
            // (mitcad#510): it runs on from its own start, a backing stud
            // each side of that wall where they fit.
            if let Some(tj) = line_param(w.start, w.u, x.start, x.u) {
                let (dx, ts) = (x.depth(), w.stud.thick());
                let key = format!("junction.{}.0", x.name.replace(' ', "_"));
                span.extra.push((tj - dx / 2.0 - ts, format!("{key}.a")));
                span.extra.push((tj + dx / 2.0, format!("{key}.b")));
            }
            continue;
        }
        let middle = w.at(w.length / 2.0, 0.0, 0.0);
        let side = if dot(sub(middle, x.start), x.n) >= 0.0 {
            1.0
        } else {
            -1.0
        };
        let t = (side * x.depth() / 2.0 - dot(sub(w.start, x.start), x.n)) / along;
        if k == 0 {
            span.t0 = t;
        } else {
            span.t1 = t;
        }
        // At a corner (its centre line meets the other's within its depth of
        // an end, as the other wall finds it) its plates lap over the other's,
        // to that wall's outer face; butting, they stop at its face.
        if !w.butt
            && let Some(tx) = line_param(x.start, x.u, w.start, w.u)
            && (tx < w.depth() || tx > x.length - w.depth())
        {
            span.laps[k] = Some(Lap {
                to: (-side * x.depth() / 2.0 - dot(sub(w.start, x.start), x.n)) / along,
                over: true,
                with: x.lead,
            });
        }
    }
    let Some(me) = w.lead else {
        return span;
    };
    let ts = w.stud.thick();
    for b in walls {
        if b.lead == w.lead {
            continue;
        }
        for (k, end) in b.joins.iter().enumerate() {
            if *end != Some(me) {
                continue;
            }
            // Its end face against a wall started from it: nothing of it
            // changes (mitcad#510).
            if k == 0 && b.start_on_end {
                continue;
            }
            let Some(tj) = line_param(w.start, w.u, b.start, b.u) else {
                continue;
            };
            let db = b.depth();
            let key = format!("junction.{}.{k}", b.name.replace(' ', "_"));
            if b.butt {
                // A wall that butts against it (mitcad#510): neither run on
                // nor lapped, a backing stud each side of it where they fit
                // (as at a T).
                span.extra.push((tj - db / 2.0 - ts, format!("{key}.a")));
                span.extra.push((tj + db / 2.0, format!("{key}.b")));
            } else if tj < db {
                // A corner at its start: it runs to the other's outer face,
                // its plates lapped under the other's.
                span.t0 = span.t0.min(tj - db / 2.0);
                span.extra.push((tj + db / 2.0, key));
                span.laps[0] = Some(Lap {
                    to: tj + db / 2.0,
                    over: false,
                    with: b.lead,
                });
            } else if tj > w.length - db {
                span.t1 = span.t1.max(tj + db / 2.0);
                span.extra.push((tj - db / 2.0 - ts, key));
                span.laps[1] = Some(Lap {
                    to: tj - db / 2.0,
                    over: false,
                    with: b.lead,
                });
            } else {
                span.extra.push((tj - db / 2.0 - ts, format!("{key}.a")));
                span.extra.push((tj + db / 2.0, format!("{key}.b")));
            }
        }
    }
    span
}

/// An opening's place in a wall, worked out.
struct WallZone {
    /// Its number (from 1), where its studs start and end along the wall,
    /// its offset and width, its sill (0 for a door) and its header's
    /// underside.
    number: usize,
    lo: f64,
    hi: f64,
    offset: f64,
    width: f64,
    sill: f64,
    head: f64,
}

/// A wall's members: its plates, studs at the centre distance from its
/// start, its end studs, the studs where other walls meet it, and each
/// opening's king and trimmer studs, header, sill and cripple studs.
fn wall_layout(w: &WallGeo, span: &WallSpan) -> Result<Layout, ModelError> {
    let (t0, t1) = (span.t0, span.t1);
    let ts = w.stud.thick();
    let tp = w.plate.thick();
    let hd = w.header.depth();
    let mut out = Layout {
        warnings: [w.warnings.clone(), span.warnings.clone()].concat(),
        ..Layout::default()
    };
    if t1 - t0 < 2.0 * ts - EPS {
        return Err(refuse(format!(
            "{} is {} mm long: shorter than its two end studs",
            w.name,
            round(t1 - t0)
        )));
    }
    if w.spacing <= ts + EPS {
        return Err(refuse(format!(
            "the centre distance of {} mm is not more than a stud's {} mm",
            round(w.spacing),
            round(ts)
        )));
    }
    let zb = w.bottom as f64 * tp;
    let zt = w.height - w.top as f64 * tp;
    if zt - zb < 1.0 {
        return Err(refuse(format!(
            "{} is {} mm high: no room for studs between its plates",
            w.name,
            round(w.height)
        )));
    }
    let across = w.n;
    let stud_wide = across;
    let mut studs: Vec<(String, &'static str, f64, f64, f64)> = Vec::new();
    // The openings (their headers and sills after the plates and studs).
    let mut framing = Layout::default();
    let mut zones: Vec<WallZone> = Vec::new();
    let mut doors: Vec<(f64, f64)> = Vec::new();
    for (i, op) in w.openings.iter().enumerate() {
        let n = i + 1;
        let OpeningOffset::Along(o) = op.offset else {
            return Err(refuse(format!(
                "opening {n}: a wall's opening is at an offset along it (a number)"
            )));
        };
        let width = op
            .width
            .filter(|v| *v > EPS)
            .ok_or_else(|| refuse(format!("opening {n} needs its width")))?;
        let height = op
            .height
            .filter(|v| *v > EPS)
            .ok_or_else(|| refuse(format!("opening {n} needs its height")))?;
        let sill = op.sill.unwrap_or(0.0).max(0.0);
        let door = sill <= EPS;
        let head = sill + height;
        let zone = WallZone {
            number: n,
            lo: o - 2.0 * ts,
            hi: o + width + 2.0 * ts,
            offset: o,
            width,
            sill: if door { 0.0 } else { sill },
            head,
        };
        if zone.lo < t0 + ts - EPS || zone.hi > t1 - ts + EPS {
            return Err(refuse(format!(
                "opening {n} at {} mm, {} mm wide, does not fit between the end studs of {}",
                round(o),
                round(width),
                w.name
            )));
        }
        if let Some(j) = zones
            .iter()
            .position(|z| zone.lo < z.hi - EPS && z.lo < zone.hi - EPS)
        {
            return Err(refuse(format!(
                "openings {} and {n} overlap with their studs",
                j + 1
            )));
        }
        if head + hd > zt + EPS {
            return Err(refuse(format!(
                "opening {n}: its header reaches {} mm, above the top plate at {} mm",
                round(head + hd),
                round(zt)
            )));
        }
        if !door && sill - tp < zb - EPS {
            return Err(refuse(format!(
                "opening {n}: its sill at {} mm is below the top of the bottom plate ({} mm)",
                round(sill),
                round(zb + tp)
            )));
        }
        if head <= zb + EPS {
            return Err(refuse(format!(
                "opening {n} is no higher than the bottom plate"
            )));
        }
        let p = format!("opening.{n}");
        studs.push((format!("{p}.king.a"), "king stud", o - 2.0 * ts, zb, zt));
        studs.push((format!("{p}.king.b"), "king stud", o + width + ts, zb, zt));
        studs.push((format!("{p}.trimmer.a"), "trimmer stud", o - ts, zb, head));
        studs.push((
            format!("{p}.trimmer.b"),
            "trimmer stud",
            o + width,
            zb,
            head,
        ));
        let plies = ((w.depth() + 1.0) / w.header.thick()).floor().max(1.0) as usize;
        for k in 0..plies {
            let a = (k as f64 - (plies - 1) as f64 / 2.0) * w.header.thick();
            let z = head + hd / 2.0;
            framing.add(
                format!("{p}.header.{k}"),
                "header",
                Which::Header,
                [w.at(o - ts, z, a), w.at(o + width + ts, z, a)],
                Z,
            );
        }
        if door {
            doors.push((o, o + width));
        } else {
            let z = sill - tp / 2.0;
            framing.add(
                format!("{p}.sill"),
                "sill",
                Which::Plate,
                [w.at(o, z, 0.0), w.at(o + width, z, 0.0)],
                across,
            );
        }
        zones.push(zone);
    }
    // The plates: the top ones through, the bottom ones cut at the doors;
    // at a lapped corner the outer layer runs over the corner on the wall
    // that butts and stops at its face on the other, one plate of each is
    // half-lapped (that wall's half outside).
    let pw = w.plate.depth();
    let plate_run = |top: bool, layer: usize, z: f64| -> ([f64; 2], Vec<Notch>) {
        let count = if top { w.top } else { w.bottom };
        let mut ends = [t0, t1];
        let mut notches = Vec::new();
        for (k, lap) in span.laps.iter().enumerate() {
            let Some(lap) = lap else { continue };
            if count == 1 {
                let face = ends[k];
                if lap.over {
                    ends[k] = lap.to;
                }
                let (mut lo, mut hi) = (face.min(lap.to), face.max(lap.to));
                if k == 0 {
                    lo -= 1.0;
                } else {
                    hi += 1.0;
                }
                // The half cut away: the inner one where it lies over the
                // other's, else the outer one.
                let upper = top != lap.over;
                let z0 = if upper { z } else { z - tp / 2.0 - 1.0 };
                notches.push(Notch {
                    kind: "half_lap",
                    origin: w.at(lo, z0, -pw / 2.0 - 1.0),
                    x: w.u,
                    y: w.n,
                    size: [hi - lo, pw + 2.0, tp / 2.0 + 1.0],
                    with: lap.with,
                });
            } else if layer == 0 {
                ends[k] = lap.to;
            }
        }
        (ends, notches)
    };
    for j in 0..w.top {
        let z = w.height - (j as f64 + 0.5) * tp;
        let (ends, notches) = plate_run(true, j, z);
        out.add(
            format!("plate.top.{j}"),
            "top plate",
            Which::Plate,
            [w.at(ends[0], z, 0.0), w.at(ends[1], z, 0.0)],
            across,
        );
        if let Some(m) = out.members.last_mut() {
            m.notches = notches;
        }
    }
    doors.sort_by(|a, b| a.0.total_cmp(&b.0));
    for i in 0..w.bottom {
        let z = (i as f64 + 0.5) * tp;
        let (ends, notches) = plate_run(false, i, z);
        let mut pieces = Vec::new();
        let mut from = ends[0];
        for (a, b) in &doors {
            pieces.push((from, *a));
            from = *b;
        }
        pieces.push((from, ends[1]));
        let many = pieces.len() > 1;
        for (k, (a, b)) in pieces.into_iter().enumerate() {
            if b - a < 1.0 {
                continue;
            }
            let key = if many {
                format!("plate.bottom.{i}.{k}")
            } else {
                format!("plate.bottom.{i}")
            };
            out.add(
                key,
                "bottom plate",
                Which::Plate,
                [w.at(a, z, 0.0), w.at(b, z, 0.0)],
                across,
            );
            // A piece takes the laps along it.
            let along = |n: &Notch| dot(sub(n.origin, w.start), w.u);
            let mine: Vec<Notch> = notches
                .iter()
                .filter(|n| along(n) < b - EPS && along(n) + n.size[0] > a + EPS)
                .cloned()
                .collect();
            if let Some(m) = out.members.last_mut() {
                m.notches = mine;
            }
        }
    }
    // The studs: the ends, the junctions, the centre distance.
    studs.push(("stud.start".to_owned(), "stud", t0, zb, zt));
    studs.push(("stud.end".to_owned(), "stud", t1 - ts, zb, zt));
    let in_zone = |near: f64| {
        zones
            .iter()
            .find(|z| near < z.hi - EPS && near + ts > z.lo + EPS)
    };
    let mut junction_studs = Vec::new();
    for (near, key) in &span.extra {
        if *near < t0 + ts - EPS || *near + ts > t1 - ts + EPS {
            continue;
        }
        if in_zone(*near).is_some() {
            out.warnings.push(format!(
                "{}: the stud where another wall meets it at {} mm is in an opening's framing: left out",
                w.name,
                round(*near)
            ));
            continue;
        }
        junction_studs.push(*near);
        studs.push((key.clone(), "stud", *near, zb, zt));
    }
    for k in 1.. {
        let c = t0 + k as f64 * w.spacing;
        let near = c - ts / 2.0;
        if near + ts > t1 - ts + EPS {
            break;
        }
        if junction_studs
            .iter()
            .any(|j| near < j + ts - EPS && *j < near + ts - EPS)
        {
            continue;
        }
        match in_zone(near) {
            None => studs.push((format!("stud.{k}"), "stud", near, zb, zt)),
            Some(z) if near >= z.offset - EPS && near + ts <= z.offset + z.width + EPS => {
                // Cripple studs above the header and below the sill.
                let (n, sill, head) = (z.number, z.sill, z.head);
                if zt - (head + hd) > 1.0 {
                    studs.push((
                        format!("opening.{n}.cripple.top.{k}"),
                        "cripple stud",
                        near,
                        head + hd,
                        zt,
                    ));
                }
                if sill > EPS && sill - tp - zb > 1.0 {
                    studs.push((
                        format!("opening.{n}.cripple.bottom.{k}"),
                        "cripple stud",
                        near,
                        zb,
                        sill - tp,
                    ));
                }
            }
            Some(_) => {}
        }
    }
    for (key, role, near, z0, z1) in studs {
        let c = near + ts / 2.0;
        out.add(
            key,
            role,
            Which::Part,
            [w.at(c, z0, 0.0), w.at(c, z1, 0.0)],
            stud_wide,
        );
    }
    out.members.extend(framing.members);
    Ok(out)
}

// --- Floors.

/// A floor frame's definition worked out: its outline, the layout
/// direction `a` (across the joists) and the joists' direction `v`.
#[derive(Debug, Clone)]
struct FloorGeo {
    name: String,
    outline: Vec<V3>,
    z0: f64,
    a: V3,
    v: V3,
    spacing: f64,
    blocking: usize,
    trimmers: usize,
    openings: Vec<Opening>,
    joist: FramePart,
    rim: FramePart,
}

/// A run of a floor's members along the joists: its centre across them
/// (`a`), its thickness and where it runs (`v`).
#[derive(Debug, Clone, Copy)]
struct Run {
    a: f64,
    half: f64,
    lo: f64,
    hi: f64,
}

/// A floor's members: rim joists along its outline (the ones across the
/// joists running through the corners), joists at the centre distance
/// from rim to rim, rows of blocking, and each opening's trimmers, headers
/// and tail joists.
fn floor_layout(f: &FloorGeo) -> Result<Layout, ModelError> {
    let mut out = Layout::default();
    let pts: Vec<V3> = f.outline.iter().map(|p| [p[0], p[1], f.z0]).collect();
    let n = pts.len();
    if n < 3 {
        return Err(refuse("a floor's outline has three points at least"));
    }
    let o = pts[0];
    let ca = |p: V3| dot(sub(p, o), f.a);
    let cv = |p: V3| dot(sub(p, o), f.v);
    let at = |a: f64, v: f64, z: f64| add(add(add(o, scale(f.a, a)), scale(f.v, v)), [0.0, 0.0, z]);
    // Its turn: the inward normals.
    let area: f64 = (0..n)
        .map(|i| cross2(pts[i], pts[(i + 1) % n]))
        .sum::<f64>()
        / 2.0;
    if area.abs() < 1.0 {
        return Err(refuse("a floor's outline encloses no area"));
    }
    let edges: Vec<(V3, V3, f64, V3)> = (0..n)
        .map(|i| {
            let p = pts[i];
            let q = pts[(i + 1) % n];
            let e = unit(sub(q, p)).unwrap_or(f.a);
            let left = cross(Z, e);
            let m = if area > 0.0 { left } else { scale(left, -1.0) };
            (p, e, norm(sub(q, p)), m)
        })
        .collect();
    let tr = f.rim.thick();
    let wr = f.rim.depth();
    let tj = f.joist.thick();
    let wj = f.joist.depth();
    if f.spacing <= tj + EPS {
        return Err(refuse(format!(
            "the centre distance of {} mm is not more than a joist's {} mm",
            round(f.spacing),
            round(tj)
        )));
    }
    let through: Vec<bool> = edges.iter().map(|e| dot(e.1, f.v).abs() < 0.5).collect();
    // The rims: at each corner the one across the joists runs through.
    let inner = |i: usize, j: usize| -> V3 {
        let (p, e, _, m) = edges[i];
        let (q, g, _, l) = edges[j];
        let pi = add(p, scale(m, tr));
        let qj = add(q, scale(l, tr));
        line_param(pi, e, qj, g).map_or(add(edges[j].0, scale(l, tr)), |t| add(pi, scale(e, t)))
    };
    for i in 0..n {
        let prev = (i + n - 1) % n;
        let next = (i + 1) % n;
        let (p, e, len, m) = edges[i];
        // Who runs through at a corner: the one across the joists, else the
        // earlier side.
        let runs_start = through[i] && !through[prev];
        let runs_end = through[i] || !through[next];
        let v_start = dot(sub(inner(prev, i), p), e);
        let v_end = dot(sub(inner(i, next), p), e);
        let s0 = if runs_start {
            v_start.min(0.0)
        } else {
            v_start.max(0.0)
        };
        let s1 = if runs_end {
            v_end.max(len)
        } else {
            v_end.min(len)
        };
        if s1 - s0 < 1.0 {
            continue;
        }
        let base = add(add(p, scale(m, tr / 2.0)), [0.0, 0.0, wr / 2.0]);
        out.add(
            format!("rim.{i}"),
            "rim joist",
            Which::Rim,
            [add(base, scale(e, s0)), add(base, scale(e, s1))],
            Z,
        );
    }
    // The rims along the joists, as runs (their strips across).
    let mut runs: Vec<Run> = Vec::new();
    for (i, (p, e, len, m)) in edges.iter().enumerate() {
        if through[i] {
            continue;
        }
        let a0 = ca(*p);
        let a1 = a0 + tr * dot(*m, f.a);
        let (v0, v1) = (cv(*p), cv(add(*p, scale(*e, *len))));
        runs.push(Run {
            a: (a0 + a1) / 2.0,
            half: (a1 - a0).abs() / 2.0,
            lo: v0.min(v1),
            hi: v0.max(v1),
        });
    }
    let rim_runs = runs.clone();
    let (amin, amax) = pts
        .iter()
        .map(|p| ca(*p))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
    let (vmin, vmax) = pts
        .iter()
        .map(|p| cv(*p))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
    // Where a line along the joists at `a` is inside the outline, to the
    // rims' inner faces.
    let spans_at = |a: f64| -> Vec<(f64, f64)> {
        let mut hits: Vec<(f64, usize)> = Vec::new();
        for (i, (p, e, len, _)) in edges.iter().enumerate() {
            let q = add(*p, scale(*e, *len));
            let (pa, qa) = (ca(*p), ca(q));
            if (pa <= a && a < qa) || (qa <= a && a < pa) {
                let v = cv(*p) + (a - pa) / (qa - pa) * (cv(q) - cv(*p));
                hits.push((v, i));
            }
        }
        hits.sort_by(|x, y| x.0.total_cmp(&y.0));
        let face = |(v, i): (f64, usize)| -> f64 {
            let (p, _, _, m) = edges[i];
            let mv = dot(m, f.v);
            if mv.abs() < 0.05 {
                return v;
            }
            (tr - dot(sub(at(a, 0.0, 0.0), p), m)) / mv
        };
        hits.chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (face(c[0]), face(c[1])))
            .filter(|(lo, hi)| hi - lo > 1.0)
            .collect()
    };
    let overlaps_rim = |a: f64, half: f64, lo: f64, hi: f64| {
        rim_runs
            .iter()
            .any(|r| (a - r.a).abs() < half + r.half - EPS && lo < r.hi - EPS && r.lo < hi - EPS)
    };
    // The openings, in (a, v).
    let nt = f.trimmers.max(1);
    let mut holes: Vec<[f64; 4]> = Vec::new();
    for (i, op) in f.openings.iter().enumerate() {
        let k = i + 1;
        let OpeningOffset::At([oa, ov]) = op.offset else {
            return Err(refuse(format!(
                "opening {k}: a floor's opening is at [a, b] from the outline's corner"
            )));
        };
        let [sa, sv] = op
            .size
            .filter(|s| s[0] > EPS && s[1] > EPS)
            .ok_or_else(|| refuse(format!("opening {k} needs its size")))?;
        let r = [amin + oa, amin + oa + sa, vmin + ov, vmin + ov + sv];
        let wide = nt as f64 * tj;
        if r[0] - wide < amin + tr - EPS
            || r[1] + wide > amax - tr + EPS
            || r[2] - wide < vmin + tr - EPS
            || r[3] + wide > vmax - tr + EPS
        {
            return Err(refuse(format!(
                "opening {k} with its trimmers and headers does not fit inside the rims of {}",
                f.name
            )));
        }
        holes.push(r);
        let p = format!("opening.{k}");
        for t in 0..nt {
            for (side, a) in [
                ("a", r[0] - (t as f64 + 0.5) * tj),
                ("b", r[1] + (t as f64 + 0.5) * tj),
            ] {
                for (j, (lo, hi)) in spans_at(a).into_iter().enumerate() {
                    let key = if j == 0 {
                        format!("{p}.trimmer.{side}.{t}")
                    } else {
                        format!("{p}.trimmer.{side}.{t}.{j}")
                    };
                    out.add(
                        key,
                        "trimmer joist",
                        Which::Part,
                        [at(a, lo, wj / 2.0), at(a, hi, wj / 2.0)],
                        Z,
                    );
                    runs.push(Run {
                        a,
                        half: tj / 2.0,
                        lo,
                        hi,
                    });
                }
            }
            for (side, v) in [
                ("a", r[2] - (t as f64 + 0.5) * tj),
                ("b", r[3] + (t as f64 + 0.5) * tj),
            ] {
                out.add(
                    format!("{p}.header.{side}.{t}"),
                    "header joist",
                    Which::Part,
                    [at(r[0], v, wj / 2.0), at(r[1], v, wj / 2.0)],
                    Z,
                );
            }
        }
    }
    // The joists at the centre distance from the outline's start.
    for k in 1.. {
        let a = amin + k as f64 * f.spacing;
        if a + tj / 2.0 > amax - EPS {
            break;
        }
        for (j, (lo, hi)) in spans_at(a).into_iter().enumerate() {
            if overlaps_rim(a, tj / 2.0, lo, hi) {
                continue;
            }
            let key = if j == 0 {
                format!("joist.{k}")
            } else {
                format!("joist.{k}.{j}")
            };
            let wide = nt as f64 * tj;
            let hole = holes.iter().position(|r| {
                a + tj / 2.0 > r[0] - wide + EPS
                    && a - tj / 2.0 < r[1] + wide - EPS
                    && lo < r[3]
                    && r[2] < hi
            });
            match hole {
                None => {
                    out.add(
                        key,
                        "joist",
                        Which::Part,
                        [at(a, lo, wj / 2.0), at(a, hi, wj / 2.0)],
                        Z,
                    );
                    runs.push(Run {
                        a,
                        half: tj / 2.0,
                        lo,
                        hi,
                    });
                }
                Some(h) => {
                    let r = holes[h];
                    if a - tj / 2.0 < r[0] - EPS || a + tj / 2.0 > r[1] + EPS {
                        continue;
                    }
                    // Tail joists from the rims to the headers.
                    for (side, from, to) in [("a", lo, r[2] - wide), ("b", r[3] + wide, hi)] {
                        if to - from > 1.0 {
                            out.add(
                                format!("{key}.tail.{side}"),
                                "tail joist",
                                Which::Part,
                                [at(a, from, wj / 2.0), at(a, to, wj / 2.0)],
                                Z,
                            );
                            runs.push(Run {
                                a,
                                half: tj / 2.0,
                                lo: from,
                                hi: to,
                            });
                        }
                    }
                }
            }
        }
    }
    // Rows of blocking between neighbouring runs.
    runs.sort_by(|x, y| x.a.total_cmp(&y.a));
    for row in 1..=f.blocking {
        let v = vmin + (vmax - vmin) * row as f64 / (f.blocking + 1) as f64;
        let across: Vec<&Run> = runs
            .iter()
            .filter(|r| r.lo <= v - tj / 2.0 + EPS && v + tj / 2.0 <= r.hi + EPS)
            .collect();
        for (k, pair) in across.windows(2).enumerate() {
            let from = pair[0].a + pair[0].half;
            let to = pair[1].a - pair[1].half;
            if to - from < 1.0 {
                continue;
            }
            let middle = (from + to) / 2.0;
            if holes
                .iter()
                .any(|r| middle > r[0] && middle < r[1] && v > r[2] && v < r[3])
            {
                continue;
            }
            out.add(
                format!("block.{row}.{k}"),
                "blocking",
                Which::Part,
                [at(from, v, wj / 2.0), at(to, v, wj / 2.0)],
                Z,
            );
        }
    }
    Ok(out)
}

// --- Roofs.

/// A roof frame's definition worked out.
#[derive(Debug, Clone)]
struct RoofGeo {
    name: String,
    kind: RoofKind,
    walls: [WallGeo; 2],
    spans: [WallSpan; 2],
    pitch: Option<f64>,
    spacing: f64,
    overhang: f64,
    rafter: FramePart,
    ridge: FramePart,
}

/// A roof's rafters at the centre distance along the walls (flush with
/// their common ends), each notched by a birdsmouth over each wall's top
/// plate and plumb cut at its ends, and a gable roof's ridge board.
fn roof_layout(r: &RoofGeo) -> Result<Layout, ModelError> {
    let mut out = Layout::default();
    let [wa, wb] = &r.walls;
    let u = wa.u;
    if dot(u, wb.u).abs() < 1.0 - 1e-3 {
        return Err(refuse(format!(
            "{} and {} are not parallel: a roof stands on two parallel walls",
            wa.name, wb.name
        )));
    }
    let d = sub(wb.start, wa.start);
    let s = level(sub(d, scale(u, dot(d, u))))
        .filter(|_| norm(sub(d, scale(u, dot(d, u)))) > 1.0)
        .ok_or_else(|| refuse(format!("{} and {} are on one line", wa.name, wb.name)))?;
    // Along the walls: where both run.
    let range = |w: &WallGeo, sp: &WallSpan| {
        let base = dot(w.start, u);
        let k = dot(w.u, u);
        let (x, y) = (base + k * sp.t0, base + k * sp.t1);
        (x.min(y), x.max(y))
    };
    let (a0, a1) = range(wa, &r.spans[0]);
    let (b0, b1) = range(wb, &r.spans[1]);
    let (r0, r1) = (a0.max(b0), a1.min(b1));
    let tr = r.rafter.thick();
    let dr = r.rafter.depth();
    if r1 - r0 < tr {
        return Err(refuse(format!(
            "{} and {} do not run side by side",
            wa.name, wb.name
        )));
    }
    let c = dot(d, s);
    let (da, db) = (wa.depth(), wb.depth());
    let (ha, hb) = (wa.top_z(), wb.top_z());
    let a_out = -da / 2.0;
    let a_in = da / 2.0;
    let b_in = c - db / 2.0;
    let b_out = c + db / 2.0;
    // In the plane across the walls: x along s from wall A's centre line.
    let origin = [wa.start[0], wa.start[1], 0.0];
    let origin = sub(origin, scale(u, dot(origin, u)));
    let point = |x: f64, z: f64, along: f64| {
        add(
            add(add(origin, scale(s, x)), scale(u, along)),
            [0.0, 0.0, z],
        )
    };
    let pitch = match (r.kind, r.pitch) {
        (_, Some(p)) => p,
        (RoofKind::Gable, None) => 30f64.to_radians(),
        (RoofKind::Mono, None) => {
            if (hb - ha).abs() < 1.0 {
                return Err(refuse(
                    "a mono-pitch roof on walls of one height needs its pitch",
                ));
            }
            (hb - ha).atan2(b_in - a_out)
        }
    };
    // A mono-pitch roof rises from the lower wall.
    let rising = match r.kind {
        RoofKind::Mono if hb < ha => -1.0,
        _ => 1.0,
    };
    let p = pitch.abs();
    if !(5f64.to_radians() - EPS..=75f64.to_radians() + EPS).contains(&p) {
        return Err(refuse(format!(
            "the pitch of {} degrees is not between 5 and 75",
            round(p.to_degrees())
        )));
    }
    if r.kind == RoofKind::Gable && (ha - hb).abs() > 0.5 {
        return Err(refuse(format!(
            "a gable roof stands on walls of one height: {} is {} mm high, {} {} mm",
            wa.name,
            round(ha),
            wb.name,
            round(hb)
        )));
    }
    let (sin, cos, tan) = (p.sin(), p.cos(), p.tan());
    let seat = da.min(db).min(dr / (3.0 * sin));
    let plumb = dr / cos;
    // The rafters' places along the walls.
    if r.spacing <= tr + EPS {
        return Err(refuse(format!(
            "the centre distance of {} mm is not more than a rafter's {} mm",
            round(r.spacing),
            round(tr)
        )));
    }
    let mut places: Vec<(String, f64)> = vec![("start".to_owned(), r0 + tr / 2.0)];
    for k in 1.. {
        let centre = r0 + k as f64 * r.spacing;
        if centre + tr / 2.0 > r1 - tr + EPS {
            break;
        }
        places.push((k.to_string(), centre));
    }
    places.push(("end".to_owned(), r1 - tr / 2.0));
    // A birdsmouth over a wall's top plate: the box over its plate's column,
    // below its top.
    let e3 = cross(s, Z);
    let notch = |x0: f64, x1: f64, h: f64, along: f64, with: Option<ComponentUid>| Notch {
        kind: "birdsmouth",
        origin: sub(
            point(x0, h - plumb - (x1 - x0) * tan - 50.0, along),
            scale(e3, tr / 2.0 + 1.0),
        ),
        x: s,
        y: Z,
        size: [x1 - x0, plumb + (x1 - x0) * tan + 50.0, tr + 2.0],
        with,
    };
    // The depth of a birdsmouth across the rafter, where its bottom line
    // is `bottom(x)`.
    let depth_of = |bottom: &dyn Fn(f64) -> f64, x0: f64, x1: f64, h: f64| {
        [x0, x1]
            .iter()
            .map(|x| (h - bottom(*x)) * cos)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let check = |name: &str, depth: f64, out: &mut Layout| {
        if depth <= EPS {
            out.warnings.push(format!(
                "{}: the rafters do not bear on {name}: they pass {} mm above its top plate",
                r.name,
                round(-depth / cos)
            ));
        } else if depth > dr / 3.0 + 0.5 {
            out.warnings.push(format!(
                "{}: the birdsmouth on {name} is {} mm deep, more than a third of the rafter's {} mm",
                r.name,
                round(depth),
                round(dr)
            ));
        }
    };
    let leans = |up: V3| [up, scale(up, -1.0)];
    match r.kind {
        RoofKind::Gable => {
            let tk = r.ridge.thick();
            let middle = (a_out + b_out) / 2.0;
            // Side A: up from its eave to the ridge board's face.
            let bottom_a = |x: f64| ha + (x - (a_out + seat)) * tan;
            let x_r = middle - tk / 2.0;
            let x_e = a_out - r.overhang;
            let up_a = add(scale(s, -sin), [0.0, 0.0, cos]);
            let up_b = add(scale(s, sin), [0.0, 0.0, cos]);
            let top_ridge = bottom_a(x_r) + plumb;
            check(&wa.name, depth_of(&bottom_a, a_out, a_in, ha), &mut out);
            for (key, along) in &places {
                let start = add(point(x_e, bottom_a(x_e), *along), scale(up_a, dr / 2.0));
                let end = sub(
                    point(x_r, bottom_a(x_r) + plumb, *along),
                    scale(up_a, dr / 2.0),
                );
                out.members.push(MemberSpec {
                    key: format!("rafter.a.{key}"),
                    role: "rafter",
                    which: Which::Part,
                    start,
                    end,
                    wide: up_a,
                    plumb: Some((p, leans(up_a))),
                    notches: vec![notch(a_out, a_in, ha, *along, wa.lead)],
                });
            }
            // Side B, the mirror.
            let bottom_b = |x: f64| hb + ((b_out - seat) - x) * tan;
            let x_rb = middle + tk / 2.0;
            let x_eb = b_out + r.overhang;
            check(&wb.name, depth_of(&bottom_b, b_in, b_out, hb), &mut out);
            for (key, along) in &places {
                let start = add(point(x_eb, bottom_b(x_eb), *along), scale(up_b, dr / 2.0));
                let end = sub(
                    point(x_rb, bottom_b(x_rb) + plumb, *along),
                    scale(up_b, dr / 2.0),
                );
                out.members.push(MemberSpec {
                    key: format!("rafter.b.{key}"),
                    role: "rafter",
                    which: Which::Part,
                    start,
                    end,
                    wide: up_b,
                    plumb: Some((p, leans(up_b))),
                    notches: vec![notch(b_in, b_out, hb, *along, wb.lead)],
                });
            }
            // The ridge board on edge, its top at the rafters' tops.
            let wk = r.ridge.depth();
            if wk + 0.5 < plumb {
                out.warnings.push(format!(
                    "{}: the ridge board is {} mm deep, less than the rafters' plumb cut of {} mm",
                    r.name,
                    round(wk),
                    round(plumb)
                ));
            }
            let z = top_ridge - wk / 2.0;
            out.members.insert(
                0,
                MemberSpec {
                    key: "ridge".to_owned(),
                    role: "ridge board",
                    which: Which::Ridge,
                    start: point(middle, z, r0),
                    end: point(middle, z, r1),
                    wide: Z,
                    plumb: None,
                    notches: Vec::new(),
                },
            );
        }
        RoofKind::Mono => {
            // From the lower wall (`low`) over the higher one; x runs from
            // the lower one's outer face towards the other.
            let (lo_out, lo_in, hi_in, hi_out, h_lo, h_hi, lo_lead, hi_lead, lo_name, hi_name) =
                if rising > 0.0 {
                    (
                        a_out, a_in, b_in, b_out, ha, hb, wa.lead, wb.lead, &wa.name, &wb.name,
                    )
                } else {
                    (
                        b_out, b_in, a_in, a_out, hb, ha, wb.lead, wa.lead, &wb.name, &wa.name,
                    )
                };
            let dir = if rising > 0.0 { 1.0 } else { -1.0 };
            // x' along the slope's run from the low wall's outer face.
            let run = |xp: f64| lo_out + dir * xp;
            let bottom = |xp: f64| h_lo + (xp - seat) * tan;
            let up = add(scale(s, -sin * dir), [0.0, 0.0, cos]);
            let (hi_in_p, hi_out_p) = ((hi_in - lo_out) * dir, (hi_out - lo_out) * dir);
            let (lo_in_p, lo_out_p) = ((lo_in - lo_out) * dir, 0.0);
            check(
                lo_name,
                depth_of(&bottom, lo_out_p, lo_in_p, h_lo),
                &mut out,
            );
            check(
                hi_name,
                depth_of(&bottom, hi_in_p, hi_out_p, h_hi),
                &mut out,
            );
            let x_e = -r.overhang;
            let x_h = hi_out_p + r.overhang;
            for (key, along) in &places {
                let start = add(point(run(x_e), bottom(x_e), *along), scale(up, dr / 2.0));
                let end = sub(
                    point(run(x_h), bottom(x_h) + plumb, *along),
                    scale(up, dr / 2.0),
                );
                let column = |x0: f64, x1: f64| (run(x0).min(run(x1)), run(x0).max(run(x1)));
                let (l0, l1) = column(lo_out_p, lo_in_p);
                let (h0, h1) = column(hi_in_p, hi_out_p);
                out.members.push(MemberSpec {
                    key: format!("rafter.{key}"),
                    role: "rafter",
                    which: Which::Part,
                    start,
                    end,
                    wide: up,
                    plumb: Some((p, leans(up))),
                    notches: vec![
                        notch(l0, l1, h_lo, *along, lo_lead),
                        notch(h0, h1, h_hi, *along, hi_lead),
                    ],
                });
            }
        }
    }
    Ok(out)
}

// --- Planning against the document.

impl<K: Kernel> Document<K> {
    /// A part of a frame: timber with a section.
    fn frame_part(&self, part: &PartInput) -> Result<FramePart, ModelError> {
        let source = self.source_use(part)?;
        let label = source.def.label();
        if !source.meta.is_timber() {
            return Err(refuse(format!(
                "{label} is no timber: a frame is made of timber members"
            )));
        }
        if source.meta.role != BuildRole::Profile || source.meta.length.is_none() {
            return Err(refuse(format!("{label} is not cut to length")));
        }
        let [w, t] = source
            .meta
            .section_size()
            .ok_or_else(|| refuse(format!("{label} has no section (build.section)")))?;
        Ok(FramePart {
            input: part.clone(),
            source,
            w,
            t,
        })
    }

    /// The lead of the frame a component is a member of.
    pub(crate) fn frame_lead(&self, component: ComponentUid) -> Option<ComponentUid> {
        self.assembly().component(component)?.build.as_ref()?.frame
    }

    /// A frame's members in the order they were made.
    pub(crate) fn frame_members(&self, lead: ComponentUid) -> Vec<ComponentUid> {
        self.assembly()
            .components
            .iter()
            .filter(|c| c.build.as_ref().is_some_and(|b| b.frame == Some(lead)))
            .map(|c| c.uid)
            .collect()
    }

    /// The fasteners driven at junctions of a frame's members (each
    /// junction's first, which takes the others with it).
    pub(crate) fn frame_fasteners(&self, lead: ComponentUid) -> Vec<ComponentUid> {
        let members: BTreeSet<ComponentUid> = self.frame_members(lead).into_iter().collect();
        self.assembly()
            .components
            .iter()
            .filter(|c| {
                c.build.as_ref().is_some_and(|b| {
                    b.role == BuildRole::Fastener
                        && b.added_by.is_none()
                        && b.junction.as_deref().is_some_and(|j| {
                            j.split('|').all(|side| {
                                side.split(':')
                                    .next()
                                    .and_then(|c| c.parse::<ComponentUid>().ok())
                                    .is_some_and(|c| members.contains(&c))
                            })
                        })
                })
            })
            .map(|c| c.uid)
            .collect()
    }

    /// The frames of the design.
    fn known_frames(&self) -> Vec<Known> {
        self.assembly()
            .components
            .iter()
            .filter_map(|c| {
                let record = c.build.as_ref()?;
                let def: FrameDef = serde_json::from_value(record.framing.clone()?).ok()?;
                Some(Known {
                    lead: Some(c.uid),
                    def: def.frame,
                })
            })
            .collect()
    }

    /// The lead of the frame a reference names (any member of it).
    fn lead_of_ref(&self, text: &str) -> Result<ComponentUid, ModelError> {
        let c = self.component_by_ref(text)?;
        self.frame_lead(c)
            .ok_or_else(|| refuse(format!("{text} is in no timber frame")))
    }

    fn mm(&self, value: &Option<ValueInput>, default: f64) -> Result<f64, ModelError> {
        value
            .as_ref()
            .map_or(Ok(default), |v| value_in(self, v, Unit::MM))
    }

    /// A wall's definition worked out.
    fn wall_geo(&self, known: &Known) -> Result<WallGeo, ModelError> {
        let def = &known.def;
        let name = def.name.clone().unwrap_or_else(|| "Wall".to_owned());
        let start = def
            .start
            .ok_or_else(|| refuse(format!("{name} needs its start")))?;
        let end = def
            .end
            .ok_or_else(|| refuse(format!("{name} needs its end")))?;
        let u = level(sub(end, start))
            .ok_or_else(|| refuse(format!("{name}: its start and end are one point")))?;
        let mut length = dot(sub(end, start), u);
        if let Some(l) = &def.length {
            length = value_in(self, l, Unit::MM)?;
        }
        let part = def
            .part
            .as_ref()
            .ok_or_else(|| refuse(format!("{name} needs its timber (part)")))?;
        let stud = self.frame_part(part)?;
        let plate = match &def.plate {
            Some(p) => self.frame_part(p)?,
            None => stud.clone(),
        };
        let header = match &def.header {
            Some(p) => self.frame_part(p)?,
            None => stud.clone(),
        };
        let plates = def.plates.clone().unwrap_or_default();
        let count = |n: Option<u32>, which: &str| -> Result<usize, ModelError> {
            match n.unwrap_or(1) {
                n @ 1..=2 => Ok(n as usize),
                n => Err(refuse(format!("{name}: {n} {which} plates; one or two"))),
            }
        };
        let mut warnings = Vec::new();
        let mut joins = [None, None];
        if let Some(j) = &def.joins {
            for (k, text) in [&j.start, &j.end].into_iter().enumerate() {
                if let Some(text) = text {
                    match self.lead_of_ref(text) {
                        Ok(lead) => joins[k] = Some(lead),
                        Err(e) => warnings.push(format!("{name}: {e}")),
                    }
                }
            }
        }
        Ok(WallGeo {
            lead: known.lead,
            start,
            u,
            n: cross(Z, u),
            length,
            height: self.mm(&def.height, 2400.0)?,
            spacing: self.mm(&def.spacing, 600.0)?,
            bottom: count(plates.bottom, "bottom")?,
            top: count(plates.top, "top")?,
            stud,
            plate,
            header,
            openings: def.openings.clone().unwrap_or_default(),
            joins,
            butt: def.corner == Some(WallCorner::Butt),
            start_on_end: def
                .joins
                .as_ref()
                .is_some_and(|j| j.start_on == Some(JoinOn::End)),
            warnings,
            name,
        })
    }

    /// The walls of a world of frames (those that cannot be worked out are
    /// left out).
    fn walls_of(&self, world: &[Known]) -> Vec<WallGeo> {
        world
            .iter()
            .filter(|k| k.def.kind() == FrameKind::Wall)
            .filter_map(|k| self.wall_geo(k).ok())
            .collect()
    }

    fn floor_geo(&self, known: &Known) -> Result<FloorGeo, ModelError> {
        let def = &known.def;
        let name = def.name.clone().unwrap_or_else(|| "Floor".to_owned());
        let outline = def
            .outline
            .clone()
            .filter(|o| o.len() >= 3)
            .ok_or_else(|| refuse(format!("{name} needs its outline (three points at least)")))?;
        let part = def
            .part
            .as_ref()
            .ok_or_else(|| refuse(format!("{name} needs its timber (part)")))?;
        let joist = self.frame_part(part)?;
        let rim = match &def.rim {
            Some(p) => self.frame_part(p)?,
            None => joist.clone(),
        };
        let first = level(sub(outline[1], outline[0]))
            .ok_or_else(|| refuse(format!("{name}: its outline's first side has no length")))?;
        let v = match def.direction.and_then(level) {
            Some(v) => v,
            None => {
                // Square to the first side, into the outline.
                let area: f64 = (0..outline.len())
                    .map(|i| cross2(outline[i], outline[(i + 1) % outline.len()]))
                    .sum();
                let left = cross(Z, first);
                if area >= 0.0 { left } else { scale(left, -1.0) }
            }
        };
        let a = cross(v, Z);
        Ok(FloorGeo {
            z0: outline[0][2],
            outline,
            a,
            v,
            spacing: self.mm(&def.spacing, 400.0)?,
            blocking: def.blocking.unwrap_or(0) as usize,
            trimmers: def.trimmers.unwrap_or(2) as usize,
            openings: def.openings.clone().unwrap_or_default(),
            joist,
            rim,
            name,
        })
    }

    fn roof_geo(&self, known: &Known, world: &[Known]) -> Result<RoofGeo, ModelError> {
        let def = &known.def;
        let name = def.name.clone().unwrap_or_else(|| "Roof".to_owned());
        let refs = def
            .walls
            .clone()
            .filter(|w| w.len() == 2)
            .ok_or_else(|| refuse(format!("{name} stands on two wall frames (walls)")))?;
        let walls = self.walls_of(world);
        let mut found = Vec::new();
        for text in &refs {
            let lead = self.lead_of_ref(text)?;
            let wall = walls
                .iter()
                .find(|w| w.lead == Some(lead))
                .cloned()
                .ok_or_else(|| refuse(format!("{text} is no wall frame")))?;
            found.push(wall);
        }
        let part = def
            .part
            .as_ref()
            .ok_or_else(|| refuse(format!("{name} needs its timber (part)")))?;
        let rafter = self.frame_part(part)?;
        let ridge = match &def.ridge {
            Some(p) => self.frame_part(p)?,
            None => rafter.clone(),
        };
        let spans = [wall_span(&found[0], &walls), wall_span(&found[1], &walls)];
        let pitch = match &def.pitch {
            Some(v) => Some(value_in(self, v, Unit::DEG)?),
            None => None,
        };
        let [a, b]: [WallGeo; 2] = found.try_into().expect("two walls");
        Ok(RoofGeo {
            kind: def.roof.unwrap_or_default(),
            walls: [a, b],
            spans,
            pitch,
            spacing: self.mm(&def.spacing, 600.0)?,
            overhang: self.mm(&def.overhang, 300.0)?,
            rafter,
            ridge,
            name,
        })
    }

    /// Lays a frame out and plans its members.
    fn plan_frame(&self, target: &Known, world: &[Known]) -> Result<FramePlan, ModelError> {
        let kind = target.def.kind();
        let name = target
            .def
            .name
            .clone()
            .unwrap_or_else(|| kind.title().to_owned());
        let (layout, parts) = match kind {
            FrameKind::Wall => {
                let me = self.wall_geo(target)?;
                let mut walls: Vec<WallGeo> = self
                    .walls_of(world)
                    .into_iter()
                    .filter(|w| w.lead != target.lead)
                    .collect();
                walls.push(me.clone());
                let span = wall_span(&me, &walls);
                let parts = BTreeMap::from([
                    (Which::Part, me.stud.clone()),
                    (Which::Plate, me.plate.clone()),
                    (Which::Header, me.header.clone()),
                ]);
                (wall_layout(&me, &span)?, parts)
            }
            FrameKind::Floor => {
                let f = self.floor_geo(target)?;
                let parts =
                    BTreeMap::from([(Which::Part, f.joist.clone()), (Which::Rim, f.rim.clone())]);
                (floor_layout(&f)?, parts)
            }
            FrameKind::Roof => {
                let r = self.roof_geo(target, world)?;
                let parts = BTreeMap::from([
                    (Which::Part, r.rafter.clone()),
                    (Which::Ridge, r.ridge.clone()),
                ]);
                (roof_layout(&r)?, parts)
            }
        };
        let mut warnings = layout.warnings;
        let mut members = Vec::new();
        for spec in layout.members {
            let part = &parts[&spec.which];
            let length = norm(sub(spec.end, spec.start));
            let least = part.least();
            if length < least - EPS {
                warnings.push(format!(
                    "{name}: the {} {} would be {} mm, shorter than its timber's least length of {} mm: left out",
                    spec.role,
                    spec.key,
                    round(length),
                    round(least)
                ));
                continue;
            }
            let input = PlaceInput {
                part: part.input.clone(),
                at: Some(PlaceAt {
                    point: Some(spec.start),
                    ..PlaceAt::default()
                }),
                to: Some(PointTo { point: spec.end }),
                ..PlaceInput::default()
            };
            let mut plan = self
                .plan_place(&input)
                .map_err(|e| refuse(format!("{name}: the {} {}: {e}", spec.role, spec.key)))?;
            // Its axes exactly as the frame wants them.
            let d = unit(sub(spec.end, spec.start)).unwrap_or(Z);
            let x = if part.w >= part.t {
                spec.wide
            } else {
                cross(d, spec.wide)
            };
            let end_a = plan.mounts.get("end_a").map_or([0.0; 3], |t| t.translation);
            let placement = super::geometry::frame(spec.start, x, d)
                .after(&Transform::translation(scale(end_a, -1.0)));
            plan.spot = Spot::Grounded(placement);
            plan.placement = placement;
            plan.points = None;
            let to_part = inverse(&placement);
            if let Some((angle, leans)) = &spec.plumb {
                let cut = |lean: V3| {
                    Tilt::towards(to_part.apply_vector(lean)).map(|tilt| EndCut {
                        angle: ValueInput::Number(*angle),
                        tilt: Some(tilt),
                    })
                };
                plan.own_ends = Ends {
                    a: cut(leans[0]),
                    b: cut(leans[1]),
                };
            }
            let tidy = |v: f64| ValueInput::Number((v * 1e6).round() / 1e6);
            let cuts = spec
                .notches
                .iter()
                .map(|n| {
                    let o = to_part.apply_point(n.origin);
                    PartCut {
                        kind: n.kind.to_owned(),
                        with: n.with,
                        junction: None,
                        origin: o.map(tidy),
                        x: to_part.apply_vector(n.x),
                        y: to_part.apply_vector(n.y),
                        size: n.size.map(tidy),
                        depth: None,
                    }
                })
                .collect();
            members.push(PlannedMember { spec, plan, cuts });
        }
        if members.is_empty() {
            return Err(refuse(format!("{name} has no members")));
        }
        // The members that stay: by their place, made of the same source.
        let mut keep = BTreeMap::new();
        if let Some(lead) = target.lead {
            for c in self.frame_members(lead) {
                let Some(record) = self.assembly().component(c).and_then(|c| c.build.as_ref())
                else {
                    continue;
                };
                let Some(key) = &record.member else { continue };
                let same = members.iter().any(|m| {
                    &m.spec.key == key
                        && !m.plan.source.new
                        && record.source == Some(m.plan.source.def.id)
                });
                if same {
                    keep.insert(key.clone(), c);
                }
            }
        }
        Ok(FramePlan {
            lead: target.lead,
            name,
            kind,
            def: target.def.clone(),
            parts,
            members,
            keep,
            warnings,
        })
    }

    /// The frames a change of `target` lays out again: itself, the walls
    /// it butts against (before and after the change), the walls that butt
    /// against it, and the roofs on any of these walls.
    fn affected(&self, target: usize, before: Option<&FrameInput>, world: &[Known]) -> Vec<usize> {
        let mut out = vec![target];
        let index_of = |lead: ComponentUid| world.iter().position(|k| k.lead == Some(lead));
        let joins_of = |def: &FrameInput| -> Vec<ComponentUid> {
            def.joins
                .iter()
                .flat_map(|j| [&j.start, &j.end])
                .flatten()
                .filter_map(|t| self.lead_of_ref(t).ok())
                .collect()
        };
        let me = &world[target];
        if me.def.kind() == FrameKind::Wall {
            let mut joined = joins_of(&me.def);
            if let Some(before) = before {
                joined.extend(joins_of(before));
            }
            for lead in joined {
                if let Some(i) = index_of(lead) {
                    out.push(i);
                }
            }
            if let Some(lead) = me.lead {
                for (i, k) in world.iter().enumerate() {
                    if k.def.kind() == FrameKind::Wall && joins_of(&k.def).contains(&lead) {
                        out.push(i);
                    }
                }
            }
            let walls: Vec<ComponentUid> = out.iter().filter_map(|i| world[*i].lead).collect();
            for (i, k) in world.iter().enumerate() {
                if k.def.kind() != FrameKind::Roof {
                    continue;
                }
                let on = k
                    .def
                    .walls
                    .iter()
                    .flatten()
                    .filter_map(|t| self.lead_of_ref(t).ok())
                    .any(|l| walls.contains(&l));
                if on {
                    out.push(i);
                }
            }
        }
        let mut seen = BTreeSet::new();
        out.retain(|i| seen.insert(*i));
        out
    }

    /// A free frame name: `Wall 1`, `Wall 2`, ...
    fn free_frame_name(&self, kind: FrameKind, world: &[Known]) -> String {
        (1..)
            .map(|n| format!("{} {n}", kind.title()))
            .find(|n| {
                !world
                    .iter()
                    .any(|k| k.def.name.as_deref() == Some(n.as_str()))
            })
            .expect("a free name exists")
    }

    /// `build_frame`: a wall, floor or roof frame.
    pub fn build_frame(&mut self, input: &FrameInput) -> Result<Framed, ModelError> {
        let kind = input
            .kind
            .ok_or_else(|| refuse("a frame's kind is wall, floor or roof"))?;
        if input.part.is_none() {
            return Err(refuse("a frame needs its timber (part)"));
        }
        let mut world = self.known_frames();
        let mut def = input.clone();
        def.kind = Some(kind);
        def.fastener = def.fastener.map(|f| self.kept_fastener(&f));
        let junction = self.square_start(&mut def, &world)?;
        match &def.name {
            Some(name) if world.iter().any(|k| k.def.name.as_deref() == Some(name)) => {
                return Err(refuse(format!("a frame is already named '{name}'")));
            }
            Some(_) => {}
            None => def.name = Some(self.free_frame_name(kind, &world)),
        }
        // A roof's walls and a wall's joins are kept as the frames' leads.
        if let Some(walls) = &mut def.walls {
            for w in walls.iter_mut() {
                *w = self.lead_of_ref(w)?.to_string();
            }
        }
        if let Some(joins) = &mut def.joins {
            for j in [&mut joins.start, &mut joins.end].into_iter().flatten() {
                *j = self.lead_of_ref(j)?.to_string();
            }
        }
        world.push(Known { lead: None, def });
        let target = world.len() - 1;
        let affected = self.affected(target, None, &world);
        let mut framed = self.lay_out(&world, &affected, None, Vec::new())?;
        framed.junction = junction;
        Ok(framed)
    }

    /// A wall started on another wall's side (`square`, mitcad#493): its
    /// start and end made square to that wall, as its `corner` has it.
    /// Butting (mitcad#510): a start on that wall's side starts it there,
    /// its centre line at the point's place along the wall, kept off its
    /// ends by half its own depth (`side`); a start on one of its end faces
    /// starts it beyond that end, its side against the end face and its
    /// start flush with that wall's far side, still joined to it (`end`,
    /// `joins.start_on: end`). Lapped (left out, mitcad#493): a start near
    /// one of that wall's ends (within half its stud spacing, at least its
    /// depth) starts it at that end's corner, an L, else at the click, a T;
    /// both on that wall's centre line. At that wall's base, it runs to the side
    /// clicked (the end's side when the click is on the centre line; at an
    /// end face the end's side first), as far as `end` is along it, else
    /// `length`.
    fn square_start(
        &self,
        def: &mut FrameInput,
        world: &[Known],
    ) -> Result<Option<StartJunction>, ModelError> {
        if def.square.take() != Some(true) {
            return Ok(None);
        }
        let name = def.name.clone().unwrap_or_else(|| "The wall".to_owned());
        if def.kind() != FrameKind::Wall {
            return Err(refuse("only a wall starts square on another wall (square)"));
        }
        let text = def
            .joins
            .as_ref()
            .and_then(|j| j.start.clone())
            .ok_or_else(|| {
                refuse(format!(
                    "{name}: square needs the wall it starts on (joins.start)"
                ))
            })?;
        let lead = self.lead_of_ref(&text)?;
        let known = world
            .iter()
            .find(|k| k.lead == Some(lead) && k.def.kind() == FrameKind::Wall)
            .ok_or_else(|| refuse(format!("{text} is no wall frame")))?;
        let x = self.wall_geo(known)?;
        // Where its plates end now (after its own corners): its end faces.
        let ends = {
            let span = wall_span(&x, &self.walls_of(world));
            [span.t0, span.t1]
        };
        let click = def
            .start
            .ok_or_else(|| refuse(format!("{name} needs its start")))?;
        let rel = sub(click, x.start);
        let (t, a) = (dot(rel, x.u), dot(rel, x.n));
        if t < ends[0] - x.depth() || t > ends[1] + x.depth() {
            return Err(refuse(format!(
                "{name}'s start is not on {}'s side: {} mm along it, beyond its ends",
                x.name,
                round(t)
            )));
        }
        let end_side = def
            .end
            .map(|e| dot(sub(e, x.start), x.n))
            .filter(|b| b.abs() > EPS)
            .map(f64::signum);
        let click_side = (a.abs() > EPS).then_some(a.signum());
        let butt = def.corner == Some(WallCorner::Butt);
        // A butting start on an end face: within half a millimetre of it and
        // inside the wall's depth, or beyond the end.
        const ON: f64 = 0.5;
        let inside = a.abs() < x.depth() / 2.0 - ON;
        let on_end = if !butt {
            None
        } else if t < ends[0] + ON && (inside || t < ends[0] - ON) {
            Some(0)
        } else if t > ends[1] - ON && (inside || t > ends[1] + ON) {
            Some(1)
        } else {
            None
        };
        let side = match on_end {
            Some(_) => end_side.or(click_side),
            None => click_side.or(end_side),
        }
        .unwrap_or(1.0);
        let d = scale(x.n, side);
        // Its own depth (butting).
        let depth = match (&def.part, butt) {
            (Some(part), true) => self.frame_part(part)?.depth(),
            (None, true) => return Err(refuse(format!("{name} needs its timber (part)"))),
            (_, false) => 0.0,
        };
        // The junction, the wall's start and how far it must run at least.
        let (kind, end, at, start, least) = match on_end {
            Some(k) => {
                // Its side against the end face, from the far side.
                let out = if k == 0 { -1.0 } else { 1.0 };
                let at = ends[k] + out * depth / 2.0;
                let start = x.at(at, 0.0, -side * x.depth() / 2.0);
                ("end", Some(["a", "b"][k]), at, start, x.depth())
            }
            None if butt => {
                // Its faces within that wall's end faces: no overhang.
                let at = t.min(ends[1] - depth / 2.0).max(ends[0] + depth / 2.0);
                ("side", None, at, x.at(at, 0.0, 0.0), x.depth() / 2.0)
            }
            None => {
                let near = (x.spacing / 2.0).max(x.depth()).min(x.length / 2.0);
                let (kind, end, at) = if t <= near {
                    ("L", Some("a"), 0.0)
                } else if t >= x.length - near {
                    ("L", Some("b"), x.length)
                } else {
                    ("T", None, t)
                };
                (kind, end, at, x.at(at, 0.0, 0.0), x.depth() / 2.0)
            }
        };
        let length = match (def.end, &def.length) {
            (Some(e), _) => dot(sub(e, start), d),
            (None, Some(l)) => value_in(self, l, Unit::MM)?,
            (None, None) => {
                return Err(refuse(format!("{name} needs its end or its length")));
            }
        };
        if length <= least + EPS {
            return Err(refuse(if on_end.is_some() {
                format!(
                    "{name} would be {} mm long from {}'s far side, not beyond its other side",
                    round(length),
                    x.name
                )
            } else {
                format!(
                    "{name} would end {} mm from {}'s centre line, not beyond its side",
                    round(length),
                    x.name
                )
            }));
        }
        def.start = Some(start);
        def.end = Some(add(start, scale(d, length)));
        def.length = None;
        // At an end it stays joined to that wall, whose end face butts
        // against its side (laid out again with it).
        if let Some(joins) = &mut def.joins {
            joins.start_on = on_end.map(|_| JoinOn::End);
        }
        Ok(Some(StartJunction {
            kind,
            corner: def.corner.unwrap_or(WallCorner::Lapped).as_str(),
            wall: x.name.clone(),
            frame: lead,
            end,
            at,
            length,
        }))
    }

    /// A frame's `build_edit`: its definition with the edit's fields over
    /// it (`config` the members' row), laid out again with the frames
    /// that depend on it.
    pub(crate) fn edit_frame(
        &mut self,
        component: ComponentUid,
        edit: &FrameInput,
        config: Option<&String>,
    ) -> Result<Framed, ModelError> {
        let lead = self.frame_lead(component).ok_or_else(|| {
            refuse(format!(
                "{} is in no timber frame",
                self.assembly().name(component)
            ))
        })?;
        let mut world = self.known_frames();
        let target = world
            .iter()
            .position(|k| k.lead == Some(lead))
            .ok_or_else(|| refuse("the frame's definition is gone"))?;
        let before = world[target].def.clone();
        let mut def = self.edited_def(&before, edit)?;
        def.fastener = def.fastener.map(|f| self.kept_fastener(&f));
        if let Some(config) = config
            && let Some(part) = &mut def.part
        {
            part.config = Some(config.clone());
        }
        world[target].def = def;
        let affected = self.affected(target, Some(&before), &world);
        let mut changed = vec!["frame".to_owned()];
        if config.is_some() {
            changed.push("config".to_owned());
        }
        self.lay_out(&world, &affected, Some(before), changed)
    }

    /// An edit over a definition: a wall's `length` moves its end along it,
    /// joins and walls are kept as leads.
    fn edited_def(&self, before: &FrameInput, edit: &FrameInput) -> Result<FrameInput, ModelError> {
        let mut def = before.merged(edit);
        def.kind = before.kind;
        if let (Some(length), Some(start), Some(end)) = (&edit.length, def.start, def.end) {
            let l = value_in(self, length, Unit::MM)?;
            if l <= EPS {
                return Err(refuse("a wall's length must be greater than zero"));
            }
            let u = level(sub(end, start)).unwrap_or([1.0, 0.0, 0.0]);
            def.end = Some(add(start, scale(u, l)));
        }
        def.length = None;
        if let Some(joins) = &mut def.joins {
            for j in [&mut joins.start, &mut joins.end].into_iter().flatten() {
                *j = self.lead_of_ref(j)?.to_string();
            }
        }
        if let Some(walls) = &mut def.walls {
            for w in walls.iter_mut() {
                *w = self.lead_of_ref(w)?.to_string();
            }
        }
        Ok(def)
    }

    /// The `build_preview` of a frame: its members as `build_frame` (or a
    /// frame's edit, `component` given) would lay them out.
    pub(crate) fn preview_frame(
        &self,
        input: &FrameInput,
        component: Option<ComponentUid>,
    ) -> Result<Value, ModelError> {
        let mut world = self.known_frames();
        let mut junction = None;
        let target = match component {
            Some(c) => {
                let lead = self
                    .frame_lead(c)
                    .ok_or_else(|| refuse("the part is in no timber frame"))?;
                let i = world
                    .iter()
                    .position(|k| k.lead == Some(lead))
                    .ok_or_else(|| refuse("the frame's definition is gone"))?;
                world[i].def = self.edited_def(&world[i].def.clone(), input)?;
                i
            }
            None => {
                let mut def = input.clone();
                junction = self.square_start(&mut def, &world)?;
                if def.name.is_none() {
                    def.name = Some(self.free_frame_name(def.kind(), &world));
                }
                world.push(Known { lead: None, def });
                world.len() - 1
            }
        };
        let plan = self.plan_frame(&world[target], &world)?;
        let mut roles: BTreeMap<&str, usize> = BTreeMap::new();
        for m in &plan.members {
            *roles.entry(m.spec.role).or_default() += 1;
        }
        let host = self.host_placement();
        Ok(json!({
            "place": "frame",
            "kind": plan.kind.as_str(),
            "name": plan.name,
            "count": plan.members.len(),
            "roles": roles,
            "members": plan.members.iter().map(|m| json!({
                "member": m.spec.key,
                "role": m.spec.role,
                "start": host.apply_point(m.spec.start),
                "end": host.apply_point(m.spec.end),
                "length": norm(sub(m.spec.end, m.spec.start)),
                // Its section's width along `x`, its thickness across.
                "x": host.apply_vector(m.plan.placement.apply_vector(super::geometry::X)),
                "section": [plan.parts[&m.spec.which].w, plan.parts[&m.spec.which].t],
            })).collect::<Vec<_>>(),
            "warnings": plan.warnings,
            "problems": [],
            "junction": junction.as_ref().map(StartJunction::to_json),
        }))
    }

    /// Lays the `affected` frames of a world out and makes them in one undo
    /// step: the fasteners of the frames already there taken away, the
    /// members kept, added and removed, then every frame's fasteners
    /// driven.
    fn lay_out(
        &mut self,
        world: &[Known],
        affected: &[usize],
        before: Option<FrameInput>,
        changed: Vec<String>,
    ) -> Result<Framed, ModelError> {
        // The frame itself must lay out; one that depends on it and no
        // longer does stays as it is.
        let mut plans = Vec::new();
        let mut kept_back = Vec::new();
        for (n, i) in affected.iter().enumerate() {
            match self.plan_frame(&world[*i], world) {
                Ok(plan) => plans.push(plan),
                Err(e) if n > 0 => kept_back.push(format!(
                    "{} was not laid out again: {e}",
                    world[*i].def.name.clone().unwrap_or_default()
                )),
                Err(e) => return Err(e),
            }
        }
        let mut removals: Vec<RemovePlan> = Vec::new();
        for plan in &plans {
            let Some(lead) = plan.lead else { continue };
            for f in self.frame_fasteners(lead) {
                removals.push(self.plan_remove(f)?);
            }
            for c in self.frame_members(lead) {
                if !plan.keep.values().any(|k| *k == c) {
                    removals.push(self.plan_remove(c)?);
                }
            }
        }
        let active = self.build_host();
        self.state().editable(active)?;
        let first = &plans[0];
        let label = match before {
            Some(_) => format!("Edit {}", first.name),
            None => format!("Place {} frame {}", first.kind.as_str(), first.name),
        };
        let depth = self.undo_depth();
        let made = self.apply(|state| {
            for removal in &removals {
                apply_remove(state, removal)?;
            }
            let mut made: Vec<(ComponentUid, Vec<(ComponentUid, String)>)> = Vec::new();
            for plan in &plans {
                let first = made.first().map(|(lead, _)| *lead);
                made.push(apply_frame(state, plan, active, first)?);
            }
            Ok((label.clone(), made))
        })?;
        // The fasteners, each frame's anew.
        let mut warnings: Vec<String> = plans.iter().flat_map(|p| p.warnings.clone()).collect();
        warnings.extend(kept_back);
        let mut fasteners = 0;
        for (plan, (lead, _)) in plans.iter().zip(&made) {
            if let Some(part) = &plan.def.fastener {
                fasteners += self.fasten_frame(*lead, part, plan.def.fastener_count, &mut warnings);
            }
        }
        self.merge_undo(depth, &label);
        let (lead, members) = made[0].clone();
        let mut seen = BTreeSet::new();
        warnings.retain(|w| seen.insert(w.clone()));
        Ok(Framed {
            lead: Some(lead),
            name: first.name.clone(),
            kind: first.kind.as_str(),
            members,
            fasteners: if first.def.fastener.is_some() {
                fasteners
            } else {
                0
            },
            warnings,
            relaid: made[1..].iter().map(|(l, _)| *l).collect(),
            changed,
            junction: None,
        })
    }

    /// Drives the frame's fasteners (`count` at each, default 2; sized
    /// from the members unless `part` names a row) at every junction of a
    /// frame's members without a connector; returns how many junctions got
    /// them. A junction they are refused at is left as it is.
    fn fasten_frame(
        &mut self,
        lead: ComponentUid,
        part: &PartInput,
        count: Option<u32>,
        warnings: &mut Vec<String>,
    ) -> usize {
        let members: BTreeSet<ComponentUid> = self.frame_members(lead).into_iter().collect();
        let list: Vec<ComponentUid> = members.iter().copied().collect();
        let junctions: Vec<String> = self
            .build_junctions(Some(&list))
            .into_iter()
            .filter(|j| members.contains(&j.a.component) && members.contains(&j.b.component))
            .filter(|j| j.connectors.is_empty())
            .map(|j| j.id)
            .collect();
        self.fasten_junctions(&junctions, part, count, warnings)
    }

    /// Drives `count` wood fasteners (default 2) of `part` at each of the
    /// junctions (mitcad#479, mitcad#497); returns how many got them, the
    /// first ten fastener warnings added. A junction they are refused at is
    /// left as it is.
    pub(crate) fn fasten_junctions(
        &mut self,
        junctions: &[String],
        part: &PartInput,
        count: Option<u32>,
        warnings: &mut Vec<String>,
    ) -> usize {
        let mut done = 0;
        let mut own = Vec::new();
        for id in junctions {
            let mut fields = json!({"junction": id, "part": part});
            if let Some(n) = count {
                fields["count"] = json!(n.max(1));
            }
            let input: super::ConnectInput = match serde_json::from_value(fields) {
                Ok(input) => input,
                Err(_) => continue,
            };
            match self.build_connect(&input) {
                Ok(placed) => {
                    done += 1;
                    own.extend(placed.warnings);
                }
                Err(_) => continue,
            }
        }
        const SHOWN: usize = 10;
        let more = own.len().saturating_sub(SHOWN);
        warnings.extend(own.into_iter().take(SHOWN));
        if more > 0 {
            warnings.push(format!("and {more} more fastener warning(s)"));
        }
        done
    }
}

/// The cuts a frame makes in its members (laid out anew with it): a
/// rafter's birdsmouth, a lapped corner's half lap (no junction).
fn frame_cut<P>(c: &PartCut<P>) -> bool {
    c.kind == "birdsmouth" || (c.kind == "half_lap" && c.junction.is_none())
}

/// Makes a planned frame in `state`: the sources it adds, its members kept
/// (placed anew), made or notched, its definition on its lead. Returns the
/// lead and the members. `made_first`: the lead of the frame made first in
/// the step, for the cuts with the frame being made.
fn apply_frame(
    state: &mut DocState,
    plan: &FramePlan,
    active: ComponentUid,
    made_first: Option<ComponentUid>,
) -> Result<(ComponentUid, Vec<(ComponentUid, String)>), ModelError> {
    // The sources, each once.
    let mut ids: BTreeMap<Which, SourceId> = BTreeMap::new();
    for (which, part) in &plan.parts {
        let source = &part.source;
        let id = if !source.new {
            source.def.id
        } else if let Some(id) = state.library_sources.find(
            &source.def.library.library,
            &source.def.library.component,
            &source.def.library.rev,
        ) {
            id
        } else {
            let mut def = source.def.clone();
            def.id = state.library_sources.next_id();
            let id = def.id;
            state.library_sources.0.push(def);
            id
        };
        ids.insert(*which, id);
    }
    let as_source = |which: Which, input: &PartInput| PartInput {
        source: Some(ids[&which]),
        config: input.config.clone(),
        ..PartInput::default()
    };
    let mut def = plan.def.clone();
    for (which, slot) in [
        (Which::Part, &mut def.part),
        (Which::Plate, &mut def.plate),
        (Which::Header, &mut def.header),
        (Which::Rim, &mut def.rim),
        (Which::Ridge, &mut def.ridge),
    ] {
        if let Some(input) = slot.as_ref()
            && ids.contains_key(&which)
        {
            *slot = Some(as_source(which, input));
        }
    }
    let mut made = Vec::new();
    for m in &plan.members {
        let mut p = m.plan.clone();
        p.source.def.id = ids[&m.spec.which];
        p.source.new = false;
        let kept = plan
            .keep
            .get(&m.spec.key)
            .copied()
            .filter(|c| state.assembly.exists(*c));
        let c = match kept {
            Some(c) => {
                replace_member(state, &p, active, c)?;
                c
            }
            None => {
                p.name = Some(state.free_component_name(&format!("{} {}", plan.name, m.spec.role)));
                make_part(state, &p, active)?
                    .component
                    .expect("a member is made")
            }
        };
        let had = state
            .assembly
            .component(c)
            .and_then(|c| c.build.as_ref())
            .and_then(|b| b.part);
        if let Some(part) = had {
            // A cut with the frame being made: with its lead.
            let notches: Vec<PartCut<ValueInput>> = m
                .cuts
                .iter()
                .cloned()
                .map(|mut c| {
                    c.with = c.with.or(made_first);
                    c
                })
                .collect();
            let has = state.entry(part).is_some_and(|e| match &e.def {
                crate::features::FeatureDef::DerivedPart(d) => d.cuts.iter().any(frame_cut),
                _ => false,
            });
            if has || !notches.is_empty() {
                edit_part(state, part, |d| {
                    d.cuts.retain(|c| !frame_cut(c));
                    d.cuts.extend(notches);
                })?;
            }
        }
        made.push((c, m.spec.key.clone()));
    }
    let lead = plan
        .lead
        .filter(|l| made.iter().any(|(c, _)| c == l))
        .unwrap_or(made[0].0);
    let value = serde_json::to_value(FrameDef {
        frame: def,
        count: made.len(),
    })
    .map_err(|e| refuse(e.to_string()))?;
    for (c, key) in &made {
        if let Some(record) = state
            .assembly
            .component_mut(*c)
            .and_then(|c| c.build.as_mut())
        {
            record.frame = Some(lead);
            record.member = Some(key.clone());
            record.framing = (*c == lead).then(|| value.clone());
            record.set = None;
            record.spacing = None;
        }
    }
    Ok((lead, made))
}

/// When a frame's lead goes alone, the next member that stays takes the
/// frame over.
pub(crate) fn hand_over_frame(state: &mut DocState, going: &BTreeSet<ComponentUid>) {
    let leads: Vec<(ComponentUid, Value)> = going
        .iter()
        .filter_map(|c| {
            let record = state.assembly.component(*c)?.build.as_ref()?;
            Some((*c, record.framing.clone()?))
        })
        .collect();
    for (lead, framing) in leads {
        let members: Vec<ComponentUid> = state
            .assembly
            .components
            .iter()
            .filter(|c| c.build.as_ref().is_some_and(|b| b.frame == Some(lead)))
            .map(|c| c.uid)
            .collect();
        let Some(heir) = members.iter().copied().find(|c| !going.contains(c)) else {
            continue;
        };
        for c in members {
            if let Some(r) = state
                .assembly
                .component_mut(c)
                .and_then(|c| c.build.as_mut())
            {
                r.frame = Some(heir);
                r.framing = (c == heir).then(|| framing.clone());
            }
        }
    }
}
