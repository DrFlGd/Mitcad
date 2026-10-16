// SPDX-License-Identifier: MIT
//! What each building element is built of (mitcad#275): its solids and the
//! cuts made in them, as the IFC import's geometry items
//! ([`mitcad_ifc::geometry::Item`], millimetres, model coordinates), which
//! [`crate::ifc::build::Builder`] turns into sketches, extrusions and cuts.
//!
//! - A **wall**: its footprint ([`super::footprint::wall_footprint`])
//!   extruded from its base by its height; the cut loops of the inserts it
//!   hosts (doors, windows: their type's `m_pCutLoops` at the instance's
//!   transform) cut through it along their cut direction; the elements
//!   whose geometry cuts it (`CutPairGStep`) cut it with their own solids.
//! - A **floor** (and a flat roof): its boundary loops extruded down from
//!   its top by its thickness (`m_top - m_bottom`, else its type's width).
//! - A **ceiling** (mitcad#333): as a floor, between its two horizontal
//!   reference faces (`m_pRefFaces`), else down by its type's width.
//! - A **footprint roof** with slopes: the footprint's prism below every
//!   top plane of its slopes, less the prism below every bottom plane (the
//!   layer reference faces with the same normal: the outermost two).
//! - A **column**: its type's section (rectangular from `Width`/`Depth`,
//!   `b`/`h`; round from `Diameter`; an I section from the structural
//!   section parameters) at the instance's transform, from its base level
//!   and offset to its top level and offset; cut by the walls that cut it.
//! - A **beam**: its type's section along its straight location line, at
//!   the instance's transform, less the setbacks at its ends.
//!
//! Anything else (or a value that does not read) gives an error: the
//! element then comes in as its phase-1 mesh or box, with the reason.

use mitcad_ifc::geometry::{Area, BoolOp, Item, Path, Profile};
use mitcad_ifc::math::{self, Matrix};
use mitcad_rvt::building::{self, Building, Curve, Instance, P3, Slab, Wall};
use mitcad_rvt::categories;
use mitcad_rvt::elements::{Element, FOOT_MM, Model};

use super::footprint::{self, P2, areas, circle, loop_path, polygon, profile_mm};

/// What an element is built of.
#[derive(Debug, Clone)]
pub struct Plan {
    /// `wall`, `floor`, `roof`, `ceiling`, `column`, `beam`, `stair`.
    pub kind: &'static str,
    /// The solid (a boolean of items for a sloped roof).
    pub solid: Item,
    /// The cuts made in it: what, and the tool.
    pub cuts: Vec<(String, Item)>,
    /// Further solids of its own, bodies apart from the first (a sweep's
    /// separate paths).
    pub more: Vec<Item>,
    pub notes: Vec<String>,
    /// The material its appearance comes from.
    pub material: Option<i64>,
}

const OST_STRUCTURAL_COLUMNS: i64 = -2_001_330;
const OST_STRUCTURAL_FRAMING: i64 = -2_001_320;
const OST_CEILINGS: i64 = -2_000_038;
const OST_CURTAIN_PANELS: i64 = -2_000_170;
const OST_CURTAIN_MULLIONS: i64 = -2_000_171;

/// Whether elements of a kind are structural: their boxes stay visible
/// (mitcad#333; curtain panels and mullions are not).
pub fn structural(kind: &str) -> bool {
    !matches!(kind, "mullion" | "panel")
}

/// The kind an element is built as, if any.
pub fn kind_of(model: &Model, e: &Element) -> Option<&'static str> {
    let b = &model.building;
    if b.walls.contains_key(&e.id) {
        return Some("wall");
    }
    if let Some(s) = b.slabs.get(&e.id) {
        return Some(match s.class.as_str() {
            "Floor" | "BuildingPad" => "floor",
            "Roof" | "ProfileRoof" | "FaceRoof" => "roof",
            "Ceiling" => "ceiling",
            _ => return None,
        });
    }
    if b.stair_runs.contains_key(&e.id) {
        return Some("stair");
    }
    if b.landings.contains_key(&e.id) {
        return Some("landing");
    }
    if let Some(s) = b.sweeps.get(&e.id) {
        return Some(sweep_kind(s));
    }
    if b.instances.contains_key(&e.id) {
        return match e.category {
            Some(categories::OST_COLUMNS | OST_STRUCTURAL_COLUMNS) => Some("column"),
            Some(OST_STRUCTURAL_FRAMING) => Some("beam"),
            Some(OST_CURTAIN_MULLIONS) => Some("mullion"),
            Some(OST_CURTAIN_PANELS) => Some("panel"),
            _ => None,
        };
    }
    match e.category {
        Some(categories::OST_WALLS) => Some("wall"),
        Some(categories::OST_FLOORS) => Some("floor"),
        Some(categories::OST_ROOFS) => Some("roof"),
        Some(OST_CEILINGS) => Some("ceiling"),
        _ => None,
    }
}

fn mm(p: P3) -> P3 {
    p.map(|v| v * FOOT_MM)
}

/// A translation (mm) as a position.
fn at_height(z: f64) -> Matrix {
    math::from_axes(
        [0.0, 0.0, z * FOOT_MM],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    )
}

fn extrusion(position: Matrix, profile: Profile, up: bool, depth_ft: f64) -> Item {
    Item::Extrusion {
        position,
        profile,
        direction: [0.0, 0.0, if up { 1.0 } else { -1.0 }],
        depth: depth_ft * FOOT_MM,
    }
}

fn level_z(model: &Model, level: i64) -> Option<f64> {
    model.level(level).map(|l| l.elevation)
}

/// The material of a type's layers: the exterior one's, else the first
/// that has one.
fn layer_material(b: &Building, type_id: Option<i64>) -> Option<i64> {
    let layers = b.structures.get(&type_id?)?;
    layers
        .first()
        .and_then(|l| l.material)
        .or_else(|| layers.iter().find_map(|l| l.material))
}

/// The plan of an element of a kind [`kind_of`] names.
pub fn plan(model: &Model, e: &Element) -> Result<Plan, String> {
    let b = &model.building;
    let kind = kind_of(model, e).ok_or("not an element built as features")?;
    if let Some(why) = b.unread.get(&e.id) {
        return Err(format!("its parameters did not read: {why}"));
    }
    match kind {
        "wall" => {
            let w = b.walls.get(&e.id).ok_or("no wall parameters")?;
            wall(model, w)
        }
        "floor" | "roof" | "ceiling" => {
            let s = b.slabs.get(&e.id).ok_or("no slab parameters")?;
            slab(model, s, kind, true)
        }
        "column" => column(model, e, b.instances.get(&e.id).ok_or("no instance")?),
        "beam" => beam(model, e, b.instances.get(&e.id).ok_or("no instance")?),
        "stair" => stair(model, e, b.stair_runs.get(&e.id).ok_or("no stair run")?),
        "mullion" => mullion(model, e, b.instances.get(&e.id).ok_or("no instance")?),
        "landing" => landing(model, e, b.landings.get(&e.id).ok_or("no landing")?),
        "sweep" | "rail" => sweep(model, e, b.sweeps.get(&e.id).ok_or("no sweep")?),
        "panel" => panel(b, b.instances.get(&e.id).ok_or("no instance")?),
        other => Err(format!("{other}s are not built")),
    }
}

/// A wall's solid alone (also a cut tool of the elements it cuts).
fn wall_solid(model: &Model, w: &Wall) -> Result<(Item, Vec<String>), String> {
    if w.steps.iter().any(|s| s == "ExtrusionGStep") {
        return edited_wall(model, w);
    }
    let base = w.base.ok_or("no base height")?;
    let height = w.height.filter(|h| *h > 0.0).ok_or("no height")?;
    let (path, notes) = footprint::wall_footprint(&model.building, w)?;
    let profile = profile_mm(vec![Area {
        outer: path,
        holes: Vec::new(),
    }]);
    Ok((extrusion(at_height(base), profile, true, height), notes))
}

/// A wall whose profile is edited: the profile's loops (in the wall's
/// vertical plane) extruded across the wall's width about its location
/// line.
fn edited_wall(model: &Model, w: &Wall) -> Result<(Item, Vec<String>), String> {
    if w.profile.is_empty() {
        return Err("its profile is edited, and its loops did not read".into());
    }
    let Curve::Line { start, end } = &w.curve else {
        return Err("an arc wall with an edited profile".into());
    };
    let [lo, hi] =
        footprint::sides(&model.building, w).ok_or("its type has no layers with a width")?;
    let x = math::normalize([end[0] - start[0], end[1] - start[1], 0.0])
        .ok_or("its location line has no length")?;
    let y = [0.0, 0.0, 1.0];
    let z = math::cross(x, y);
    // z is the right normal: the wall spans -hi to -lo along it.
    let origin = math::sub([start[0], start[1], 0.0], math::scale(z, hi));
    let frame = math::from_axes(origin, x, y, z);
    let inverse = math::inverse(&frame).ok_or("a degenerate frame")?;
    let paths: Vec<Path> = w
        .profile
        .iter()
        .map(|l| {
            let local: Vec<Curve> = l.iter().map(|c| transformed(c, &inverse)).collect();
            loop_path(&local, 1e-4).ok_or("a profile loop that does not close")
        })
        .collect::<Result<_, _>>()?;
    let item = Item::Extrusion {
        position: math::from_axes(mm(origin), x, y, z),
        profile: profile_mm(areas(paths)),
        direction: [0.0, 0.0, 1.0],
        depth: (hi - lo) * FOOT_MM,
    };
    // Its ends where its joins (or its location line's ends) put them: the
    // profile within its footprint's prism.
    let (footprint, mut notes) = footprint::wall_footprint(&model.building, w)?;
    let zs: Vec<f64> = w
        .profile
        .iter()
        .flatten()
        .flat_map(|c| [c.start()[2], c.end()[2]])
        .collect();
    let lo = zs.iter().copied().fold(f64::INFINITY, f64::min) - 1.0;
    let hi = zs.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 1.0;
    let prism = extrusion(
        at_height(lo),
        profile_mm(vec![Area {
            outer: footprint,
            holes: Vec::new(),
        }]),
        true,
        hi - lo,
    );
    notes.insert(
        0,
        "its edited profile across its width, within its footprint".into(),
    );
    Ok((
        Item::Boolean {
            op: BoolOp::Intersection,
            first: Box::new(item),
            second: Box::new(prism),
        },
        notes,
    ))
}

fn wall(model: &Model, w: &Wall) -> Result<Plan, String> {
    let b = &model.building;
    if w.type_id.and_then(|t| b.structures.get(&t)).is_none() {
        return Err("its type has no layers (a curtain or stacked wall)".into());
    }
    let (solid, mut notes) = wall_solid(model, w)?;
    let width = w.type_id.and_then(|t| b.width(t)).unwrap_or(1.0);
    let mut cuts = Vec::new();
    // The inserts it hosts, and those whose cut it takes.
    let mut inserts: Vec<i64> = b
        .instances
        .values()
        .filter(|i| i.host == Some(w.id))
        .map(|i| i.id)
        .collect();
    for c in &w.cutouts {
        if !inserts.contains(c) {
            inserts.push(*c);
        }
    }
    inserts.sort_unstable();
    for id in inserts {
        match insert_cuts(b, id, width) {
            Ok(tools) if tools.is_empty() => {}
            Ok(tools) => cuts.extend(tools.into_iter().map(|t| (format!("insert {id}"), t))),
            Err(why) => notes.push(format!("insert {id} is not cut: {why}")),
        }
    }
    for other in &w.cut_by {
        match tool(model, *other) {
            Ok(t) => cuts.push((format!("element {other}"), t)),
            Err(why) => notes.push(format!("the cut by element {other} is not made: {why}")),
        }
    }
    Ok(Plan {
        kind: "wall",
        solid,
        cuts,
        more: Vec::new(),
        notes,
        material: layer_material(b, w.type_id),
    })
}

/// The cut tools of an insert: its type's cut loops at its transform,
/// through the host along the cut direction.
fn insert_cuts(b: &Building, id: i64, host_width: f64) -> Result<Vec<Item>, String> {
    let inst = b.instances.get(&id).ok_or("not an instance read")?;
    let t = inst.transform.ok_or("no transform")?;
    let symbol = [inst.symbol, inst.master]
        .into_iter()
        .flatten()
        .filter_map(|s| b.symbols.get(&s))
        .find(|s| !s.cut_loops.is_empty());
    let Some(symbol) = symbol else {
        return Ok(Vec::new());
    };
    let dir = symbol
        .cut_dirs
        .first()
        .copied()
        .ok_or("its type gives no cut direction")?;
    let z = math::normalize(building::turn(&t, dir)).ok_or("a cut direction of no length")?;
    let reach = host_width + 2.0;
    let mut tools = Vec::new();
    for l in &symbol.cut_loops {
        let curves: Vec<Curve> = l.iter().map(|c| c.transformed(&t)).collect();
        let origin = curves[0].start();
        let x = curves
            .iter()
            .find_map(|c| {
                let v = math::sub(c.end(), c.start());
                math::normalize(math::sub(v, math::scale(z, math::dot(v, z))))
            })
            .ok_or("a cut loop of no extent")?;
        let y = math::cross(z, x);
        // From the loop's plane, back by the reach, through twice the reach.
        let start = math::sub(origin, math::scale(z, reach));
        let frame_ft = math::from_axes(start, x, y, z);
        let inverse = math::inverse(&frame_ft).ok_or("a degenerate cut frame")?;
        let local: Vec<Curve> = curves.iter().map(|c| transformed(c, &inverse)).collect();
        let path = loop_path(&local, 1e-4).ok_or("its cut loop does not close")?;
        let profile = profile_mm(areas(vec![path]));
        tools.push(Item::Extrusion {
            position: math::from_axes(mm(start), x, y, z),
            profile,
            direction: [0.0, 0.0, 1.0],
            depth: 2.0 * reach * FOOT_MM,
        });
    }
    Ok(tools)
}

/// A curve moved by an IFC matrix.
fn transformed(c: &Curve, m: &Matrix) -> Curve {
    let t = [
        m[0][0], m[0][1], m[0][2], m[1][0], m[1][1], m[1][2], m[2][0], m[2][1], m[2][2], m[0][3],
        m[1][3], m[2][3],
    ];
    c.transformed(&t)
}

/// An element's own solid as a cut tool of another.
fn tool(model: &Model, id: i64) -> Result<Item, String> {
    let b = &model.building;
    if let Some(w) = b.walls.get(&id) {
        return wall_solid(model, w).map(|(s, _)| s);
    }
    if let Some(s) = b.slabs.get(&id) {
        let kind = match s.class.as_str() {
            "Floor" | "BuildingPad" => "floor",
            "Ceiling" => "ceiling",
            _ => "roof",
        };
        return slab(model, s, kind, false).map(|p| p.solid);
    }
    if let Some(e) = model.element(id)
        && let Some(i) = b.instances.get(&id)
    {
        return match kind_of(model, e) {
            Some("column") => column_solid(model, e, i).map(|(s, _)| s),
            Some("beam") => beam(model, e, i).map(|p| p.solid),
            _ => Err("its solid is not built".into()),
        };
    }
    Err("its solid is not built".into())
}

/// A slab's loops as profile areas (feet) and the loops' height.
fn slab_areas(s: &Slab) -> Result<(Vec<Area>, f64), String> {
    if !s.unread_curves.is_empty() {
        return Err(format!("its boundary has {}", s.unread_curves[0]));
    }
    if s.loops.is_empty() {
        return Err("no boundary loops".into());
    }
    let z = s.loops[0][0].start()[2];
    let paths: Vec<Path> = s
        .loops
        .iter()
        .map(|l| loop_path(l, 1e-4).ok_or("a boundary loop that does not close"))
        .collect::<Result<_, _>>()?;
    Ok((areas(paths), z))
}

fn slab(model: &Model, s: &Slab, kind: &'static str, with_cuts: bool) -> Result<Plan, String> {
    let b = &model.building;
    if s.shape_edited {
        return Err("its shape is edited (points moved), which is not read".into());
    }
    let open_profile = s.class == "Roof" && s.extrusion.is_some();
    let (areas_ft, loops_z) = if open_profile {
        (
            Vec::new(),
            s.loops
                .first()
                .and_then(|l| l.first())
                .map_or(0.0, |c| c.start()[2]),
        )
    } else {
        slab_areas(s)?
    };
    let width = s.type_id.and_then(|t| b.width(t));
    // The widths of its layers that have one, from the top.
    let layers: Vec<f64> = s
        .type_id
        .and_then(|t| b.structures.get(&t))
        .map(|l| l.iter().map(|l| l.width).filter(|w| *w > 1e-9).collect())
        .unwrap_or_default();
    let mut notes = Vec::new();
    let solid = if matches!(s.class.as_str(), "Floor" | "BuildingPad" | "Ceiling") {
        // The top and bottom: the floor's own, else its horizontal
        // reference faces' (a pad's).
        let level: Vec<f64> = s
            .ref_faces
            .iter()
            .filter(|p| math::cross(p.x, p.y)[2].abs() > 1.0 - 1e-9)
            .map(|p| p.origin[2])
            .collect();
        let faces_top = level.iter().copied().reduce(f64::max);
        let faces_bottom = level.iter().copied().reduce(f64::min);
        let top = s.top.or(faces_top).unwrap_or(loops_z);
        let bottom = s.bottom.or(faces_bottom.filter(|b| *b < top));
        let thickness = match bottom {
            Some(b) if top > b => top - b,
            _ => width.ok_or("no thickness")?,
        };
        extrusion(at_height(top), profile_mm(areas_ft), false, thickness)
    } else if s.class == "ProfileRoof" || s.class == "Roof" && !open_profile {
        // A footprint roof.
        roof_solid(s, areas_ft, loops_z, width, &layers, &mut notes)?
    } else if open_profile {
        // An extrusion roof: its profile's span across the extents of the
        // extrusion, under its slopes.
        let rect = extrusion_span(s)?;
        notes.push("an extrusion roof".into());
        roof_solid(s, rect, loops_z, width, &layers, &mut notes)?
    } else {
        return Err(format!("a {} is not built", s.class));
    };
    let mut cuts = Vec::new();
    // (A tool of another element's cut is the solid alone: cuts by each
    // other would not end.)
    for other in s.cut_by.iter().filter(|_| with_cuts) {
        match tool(model, *other) {
            Ok(t) => cuts.push((format!("element {other}"), t)),
            Err(why) => notes.push(format!("the cut by element {other} is not made: {why}")),
        }
    }
    // The inserts whose cut it takes (skylights, floor openings).
    for id in s.cutouts.iter().filter(|_| with_cuts) {
        match insert_cuts(b, *id, width.unwrap_or(1.0)) {
            Ok(tools) => cuts.extend(tools.into_iter().map(|t| (format!("insert {id}"), t))),
            Err(why) => notes.push(format!("insert {id} is not cut: {why}")),
        }
    }
    Ok(Plan {
        kind,
        solid,
        cuts,
        more: Vec::new(),
        notes,
        material: layer_material(b, s.type_id),
    })
}

/// A footprint roof: flat (its loops extruded up by its thickness), or the
/// prism under its slopes' top planes less the prism under their bottom
/// planes.
fn roof_solid(
    s: &Slab,
    areas_ft: Vec<Area>,
    loops_z: f64,
    width: Option<f64>,
    layers: &[f64],
    notes: &mut Vec<String>,
) -> Result<Item, String> {
    let unit = |v: P3| math::normalize(v);
    // The slopes: planes grouped by their normal (pointing up).
    let mut groups: Vec<(P3, Vec<(f64, &building::Plane)>)> = Vec::new();
    for p in &s.layer_faces {
        let Some(mut n) = unit(math::cross(p.x, p.y)) else {
            continue;
        };
        if n[2] < 0.0 {
            n = math::scale(n, -1.0);
        }
        let offset = math::dot(n, p.origin);
        match groups
            .iter_mut()
            .find(|(m, _)| math::dot(*m, n) > 1.0 - 1e-6)
        {
            Some((_, list)) => list.push((offset, p)),
            None => groups.push((n, vec![(offset, p)])),
        }
    }
    // A slope's top and bottom (offsets along its normal): its faces are
    // the boundaries of its type's layers, the outer ones among them (one
    // more than the layers) or not (one fewer: then the outer layers' widths
    // beyond them).
    let bounds = |list: &[(f64, &building::Plane)]| -> Result<(f64, f64), String> {
        let top = list
            .iter()
            .map(|(o, _)| *o)
            .fold(f64::NEG_INFINITY, f64::max);
        let bottom = list.iter().map(|(o, _)| *o).fold(f64::INFINITY, f64::min);
        let (k, l) = (list.len(), layers.len());
        if l > 0 && k + 1 == l {
            Ok((top + layers[0], bottom - layers[l - 1]))
        } else if k == l + 1 && top - bottom > 1e-6 {
            Ok((top, bottom))
        } else if let Some(w) = width.filter(|_| k >= 1) {
            Ok((top, top - w))
        } else {
            Err(format!("its {k} layer faces do not bound its {l} layers"))
        }
    };
    let flat = groups.iter().all(|(n, _)| n[2] > 1.0 - 1e-6);
    if flat && groups.len() == 1 {
        let (top, bottom) = bounds(&groups[0].1)?;
        return Ok(extrusion(
            at_height(bottom),
            profile_mm(areas_ft),
            true,
            top - bottom,
        ));
    }
    if flat {
        let thickness = width.ok_or("no thickness")?;
        return Ok(extrusion(
            at_height(loops_z),
            profile_mm(areas_ft),
            true,
            thickness,
        ));
    }
    if groups.len() > 1 {
        notes.push(format!("a roof of {} slopes", groups.len()));
    }
    // The prism, from below the lowest plane to above the highest over the
    // footprint.
    let corners: Vec<P2> = areas_ft
        .iter()
        .flat_map(|a| a.outer.segments.iter().flat_map(|s| s.sample(4)))
        .map(|p| [p[0], p[1]])
        .collect();
    let height_at = |n: P3, offset: f64, p: P2| (offset - n[0] * p[0] - n[1] * p[1]) / n[2];
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for (n, list) in &groups {
        if n[2] < 1e-3 {
            return Err("a roof slope that stands vertical".into());
        }
        for (offset, _) in list {
            for c in &corners {
                let z = height_at(*n, *offset, *c);
                lo = lo.min(z);
                hi = hi.max(z);
            }
        }
    }
    lo = lo.min(loops_z) - 1.0;
    hi += 1.0;
    let prism = || extrusion(at_height(lo), profile_mm(areas_ft.clone()), true, hi - lo);
    // Each slope's top and bottom planes.
    let mut tops = Vec::new();
    let mut bottoms = Vec::new();
    for (n, list) in &groups {
        let (top, bottom) = bounds(list)?;
        if top - bottom < 1e-6 {
            return Err("a slope of no thickness".into());
        }
        tops.push((*n, top));
        bottoms.push((*n, bottom));
    }
    let above = |n: P3, offset: f64| {
        let origin = math::scale(n, offset * FOOT_MM);
        let x = math::normalize(math::cross([0.0, 0.0, 1.0], n))
            .or_else(|| math::normalize(math::cross([1.0, 0.0, 0.0], n)))
            .unwrap_or([1.0, 0.0, 0.0]);
        Item::HalfSpace {
            plane: math::from_axes(origin, x, math::cross(n, x), n),
            below: false,
            boundary: None,
        }
    };
    let under = |planes: &[(P3, f64)]| {
        planes.iter().fold(prism(), |acc, (n, o)| Item::Boolean {
            op: BoolOp::Difference,
            first: Box::new(acc),
            second: Box::new(above(*n, *o)),
        })
    };
    Ok(Item::Boolean {
        op: BoolOp::Difference,
        first: Box::new(under(&tops)),
        second: Box::new(under(&bottoms)),
    })
}

/// The structural section parameters (the program's public built-in
/// parameter ids; verified on the corpus's steel sections: the values
/// are the sections' published dimensions).
mod section_params {
    pub const WIDTH: i64 = -1_005_502;
    pub const HEIGHT: i64 = -1_005_503;
    pub const DIAMETER: i64 = -1_005_504;
    pub const WALL_NOMINAL: i64 = -1_005_505;
    pub const WALL_DESIGN: i64 = -1_005_506;
    pub const FLANGE: i64 = -1_005_524;
    pub const WEB: i64 = -1_005_525;
    pub const INNER_RADIUS: i64 = -1_005_529;
    pub const OUTER_RADIUS: i64 = -1_005_530;
}

/// A type's section (feet, its box centred on the origin) and its kind:
/// rectangular, round, I, L and U sections, hollow rectangles and circles.
fn section(model: &Model, e: &Element, symbol: i64) -> Result<(Profile, &'static str), String> {
    use section_params::*;
    let b = &model.building;
    let p = |ids: &[i64], names: &[&str]| b.param(symbol, ids, names).filter(|v| *v > 0.0);
    let family = e
        .family_name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| family.contains(w));
    let rect = |w: f64, d: f64| {
        polygon(&[
            [-w / 2.0, -d / 2.0],
            [w / 2.0, -d / 2.0],
            [w / 2.0, d / 2.0],
            [-w / 2.0, d / 2.0],
        ])
    };
    let one = |path: Path, holes: Vec<Path>, kind: &'static str| {
        Ok((
            Profile {
                kind: String::new(),
                name: String::new(),
                areas: vec![Area { outer: path, holes }],
            },
            kind,
        ))
    };
    // The built-in section parameters, else the family's own (older
    // steel families name them bf, d, tf, tw; angles b, d, t).
    let thickness = p(&[], &["t"]);
    let (w, h) = (p(&[WIDTH], &["bf", "b"]), p(&[HEIGHT], &["d"]));
    let (tf, tw) = (
        p(&[FLANGE], &["tf"]).or(thickness),
        p(&[WEB], &["tw"]).or(thickness),
    );
    let wall = p(&[WALL_DESIGN, WALL_NOMINAL], &[]);
    // Hollow circles and round sections.
    if let Some(d) = p(&[DIAMETER], &[]) {
        return match wall.filter(|t| *t < d / 2.0) {
            Some(t) => one(
                circle(d / 2.0),
                vec![circle(d / 2.0 - t).reversed()],
                "hollow round",
            ),
            None => one(circle(d / 2.0), Vec::new(), "round"),
        };
    }
    if has(&["round", "circular", "pipe", "bar", "rund"])
        && let Some(d) = p(&[], &["Diameter", "Durchmesser", "d", "b", "D"])
            .or_else(|| p(&[], &["Radius"]).map(|r| 2.0 * r))
            .or(w)
    {
        return one(circle(d / 2.0), Vec::new(), "round");
    }
    // Hollow rectangles: the wall thickness and the corner radii.
    if has(&["hss", "rhs", "shs", "hollow"])
        && let (Some(w), Some(h), Some(t)) = (w, h, wall)
    {
        let ro = p(&[OUTER_RADIUS], &[]).unwrap_or(0.0);
        let ri = p(&[INNER_RADIUS], &[]).unwrap_or(0.0);
        let (x, y) = (w / 2.0, h / 2.0);
        let outer = mitcad_ifc::geometry::rounded(&[[-x, -y], [x, -y], [x, y], [-x, y]], &[ro; 4]);
        let (xi, yi) = (x - t, y - t);
        let inner =
            mitcad_ifc::geometry::rounded(&[[-xi, -yi], [xi, -yi], [xi, yi], [-xi, yi]], &[ri; 4]);
        return one(outer, vec![inner.reversed()], "hollow rectangular");
    }
    if let (Some(w), Some(h), Some(tf), Some(tw)) = (w, h, tf, tw) {
        let (x, y) = (w / 2.0, h / 2.0);
        if has(&["channel", "pfc", "u-", "c-", "c shape", "u shape"]) {
            let pts = [
                [-x, -y],
                [x, -y],
                [x, -y + tf],
                [-x + tw, -y + tf],
                [-x + tw, y - tf],
                [x, y - tf],
                [x, y],
                [-x, y],
            ];
            return one(polygon(&pts), Vec::new(), "U");
        }
        if has(&["angle", "l-"]) {
            let pts = [
                [-x, -y],
                [x, -y],
                [x, -y + tf],
                [-x + tw, -y + tf],
                [-x + tw, y],
                [-x, y],
            ];
            return one(polygon(&pts), Vec::new(), "L");
        }
        if has(&[
            "w shape",
            "wide flange",
            "universal",
            "ub-",
            "uc-",
            "i-",
            "ipe",
            "hea",
            "heb",
            "beam",
        ]) {
            let t = tw / 2.0;
            let pts = [
                [-x, -y],
                [x, -y],
                [x, -y + tf],
                [t, -y + tf],
                [t, y - tf],
                [x, y - tf],
                [x, y],
                [-x, y],
                [-x, y - tf],
                [-t, y - tf],
                [-t, -y + tf],
                [-x, -y + tf],
            ];
            return one(polygon(&pts), Vec::new(), "I");
        }
    }
    if has(&["hss", "rhs", "shs", "hollow", "chs"]) {
        return Err("a hollow section without its wall thickness".into());
    }
    let width = p(&[], &["Width", "b"]).or(w);
    let depth = p(&[], &["Depth", "h", "Height"])
        .or(h)
        .or_else(|| width.filter(|_| has(&["square"])));
    if let (Some(wd), Some(d)) = (width, depth)
        && has(&[
            "rect", "square", "sqaure", "timber", "concrete", "column", "precast",
        ])
    {
        return one(rect(wd, d), Vec::new(), "rectangular");
    }
    Err(format!(
        "the section of family '{}' is none of the known (rectangular, round, I, L, U, hollow)",
        e.family_name.as_deref().unwrap_or("?")
    ))
}

/// A column's solid and notes.
fn column_solid(model: &Model, e: &Element, i: &Instance) -> Result<(Item, Vec<String>), String> {
    let symbol = i.symbol.or(i.master).ok_or("no type")?;
    let (profile, kind) =
        section(model, e, i.master.unwrap_or(symbol)).or_else(|_| section(model, e, symbol))?;
    let t = i.transform.ok_or("no transform")?;
    if i.levels.len() < 2 {
        return Err("no base and top level".into());
    }
    let z0 =
        level_z(model, i.levels[0].level).ok_or("its base level is not read")? + i.levels[0].offset;
    let z1 =
        level_z(model, i.levels[1].level).ok_or("its top level is not read")? + i.levels[1].offset;
    if z1 <= z0 {
        return Err("its top is not above its base".into());
    }
    // The section's centre: the type box's, at the instance's transform.
    let outline = model
        .outlines
        .get(&symbol)
        .or_else(|| model.outlines.get(&i.master?));
    let (cx, cy) = outline.map_or((0.0, 0.0), |o| ((o[0] + o[3]) / 2.0, (o[1] + o[4]) / 2.0));
    let c = building::apply(&t, [cx, cy, 0.0]);
    let x = building::turn(&t, [1.0, 0.0, 0.0]);
    let y = building::turn(&t, [0.0, 1.0, 0.0]);
    if (x[2].abs() + y[2].abs()) > 1e-6 {
        return Err("a slanted column".into());
    }
    let position = math::from_axes(
        [c[0] * FOOT_MM, c[1] * FOOT_MM, z0 * FOOT_MM],
        x,
        y,
        [0.0, 0.0, 1.0],
    );
    let profile = Profile {
        areas: profile
            .areas
            .iter()
            .map(|a| Area {
                outer: footprint::to_mm(&a.outer),
                holes: a.holes.iter().map(footprint::to_mm).collect(),
            })
            .collect(),
        ..profile
    };
    Ok((
        extrusion(position, profile, true, z1 - z0),
        vec![format!("a {kind} section")],
    ))
}

fn column(model: &Model, e: &Element, i: &Instance) -> Result<Plan, String> {
    let (solid, mut notes) = column_solid(model, e, i)?;
    let mut cuts = Vec::new();
    for other in &i.cut_by {
        match tool(model, *other) {
            Ok(t) => cuts.push((format!("element {other}"), t)),
            Err(why) => notes.push(format!("the cut by element {other} is not made: {why}")),
        }
    }
    let material = model
        .building
        .symbols
        .get(&i.symbol.or(i.master).unwrap_or(-1))
        .and_then(|s| s.materials.first().copied());
    Ok(Plan {
        kind: "column",
        solid,
        cuts,
        more: Vec::new(),
        notes,
        material,
    })
}

fn beam(model: &Model, e: &Element, i: &Instance) -> Result<Plan, String> {
    let symbol = i.symbol.or(i.master).ok_or("no type")?;
    let (profile, kind) =
        section(model, e, i.master.unwrap_or(symbol)).or_else(|_| section(model, e, symbol))?;
    let t = i.transform.ok_or("no transform")?;
    let curve = i.curve.as_ref().ok_or("no location curve")?;
    let Curve::Line { start, end } = curve else {
        return Err("a curved beam".into());
    };
    let x = building::turn(&t, [1.0, 0.0, 0.0]);
    let y = building::turn(&t, [0.0, 1.0, 0.0]);
    let z = building::turn(&t, [0.0, 0.0, 1.0]);
    let o = building::apply(&t, [0.0; 3]);
    let along = |p: P3| math::dot(math::sub(p, o), x);
    let (mut s0, mut s1) = (along(*start), along(*end));
    let (mut b0, mut b1) = (i.setbacks[0], i.setbacks[1]);
    if s0 > s1 {
        std::mem::swap(&mut s0, &mut s1);
        std::mem::swap(&mut b0, &mut b1);
    }
    s0 += b0;
    s1 -= b1;
    if s1 - s0 <= 1e-6 {
        return Err("no length left between its setbacks".into());
    }
    // The section's box centred where its type's box is across the beam.
    let outline = model
        .outlines
        .get(&symbol)
        .or_else(|| i.master.and_then(|m| model.outlines.get(&m)));
    let (cy, cz) = outline.map_or((0.0, 0.0), |b| ((b[1] + b[4]) / 2.0, (b[2] + b[5]) / 2.0));
    let origin = math::add(
        math::add(o, math::scale(x, s0)),
        math::add(math::scale(y, cy), math::scale(z, cz)),
    );
    let position = math::from_axes(mm(origin), y, z, x);
    let profile = Profile {
        areas: profile
            .areas
            .iter()
            .map(|a| Area {
                outer: footprint::to_mm(&a.outer),
                holes: a.holes.iter().map(footprint::to_mm).collect(),
            })
            .collect(),
        ..profile
    };
    let material = model
        .building
        .symbols
        .get(&symbol)
        .and_then(|s| s.materials.first().copied());
    let mut notes = vec![format!("a {kind} section")];
    if !i.cut_by.is_empty() {
        notes.push(format!(
            "{} cuts by other elements are not made",
            i.cut_by.len()
        ));
    }
    Ok(Plan {
        kind: "beam",
        solid: extrusion(position, profile, true, s1 - s0),
        cuts: Vec::new(),
        more: Vec::new(),
        notes,
        material,
    })
}

/// An extrusion roof's span (feet): the rectangle its open profile (in a
/// vertical plane) covers across the extents of its extrusion along the
/// plane's normal.
fn extrusion_span(s: &Slab) -> Result<Vec<Area>, String> {
    let [d0, d1] = s.extrusion.ok_or("no extrusion extents")?;
    let curves: Vec<&Curve> = s.loops.iter().flatten().collect();
    let first = curves.first().ok_or("no profile")?.start();
    let last = curves.last().ok_or("no profile")?.end();
    let along = math::normalize([last[0] - first[0], last[1] - first[1], 0.0])
        .ok_or("a profile that does not run across")?;
    let n = math::cross(along, [0.0, 0.0, 1.0]);
    let (r0, r1) = (-d0.max(d1), -d0.min(d1));
    let at = |p: P3, r: f64| [p[0] + n[0] * r, p[1] + n[1] * r];
    let pts = [at(first, r0), at(last, r0), at(last, r1), at(first, r1)];
    Ok(areas(vec![polygon(&pts)]))
}

/// A monolithic stair run: its steps' profile (risers of equal height
/// from its bottom to its top, its treads' depths) down to the underside
/// of its support, across its width (its start riser line).
fn stair(model: &Model, e: &Element, r: &building::StairRun) -> Result<Plan, String> {
    let (monolithic, depth) = e
        .type_id
        .and_then(|t| model.building.run_types.get(&t).copied())
        .ok_or("its run type is not read")?;
    if monolithic && depth <= 0.0 {
        return Err("a monolithic run without its support's depth".into());
    }
    let Some(Curve::Line { start: a, end: b }) = &r.start else {
        return Err("its start riser line is not read".into());
    };
    let n = usize::try_from(r.risers).unwrap_or(0);
    if n == 0 || r.treads.len() + 1 < n || r.top <= r.bottom {
        return Err("its risers and treads do not read".into());
    }
    let w = [b[0] - a[0], b[1] - a[1], 0.0];
    let width = math::length(w);
    let w = math::normalize(w).ok_or("its start riser has no length")?;
    let up = [0.0, 0.0, 1.0];
    // The run's direction: across the risers, towards the end riser.
    let mut x = math::cross(up, w);
    let toward = r
        .end
        .as_ref()
        .map(|e| math::sub(e.start(), *a))
        .or_else(|| r.support.as_ref().map(|s| math::sub(s.end(), s.start())))
        .ok_or("no end riser or support to give its direction")?;
    if math::dot(toward, x) < 0.0 {
        x = math::scale(x, -1.0);
    }
    let h = (r.top - r.bottom) / n as f64;
    if !monolithic {
        return stair_parts(model, e, r, n, h, a, b, w, x, width);
    }
    let mut pts: Vec<P2> = vec![[0.0, 0.0]];
    let mut s = 0.0;
    for k in 0..n {
        pts.push([s, (k + 1) as f64 * h]);
        if let Some(d) = r.treads.get(k) {
            s += d;
            pts.push([s, (k + 1) as f64 * h]);
        }
    }
    let length = s;
    let total = r.top - r.bottom;
    // The underside: the support's line in the run's plane.
    let support = r.support.as_ref().ok_or("no support line")?;
    let at = |p: P3| [math::dot(math::sub(p, *a), x), p[2] - r.bottom];
    let (p, q) = (at(support.start()), at(support.end()));
    if (q[0] - p[0]).abs() < 1e-9 {
        return Err("its support's line runs across the run".into());
    }
    // (The support line runs along the nosings: the underside lies the
    // support's depth below it, square to it.)
    let slope = (q[1] - p[1]) / (q[0] - p[0]);
    let drop = depth * (1.0 + slope * slope).sqrt();
    let u = |s: f64| p[1] - drop + (s - p[0]) * slope;
    if u(length) >= total - 1e-6 {
        return Err("its support's underside reaches above its top".into());
    }
    pts.push([length, u(length).max(0.0)]);
    if u(0.0) > 0.0 {
        pts.push([0.0, u(0.0).min(h)]);
    } else {
        let s0 = p[0] + (drop - p[1]) / slope;
        pts.push([s0.clamp(0.0, length), 0.0]);
    }
    pts.dedup_by(|m, n| (m[0] - n[0]).hypot(m[1] - n[1]) < 1e-9);
    if pts.len() > 1
        && (pts[0][0] - pts[pts.len() - 1][0]).hypot(pts[0][1] - pts[pts.len() - 1][1]) < 1e-9
    {
        pts.pop();
    }
    let z = math::cross(x, up);
    let origin = if math::dot(z, w) > 0.0 { *a } else { *b };
    let origin = [origin[0], origin[1], r.bottom];
    Ok(Plan {
        kind: "stair",
        solid: Item::Extrusion {
            position: math::from_axes(mm(origin), x, up, z),
            profile: profile_mm(areas(vec![polygon(&pts)])),
            direction: [0.0, 0.0, 1.0],
            depth: width * FOOT_MM,
        },
        cuts: Vec::new(),
        more: Vec::new(),
        notes: vec![format!("a run of {n} risers")],
        material: None,
    })
}

/// A prism (mitcad#333): closed planar loops (model feet, the first the
/// outer one) moved `depth` (feet) along the unit vector `z`, square to
/// their plane.
fn prism(loops: &[Vec<Curve>], z: P3, depth: f64) -> Result<Item, String> {
    let origin = loops
        .first()
        .and_then(|l| l.first())
        .ok_or("no loop")?
        .start();
    let z = math::normalize(z).ok_or("a direction of no length")?;
    let x = loops
        .iter()
        .flatten()
        .find_map(|c| {
            let v = math::sub(c.end(), c.start());
            math::normalize(math::sub(v, math::scale(z, math::dot(v, z))))
        })
        .ok_or("a loop of no extent")?;
    let y = math::cross(z, x);
    let frame = math::from_axes(origin, x, y, z);
    let inverse = math::inverse(&frame).ok_or("a degenerate frame")?;
    let paths: Vec<Path> = loops
        .iter()
        .map(|l| {
            let local: Vec<Curve> = l.iter().map(|c| transformed(c, &inverse)).collect();
            loop_path(&local, 1e-4).ok_or("a loop that does not close")
        })
        .collect::<Result<_, _>>()?;
    if depth <= 1e-9 {
        return Err(format!("no thickness ({depth} ft)"));
    }
    Ok(Item::Extrusion {
        position: math::from_axes(mm(origin), x, y, z),
        profile: profile_mm(areas(paths)),
        direction: [0.0, 0.0, 1.0],
        depth: depth * FOOT_MM,
    })
}

/// A closed polygon of lines through points.
fn polyline(points: &[P3]) -> Vec<Curve> {
    (0..points.len())
        .map(|k| Curve::Line {
            start: points[k],
            end: points[(k + 1) % points.len()],
        })
        .collect()
}

/// A curtain mullion (mitcad#333): its rectangular profile (its widths
/// across its path, at a border both on one side; its thickness along its
/// normal about its offset) along its straight path, from its own symbol
/// at the instance's transform.
fn mullion(model: &Model, e: &Element, i: &Instance) -> Result<Plan, String> {
    let b = &model.building;
    let m = i
        .symbol
        .and_then(|s| b.symbols.get(&s))
        .and_then(|s| s.mullion.as_ref())
        .ok_or("its symbol gives no mullion profile")?;
    if m.profile == "SysCustMullionProfile" {
        return custom_mullion(model, e, i, m);
    }
    let quad = m.profile == "SysQuadMullionProfile";
    if m.profile != "SysRectMullionProfile" && !quad {
        return Err(format!(
            "a mullion profile of class {} (rectangular, corner and custom ones are built)",
            m.profile
        ));
    }
    let t = i.transform.ok_or("no transform")?;
    let Some(Curve::Line { start, end }) = &m.path else {
        return Err("a mullion along a curve".into());
    };
    let length = math::length(math::sub(*end, *start));
    let d = math::normalize(math::sub(*end, *start)).ok_or("a path of no length")?;
    let n = math::normalize(m.normal).ok_or("no normal")?;
    let a = math::normalize(math::cross(n, d)).ok_or("a normal along the path")?;
    let [w0, w1] = m.widths;
    let (a0, a1, n0, n1) = if quad {
        // A corner mullion: a rectangle of its two depths about its path.
        let [d0, d1] = m.depths;
        (
            -d0 / 2.0,
            d0 / 2.0,
            m.offset - d1 / 2.0,
            m.offset + d1 / 2.0,
        )
    } else {
        let (a0, a1) = if m.flags & 4 != 0 {
            (0.0, w0 + w1)
        } else {
            (-w0, w1)
        };
        (
            a0,
            a1,
            m.offset - m.thickness / 2.0,
            m.offset + m.thickness / 2.0,
        )
    };
    if a1 - a0 <= 1e-9 || n1 - n0 <= 1e-9 {
        return Err("a profile of no size".into());
    }
    let at = |u: f64, v: f64| {
        building::apply(
            &t,
            math::add(*start, math::add(math::scale(a, u), math::scale(n, v))),
        )
    };
    let outline = polyline(&[at(a0, n0), at(a1, n0), at(a1, n1), at(a0, n1)]);
    let solid = prism(&[outline], building::turn(&t, d), length)?;
    let material = i
        .master
        .and_then(|s| b.symbols.get(&s))
        .and_then(|s| s.materials.first().copied());
    Ok(Plan {
        kind: "mullion",
        solid,
        cuts: Vec::new(),
        more: Vec::new(),
        notes: vec![format!(
            "a {} mullion {:.0} × {:.0} mm",
            if quad { "corner" } else { "rectangular" },
            (a1 - a0) * FOOT_MM,
            (n1 - n0) * FOOT_MM
        )],
        material,
    })
}

/// A curtain panel (mitcad#333): its cell's loops moved to its offset and
/// extruded by its thickness, from its own symbol at the instance's
/// transform.
fn panel(b: &Building, i: &Instance) -> Result<Plan, String> {
    let p = i
        .symbol
        .and_then(|s| b.symbols.get(&s))
        .and_then(|s| s.panel.as_ref())
        .ok_or("its symbol gives no panel cell")?;
    if p.empty {
        return Err("an empty panel".into());
    }
    if p.thickness <= 1e-9 {
        return Err(
            "a panel family's panel (its shape is its family's, not a system panel's)".into(),
        );
    }
    let t = i.transform.ok_or("no transform")?;
    let plane = p.plane.as_ref().ok_or("its cell's plane did not read")?;
    let mut nn = math::normalize(math::cross(plane.y, plane.x)).ok_or("a plane of no normal")?;
    if p.flipped {
        nn = math::scale(nn, -1.0);
    }
    let back = math::scale(nn, p.offset - p.thickness / 2.0);
    let shift = [
        1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, back[0], back[1], back[2],
    ];
    let loops: Vec<Vec<Curve>> = p
        .loops
        .iter()
        .map(|l| {
            l.iter()
                .map(|c| c.transformed(&shift).transformed(&t))
                .collect()
        })
        .collect();
    if loops.is_empty() {
        return Err("its cell has no loops".into());
    }
    let solid = prism(&loops, building::turn(&t, nn), p.thickness)?;
    let material = i
        .master
        .and_then(|s| b.symbols.get(&s))
        .and_then(|s| s.materials.first().copied());
    Ok(Plan {
        kind: "panel",
        solid,
        cuts: Vec::new(),
        more: Vec::new(),
        notes: vec![format!("a panel {:.0} mm thick", p.thickness * FOOT_MM)],
        material,
    })
}

/// A run of treads and risers (mitcad#333): per step its riser (its front
/// on the riser line, its thickness behind it, up to the tread) and its
/// tread (its top at the step's height, its thickness below, reaching the
/// nosing length over the riser), in the run's vertical plane across its
/// width.
#[allow(clippy::too_many_arguments)]
fn stair_parts(
    model: &Model,
    e: &Element,
    r: &building::StairRun,
    n: usize,
    h: f64,
    a: &P3,
    b: &P3,
    w: P3,
    x: P3,
    width: f64,
) -> Result<Plan, String> {
    let parts = e
        .type_id
        .and_then(|t| model.building.stair_parts.get(&t))
        .ok_or("its run type's treads and risers are not read")?;
    if parts.tread.is_none() && parts.riser.is_none() {
        return Err("a run of neither treads nor risers".into());
    }
    let tt = parts.tread.unwrap_or(0.0);
    let mut polys: Vec<Path> = Vec::new();
    let mut s = 0.0;
    for k in 0..n {
        let top = (k + 1) as f64 * h;
        let base = k as f64 * h;
        let d = r.treads.get(k).copied();
        // The riser: its front on the riser line, up to the tread above.
        // (The treads' depths run from nosing to nosing: a riser stands the
        // nosing length behind its tread's front edge.)
        let r0 = s + parts.nosing;
        if let Some(rt) = parts.riser {
            let up_to = if d.is_some() { top - tt } else { top };
            if up_to - base > 1e-9 {
                polys.push(polygon(&[
                    [r0, base],
                    [r0 + rt, base],
                    [r0 + rt, up_to],
                    [r0, up_to],
                ]));
            }
        }
        if let (Some(d), Some(tt)) = (d, parts.tread) {
            polys.push(polygon(&[
                [s, top - tt],
                [s + d, top - tt],
                [s + d, top],
                [s, top],
            ]));
        }
        s += d.unwrap_or(0.0);
    }
    if polys.is_empty() {
        return Err("no tread or riser".into());
    }
    let up = [0.0, 0.0, 1.0];
    let z = math::cross(x, up);
    let origin = if math::dot(z, w) > 0.0 { *a } else { *b };
    let origin = [origin[0], origin[1], r.bottom];
    let steps = polys.len();
    Ok(Plan {
        kind: "stair",
        solid: Item::Extrusion {
            position: math::from_axes(mm(origin), x, up, z),
            profile: profile_mm(areas(polys)),
            direction: [0.0, 0.0, 1.0],
            depth: width * FOOT_MM,
        },
        cuts: Vec::new(),
        more: Vec::new(),
        notes: vec![format!("a run of {n} risers: {steps} treads and risers")],
        material: None,
    })
}

/// A stair landing (mitcad#333): its boundary down from its top by its
/// type's thickness (monolithic) or tread thickness.
fn landing(model: &Model, e: &Element, l: &building::Landing) -> Result<Plan, String> {
    let parts = e
        .type_id
        .and_then(|t| model.building.stair_parts.get(&t))
        .ok_or("its landing type is not read")?;
    let thickness = if parts.monolithic {
        parts.thickness
    } else {
        parts.tread.ok_or("a landing without a tread")?
    };
    if thickness <= 1e-9 {
        return Err("a landing of no thickness".into());
    }
    if l.boundary.is_empty() {
        return Err("its boundary did not read".into());
    }
    // The boundary at its top.
    let lifted: Vec<Vec<Curve>> = l
        .boundary
        .iter()
        .map(|lp| {
            lp.iter()
                .map(|c| {
                    let dz = l.top - c.start()[2];
                    c.transformed(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, dz])
                })
                .collect()
        })
        .collect();
    let paths: Vec<Path> = lifted
        .iter()
        .map(|lp| loop_path(lp, 1e-4).ok_or("a boundary that does not close"))
        .collect::<Result<_, _>>()?;
    Ok(Plan {
        kind: "landing",
        solid: extrusion(at_height(l.top), profile_mm(areas(paths)), false, thickness),
        cuts: Vec::new(),
        more: Vec::new(),
        notes: vec![format!(
            "a {} landing {:.0} mm thick",
            if parts.monolithic {
                "monolithic"
            } else {
                "tread"
            },
            thickness * FOOT_MM
        )],
        material: None,
    })
}

/// Curves grouped into the loops their ends join them in.
fn split_loops(curves: &[Curve], tolerance: f64) -> Vec<Vec<Curve>> {
    let close = |a: P3, b: P3| math::length(math::sub(a, b)) <= tolerance;
    let mut left: Vec<Curve> = curves.to_vec();
    let mut out = Vec::new();
    while let Some(first) = left.pop() {
        let mut group = vec![first];
        let mut grown = true;
        while grown {
            grown = false;
            let ends: Vec<P3> = group.iter().flat_map(|c| [c.start(), c.end()]).collect();
            if let Some(i) = left.iter().position(|c| {
                ends.iter()
                    .any(|p| close(*p, c.start()) || close(*p, c.end()))
            }) {
                group.push(left.remove(i));
                grown = true;
            }
        }
        out.push(group);
    }
    out
}

/// The kinds of sweeps: a wall sweep or slab edge (`sweep`), a rail.
fn sweep_kind(s: &building::Sweep) -> &'static str {
    if s.path3d.is_some() || s.class.contains("Rail") {
        "rail"
    } else {
        "sweep"
    }
}

/// Curves joined end to start into open or closed chains (each turned as
/// needed), in order.
fn chains(curves: &[Curve], tolerance: f64) -> Vec<Vec<Curve>> {
    let close = |a: P3, b: P3| math::length(math::sub(a, b)) <= tolerance;
    let reversed = |c: &Curve| match c {
        Curve::Line { start, end } => Curve::Line {
            start: *end,
            end: *start,
        },
        Curve::Arc {
            center,
            x_axis,
            y_axis,
            radius,
            start,
            end,
        } => Curve::Arc {
            center: *center,
            x_axis: *x_axis,
            y_axis: math::scale(*y_axis, -1.0),
            radius: *radius,
            start: -end,
            end: -start,
        },
    };
    let mut left: Vec<Curve> = curves.to_vec();
    let mut out = Vec::new();
    while !left.is_empty() {
        let mut chain = vec![left.remove(0)];
        loop {
            let tail = chain.last().expect("one").end();
            let head = chain[0].start();
            if let Some(i) = left
                .iter()
                .position(|c| close(c.start(), tail) || close(c.end(), tail))
            {
                let c = left.remove(i);
                chain.push(if close(c.start(), tail) {
                    c
                } else {
                    reversed(&c)
                });
            } else if let Some(i) = left
                .iter()
                .position(|c| close(c.end(), head) || close(c.start(), head))
            {
                let c = left.remove(i);
                chain.insert(
                    0,
                    if close(c.end(), head) {
                        c
                    } else {
                        reversed(&c)
                    },
                );
            } else {
                break;
            }
        }
        out.push(chain);
    }
    out
}

/// A chain's lines: its lines, and its arcs as chords within a millimetre.
fn chain_lines(chain: &[Curve]) -> Vec<(P3, P3)> {
    let mut out = Vec::new();
    for c in chain {
        let n = match c {
            Curve::Line { .. } => 1,
            Curve::Arc {
                radius, start, end, ..
            } => {
                let tol = 1.0 / FOOT_MM;
                let step = 2.0 * (1.0 - tol / radius.max(tol)).clamp(-1.0, 1.0).acos();
                (((end - start) / step.max(1e-3)).ceil() as usize).clamp(1, 256)
            }
        };
        for k in 0..n {
            let (a, b) = (c.at(k as f64 / n as f64), c.at((k + 1) as f64 / n as f64));
            if math::length(math::sub(b, a)) > 1e-9 {
                out.push((a, b));
            }
        }
    }
    out
}

/// A profile swept along paths (mitcad#333): the profile family's loops
/// (its xy plane: x across the path, y along `up`, moved by the offsets)
/// at right angles to the path, extruded along each of its lines (an arc's
/// chords within a millimetre), each a body. Of the profile's four mirror
/// images (and for a wall sweep, of its wall's faces) the one whose solid
/// fits the element's stored box best.
fn sweep(model: &Model, e: &Element, s: &building::Sweep) -> Result<Plan, String> {
    let b = &model.building;
    let symbol = s
        .profile
        .filter(|p| *p > 0)
        .or_else(|| s.type_id.and_then(|t| b.profile_types.get(&t).copied()))
        .ok_or("no profile")?;
    let profile = profile_loops(model, symbol)?;
    let paths: Vec<Vec<Curve>> = match s.path3d {
        Some(p) => b.paths3d.get(&p).cloned().ok_or("its path is not read")?,
        None => s.paths.clone(),
    };
    let all: Vec<Curve> = paths
        .into_iter()
        .flatten()
        .filter(|c| c.length() > 1e-6)
        .collect();
    let chains = chains(&all, 1e-4);
    if chains.is_empty() {
        return Err("no path".into());
    }
    let up = s.up.and_then(math::normalize).unwrap_or([0.0, 0.0, 1.0]);
    let (h, v) = (s.offsets[0], s.offsets[1]);
    // A wall sweep's path is its wall's location line: its profile at one
    // of the wall's faces (their offsets from the line, either way).
    let mut shifts = vec![0.0];
    if let Some(w) = s.host.and_then(|h| b.walls.get(&h)) {
        let faces = w.faces.or_else(|| {
            let width = w.type_id.and_then(|t| b.width(t))?;
            Some([-width / 2.0, width / 2.0])
        });
        if let Some([lo, hi]) = faces {
            shifts.extend([lo, hi, -lo, -hi]);
        }
    }
    let mut best: Option<(f64, Vec<Item>)> = None;
    let mirrors = [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
    for ((mx, my), shift) in mirrors
        .iter()
        .flat_map(|m| shifts.iter().map(move |s| (*m, *s)))
    {
        // The profile in its own plane, mirrored and moved (feet).
        let flat = [mx, 0.0, 0.0, 0.0, my, 0.0, 0.0, 0.0, 1.0, h + shift, v, 0.0];
        let profile2: Vec<Vec<Curve>> = profile
            .iter()
            .map(|l| l.iter().map(|c| c.transformed(&flat)).collect())
            .collect();
        let mut items = Vec::new();
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for chain in &chains {
            let lines = chain_lines(chain);
            // Across: square to the first line that is not along `up`.
            let x0 = lines
                .iter()
                .find_map(|(a, b)| {
                    let t = math::normalize(math::sub(*b, *a))?;
                    math::normalize(math::cross(up, t))
                })
                .ok_or("a path along its up direction")?;
            let mut last_x = x0;
            let mut prisms = Vec::new();
            for (a, z) in &lines {
                let d = math::sub(*z, *a);
                let length = math::length(d);
                let t = math::scale(d, 1.0 / length);
                let x = math::normalize(math::cross(up, t)).unwrap_or(last_x);
                last_x = x;
                let y = math::cross(t, x);
                let m = [
                    x[0], y[0], t[0], x[1], y[1], t[1], x[2], y[2], t[2], a[0], a[1], a[2],
                ];
                let placed: Vec<Vec<Curve>> = profile2
                    .iter()
                    .map(|l| l.iter().map(|c| c.transformed(&m)).collect())
                    .collect();
                for c in placed.iter().flatten() {
                    for f in [0.0, 0.25, 0.5, 0.75, 1.0] {
                        let p = c.at(f);
                        for q in [p, math::add(p, d)] {
                            for k in 0..3 {
                                lo[k] = lo[k].min(q[k]);
                                hi[k] = hi[k].max(q[k]);
                            }
                        }
                    }
                }
                prisms.push((placed, t, length));
            }
            for (placed, t, length) in prisms {
                items.push(prism(&placed, t, length)?);
            }
        }
        let fit = e.bbox.map_or(0.0, |s| {
            (0..3)
                .map(|k| (s[k] - lo[k]).max(hi[k] - s[k + 3]).max(0.0))
                .fold(0.0, f64::max)
        });
        if best.as_ref().is_none_or(|(f, _)| fit < *f - 1e-9) {
            best = Some((fit, items));
        }
    }
    let (_, mut items) = best.expect("four candidates");
    let solid = items.remove(0);
    Ok(Plan {
        kind: sweep_kind(s),
        solid,
        cuts: Vec::new(),
        more: items,
        notes: vec![format!(
            "its profile ({} loops) along {} paths",
            profile.len(),
            chains.len()
        )],
        material: None,
    })
}

/// A profile family type's closed loops (feet, the family's xy plane), the
/// largest first (mitcad#333).
fn profile_loops(model: &Model, symbol: i64) -> Result<Vec<Vec<Curve>>, String> {
    let family = model
        .element(symbol)
        .and_then(|p| p.family_id)
        .ok_or("its profile's family is not read")?;
    let curves = model
        .profiles
        .get(&family)
        .ok_or("its profile family's document gives no profile")?;
    let mut loops: Vec<(f64, Vec<Curve>)> = split_loops(curves, 1e-6)
        .into_iter()
        .filter_map(|g| {
            let p = loop_path(&g, 1e-6)?;
            Some((footprint::signed_area(&p).abs(), g))
        })
        .collect();
    if loops.is_empty() {
        return Err("its profile has no closed loop".into());
    }
    loops.sort_by(|a, b| b.0.total_cmp(&a.0));
    Ok(loops.into_iter().map(|(_, g)| g).collect())
}

/// How far (feet) points reach out of a box (min and max corner).
fn outside(points: impl Iterator<Item = P3>, bbox: Option<[f64; 6]>) -> f64 {
    let Some(s) = bbox else { return 0.0 };
    points
        .map(|q| {
            (0..3)
                .map(|k| (s[k] - q[k]).max(q[k] - s[k + 3]).max(0.0))
                .fold(0.0, f64::max)
        })
        .fold(0.0, f64::max)
}

/// A curtain mullion of a custom profile (mitcad#333): its profile
/// family's loops square to its straight path, the profile's x across the
/// path or along its normal, mirrored or not, at a border on one side of
/// its path: the one that
/// fits the element's stored box best.
fn custom_mullion(
    model: &Model,
    e: &Element,
    i: &Instance,
    m: &building::MullionSym,
) -> Result<Plan, String> {
    let symbol = m.profile_id.ok_or("a custom mullion without its profile")?;
    let profile = profile_loops(model, symbol)?;
    let t = i.transform.ok_or("no transform")?;
    let Some(Curve::Line { start, end }) = &m.path else {
        return Err("a mullion along a curve".into());
    };
    let length = math::length(math::sub(*end, *start));
    let d = math::normalize(math::sub(*end, *start)).ok_or("a path of no length")?;
    let n = math::normalize(m.normal).ok_or("no normal")?;
    let a = math::normalize(math::cross(n, d)).ok_or("a normal along the path")?;
    let mut best: Option<(f64, Vec<Vec<Curve>>)> = None;
    // The profile's points, for its extent across.
    let samples: Vec<P3> = profile
        .iter()
        .flatten()
        .flat_map(|c| [0.0, 0.25, 0.5, 0.75, 1.0].map(|f| c.at(f)))
        .collect();
    let borders: &[bool] = if m.flags & 4 != 0 {
        &[false, true]
    } else {
        &[false]
    };
    for (swap, border) in [false, true]
        .into_iter()
        .flat_map(|s| borders.iter().map(move |b| (s, *b)))
    {
        for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let (u, v) = if swap { (n, a) } else { (a, n) };
            let (u, v) = (math::scale(u, sx), math::scale(v, sy));
            // At a border its width lies on one side of its path.
            let shift = if border {
                -samples
                    .iter()
                    .map(|p| math::dot(u, a) * p[0] + math::dot(v, a) * p[1])
                    .fold(f64::INFINITY, f64::min)
            } else {
                0.0
            };
            let o = math::add(
                *start,
                math::add(math::scale(n, m.offset), math::scale(a, shift)),
            );
            let local = [
                u[0], v[0], d[0], u[1], v[1], d[1], u[2], v[2], d[2], o[0], o[1], o[2],
            ];
            let placed: Vec<Vec<Curve>> = profile
                .iter()
                .map(|l| {
                    l.iter()
                        .map(|c| c.transformed(&local).transformed(&t))
                        .collect()
                })
                .collect();
            let dm = building::turn(&t, math::scale(d, length));
            let fit = outside(
                placed.iter().flatten().flat_map(|c| {
                    [0.0, 0.25, 0.5, 0.75, 1.0]
                        .into_iter()
                        .flat_map(move |f| [c.at(f), math::add(c.at(f), dm)])
                }),
                e.bbox,
            );
            if best.as_ref().is_none_or(|(f, _)| fit < *f - 1e-9) {
                best = Some((fit, placed));
            }
        }
    }
    let (_, placed) = best.expect("eight candidates");
    let solid = prism(&placed, building::turn(&t, d), length)?;
    Ok(Plan {
        kind: "mullion",
        solid,
        cuts: Vec::new(),
        more: Vec::new(),
        notes: vec![format!(
            "a mullion of a custom profile ({} loops)",
            profile.len()
        )],
        material: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mitcad_rvt::building::{MullionSym, PanelSym, Plane, StairParts, Symbol};

    fn extrusion_of(item: &Item) -> (Matrix, f64, usize) {
        match item {
            Item::Extrusion {
                position,
                depth,
                profile,
                ..
            } => (*position, *depth, profile.areas.len()),
            other => panic!("{other:?}"),
        }
    }

    fn with_symbol(symbol: Symbol) -> (Model, Element) {
        let mut model = Model::default();
        let inst = Instance {
            id: 2,
            symbol: Some(1),
            transform: Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0]),
            ..Instance::default()
        };
        model.building.symbols.insert(1, symbol);
        model.building.instances.insert(2, inst);
        let e = Element {
            id: 2,
            category: Some(OST_CURTAIN_MULLIONS),
            ..Element::default()
        };
        (model, e)
    }

    #[test]
    fn a_rectangular_mullion_along_its_path() {
        let m = MullionSym {
            profile: "SysRectMullionProfile".into(),
            depths: [0.0; 2],
            profile_id: None,
            widths: [0.1, 0.1],
            thickness: 0.4,
            offset: 0.0,
            path: Some(Curve::Line {
                start: [0.0, 0.0, 0.0],
                end: [0.0, 0.0, -8.0],
            }),
            normal: [0.0, 1.0, 0.0],
            flags: 4,
        };
        let (model, e) = with_symbol(Symbol {
            id: 1,
            mullion: Some(m),
            ..Symbol::default()
        });
        assert_eq!(kind_of(&model, &e), Some("mullion"));
        assert!(!structural("mullion"));
        let p = plan(&model, &e).unwrap();
        let (position, depth, areas) = extrusion_of(&p.solid);
        assert!((depth - 8.0 * FOOT_MM).abs() < 1e-9);
        assert_eq!(areas, 1);
        // Along -z from the instance's origin.
        assert!((math::column(&position, 2)[2] + 1.0).abs() < 1e-9);
        assert!((math::origin(&position)[0] - 10.0 * FOOT_MM).abs() < 0.5 * FOOT_MM);
    }

    #[test]
    fn a_system_panel_from_its_cell() {
        let square = vec![polyline(&[
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 0.0, 3.0],
            [0.0, 0.0, 3.0],
        ])];
        let p = PanelSym {
            loops: square,
            plane: Some(Plane {
                origin: [0.0; 3],
                x: [1.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0],
                u: [0.0, 2.0],
                v: [0.0, 3.0],
            }),
            thickness: 0.1,
            offset: 0.2,
            empty: false,
            flipped: false,
        };
        let (model, mut e) = with_symbol(Symbol {
            id: 1,
            panel: Some(p),
            ..Symbol::default()
        });
        e.category = Some(OST_CURTAIN_PANELS);
        let plan = plan(&model, &e).unwrap();
        let (position, depth, _) = extrusion_of(&plan.solid);
        assert!((depth - 0.1 * FOOT_MM).abs() < 1e-9);
        // From 0.15 ft along y × x (+y), through the thickness.
        let o = math::origin(&position);
        assert!((o[1] - 0.15 * FOOT_MM).abs() < 1e-6, "{o:?}");
        assert!((math::column(&position, 2)[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn treads_and_risers_and_a_landing() {
        let mut model = Model::default();
        let run = building::StairRun {
            id: 5,
            bottom: 0.0,
            top: 3.0,
            risers: 3,
            treads: vec![1.0, 1.0],
            start: Some(Curve::Line {
                start: [0.0, 0.0, 0.0],
                end: [0.0, 4.0, 0.0],
            }),
            end: Some(Curve::Line {
                start: [2.0, 0.0, 0.0],
                end: [2.0, 4.0, 0.0],
            }),
            support: None,
        };
        model.building.stair_runs.insert(5, run);
        model.building.run_types.insert(9, (false, 0.0));
        model.building.stair_parts.insert(
            9,
            StairParts {
                monolithic: false,
                tread: Some(0.1),
                riser: Some(0.05),
                nosing: 0.1,
                thickness: 0.0,
            },
        );
        let e = Element {
            id: 5,
            type_id: Some(9),
            ..Element::default()
        };
        let p = plan(&model, &e).unwrap();
        // Three risers and two treads.
        let (_, depth, areas) = extrusion_of(&p.solid);
        assert_eq!(areas, 5);
        assert!((depth - 4.0 * FOOT_MM).abs() < 1e-9);
        // A landing down from its top by its tread.
        let landing = building::Landing {
            id: 6,
            boundary: vec![polyline(&[
                [0.0, 0.0, 3.0],
                [4.0, 0.0, 3.0],
                [4.0, 4.0, 3.0],
                [0.0, 4.0, 3.0],
            ])],
            top: 3.0,
        };
        model.building.landings.insert(6, landing);
        let e = Element {
            id: 6,
            type_id: Some(9),
            ..Element::default()
        };
        assert_eq!(kind_of(&model, &e), Some("landing"));
        let p = plan(&model, &e).unwrap();
        let (position, depth, _) = extrusion_of(&p.solid);
        assert!((depth - 0.1 * FOOT_MM).abs() < 1e-9);
        assert!((math::origin(&position)[2] - 3.0 * FOOT_MM).abs() < 1e-9);
    }

    #[test]
    fn a_profile_along_a_bent_path() {
        // A 0.2 × 0.1 profile along an L of two lines: a body per line,
        // the profile's mirror image the one that fits the stored box.
        let mut model = Model::default();
        let profile = polyline(&[
            [0.0, 0.0, 0.0],
            [0.2, 0.0, 0.0],
            [0.2, 0.1, 0.0],
            [0.0, 0.1, 0.0],
        ]);
        model.profiles.insert(70, profile);
        model.elements.push(Element {
            id: 71,
            family_id: Some(70),
            is_type: true,
            ..Element::default()
        });
        let s = building::Sweep {
            id: 72,
            class: "SweepHost".into(),
            paths: vec![vec![
                Curve::Line {
                    start: [0.0, 0.0, 0.0],
                    end: [5.0, 0.0, 0.0],
                },
                Curve::Line {
                    start: [5.0, 0.0, 0.0],
                    end: [5.0, 5.0, 0.0],
                },
            ]],
            path3d: None,
            profile: Some(71),
            type_id: None,
            up: Some([0.0, 0.0, 1.0]),
            offsets: [0.0, 0.0],
            flipped: false,
            host: None,
        };
        model.building.sweeps.insert(72, s);
        let e = Element {
            id: 72,
            bbox: Some([-0.2, -0.2, 0.0, 5.2, 5.0, 0.1]),
            ..Element::default()
        };
        assert_eq!(kind_of(&model, &e), Some("sweep"));
        let p = plan(&model, &e).unwrap();
        assert_eq!(p.more.len(), 1);
    }
}
