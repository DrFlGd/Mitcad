// SPDX-License-Identifier: MIT
//! What each building element is built of (mitcad#275): its solids and the
//! cuts made in them, as the IFC import's geometry items
//! ([`mitcad_ifc::geometry::Item`], millimetres, model coordinates), which
//! [`crate::ifc::build::Builder`] turns into sketches, extrusions and cuts.
//!
//! - A **wall**: its footprint ([`super::footprint::wall_footprint`])
//!   extruded from its base by its height, its top and base attached to a
//!   roof, floor, ceiling or reference plane following that element's
//!   surface (mitcad#348: the prism runs on to the surface, the half spaces
//!   beyond its planes cut away); the cut loops of the inserts it
//!   hosts (doors, windows: their type's `m_pCutLoops` at the instance's
//!   transform) cut through it along their cut direction, else their
//!   type's void geometry (`m_complexCutGeoms`, mitcad#345); its opening
//!   elements (a rectangular opening's rectangle, an opening's profile) cut
//!   through it square to it; the elements whose geometry cuts it
//!   (`CutPairGStep`) cut it with their own solids. An insert or opening
//!   without a tool is listed with the reason ([`Plan::openings`]).
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

use mitcad_ifc::geometry::{Area, BoolOp, Item, Path, Profile, Seg};
use mitcad_ifc::math::{self, Matrix};
use mitcad_rvt::building::{self, Building, Curve, Instance, P3, Slab, Symbol, Wall};
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
    /// A wall's top and base attachments (mitcad#348), applied or not:
    /// its body's heights are then checked both ways against its stored
    /// box.
    pub attached: Vec<String>,
    /// A wall's openings (mitcad#345): every insert and opening element
    /// that cuts it, its cuts among [`Plan::cuts`] or why it has none.
    pub openings: Vec<WallOpening>,
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
        "sweep" | "rail" => sweep(model, e, b.sweeps.get(&e.id).ok_or("no sweep")?, true),
        "panel" => panel(b, b.instances.get(&e.id).ok_or("no instance")?),
        other => Err(format!("{other}s are not built")),
    }
}

/// A wall's solid alone (also a cut tool of the elements it cuts), its
/// notes and its attachments (mitcad#348: what its top and base are
/// attached to, applied or not).
fn wall_solid(model: &Model, w: &Wall) -> Result<(Item, Vec<String>, Vec<String>), String> {
    if w.steps.iter().any(|s| s == "ExtrusionGStep") {
        let (item, mut notes) = edited_wall(model, w)?;
        let attached = attachment_names(w);
        if !attached.is_empty() {
            notes.push(format!(
                "{} not applied: its edited profile gives its shape",
                attached.join(", ")
            ));
        }
        return Ok((item, notes, attached));
    }
    let base = w.base.ok_or("no base height")?;
    let height = w.height.filter(|h| *h > 0.0).ok_or("no height")?;
    let (path, mut notes) = footprint::wall_footprint(&model.building, w)?;
    let a = attach(model, w, &path, base, base + height);
    notes.extend(a.notes);
    let profile = profile_mm(vec![Area {
        outer: path,
        holes: Vec::new(),
    }]);
    let mut solid = extrusion(at_height(a.lo), profile.clone(), true, a.hi - a.lo);
    // Where a roof rises over the wall's own top: the wall on up within
    // the roof's outline, to under its slopes.
    for e in a.extensions {
        let mut ext = Item::Boolean {
            op: BoolOp::Intersection,
            first: Box::new(extrusion(
                at_height(e.from),
                profile.clone(),
                true,
                e.to - e.from,
            )),
            second: Box::new(extrusion(
                at_height(e.from - 1.0),
                profile_mm(areas(e.outline)),
                true,
                e.to - e.from + 2.0,
            )),
        };
        for (n, o) in e.planes {
            ext = Item::Boolean {
                op: BoolOp::Difference,
                first: Box::new(ext),
                second: Box::new(half_space(n, o, false, None)),
            };
        }
        solid = Item::Boolean {
            op: BoolOp::Union,
            first: Box::new(solid),
            second: Box::new(ext),
        };
    }
    for (n, offset, below, band) in a.cuts {
        solid = Item::Boolean {
            op: BoolOp::Difference,
            first: Box::new(solid),
            second: Box::new(half_space(n, offset, below, band.as_ref())),
        };
    }
    Ok((solid, notes, a.attached))
}

/// The half of space above a plane (feet: its unit normal and its offset
/// along it), or below it; within the vertical prism of a band in plan
/// (feet) when one is given.
fn half_space(n: P3, offset: f64, below: bool, band: Option<&Path>) -> Item {
    let origin = math::scale(n, offset * FOOT_MM);
    let x = math::normalize(math::cross([0.0, 0.0, 1.0], n))
        .or_else(|| math::normalize(math::cross([1.0, 0.0, 0.0], n)))
        .unwrap_or([1.0, 0.0, 0.0]);
    Item::HalfSpace {
        plane: math::from_axes(origin, x, math::cross(n, x), n),
        below,
        boundary: band.map(|b| {
            let plan = math::from_axes([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
            (plan, footprint::to_mm(b))
        }),
    }
}

/// A wall's attachments by name: `its top attached to 243274`.
fn attachment_names(w: &Wall) -> Vec<String> {
    w.attachments
        .iter()
        .map(|a| {
            format!(
                "its {} attached to {}",
                if a.top { "top" } else { "base" },
                a.elements
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect()
}

/// What an attached end follows (mitcad#348), feet: the planes of the
/// element's surface (each a unit normal pointing up and the offset along
/// it; a roof's slopes) and the footprint they cover (None: a level or
/// unbounded surface, which covers the wall wherever it is), and the
/// element's own outline in plan, where it has one (a level slab's too:
/// which part of a wall under several elements each covers).
struct Surface {
    what: String,
    planes: Vec<(P3, f64)>,
    footprint: Option<Vec<Path>>,
    outline: Option<Vec<Path>>,
}

/// A floor's, pad's or ceiling's top and bottom (feet): its own, else its
/// horizontal reference faces', else its loops' height and its type's
/// width (as [`slab`] builds it).
fn level_slab_bounds(s: &Slab, width: Option<f64>) -> Result<(f64, f64), String> {
    let level: Vec<f64> = s
        .ref_faces
        .iter()
        .filter(|p| math::cross(p.x, p.y)[2].abs() > 1.0 - 1e-9)
        .map(|p| p.origin[2])
        .collect();
    let loops_z = s
        .loops
        .first()
        .and_then(|l| l.first())
        .map(|c| c.start()[2]);
    let top = s
        .top
        .or(level.iter().copied().reduce(f64::max))
        .or(loops_z)
        .ok_or("no top")?;
    let bottom = s
        .bottom
        .or(level.iter().copied().reduce(f64::min).filter(|b| *b < top));
    match bottom {
        // (A ceiling's lower reference face bounds its core, above its
        // finish layers: its type's width runs down from its top.)
        Some(b) if top > b => Ok((top, below_core(s, top, b, width))),
        _ => Ok((top, top - width.ok_or("no thickness")?)),
    }
}

/// A slab's bottom: the one its reference faces give, or for a ceiling
/// without a bottom of its own whose type is thicker, its top less its
/// type's width (*seen*: ceilings' reference faces are their top and the
/// top of their finish layers).
fn below_core(s: &Slab, top: f64, bottom: f64, width: Option<f64>) -> f64 {
    match width {
        Some(w) if s.class == "Ceiling" && s.bottom.is_none() && w > top - bottom + 1e-6 => top - w,
        _ => bottom,
    }
}

/// The surface of the element a wall's top (its lower surface) or base
/// (its upper surface) is attached to (mitcad#348).
fn attached_surface(model: &Model, id: i64, top: bool) -> Result<Surface, String> {
    let b = &model.building;
    let level = |z: f64| vec![([0.0, 0.0, 1.0], z)];
    if let Some(p) = b.ref_planes.get(&id) {
        let mut n = math::normalize(math::cross(p.plane.x, p.plane.y))
            .ok_or_else(|| format!("reference plane {id} has no normal"))?;
        if n[2] < 0.0 {
            n = math::scale(n, -1.0);
        }
        if n[2] < 1e-3 {
            return Err(format!("reference plane {id} stands vertical"));
        }
        let name = if p.name.trim().is_empty() {
            String::new()
        } else {
            format!(" ({})", p.name.trim())
        };
        return Ok(Surface {
            what: format!("reference plane {id}{name}"),
            planes: vec![(n, math::dot(n, p.plane.origin))],
            footprint: None,
            outline: None,
        });
    }
    let s = b.slabs.get(&id).ok_or_else(|| {
        format!("element {id} is no roof, floor, ceiling or reference plane read")
    })?;
    let what = format!(
        "{} {id}",
        match s.class.as_str() {
            "Floor" => "floor",
            "BuildingPad" => "pad",
            "Ceiling" => "ceiling",
            _ => "roof",
        }
    );
    if let Some(why) = b.unread.get(&id) {
        return Err(format!("{what}'s parameters did not read: {why}"));
    }
    if s.shape_edited {
        return Err(format!("{what}'s shape is edited, which is not read"));
    }
    let width = s.type_id.and_then(|t| b.width(t));
    let outline = if s.class == "Roof" && s.extrusion.is_some() {
        extrusion_span(model, s)
    } else {
        slab_areas(s).map(|(a, _)| a)
    }
    .ok()
    .map(|a| a.into_iter().map(|a| a.outer).collect::<Vec<_>>());
    match s.class.as_str() {
        "Floor" | "BuildingPad" | "Ceiling" => {
            let (t, bottom) = level_slab_bounds(s, width).map_err(|e| format!("{what}: {e}"))?;
            Ok(Surface {
                what,
                planes: level(if top { bottom } else { t }),
                footprint: None,
                outline,
            })
        }
        "Roof" | "ProfileRoof" => {
            let open_profile = s.class == "Roof" && s.extrusion.is_some();
            let groups = slope_groups(s);
            if groups.is_empty() {
                return Err(format!("{what} has no layer faces"));
            }
            let layers = layer_widths(b, s.type_id);
            let flat = groups.iter().all(|(n, _)| n[2] > 1.0 - 1e-6);
            if flat && groups.len() > 1 {
                // (Built from its loops up by its type's width.)
                let z = s
                    .loops
                    .first()
                    .and_then(|l| l.first())
                    .map_or(0.0, |c| c.start()[2]);
                let w = width.ok_or_else(|| format!("{what} has no thickness"))?;
                return Ok(Surface {
                    what,
                    planes: level(if top { z } else { z + w }),
                    footprint: None,
                    outline,
                });
            }
            let mut planes = Vec::new();
            for (n, list) in &groups {
                if n[2] < 1e-3 {
                    return Err(format!("{what} has a slope that stands vertical"));
                }
                let (t, bottom) =
                    slope_bounds(list, width, &layers).map_err(|e| format!("{what}: {e}"))?;
                planes.push((*n, if top { bottom } else { t }));
            }
            let footprint = if flat {
                None
            } else {
                let areas = if open_profile {
                    extrusion_span(model, s)
                } else {
                    slab_areas(s).map(|(a, _)| a)
                };
                let areas = areas.map_err(|e| format!("{what}'s footprint: {e}"))?;
                Some(areas.into_iter().map(|a| a.outer).collect())
            };
            Ok(Surface {
                what,
                planes,
                footprint,
                outline,
            })
        }
        other => Err(format!("{what}: the surface of a {other} is not read")),
    }
}

/// A wall's heights with its attachments applied (mitcad#348), feet.
struct Attached {
    /// The prism's bottom and top: the wall's base and top, or beyond them
    /// to the attached surfaces.
    lo: f64,
    hi: f64,
    /// The half spaces cut from it: a plane (normal, offset), whether
    /// what lies below it (a base's) or above it (a top's) goes, and the
    /// band of the wall it is bounded to (None: all of it).
    cuts: Vec<(P3, f64, bool, Option<Path>)>,
    /// The wall carried on up within a roof's outline (feet).
    extensions: Vec<Extension>,
    notes: Vec<String>,
    /// Its attachments by name, applied or not.
    attached: Vec<String>,
}

/// The wall's footprint from `from` to `to` (feet) within an outline in
/// plan, under the planes (each a unit normal up and its offset).
struct Extension {
    outline: Vec<Path>,
    planes: Vec<(P3, f64)>,
    from: f64,
    to: f64,
}

/// A wall's attachments (mitcad#348): an attached top follows the lower
/// surface of the elements it is attached to and a base their upper one,
/// below or above the wall's own top or base: the wall's prism runs from
/// its base to above the surface, and what lies above every plane of the
/// surface goes (below every plane for a base, of one plane only). The
/// surface is the elements' as the import builds them, a fixed cut: the
/// features do not follow the roof when it changes. An attachment that
/// cannot be applied leaves that end at the wall's own height, with a note
/// naming it (the check against the reference or the stored box then
/// tells whether the wall agrees).
fn attach(model: &Model, w: &Wall, footprint: &Path, base: f64, top: f64) -> Attached {
    let mut out = Attached {
        lo: base,
        hi: top,
        cuts: Vec::new(),
        extensions: Vec::new(),
        notes: Vec::new(),
        attached: attachment_names(w),
    };
    if w.attachments.is_empty() {
        return out;
    }
    let points: Vec<P2> = footprint
        .segments
        .iter()
        .flat_map(|s| s.sample(4))
        .map(|p| [p[0], p[1]])
        .collect();
    let height_at = |(n, o): (P3, f64), p: P2| (o - n[0] * p[0] - n[1] * p[1]) / n[2];
    for a in &w.attachments {
        let end = if a.top { "top" } else { "base" };
        let ids = a
            .elements
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let not_applied =
            |why: String| format!("its {end}'s attachment to {ids} is not applied: {why}");
        let surfaces: Result<Vec<Surface>, String> = a
            .elements
            .iter()
            .map(|id| attached_surface(model, *id, a.top))
            .collect();
        let mut surfaces = match surfaces {
            Ok(s) => s,
            Err(why) => {
                out.notes.push(not_applied(why));
                continue;
            }
        };
        // Several elements: one level plane they share.
        let surface = if surfaces.len() == 1 {
            surfaces.remove(0)
        } else {
            let first = &surfaces[0];
            let same = surfaces.iter().all(|s| {
                s.footprint.is_none()
                    && s.planes.len() == 1
                    && first.planes.len() == 1
                    && math::dot(s.planes[0].0, first.planes[0].0) > 1.0 - 1e-9
                    && (s.planes[0].1 - first.planes[0].1).abs() < 1e-6
            });
            if !same {
                // Each element over its part of the wall.
                let note = attach_parts(&surfaces, a.top, &points, &mut out);
                out.notes.push(note.unwrap_or_else(not_applied));
                continue;
            }
            Surface {
                what: surfaces
                    .iter()
                    .map(|s| s.what.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                planes: first.planes.clone(),
                footprint: None,
                outline: None,
            }
        };
        if !a.top && surface.planes.len() > 1 {
            out.notes.push(not_applied(format!(
                "a base on {} of {} slopes",
                surface.what,
                surface.planes.len()
            )));
            continue;
        }
        // A sloped roof covers the wall (its edge may run along the wall's
        // face); over part of it (a ridge wall between two roofs' edges,
        // a wall reaching out from under it) the rest keeps its own top.
        if let Some(fp) = &surface.footprint
            && !points
                .iter()
                .all(|p| fp.iter().any(|l| footprint::covers(l, *p, 1e-3)))
        {
            let note = attach_parts(std::slice::from_ref(&surface), a.top, &points, &mut out);
            out.notes.push(note.unwrap_or_else(not_applied));
            continue;
        }
        // The surface's heights over the wall: the highest and lowest of
        // its planes, and (a top) the highest of the lowest at each point.
        let zs: Vec<f64> = points
            .iter()
            .flat_map(|p| surface.planes.iter().map(|q| height_at(*q, *p)))
            .collect();
        let zmax = zs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let zmin = zs.iter().copied().fold(f64::INFINITY, f64::min);
        if !zmax.is_finite() || !zmin.is_finite() {
            out.notes
                .push(not_applied("no point of the wall to place it at".into()));
            continue;
        }
        if a.top {
            let under = points
                .iter()
                .map(|p| {
                    surface
                        .planes
                        .iter()
                        .map(|q| height_at(*q, *p))
                        .fold(f64::INFINITY, f64::min)
                })
                .fold(f64::NEG_INFINITY, f64::max);
            if under <= out.lo + 1e-6 {
                out.notes.push(not_applied(format!(
                    "{}'s lower surface lies below its base",
                    surface.what
                )));
                continue;
            }
            out.hi = out.hi.max(zmax) + 1.0;
            out.cuts
                .extend(surface.planes.iter().map(|(n, o)| (*n, *o, false, None)));
            out.notes.push(format!(
                "its top follows {}'s lower surface{}",
                surface.what,
                if surface.planes.len() > 1 {
                    format!(" ({} slopes)", surface.planes.len())
                } else {
                    String::new()
                }
            ));
        } else {
            if zmin >= out.hi - 1e-6 {
                out.notes.push(not_applied(format!(
                    "{}'s upper surface lies above its top",
                    surface.what
                )));
                continue;
            }
            out.lo = out.lo.min(zmin) - 1.0;
            out.cuts
                .extend(surface.planes.iter().map(|(n, o)| (*n, *o, true, None)));
            out.notes
                .push(format!("its base follows {}'s upper surface", surface.what));
        }
    }
    out
}

/// An attached top under elements that each cover part of the wall (a
/// ridge wall under the two halves of a gable, whose edges end within its
/// thickness; a wall reaching out from under its roof): within each
/// element's outline the wall follows its lower surface (the lowest of
/// them where they overlap), elsewhere it keeps its own top. Where a
/// surface lies below the wall's own top a half space bounded to the
/// outline cuts it; where it rises above, the wall's footprint carries on
/// up within the outline to under it. The note, or why not.
fn attach_parts(
    surfaces: &[Surface],
    top: bool,
    points: &[P2],
    out: &mut Attached,
) -> Result<String, String> {
    if !top {
        return Err(format!(
            "the surfaces of its {} elements differ",
            surfaces.len()
        ));
    }
    let height_at = |(n, o): (P3, f64), p: P2| (o - n[0] * p[0] - n[1] * p[1]) / n[2];
    let (lo, own) = (out.lo, out.hi);
    let mut used = Vec::new();
    let mut extensions = Vec::new();
    let mut cuts = Vec::new();
    for s in surfaces {
        let Some(outline) = &s.outline else {
            return Err(format!("{} has no outline", s.what));
        };
        // Its heights over the points of the wall within its outline.
        let zs: Vec<f64> = points
            .iter()
            .filter(|p| outline.iter().any(|l| footprint::covers(l, **p, 1e-3)))
            .flat_map(|p| s.planes.iter().map(|q| height_at(*q, *p)))
            .collect();
        if zs.is_empty() {
            continue;
        }
        let zmin = zs.iter().copied().fold(f64::INFINITY, f64::min);
        let zmax = zs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if zmax <= lo + 1e-6 {
            return Err(format!("{}'s lower surface lies below its base", s.what));
        }
        used.push(s.what.clone());
        // (Carried on up only where the outline and the wall overlap: a
        // wall along its edge has no part within it.)
        let above = points
            .iter()
            .filter(|p| outline.iter().any(|l| footprint::covers(l, **p, -1.0)))
            .any(|p| s.planes.iter().all(|q| height_at(*q, *p) > own + 1e-3));
        if above {
            extensions.push(Extension {
                outline: outline.clone(),
                planes: s.planes.clone(),
                from: own - 0.01,
                to: zmax + 1.0,
            });
        }
        if zmin < own - 1e-6 {
            for l in outline {
                cuts.extend(
                    s.planes
                        .iter()
                        .map(|(n, o)| (*n, *o, false, Some(l.clone()))),
                );
            }
        }
    }
    if used.is_empty() {
        return Err("the wall lies under none of them".into());
    }
    out.extensions.extend(extensions);
    out.cuts.extend(cuts);
    Ok(format!(
        "its top follows {} lower surface{} where {} over it, elsewhere its own top",
        used.iter()
            .map(|w| format!("{w}'s"))
            .collect::<Vec<_>>()
            .join(", "),
        if used.len() > 1 { "s" } else { "" },
        if used.len() > 1 { "they are" } else { "it is" }
    ))
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
    let (solid, mut notes, attached) = wall_solid(model, w)?;
    let mut cuts = Vec::new();
    // Its openings (mitcad#345): the inserts and opening elements that cut
    // it, each with its tools or why it has none.
    let (cutters, idle) = wall_openings(model, w);
    let mut openings = Vec::new();
    for c in cutters {
        match c.tools {
            Ok((tools, by)) => {
                if by != "cut loops" {
                    notes.push(format!("{} is cut by {by}", c.label));
                }
                cuts.extend(tools.into_iter().map(|t| (c.label.clone(), t)));
                openings.push(WallOpening {
                    id: c.id,
                    label: c.label,
                    missing: None,
                });
            }
            Err(why) => {
                notes.push(format!("{} is not cut: {why}", c.label));
                openings.push(WallOpening {
                    id: c.id,
                    label: c.label,
                    missing: Some(why),
                });
            }
        }
    }
    if idle > 0 {
        notes.push(format!(
            "{idle} elements it hosts cut nothing (no cut step of its, no cut loops, no void \
             geometry)"
        ));
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
        attached,
        openings,
        notes,
        material: layer_material(b, w.type_id),
    })
}

/// An opening of a wall in its plan (mitcad#345): the insert or opening
/// element, the label of its cuts (`insert 47`, `opening 12`), and why it
/// has no cut tool (None: its cuts are among the plan's).
#[derive(Debug, Clone, PartialEq)]
pub struct WallOpening {
    pub id: i64,
    pub label: String,
    pub missing: Option<String>,
}

/// What opens a wall (mitcad#345): an insert or an opening element, its
/// cut tools and what they are made of, or why it has none.
#[derive(Debug, Clone)]
pub struct Cutter {
    pub id: i64,
    pub label: String,
    pub tools: Result<(Vec<Item>, &'static str), String>,
}

/// What opens a wall (mitcad#345): the inserts whose cut it takes (its
/// `WallCutoutGStep`s and their kin), the doors and windows it hosts, the
/// other elements it hosts whose types have cut loops or void geometry,
/// and its opening elements (a rectangular wall opening, an opening of a
/// profile); each with its tools or why it has none. Also how many other
/// elements it hosts, which cut nothing (trims, muntins: no cut step of
/// the wall's, no cut loops, no void geometry).
pub fn wall_openings(model: &Model, w: &Wall) -> (Vec<Cutter>, usize) {
    let b = &model.building;
    let width = w.type_id.and_then(|t| b.width(t)).unwrap_or(1.0);
    let mut ids: Vec<i64> = w.cutouts.clone();
    let mut idle = 0;
    for i in b.instances.values().filter(|i| i.host == Some(w.id)) {
        if ids.contains(&i.id) {
            continue;
        }
        let door_or_window = model
            .element(i.id)
            .and_then(|e| e.category)
            .is_some_and(|c| c == categories::OST_DOORS || c == categories::OST_WINDOWS);
        let cuts = [i.symbol, i.master]
            .into_iter()
            .flatten()
            .filter_map(|s| b.symbols.get(&s))
            .any(|s| !s.cut_loops.is_empty() || !s.cut_solids.is_empty());
        if door_or_window || cuts {
            ids.push(i.id);
        } else {
            idle += 1;
        }
    }
    ids.sort_unstable();
    let mut out: Vec<Cutter> = ids
        .into_iter()
        .map(|id| match b.walls.get(&id) {
            // A wall embedded in it (a curtain wall in a basic wall).
            Some(e) => Cutter {
                id,
                label: format!("embedded wall {id}"),
                tools: embedded_tool(w, e, width).map(|t| (vec![t], "its embedded wall's extent")),
            },
            None => Cutter {
                id,
                label: format!("insert {id}"),
                tools: insert_cuts(b, id, width),
            },
        })
        .collect();
    for o in b
        .openings
        .values()
        .filter(|o| o.host == Some(w.id) && o.class != "ShaftOpening")
    {
        out.push(Cutter {
            id: o.id,
            label: format!("opening {}", o.id),
            tools: opening_tool(w, o, width).map(|(t, by)| (vec![t], by)),
        });
    }
    (out, idle)
}

/// The opening a wall embedded in another cuts in it (`WallEmbedWallGStep`,
/// a curtain wall in a basic wall): the embedded wall's location line
/// from its base to its top, through the host square to it.
fn embedded_tool(w: &Wall, e: &Wall, host_width: f64) -> Result<Item, String> {
    let base = e.base.ok_or("the embedded wall has no base height")?;
    let height = e
        .height
        .filter(|h| *h > 0.0)
        .ok_or("the embedded wall has no height")?;
    let (s, t) = (e.curve.start(), e.curve.end());
    rect_tool(
        w,
        [s[0], s[1], base],
        [t[0], t[1], base + height],
        host_width,
    )
}

/// A rectangle in a line wall's plane between two corners (feet), through
/// the wall square to it.
fn rect_tool(w: &Wall, p: P3, q: P3, host_width: f64) -> Result<Item, String> {
    let Curve::Line { start, end } = &w.curve else {
        return Err("an opening in an arc wall is not built".into());
    };
    let d = math::normalize([end[0] - start[0], end[1] - start[1], 0.0])
        .ok_or("its wall's location line has no length")?;
    let up = [0.0, 0.0, 1.0];
    let reach = host_width + 2.0;
    let along = math::dot(math::sub(q, p), d);
    let (lo, hi) = (p[2].min(q[2]), p[2].max(q[2]));
    if along.abs() < 1e-6 || hi - lo < 1e-6 {
        return Err("its rectangle has no size".into());
    }
    let x = if along > 0.0 { d } else { math::scale(d, -1.0) };
    let z = math::cross(x, up);
    // (The corners projected onto the host's plane.)
    let origin = math::sub(
        [p[0], p[1], lo],
        math::scale(z, reach + math::dot(math::sub(p, *start), z)),
    );
    let (u, v) = (along.abs(), hi - lo);
    Ok(Item::Extrusion {
        position: math::from_axes(mm(origin), x, up, z),
        profile: profile_mm(areas(vec![polygon(&[
            [0.0, 0.0],
            [u, 0.0],
            [u, v],
            [0.0, v],
        ])])),
        direction: [0.0, 0.0, 1.0],
        depth: 2.0 * reach * FOOT_MM,
    })
}

/// A wall's opening element's cut tool (mitcad#345): a rectangular
/// opening's rectangle between its two corners in the wall's plane, or an
/// opening's profile loops, through the wall square to it.
fn opening_tool(
    w: &Wall,
    o: &building::Opening,
    host_width: f64,
) -> Result<(Item, &'static str), String> {
    let Curve::Line { start, end } = &w.curve else {
        return Err("an opening in an arc wall is not built".into());
    };
    let d = math::normalize([end[0] - start[0], end[1] - start[1], 0.0])
        .ok_or("its wall's location line has no length")?;
    let up = [0.0, 0.0, 1.0];
    let reach = host_width + 2.0;
    if let Some([p, q]) = o.corners {
        let along = math::dot(math::sub(q, p), d);
        let (lo, hi) = (p[2].min(q[2]), p[2].max(q[2]));
        if along.abs() < 1e-6 || hi - lo < 1e-6 {
            return Err("its rectangle has no size".into());
        }
        let x = if along > 0.0 { d } else { math::scale(d, -1.0) };
        let z = math::cross(x, up);
        let origin = math::sub([p[0], p[1], lo], math::scale(z, reach));
        let (u, v) = (along.abs(), hi - lo);
        let tool = Item::Extrusion {
            position: math::from_axes(mm(origin), x, up, z),
            profile: profile_mm(areas(vec![polygon(&[
                [0.0, 0.0],
                [u, 0.0],
                [u, v],
                [0.0, v],
            ])])),
            direction: [0.0, 0.0, 1.0],
            depth: 2.0 * reach * FOOT_MM,
        };
        return Ok((tool, "its rectangle"));
    }
    if o.loops.is_empty() {
        return Err(format!("its boundary ({}) did not read", o.class));
    }
    let n = math::cross(d, up);
    let back = math::scale(n, -reach);
    let shift = [
        1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, back[0], back[1], back[2],
    ];
    let loops: Vec<Vec<Curve>> = o
        .loops
        .iter()
        .map(|l| l.iter().map(|c| c.transformed(&shift)).collect())
        .collect();
    prism(&loops, n, 2.0 * reach).map(|t| (t, "its profile"))
}

/// The cut tools of an insert (mitcad#345) and what they are made of: its
/// type's cut loops at its transform, through the host along the cut
/// direction; else its type's void geometry at its transform. An error
/// says why it has none.
fn insert_cuts(
    b: &Building,
    id: i64,
    host_width: f64,
) -> Result<(Vec<Item>, &'static str), String> {
    let inst = b
        .instances
        .get(&id)
        .ok_or("it is no family instance read")?;
    let t = inst.transform.ok_or("it has no transform")?;
    let symbols: Vec<&Symbol> = [inst.symbol, inst.master]
        .into_iter()
        .flatten()
        .filter_map(|s| b.symbols.get(&s))
        .collect();
    if let Some(symbol) = symbols.iter().find(|s| !s.cut_loops.is_empty()) {
        return cut_loop_tools(symbol, &t, host_width).map(|tools| (tools, "cut loops"));
    }
    if let Some(symbol) = symbols.iter().find(|s| !s.cut_solids.is_empty()) {
        return void_tools(symbol, &t).map(|tools| (tools, "its type's void geometry"));
    }
    if symbols.is_empty() {
        return Err("its type is not read".into());
    }
    match symbols.iter().find_map(|s| s.cut_notes.first()) {
        Some(n) => Err(format!("its type's void geometry did not read: {n}")),
        None => Err("its type has neither cut loops nor void geometry".into()),
    }
}

/// An insert type's void geometry at the instance's transform (mitcad#345):
/// each body's closed triangle mesh as planar faces (model millimetres).
fn void_tools(symbol: &Symbol, t: &[f64; 12]) -> Result<Vec<Item>, String> {
    let det = t[0] * (t[4] * t[8] - t[5] * t[7]) - t[1] * (t[3] * t[8] - t[5] * t[6])
        + t[2] * (t[3] * t[7] - t[4] * t[6]);
    let mut tools = Vec::new();
    for s in &symbol.cut_solids {
        // Its planar faces as they are (no slivers of triangles), else its
        // triangles.
        let faces = super::geometry::solid_faces(s)
            .map_err(|e| format!("its type's void geometry: {e}"))?;
        if let Some(faces) = faces {
            let faces = faces
                .into_iter()
                .map(|face| {
                    face.into_iter()
                        .map(|l| {
                            let mut l: Vec<P3> =
                                l.iter().map(|p| mm(building::apply(t, *p))).collect();
                            // (A mirroring transform turns the faces inside out.)
                            if det < 0.0 {
                                l.reverse();
                            }
                            l
                        })
                        .collect()
                })
                .collect();
            tools.push(Item::Faceted {
                faces,
                closed: true,
            });
            continue;
        }
        let (points, triangles) =
            super::geometry::solid_mesh(s).map_err(|e| format!("its type's void geometry: {e}"))?;
        let placed: Vec<P3> = points.iter().map(|p| mm(building::apply(t, *p))).collect();
        let faces = triangles
            .iter()
            .map(|tr| {
                let [a, b, c] = tr.map(|i| placed[i as usize]);
                // (A mirroring transform turns the faces inside out.)
                vec![if det < 0.0 {
                    vec![a, c, b]
                } else {
                    vec![a, b, c]
                }]
            })
            .collect();
        tools.push(Item::Faceted {
            faces,
            closed: true,
        });
    }
    Ok(tools)
}

/// An insert type's cut loops at the instance's transform, through the
/// host along the type's cut direction.
fn cut_loop_tools(symbol: &Symbol, t: &[f64; 12], host_width: f64) -> Result<Vec<Item>, String> {
    let t = *t;
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
        return wall_solid(model, w).map(|(s, _, _)| s);
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
            Some(b) if top > b => top - below_core(s, top, b, width),
            _ => width.ok_or("no thickness")?,
        };
        extrusion(at_height(top), profile_mm(areas_ft), false, thickness)
    } else if s.class == "ProfileRoof" || s.class == "Roof" && !open_profile {
        // A footprint roof.
        roof_solid(s, areas_ft, loops_z, width, &layers, &mut notes)?
    } else if open_profile {
        // An extrusion roof: its profile's span across the extents of the
        // extrusion, under its slopes.
        let rect = extrusion_span(model, s)?;
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
            Ok((tools, _)) => cuts.extend(tools.into_iter().map(|t| (format!("insert {id}"), t))),
            Err(why) => notes.push(format!("insert {id} is not cut: {why}")),
        }
    }
    Ok(Plan {
        kind,
        solid,
        cuts,
        more: Vec::new(),
        attached: Vec::new(),
        openings: Vec::new(),
        notes,
        material: layer_material(b, s.type_id),
    })
}

/// A roof's slopes: its layer reference faces grouped by their normal
/// (pointing up), each face with its offset along the normal.
type SlopeGroups<'s> = Vec<(P3, Vec<(f64, &'s building::Plane)>)>;

fn slope_groups(s: &Slab) -> SlopeGroups<'_> {
    let mut groups: SlopeGroups<'_> = Vec::new();
    for p in &s.layer_faces {
        let Some(mut n) = math::normalize(math::cross(p.x, p.y)) else {
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
    groups
}

/// A slope's top and bottom (offsets along its normal): its faces are the
/// boundaries of its type's layers, the outer ones among them (one more
/// than the layers) or not (one fewer: then the outer layers' widths
/// beyond them).
fn slope_bounds(
    list: &[(f64, &building::Plane)],
    width: Option<f64>,
    layers: &[f64],
) -> Result<(f64, f64), String> {
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
}

/// The widths of a type's layers that have one, from the top.
fn layer_widths(b: &Building, type_id: Option<i64>) -> Vec<f64> {
    type_id
        .and_then(|t| b.structures.get(&t))
        .map(|l| l.iter().map(|l| l.width).filter(|w| *w > 1e-9).collect())
        .unwrap_or_default()
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
    let groups = slope_groups(s);
    let bounds = |list: &[(f64, &building::Plane)]| slope_bounds(list, width, layers);
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
    let under = |planes: &[(P3, f64)]| {
        planes.iter().fold(prism(), |acc, (n, o)| Item::Boolean {
            op: BoolOp::Difference,
            first: Box::new(acc),
            second: Box::new(half_space(*n, *o, false, None)),
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
        attached: Vec::new(),
        openings: Vec::new(),
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
        attached: Vec::new(),
        openings: Vec::new(),
        notes,
        material,
    })
}

/// An extrusion roof's span (feet): the rectangle its open profile (in a
/// vertical plane) covers across the extents of its extrusion along the
/// plane's normal.
/// An extrusion roof's span in plan: its profile across the extents of its
/// extrusion, square to the profile. Which side of the profile the
/// extents run to is the side its stored box lies on (the work plane's
/// normal is not read; *seen*: two halves of a gable drawn from the eaves
/// to the ridge, their profiles running opposite ways, with the same
/// extents).
fn extrusion_span(model: &Model, s: &Slab) -> Result<Vec<Area>, String> {
    let [d0, d1] = s.extrusion.ok_or("no extrusion extents")?;
    let curves: Vec<&Curve> = s.loops.iter().flatten().collect();
    let first = curves.first().ok_or("no profile")?.start();
    let last = curves.last().ok_or("no profile")?.end();
    let along = math::normalize([last[0] - first[0], last[1] - first[1], 0.0])
        .ok_or("a profile that does not run across")?;
    let mut n = math::cross(along, [0.0, 0.0, 1.0]);
    let (r0, r1) = (-d0.max(d1), -d0.min(d1));
    if let Some(b) = model.element(s.id).and_then(|e| e.bbox) {
        let mid = (r0 + r1) / 2.0;
        let c = [
            (first[0] + last[0]) / 2.0 + n[0] * mid,
            (first[1] + last[1]) / 2.0 + n[1] * mid,
        ];
        let inside = |p: [f64; 2]| (b[0]..=b[3]).contains(&p[0]) && (b[1]..=b[4]).contains(&p[1]);
        let flipped = [
            (first[0] + last[0]) / 2.0 - n[0] * mid,
            (first[1] + last[1]) / 2.0 - n[1] * mid,
        ];
        if !inside(c) && inside(flipped) {
            n = math::scale(n, -1.0);
        }
    }
    let at = |p: P3, r: f64| [p[0] + n[0] * r, p[1] + n[1] * r];
    let pts = [at(first, r0), at(last, r0), at(last, r1), at(first, r1)];
    Ok(areas(vec![polygon(&pts)]))
}

/// A monolithic stair run: its steps' profile (risers of equal height
/// from its bottom to its top, its treads' depths) down to the underside
/// of its support, across its width (its start riser line).
fn stair(model: &Model, e: &Element, r: &building::StairRun) -> Result<Plan, String> {
    if r.spiral {
        return spiral_run(model, e, r);
    }
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
    // (mitcad#340) The treads' depths run from nosing to nosing; a riser
    // leans from its base, the nosing length behind its tread's front, up
    // to that front.
    let nosing = e
        .type_id
        .and_then(|t| model.building.stair_parts.get(&t))
        .map_or(0.0, |p| p.nosing.max(0.0));
    let run: f64 = r.treads.iter().take(n).sum();
    let mut pts: Vec<P2> = vec![[nosing, 0.0]];
    let mut s = 0.0;
    for k in 0..n {
        pts.push([s, (k + 1) as f64 * h]);
        if let Some(d) = r.treads.get(k) {
            s += d;
            // Its tread reaches the next riser's base (at most the run's
            // end).
            let back = (s + nosing).min(run);
            pts.push([back, (k + 1) as f64 * h]);
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
    // support's depth, square to it, below the line through the risers'
    // bases, a riser and the nosing's rise below the nosings.)
    let slope = (q[1] - p[1]) / (q[0] - p[0]);
    let drop = depth * (1.0 + slope * slope).sqrt();
    let u = |s: f64| p[1] - h - nosing * slope - drop + (s - p[0]) * slope;
    if u(length) >= total - 1e-6 {
        return Err("its support's underside reaches above its top".into());
    }
    pts.push([length, u(length).max(0.0)]);
    if u(nosing) > 0.0 {
        pts.push([nosing, u(nosing).min(h)]);
    } else {
        let s0 = p[0] + (h + nosing * slope + drop - p[1]) / slope;
        pts.push([s0.clamp(nosing, length), 0.0]);
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
        attached: Vec::new(),
        openings: Vec::new(),
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
    if m.profile == "SysVMullionProfile" {
        return v_mullion(b, i, m);
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
            // (-1007300 towards `normal × path`, -1007301 the other way:
            // the off-centre mullions, mitcad#340.)
            (-w1, w0)
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
    let corners = [(a0, n0), (a1, n0), (a1, n1), (a0, n1)];
    let offset = |u: f64, v: f64| math::add(math::scale(a, u), math::scale(n, v));
    let (ends, end_note) = trimmed_ends(model, i, m, *start, d, length);
    // How far the prism reaches past each end so that its cut there
    // leaves no part of it short (the corner lines' crossings with the
    // end's plane).
    let reach = |k: usize| {
        let Some((p, o)) = ends[k] else { return 0.0 };
        corners
            .iter()
            .map(|&(u, v)| {
                let c = math::add(*start, offset(u, v));
                let s = math::dot(math::sub(p, c), o) / math::dot(d, o);
                if k == 0 { -s } else { s - length }
            })
            .fold(0.0_f64, f64::max)
            + (a1 - a0).max(n1 - n0)
    };
    let (before, after) = (reach(0), reach(1));
    let base = math::sub(*start, math::scale(d, before));
    let at = |u: f64, v: f64| building::apply(&t, math::add(base, offset(u, v)));
    let outline = polyline(&corners.map(|(u, v)| at(u, v)));
    let mut solid = prism(&[outline], building::turn(&t, d), before + length + after)?;
    for (p, o) in ends.into_iter().flatten() {
        let (p, o) = (building::apply(&t, p), building::turn(&t, o));
        solid = Item::Boolean {
            op: BoolOp::Difference,
            first: Box::new(solid),
            second: Box::new(half_space(o, math::dot(o, p), false, None)),
        };
    }
    let material = i
        .master
        .and_then(|s| b.symbols.get(&s))
        .and_then(|s| s.materials.first().copied());
    Ok(Plan {
        kind: "mullion",
        solid,
        cuts: Vec::new(),
        more: Vec::new(),
        attached: Vec::new(),
        openings: Vec::new(),
        notes: vec![format!(
            "a {} mullion {:.0} × {:.0} mm",
            if quad { "corner" } else { "rectangular" },
            (a1 - a0) * FOOT_MM,
            (n1 - n0) * FOOT_MM
        )]
        .into_iter()
        .chain(end_note)
        .collect(),
        material,
    })
}

/// The planes a trimmed mullion's ends are cut along (mitcad#385), in its
/// symbol's coordinates: a point and the normal pointing out of the
/// mullion, per end (`None`: square at its path's end). A mullion trimmed
/// to its neighbours (`MullionTrimNeighbor`) stops at a neighbour's side,
/// or meets it in a mitre, by rules of the trims' flags and the
/// neighbours' types that are not decoded (the same flags give different
/// cuts on the curtain walls of the corpus); its symbol's own stored
/// geometry keeps the result, a planar end face per end, which gives the
/// planes. With a note when they do not read.
fn trimmed_ends(
    model: &Model,
    i: &Instance,
    m: &building::MullionSym,
    start: P3,
    d: P3,
    length: f64,
) -> ([Option<(P3, P3)>; 2], Option<String>) {
    if m.trims.is_empty() {
        return ([None, None], None);
    }
    let not_read = |why: &str| {
        (
            [None, None],
            Some(format!("its ends trimmed to its neighbours: {why}")),
        )
    };
    let Some(solid) =
        i.symbol
            .and_then(|s| model.geometry.get(&s))
            .and_then(|g| match g.solids.as_slice() {
                [one] => Some(one),
                _ => None,
            })
    else {
        return not_read("its symbol keeps no single body to take its end faces from");
    };
    let mut ends: [Vec<(P3, P3)>; 2] = [Vec::new(), Vec::new()];
    for face in &solid.faces {
        // (A face without loops bounds nothing: the stored bodies of trimmed
        // mullions keep their square ends' planes so.)
        let mitcad_rvt::brep::Surface::Plane { origin, x, y } = &face.surface else {
            continue;
        };
        if face.loops.is_empty() {
            continue;
        }
        let Some(normal) = math::normalize(math::cross(*x, *y)) else {
            continue;
        };
        let along = math::dot(normal, d);
        // (The sides run along the path.)
        if along.abs() < 1e-3 {
            continue;
        }
        let s = math::dot(math::sub(*origin, start), normal) / along;
        let k = usize::from(s > length / 2.0);
        let out = if (along > 0.0) == (k == 1) {
            normal
        } else {
            math::scale(normal, -1.0)
        };
        ends[k].push((*origin, out));
    }
    let mut planes = [None, None];
    for (k, faces) in ends.iter().enumerate() {
        match faces.as_slice() {
            [] => return not_read("an end of its stored body has no planar face"),
            [(p, o)] => {
                let at_end = if k == 0 {
                    start
                } else {
                    math::add(start, math::scale(d, length))
                };
                let square = math::dot(*o, d).abs() > 1.0 - 1e-9;
                if !square || math::dot(math::sub(*p, at_end), *o).abs() > 1e-6 {
                    planes[k] = Some((*p, *o));
                }
            }
            _ => return not_read("an end of its stored body has several faces"),
        }
    }
    let note = planes.iter().any(Option::is_some).then(|| {
        "its ends cut along its symbol's stored end faces (trimmed to its neighbours)".into()
    });
    (planes, note)
}

/// A V (corner) mullion (mitcad#340): two legs of its thickness, at right
/// angles, running out along its corner angles' directions `e1`, `e2`
/// (from `a = normal × path` towards the normal); each leg's middle line is
/// its thickness away from the line through the path along it, on the far
/// side from the other leg, and reaches its leg's length from where the
/// middle lines meet (and half the thickness back past it, closing the
/// corner): an L of six corners along its straight path, from its own
/// symbol at the instance's transform.
fn v_mullion(b: &Building, i: &Instance, m: &building::MullionSym) -> Result<Plan, String> {
    let t = i.transform.ok_or("no transform")?;
    let Some(Curve::Line { start, end }) = &m.path else {
        return Err("a mullion along a curve".into());
    };
    let length = math::length(math::sub(*end, *start));
    let d = math::normalize(math::sub(*end, *start)).ok_or("a path of no length")?;
    let n = math::normalize(m.normal).ok_or("no normal")?;
    let a = math::normalize(math::cross(n, d)).ok_or("a normal along the path")?;
    let [t1, t2] = m.corner_angles;
    let turn = (t2 - t1).rem_euclid(std::f64::consts::TAU);
    if (turn - std::f64::consts::FRAC_PI_2).abs() > 1e-6
        && (turn - 3.0 * std::f64::consts::FRAC_PI_2).abs() > 1e-6
    {
        return Err(format!(
            "a V mullion whose legs meet at {:.1}° (only right angles are read)",
            turn.to_degrees()
        ));
    }
    if m.offset.abs() > 1e-9 {
        return Err("a V mullion with an offset (not read)".into());
    }
    let th = m.thickness;
    let [l1, l2] = m.legs;
    if th <= 1e-9 || l1 <= 1e-9 || l2 <= 1e-9 {
        return Err("a V mullion of no size".into());
    }
    let dir = |angle: f64| math::add(math::scale(a, angle.cos()), math::scale(n, angle.sin()));
    let (e1, e2) = (dir(t1), dir(t2));
    // (u along e2, w along e1; the middle lines meet at (-th, -th).)
    let at = |u: f64, w: f64| {
        building::apply(
            &t,
            math::add(*start, math::add(math::scale(e2, u), math::scale(e1, w))),
        )
    };
    let (o, i_) = (-1.5 * th, -0.5 * th);
    let outline = polyline(&[
        at(o, o),
        at(l2 - th, o),
        at(l2 - th, i_),
        at(i_, i_),
        at(i_, l1 - th),
        at(o, l1 - th),
    ]);
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
        attached: Vec::new(),
        openings: Vec::new(),
        notes: vec![format!(
            "a V mullion, legs {:.0} and {:.0} mm, {:.0} mm thick",
            l1 * FOOT_MM,
            l2 * FOOT_MM,
            th * FOOT_MM
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
        attached: Vec::new(),
        openings: Vec::new(),
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
        attached: Vec::new(),
        openings: Vec::new(),
        notes: vec![format!("a run of {n} risers: {steps} treads and risers")],
        material: None,
    })
}

/// A spiral run of treads (mitcad#385): tread k's front on riser line k,
/// its back on riser line k + 1 moved the nosing length on (the next tread
/// overhangs it by the nosing), between the circles of the run's inner and
/// outer radii about its centre; its top at its step's height, its
/// thickness below. Under its front the type's nosing profile (a profile
/// family's rectangle: its x into the tread, its y up from the tread's
/// top) runs from circle to circle, joined to the tread.
fn spiral_run(model: &Model, e: &Element, r: &building::StairRun) -> Result<Plan, String> {
    let s = r
        .layout
        .as_ref()
        .ok_or("a spiral run whose centre, radii and riser lines do not read")?;
    let parts = e
        .type_id
        .and_then(|t| model.building.stair_parts.get(&t))
        .ok_or("its run type's treads and risers are not read")?;
    if parts.monolithic {
        return Err("a monolithic spiral run (its support is not read)".into());
    }
    if parts.riser.is_some() {
        return Err("a spiral run with risers (where they stand is not read)".into());
    }
    let tt = parts.tread.ok_or("a spiral run without treads")?;
    let n = usize::try_from(r.risers).unwrap_or(0);
    if n < 2 || s.risers.len() != n || r.top <= r.bottom {
        return Err(format!(
            "its {} riser lines do not match its {n} risers",
            s.risers.len()
        ));
    }
    let [r_in, r_out] = s.radii;
    let nosing = parts.nosing.max(0.0);
    if r_in <= nosing || r_out <= r_in {
        return Err(format!(
            "its radii {r_in:.3} and {r_out:.3} ft do not hold its nosing of {nosing:.3} ft"
        ));
    }
    let up = [0.0, 0.0, 1.0];
    let c = [s.center[0], s.center[1], 0.0];
    let flat = |p: P3| [p[0], p[1], 0.0];
    // Riser line k: the point of its line nearest to the centre and its
    // direction outwards (in plan).
    let line = |k: usize| -> Result<(P3, P3), String> {
        let Curve::Line { start, end } = &s.risers[k] else {
            return Err(format!("its riser {k} is not a line"));
        };
        let (a, b) = (flat(*start), flat(*end));
        let mut d = math::normalize(math::sub(b, a)).ok_or("a riser line of no length")?;
        let middle = math::scale(math::add(a, b), 0.5);
        if math::dot(math::sub(middle, c), d) < 0.0 {
            d = math::scale(d, -1.0);
        }
        let foot = math::add(a, math::scale(d, math::dot(math::sub(c, a), d)));
        Ok((foot, d))
    };
    // The way the risers turn about the centre (+1 counter-clockwise).
    let sense = math::cross(line(0)?.1, line(1)?.1)[2].signum();
    if sense == 0.0 {
        return Err("its first two riser lines are parallel".into());
    }
    // Where a line (its foot, its direction) meets a circle about the
    // centre, outwards of its foot.
    let meet = |foot: P3, d: P3, radius: f64| -> Result<P3, String> {
        let q = radius * radius - math::dot(math::sub(foot, c), math::sub(foot, c));
        if q <= 1e-12 {
            return Err("a tread's side does not reach its inner circle".into());
        }
        Ok(math::add(foot, math::scale(d, q.sqrt())))
    };
    // The arc of a circle about the centre from a to b, turning `turn`.
    let arc = |a: P3, b: P3, radius: f64, turn: f64| -> Result<Curve, String> {
        let x = math::normalize(math::sub(a, c)).ok_or("a point at the centre")?;
        let y = math::scale(math::cross(up, x), turn);
        let v = math::sub(b, c);
        let end = math::dot(v, y)
            .atan2(math::dot(v, x))
            .rem_euclid(std::f64::consts::TAU);
        Ok(Curve::Arc {
            center: c,
            x_axis: x,
            y_axis: y,
            radius,
            start: 0.0,
            end,
        })
    };
    // The region between two lines (feet, one direction each) from circle
    // to circle, at a height: from the inner end of the first, out, round
    // the outer circle (turning on), in along the second and back round the
    // inner circle.
    let between = |(f0, d0): (P3, P3), (f1, d1): (P3, P3), z: f64| -> Result<Vec<Curve>, String> {
        let (a, b) = (meet(f0, d0, r_in)?, meet(f0, d0, r_out)?);
        let (p, q) = (meet(f1, d1, r_out)?, meet(f1, d1, r_in)?);
        let lift = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, z];
        Ok([
            Curve::Line { start: a, end: b },
            arc(b, p, r_out, sense)?,
            Curve::Line { start: p, end: q },
            arc(q, a, r_in, -sense)?,
        ]
        .iter()
        .map(|c| c.transformed(&lift))
        .collect())
    };
    // The nosing profile: a rectangle (x0, x1) × (y0, y1).
    let nosing_profile = match parts.nosing_profile {
        Some(p) => Some(
            rectangle(&profile_loops(model, p)?)
                .ok_or("its nosing profile is not a rectangle (only rectangular ones are read)")?,
        ),
        None => None,
    };
    let h = (r.top - r.bottom) / n as f64;
    let mut solids = Vec::new();
    for k in 0..n - 1 {
        let top = r.bottom + (k + 1) as f64 * h;
        let front = line(k)?;
        let (f1, d1) = line(k + 1)?;
        let back = (
            math::add(f1, math::scale(math::cross(up, d1), sense * nosing)),
            d1,
        );
        let tread = prism(&[between(front, back, top - tt)?], up, tt)?;
        let Some([x0, x1, y0, y1]) = nosing_profile else {
            solids.push(tread);
            continue;
        };
        // Under the tread's front, below the tread.
        let (bottom, upto) = (top + y0, top + y1.min(-tt));
        if upto - bottom <= 1e-9 {
            solids.push(tread);
            continue;
        }
        let (f0, d0) = front;
        let into = math::scale(math::cross(up, d0), sense);
        let strip = prism(
            &[between(
                (math::add(f0, math::scale(into, x0)), d0),
                (math::add(f0, math::scale(into, x1)), d0),
                bottom,
            )?],
            up,
            upto - bottom,
        )?;
        solids.push(Item::Boolean {
            op: BoolOp::Union,
            first: Box::new(tread),
            second: Box::new(strip),
        });
    }
    let treads = solids.len();
    let solid = solids.remove(0);
    Ok(Plan {
        kind: "stair",
        solid,
        cuts: Vec::new(),
        more: solids,
        attached: Vec::new(),
        openings: Vec::new(),
        notes: vec![format!(
            "a spiral run of {n} risers: {treads} treads{} about its centre",
            if nosing_profile.is_some() {
                " with their nosings"
            } else {
                ""
            }
        )],
        material: None,
    })
}

/// A profile of one loop of lines that bound a rectangle along the axes:
/// its x and y ranges.
fn rectangle(loops: &[Vec<Curve>]) -> Option<[f64; 4]> {
    let [l] = loops else { return None };
    let mut points = Vec::new();
    for c in l {
        let Curve::Line { start, end } = c else {
            return None;
        };
        if (start[0] - end[0]).abs() > 1e-9 && (start[1] - end[1]).abs() > 1e-9 {
            return None;
        }
        points.push(*start);
    }
    let min = |k: usize| points.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min);
    let max = |k: usize| {
        points
            .iter()
            .map(|p| p[k])
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let r = [min(0), max(0), min(1), max(1)];
    (l.len() == 4 && r[1] - r[0] > 1e-9 && r[3] - r[2] > 1e-9).then_some(r)
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
        attached: Vec::new(),
        openings: Vec::new(),
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
/// needed), in order. A curve's tag (a wall sweep's side: whether its
/// profile is mirrored across it) turns with it, and a curve joins a chain
/// only where their tags then agree; a chain carries its tag.
type Chain = (Vec<Curve>, Option<bool>);

fn chains(curves: &[(Curve, Option<bool>)], tolerance: f64) -> Vec<Chain> {
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
        Curve::Ellipse {
            center,
            x_axis,
            y_axis,
            rx,
            ry,
            start,
            end,
        } => Curve::Ellipse {
            center: *center,
            x_axis: *x_axis,
            y_axis: math::scale(*y_axis, -1.0),
            rx: *rx,
            ry: *ry,
            start: -end,
            end: -start,
        },
        // (The rise turns with the angles.)
        Curve::Helix {
            center,
            x_axis,
            y_axis,
            z_axis,
            radius,
            rise,
            start,
            end,
        } => Curve::Helix {
            center: *center,
            x_axis: *x_axis,
            y_axis: math::scale(*y_axis, -1.0),
            z_axis: math::scale(*z_axis, -1.0),
            radius: *radius,
            rise: *rise,
            start: -end,
            end: -start,
        },
    };
    // A tag turns with its curve.
    let turned = |(c, tag): &(Curve, Option<bool>)| (reversed(c), tag.map(|t| !t));
    let mut left: Vec<(Curve, Option<bool>)> = curves.to_vec();
    let mut out = Vec::new();
    while !left.is_empty() {
        let first = left.remove(0);
        let tag = first.1;
        let mut chain = vec![first.0];
        loop {
            let tail = chain.last().expect("one").end();
            let head = chain[0].start();
            let at_tail = |(c, t): &(Curve, Option<bool>)| {
                (close(c.start(), tail) && *t == tag)
                    || (close(c.end(), tail) && *t == tag.map(|g| !g))
            };
            let at_head = |(c, t): &(Curve, Option<bool>)| {
                (close(c.end(), head) && *t == tag)
                    || (close(c.start(), head) && *t == tag.map(|g| !g))
            };
            if let Some(i) = left.iter().position(at_tail) {
                let c = left.remove(i);
                chain.push(if close(c.0.start(), tail) && c.1 == tag {
                    c.0
                } else {
                    turned(&c).0
                });
            } else if let Some(i) = left.iter().position(at_head) {
                let c = left.remove(i);
                chain.insert(
                    0,
                    if close(c.0.end(), head) && c.1 == tag {
                        c.0
                    } else {
                        turned(&c).0
                    },
                );
            } else {
                break;
            }
        }
        out.push((chain, tag));
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
            }
            | Curve::Helix {
                radius, start, end, ..
            } => {
                let tol = 1.0 / FOOT_MM;
                let step = 2.0 * (1.0 - tol / radius.max(tol)).clamp(-1.0, 1.0).acos();
                (((end - start) / step.max(1e-3)).ceil() as usize).clamp(1, 256)
            }
            Curve::Ellipse { .. } => 64,
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

/// A profile swept along paths (mitcad#333, mitcad#340): the profile
/// type's loops (its xy plane: x across the path, y along `up`, moved by
/// the offsets) at right angles to the path, one solid along each chain of
/// the paths ([`along_chain`]). A wall sweep's profile lies on the side of
/// each path its step names (`m_side`: 1 mirrors its x) and below the
/// path when it is flipped (`m_flipped`: its y mirrored); of another
/// sweep's (a slab edge, a rail) four mirror images the one whose solid
/// fits the element's stored box best. A wall sweep's ends are mitred where
/// its returns turn and where it meets another sweep ([`with_returns`]).
/// The elements joined to it that cut
/// it (`CutPairGStep`: a coping where it meets another coping, mitcad#385)
/// cut it with their own solids (`with_cuts`; the solid alone is another
/// element's cutting tool).
fn sweep(model: &Model, e: &Element, s: &building::Sweep, with_cuts: bool) -> Result<Plan, String> {
    let b = &model.building;
    let symbol = s
        .profile
        .filter(|p| *p > 0)
        .or_else(|| s.type_id.and_then(|t| b.profile_types.get(&t).copied()))
        .ok_or("no profile")?;
    let profile = profile_loops(model, symbol)?;
    let tagged: Vec<(Curve, Option<bool>)> = match s.path3d {
        Some(p) => b
            .paths3d
            .get(&p)
            .ok_or("its path is not read")?
            .iter()
            .flatten()
            .map(|c| (c.clone(), None))
            .collect(),
        None => s
            .paths
            .iter()
            .enumerate()
            .flat_map(|(k, path)| {
                let side = s.sides.get(k).map(|side| *side == 1);
                path.iter().map(move |c| (c.clone(), side))
            })
            .collect(),
    };
    let all: Vec<(Curve, Option<bool>)> = tagged
        .into_iter()
        .filter(|(c, _)| c.length() > 1e-6)
        .collect();
    if let Some(why) = s.path3d.and_then(|p| b.paths3d_unread.get(&p)) {
        return Err(format!(
            "its path's curves do not all read ({})",
            why.join(", ")
        ));
    }
    let chains = chains(&all, 1e-4);
    if chains.is_empty() {
        return Err("no path".into());
    }
    let up = s.up.and_then(math::normalize).unwrap_or([0.0, 0.0, 1.0]);
    let (h, v) = (s.offsets[0], s.offsets[1]);
    let sided = chains.iter().all(|(_, side)| side.is_some());
    let mirrors: &[(f64, f64)] = if sided && s.flipped {
        &[(1.0, -1.0)]
    } else if sided {
        &[(1.0, 1.0)]
    } else {
        &[(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)]
    };
    // The paths' ends with returns or joined to other sweeps, by their
    // points (each once).
    // (A path may hold curves of no length away from it, and pieces of it
    // in any order, apart where an opening breaks it: its ends are the
    // farthest back and on along its first curve's direction.)
    let end_of = |path: &[Curve], end: u8| {
        let curves: Vec<&Curve> = path.iter().filter(|c| c.length() > 1e-6).collect();
        let d = math::sub(curves.first()?.end(), curves.first()?.start());
        let at = |p: P3| math::dot(p, d);
        if end == 0 {
            curves
                .iter()
                .map(|c| c.start())
                .min_by(|a, b| at(*a).total_cmp(&at(*b)))
        } else {
            curves
                .iter()
                .map(|c| c.end())
                .max_by(|a, b| at(*a).total_cmp(&at(*b)))
        }
    };
    let mut return_ends: Vec<(P3, EndAt)> = Vec::new();
    let mut add_end = |at: P3, what: EndAt| {
        if !return_ends
            .iter()
            .any(|(p, _)| math::length(math::sub(*p, at)) < 1e-4)
        {
            return_ends.push((at, what));
        }
    };
    // (A sweep joined there decides the end before a return.)
    for &(k, end, other) in &s.joined {
        let Some(at) = s.paths.get(k).and_then(|p| end_of(p, end)) else {
            continue;
        };
        // The other sweep's path leaving the point.
        let close = |q: P3| math::length(math::sub(q, at)) < 1e-4;
        // (Of its own paths, another one.)
        let away = b.sweeps.get(&other).and_then(|o| {
            o.paths
                .iter()
                .enumerate()
                .filter(|(j, _)| other != s.id || *j != k)
                .flat_map(|(_, p)| p)
                .find_map(|c| {
                    if c.length() <= 1e-6 {
                        None
                    } else if close(c.start()) {
                        math::normalize(math::sub(c.at(1e-3), c.start()))
                    } else if close(c.end()) {
                        math::normalize(math::sub(c.at(1.0 - 1e-3), c.end()))
                    } else {
                        None
                    }
                })
        });
        if let Some(away) = away {
            add_end(at, EndAt::Joined(away));
        }
    }
    // (An opening's returns before the wall end's at the same point.)
    let mut returns: Vec<&building::SweepReturn> = s.returns.iter().collect();
    returns.sort_by_key(|r| !r.at_opening);
    for r in returns {
        let Some(path) = s.paths.get(r.path) else {
            continue;
        };
        let mut points = if r.at_opening {
            opening_ends(path, r.end)
        } else {
            Vec::new()
        };
        if points.is_empty() {
            points.extend(end_of(path, r.end));
        }
        for at in points {
            add_end(
                at,
                EndAt::Return {
                    angle: r.angle,
                    length: r.length,
                },
            );
        }
    }
    let mut best: Option<(f64, Vec<Item>)> = None;
    for &(mx, my) in mirrors {
        let mut items = Vec::new();
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for (chain, side) in &chains {
            let mx = match side {
                Some(true) => -1.0,
                Some(false) => 1.0,
                None => mx,
            };
            // The profile in its own plane, mirrored and moved (feet).
            let flat = [mx, 0.0, 0.0, 0.0, my, 0.0, 0.0, 0.0, 1.0, h, v, 0.0];
            let profile2: Vec<Vec<Curve>> = profile
                .iter()
                .map(|l| l.iter().map(|c| c.transformed(&flat)).collect())
                .collect();
            let (chain, mitres) = with_returns(chain, &return_ends, &profile2, up);
            let chain = &joined_lines(chain);
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
            // Along a planar chain the sweep keeps the profile's x at its
            // angle to the plane, turning with the chain in it (the frames
            // its solid is measured in); along another, x stays square to up.
            let turning = chain_normal(chain).and_then(|n| {
                let t0 = math::normalize(math::sub(lines[0].1, lines[0].0))?;
                Some((n, math::dot(x0, n), math::dot(x0, math::cross(n, t0))))
            });
            let mut prisms = Vec::new();
            for (a, z) in &lines {
                let d = math::sub(*z, *a);
                let length = math::length(d);
                let t = math::scale(d, 1.0 / length);
                let x = match turning {
                    Some((n, p, q)) => math::normalize(math::add(
                        math::scale(n, p),
                        math::scale(math::cross(n, t), q),
                    )),
                    None => math::normalize(math::cross(up, t)),
                }
                .unwrap_or(last_x);
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
            let mut built = along_chain(chain, &profile2, x0, prisms)?;
            for (end, q, n, back) in mitres {
                let k = if end == 0 { 0 } else { built.len() - 1 };
                let mut main = Item::Boolean {
                    op: BoolOp::Difference,
                    first: Box::new(built[k].clone()),
                    second: Box::new(half_space(n, math::dot(n, q), false, None)),
                };
                if let Some(back) = back {
                    main = Item::Boolean {
                        op: BoolOp::Union,
                        first: Box::new(main),
                        second: Box::new(back),
                    };
                }
                built[k] = main;
            }
            items.extend(built);
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
    let (_, mut items) = best.expect("one candidate");
    let solid = items.remove(0);
    let mut notes = vec![format!(
        "its profile ({} loops) along {} paths",
        profile.len(),
        chains.len()
    )];
    let mut cuts = Vec::new();
    let pieces: Vec<&Item> = std::iter::once(&solid).chain(&items).collect();
    for other in s.cut_by.iter().filter(|_| with_cuts) {
        match cutting_solids(model, *other, &pieces, CUT_DEPTH) {
            Ok(tools) => cuts.extend(tools.into_iter().map(|t| (format!("element {other}"), t))),
            Err(why) => notes.push(format!("the cut by element {other} is not made: {why}")),
        }
    }
    Ok(Plan {
        kind: sweep_kind(s),
        solid,
        cuts,
        more: items,
        attached: Vec::new(),
        openings: Vec::new(),
        notes,
        material: None,
    })
}

/// An end of a chain cut at a mitre: the end (0, 1), its point, the
/// mitre's normal out of the chain's solid and the return's solid, if any.
type EndCut = (u8, P3, P3, Option<Item>);

/// A chain with what meets its ends (mitcad#385, `ends` by their points):
/// a wall sweep's return (`ReturnInfo`: the angle and the length) turns
/// from the path by its angle about `up`, away from the side the profile
/// (feet, its x across the path) lies on (around the wall's end), and runs
/// on for its length; another sweep leaves the end in its own direction.
/// The two meet in a mitre (the plane through the end square to the sum of
/// the path's and the other's directions). The chain runs on past its end
/// as far as the profile needs (to be cut at the mitre); per such end its
/// [`EndCut`], with a return's solid: the profile along the return from
/// behind the mitre to its length (none, or -1: the corner closed as a
/// return would close it), less the chain's side of the mitre.
fn with_returns(
    chain: &[Curve],
    ends: &[(P3, EndAt)],
    profile: &[Vec<Curve>],
    up: P3,
) -> (Vec<Curve>, Vec<EndCut>) {
    let close = |a: P3, b: P3| math::length(math::sub(a, b)) < 1e-4;
    let mut chain = chain.to_vec();
    let mut mitres = Vec::new();
    let (Some(first), Some(last)) = (chain.first(), chain.last()) else {
        return (chain, mitres);
    };
    let (head, tail) = (first.start(), last.end());
    if ends.is_empty() || close(head, tail) {
        return (chain, mitres);
    }
    let (xmin, xmax) = profile
        .iter()
        .flatten()
        .flat_map(|c| (0..=8).map(move |k| c.at(f64::from(k) / 8.0)[0]))
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
            (lo.min(x), hi.max(x))
        });
    if !xmin.is_finite() {
        return (chain, mitres);
    }
    let side = if xmin + xmax >= 0.0 { 1.0 } else { -1.0 };
    let reach = xmin.abs().max(xmax.abs());
    for (end, at) in [(0u8, head), (1u8, tail)] {
        let Some((_, what)) = ends.iter().find(|(p, _)| close(*p, at)) else {
            continue;
        };
        let (angle, length, joined) = match *what {
            EndAt::Return { angle, length } => (angle, length, None),
            EndAt::Joined(away) => (0.0, 0.0, Some(away)),
        };
        // (A negative length, -1, closes the corner as one of no length
        // does; of no length and no turn: the end as it is.)
        let length = length.max(0.0);
        if joined.is_none() && length < 1e-9 && angle.abs() < 1e-9 {
            continue;
        }
        let lines = chain_lines(&chain);
        let (Some(&(a, b)), Some(&(c, d))) = (lines.first(), lines.last()) else {
            continue;
        };
        let along = if end == 0 {
            math::normalize(math::sub(a, b))
        } else {
            math::normalize(math::sub(d, c))
        };
        let Some(out) = along else { continue };
        let Some(x) = math::normalize(math::cross(up, out)) else {
            continue;
        };
        // (`x` across the outward direction is the profile's x at a start
        // turned round: its side flips there.)
        let s = math::scale(x, if end == 0 { -side } else { side });
        let (r, turn) = match joined {
            // (Not back along itself.)
            Some(away) if math::dot(away, out) > -0.99 => {
                (away, math::dot(away, out).clamp(-1.0, 1.0).acos())
            }
            Some(_) => continue,
            None => {
                let turn = angle.abs();
                let r = math::add(math::scale(out, turn.cos()), math::scale(s, -turn.sin()));
                (r, turn)
            }
        };
        // (A sweep running straight on meets it square.)
        if joined.is_some() && turn < 1e-6 {
            continue;
        }
        let Some(n) = math::normalize(math::add(out, r)) else {
            continue;
        };
        // The path runs on past its end as far as the profile reaches past
        // the mitre.
        let run = reach * (turn / 2.0).tan() + reach;
        let on = math::add(at, math::scale(out, run));
        if end == 0 {
            chain.insert(0, Curve::Line { start: on, end: at });
        } else {
            chain.push(Curve::Line { start: at, end: on });
        }
        if joined.is_some() {
            // (The other sweep is the other side of the mitre.)
            mitres.push((end, at, n, None));
            continue;
        }
        // The return: the profile along it (in the direction the sweep runs)
        // from behind the mitre to its length, its side of the mitre kept.
        let (t, from) = if end == 0 {
            (math::scale(r, -1.0), math::add(at, math::scale(r, length)))
        } else {
            (r, math::sub(at, math::scale(r, run)))
        };
        let Some(x) = math::normalize(math::cross(up, t)) else {
            continue;
        };
        let y = math::cross(t, x);
        let m = [
            x[0], y[0], t[0], x[1], y[1], t[1], x[2], y[2], t[2], from[0], from[1], from[2],
        ];
        let placed: Vec<Vec<Curve>> = profile
            .iter()
            .map(|l| l.iter().map(|c| c.transformed(&m)).collect())
            .collect();
        let Ok(piece) = prism(&placed, t, run + length) else {
            continue;
        };
        let piece = Item::Boolean {
            op: BoolOp::Difference,
            first: Box::new(piece),
            second: Box::new(half_space(n, math::dot(n, at), true, None)),
        };
        mitres.push((end, at, n, Some(piece)));
    }
    (chain, mitres)
}

/// A chain with each run of lines that go straight on as one line
/// (mitcad#385): two walls in line give a wall sweep a joint that is no
/// corner, where the sweep would put an edge and a face of its own; a
/// corner's mitre that reaches back past that joint (along a piece as
/// short as the profile is wide) would leave that face with no area.
fn joined_lines(chain: Vec<Curve>) -> Vec<Curve> {
    let mut out: Vec<Curve> = Vec::with_capacity(chain.len());
    for c in chain {
        if let (Some(Curve::Line { start, end }), Curve::Line { start: s2, end: e2 }) =
            (out.last_mut(), &c)
        {
            let (a, b) = (math::sub(*end, *start), math::sub(*e2, *s2));
            let straight = math::length(math::sub(*end, *s2)) < 1e-9
                && math::dot(a, b) > 0.0
                && math::length(math::cross(a, b)) <= 1e-9 * math::length(a) * math::length(b);
            if straight {
                *end = *e2;
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Where openings break a wall sweep's path (mitcad#385): its pieces'
/// starts (`end` 0) or ends (1) with another piece before or after them
/// along it, apart from it; a piece of no length (the path's curves keep
/// one where an opening's other side lies) counts touching.
fn opening_ends(path: &[Curve], end: u8) -> Vec<P3> {
    let tol = 1e-4;
    let Some(first) = path.iter().find(|c| c.length() > 1e-6) else {
        return Vec::new();
    };
    let d = math::sub(first.end(), first.start());
    let Some(d) = math::normalize(d) else {
        return Vec::new();
    };
    let at = |p: P3| math::dot(p, d);
    let real: Vec<&Curve> = path.iter().filter(|c| c.length() > 1e-6).collect();
    let points: Vec<P3> = path
        .iter()
        .filter(|c| c.length() <= 1e-6)
        .map(Curve::start)
        .collect();
    real.iter()
        .enumerate()
        .filter_map(|(k, c)| {
            let p = if end == 0 { c.start() } else { c.end() };
            let other = real.iter().enumerate().any(|(j, q)| {
                j != k
                    && if end == 0 {
                        at(q.end()) < at(p) - tol
                    } else {
                        at(q.start()) > at(p) + tol
                    }
            });
            let point = points.iter().any(|q| {
                if end == 0 {
                    at(*q) <= at(p) + tol
                } else {
                    at(*q) >= at(p) - tol
                }
            });
            (other || point).then_some(p)
        })
        .collect()
}

/// What meets a wall sweep's path's end (mitcad#385): its return (the
/// angle it turns by, its length), or another sweep, leaving the end in a
/// direction.
#[derive(Debug, Clone, Copy)]
enum EndAt {
    Return { angle: f64, length: f64 },
    Joined(P3),
}

/// How many elements deep a cutting sweep's own cuts are made.
const CUT_DEPTH: usize = 8;

/// Whether two items' boxes meet.
fn boxes_meet(a: &Item, b: &Item) -> bool {
    match (
        crate::ifc::transform::bounds(a),
        crate::ifc::transform::bounds(b),
    ) {
        (Some((lo, hi)), Some((l, h))) => (0..3).all(|k| l[k] <= hi[k] && h[k] >= lo[k]),
        _ => true,
    }
}

/// The solids an element cuts the pieces of another with (mitcad#385): a
/// sweep's solids along its paths that reach one of the pieces (a cut by a
/// solid that misses would fail), each less the solids of the elements
/// that cut that sweep in turn (`depth` levels on: a coping cut by the
/// coping it meets cuts the next one with what is left of it); another
/// element's solid ([`tool`]).
fn cutting_solids(
    model: &Model,
    id: i64,
    pieces: &[&Item],
    depth: usize,
) -> Result<Vec<Item>, String> {
    let b = &model.building;
    let (Some(s), Some(e)) = (b.sweeps.get(&id), model.element(id)) else {
        return tool(model, id).map(|t| vec![t]);
    };
    let p = sweep(model, e, s, false)?;
    let mut out = Vec::new();
    for mut solid in std::iter::once(p.solid).chain(p.more) {
        if !pieces.iter().any(|piece| boxes_meet(piece, &solid)) {
            continue;
        }
        if depth > 0 {
            for other in &s.cut_by {
                // (A cut of the cutter that does not read is left out.)
                let Ok(tools) = cutting_solids(model, *other, &[&solid], depth - 1) else {
                    continue;
                };
                for t in tools {
                    solid = Item::Boolean {
                        op: BoolOp::Difference,
                        first: Box::new(solid),
                        second: Box::new(t),
                    };
                }
            }
        }
        out.push(solid);
    }
    Ok(out)
}

/// A curve as a piece of a path in model millimetres.
fn seg_mm(c: &Curve) -> Seg {
    match c {
        Curve::Line { start, end } => Seg::Line {
            start: mm(*start),
            end: mm(*end),
        },
        Curve::Arc {
            center,
            x_axis,
            y_axis,
            radius,
            start,
            end,
        } => Seg::Arc {
            frame: math::from_axes(mm(*center), *x_axis, *y_axis, math::cross(*x_axis, *y_axis)),
            radius: radius * FOOT_MM,
            start: *start,
            sweep: end - start,
        },
        Curve::Ellipse {
            center,
            x_axis,
            y_axis,
            rx,
            ry,
            start,
            end,
        } => Seg::Ellipse {
            frame: math::from_axes(mm(*center), *x_axis, *y_axis, math::cross(*x_axis, *y_axis)),
            a: rx * FOOT_MM,
            b: ry * FOOT_MM,
            start: *start,
            sweep: end - start,
        },
        // (mitcad#385) Its rise along `x × y`, the frame's z.
        Curve::Helix {
            center,
            x_axis,
            y_axis,
            z_axis,
            radius,
            rise,
            start,
            end,
        } => {
            let z = math::cross(*x_axis, *y_axis);
            Seg::Helix {
                frame: math::from_axes(mm(*center), *x_axis, *y_axis, z),
                radius: radius * FOOT_MM,
                pitch: rise * std::f64::consts::TAU * math::dot(*z_axis, z) * FOOT_MM,
                start: *start,
                sweep: end - start,
            }
        }
    }
}

/// A profile (mm) swept along a helix (a spiral rail, mitcad#385): its ends
/// cut by vertical planes through the helix's ends, square to it in plan,
/// the helix running on past them as far as the profile, `reach` (mm) from
/// it, can reach behind them. The profile's x lies across the helix,
/// square to up.
fn helical_rail(helix: Seg, profile: Profile, reach: f64) -> Result<Vec<Item>, String> {
    let Seg::Helix {
        frame,
        radius,
        pitch,
        start,
        sweep,
    } = helix
    else {
        return Err("a path that is not a helix".into());
    };
    let up = [0.0, 0.0, 1.0];
    let rise = pitch / std::f64::consts::TAU;
    let speed = radius.hypot(rise);
    let sign = sweep.signum();
    let at = |t: f64| math::apply(&frame, [radius * t.cos(), radius * t.sin(), rise * t]);
    let along = |t: f64| {
        math::scale(
            math::apply_vector(&frame, [-radius * t.sin(), radius * t.cos(), rise]),
            sign / speed,
        )
    };
    let level = |v: P3| math::normalize([v[0], v[1], 0.0]).ok_or("a helix that runs up its axis");
    let (t0, t1) = (start, start + sweep);
    let (n0, n1) = (level(along(t0))?, level(along(t1))?);
    let slope = math::dot(along(t0), up).abs().min(1.0 - 1e-9);
    let more = (reach * slope / (1.0 - slope * slope).sqrt() + 1.0) / speed;
    let s0 = start - sign * more;
    let x = math::normalize(math::cross(up, along(s0))).ok_or("a helix that runs up its axis")?;
    let swept = Item::Sweep {
        position: math::IDENTITY,
        profile,
        directrix: Path {
            segments: vec![Seg::Helix {
                frame,
                radius,
                pitch,
                start: s0,
                sweep: sweep + 2.0 * sign * more,
            }],
        },
        reference: Some(x),
    };
    // Each cut only near its end, within a bar of the profile's reach about
    // it, square to the plane: elsewhere the plane meets the helix's other
    // turns.
    let (wide, high) = (reach + 10.0, reach / (1.0 - slope * slope).sqrt() + 10.0);
    let cut = |solid: Item, n: P3, p: P3, below: bool| {
        let across = math::normalize(math::cross(up, n)).unwrap_or([1.0, 0.0, 0.0]);
        let plane = math::from_axes(p, across, up, n);
        Item::Boolean {
            op: BoolOp::Difference,
            first: Box::new(solid),
            second: Box::new(Item::HalfSpace {
                plane,
                below,
                boundary: Some((
                    plane,
                    polygon(&[[-wide, -high], [wide, -high], [wide, high], [-wide, high]]),
                )),
            }),
        }
    };
    Ok(vec![cut(cut(swept, n0, at(t0), true), n1, at(t1), false)])
}

/// The helices of a chain as one helix (mitcad#385): each continuing the
/// one before it about the same axis, at the same radius and pitch (a
/// spiral rail's path, kept in pieces), within 0.01 mm.
fn one_helix(chain: &[Curve]) -> Result<Seg, String> {
    let segs: Vec<Seg> = chain.iter().map(seg_mm).collect();
    let Some(Seg::Helix {
        frame,
        radius,
        pitch,
        start,
        sweep,
    }) = segs.first().cloned()
    else {
        return Err("a path that is not a helix".into());
    };
    let tau = std::f64::consts::TAU;
    let (first, mut last) = (start, start + sweep);
    let o = math::origin(&frame);
    let (x, y, z) = (
        math::column(&frame, 0),
        math::column(&frame, 1),
        math::column(&frame, 2),
    );
    let tolerance = 0.01;
    for s in &segs[1..] {
        let Seg::Helix {
            frame: f,
            radius: r,
            pitch: p,
            start: s0,
            sweep: sw,
        } = s
        else {
            return Err("a path of helices and other curves".into());
        };
        // Its angles in the first one's frame: turned by the angle of its x
        // axis there (the other way round about a reversed axis), and by
        // the whole turns its height on the axis gives. (A helix keeps its
        // rise per turn about either way of its axis.)
        let (fz, fo) = (math::column(f, 2), math::origin(f));
        let same_way = math::dot(fz, z);
        let height = math::dot(math::sub(fo, o), z);
        let off_axis = math::length(math::sub(math::sub(fo, o), math::scale(z, height)));
        if (same_way.abs() - 1.0).abs() > 1e-9
            || off_axis > tolerance
            || (r - radius).abs() > tolerance
        {
            return Err("a path of helices of other axes or radii".into());
        }
        if (p - pitch).abs() > tolerance {
            return Err("a path of helices of other pitches".into());
        }
        let fx = math::column(f, 0);
        let phi = math::dot(fx, y).atan2(math::dot(fx, x));
        let k = ((height * tau / pitch - phi) / tau).round();
        let to_first = |t: f64| phi + same_way.signum() * t + k * tau;
        let (a, b) = (to_first(*s0), to_first(s0 + sw));
        let at = |t: f64| {
            math::add(
                math::add(o, math::scale(x, radius * t.cos())),
                math::add(
                    math::scale(y, radius * t.sin()),
                    math::scale(z, pitch * t / tau),
                ),
            )
        };
        if math::length(math::sub(at(a), s.start())) > tolerance
            || math::length(math::sub(at(b), s.end())) > tolerance
            || (a - last).abs() * radius > tolerance
            || (b - a) * sweep <= 0.0
        {
            return Err("a path of helices that do not continue each other".into());
        }
        last = b;
    }
    Ok(Seg::Helix {
        frame,
        radius,
        pitch,
        start: first,
        sweep: last - first,
    })
}

/// A loop of the two halves of one ellipse (an elliptical handrail's
/// profile) as the whole ellipse: one closed edge to sweep, no seams.
fn whole_ellipse(path: Path) -> Path {
    if let [
        Seg::Ellipse {
            frame,
            a,
            b,
            start,
            sweep,
        },
        Seg::Ellipse {
            frame: f2,
            a: a2,
            b: b2,
            sweep: s2,
            ..
        },
    ] = path.segments.as_slice()
        && frame == f2
        && a == a2
        && b == b2
        && ((sweep + s2).abs() - std::f64::consts::TAU).abs() < 1e-9
    {
        return Path {
            segments: vec![Seg::Ellipse {
                frame: *frame,
                a: *a,
                b: *b,
                start: *start,
                sweep: sweep + s2,
            }],
        };
    }
    path
}

/// The normal of the plane a chain lies in, if it lies in one and is not
/// straight.
fn chain_normal(chain: &[Curve]) -> Option<P3> {
    let path = Path {
        segments: chain.iter().map(seg_mm).collect(),
    };
    let size = path
        .segments
        .iter()
        .flat_map(|s| s.sample(2))
        .map(math::length)
        .fold(1.0, f64::max);
    path.plane((size * 1e-9).max(1e-6))
        .map(|p| math::column(&p, 2))
}

/// One solid of a profile along a chain of curves (mitcad#340): a straight
/// chain as one extrusion of its whole length, a planar one (lines and
/// arcs) as one sweep of the profile (its x along `x0`, across the
/// chain's start square to its up direction, its y up), a chain that is not planar (a rail
/// up a stair) as an extrusion along each of its lines (`prisms`: the
/// profile placed at each line's start, its direction and length).
fn along_chain(
    chain: &[Curve],
    profile: &[Vec<Curve>],
    x0: P3,
    prisms: Vec<(Vec<Vec<Curve>>, P3, f64)>,
) -> Result<Vec<Item>, String> {
    // A chain of helices (a spiral rail, mitcad#385): one sweep along the
    // helix they make.
    if chain.iter().any(|c| matches!(c, Curve::Helix { .. })) {
        let paths: Vec<Path> = profile
            .iter()
            .map(|l| {
                loop_path(l, 1e-4)
                    .map(whole_ellipse)
                    .ok_or("a profile loop that does not close")
            })
            .collect::<Result<_, _>>()?;
        let reach = profile
            .iter()
            .flatten()
            .flat_map(|c| (0..=16).map(move |k| c.at(f64::from(k) / 16.0)))
            .map(|p| math::length(p) * FOOT_MM)
            .fold(0.0, f64::max);
        return helical_rail(one_helix(chain)?, profile_mm(areas(paths)), reach);
    }
    // A closed chain starts in the middle of its first line, not at a
    // corner, where the profile would stand across a mitre.
    let mut segments: Vec<Seg> = chain.iter().map(seg_mm).collect();
    let closed = math::length(math::sub(chain[0].start(), chain[chain.len() - 1].end())) < 1e-4;
    if let (true, Some(Seg::Line { start, end })) = (closed && chain.len() > 1, segments.first()) {
        let (start, end) = (*start, *end);
        let middle = math::scale(math::add(start, end), 0.5);
        segments[0] = Seg::Line { start: middle, end };
        segments.push(Seg::Line { start, end: middle });
    }
    let directrix = Path { segments };
    let first = prisms.first().ok_or("a path of no length")?;
    let straight = chain.iter().all(|c| matches!(c, Curve::Line { .. }))
        && prisms
            .iter()
            .all(|(_, t, _)| math::length(math::cross(*t, first.1)) < 1e-9);
    if straight {
        let start = chain[0].start();
        let end = chain[chain.len() - 1].end();
        return Ok(vec![prism(
            &first.0,
            first.1,
            math::length(math::sub(end, start)),
        )?]);
    }
    let size = directrix
        .segments
        .iter()
        .flat_map(|s| s.sample(2))
        .map(math::length)
        .fold(1.0, f64::max);
    if directrix.plane((size * 1e-9).max(1e-6)).is_none() {
        return prisms
            .into_iter()
            .map(|(placed, t, length)| prism(&placed, t, length))
            .collect();
    }
    let paths: Vec<Path> = profile
        .iter()
        .map(|l| {
            loop_path(l, 1e-4)
                .map(whole_ellipse)
                .ok_or("a profile loop that does not close")
        })
        .collect::<Result<_, _>>()?;
    Ok(vec![Item::Sweep {
        position: math::IDENTITY,
        profile: profile_mm(areas(paths)),
        directrix,
        reference: Some(x0),
    }])
}

/// A profile family type's closed loops (feet, the family's xy plane), the
/// largest first (mitcad#333).
fn profile_loops(model: &Model, symbol: i64) -> Result<Vec<Vec<Curve>>, String> {
    // The type's own profile (its dimensions), else its family's document's
    // curves (the family's own dimensions, mitcad#340).
    let own: Vec<Curve> = model
        .building
        .symbols
        .get(&symbol)
        .map(|s| s.profile_loops.concat())
        .unwrap_or_default();
    let curves = if own.is_empty() {
        let family = model
            .element(symbol)
            .and_then(|p| p.family_id)
            .ok_or("its profile's family is not read")?;
        model
            .profiles
            .get(&family)
            .ok_or("its profile family's document gives no profile")?
    } else {
        &own
    };
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
        attached: Vec::new(),
        openings: Vec::new(),
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

    /// A wall's solid taken apart: its prism's bottom and depth (mm) and
    /// the half spaces cut from it (their planes, mm, and whether below).
    fn prism_and_cuts(item: &Item) -> (f64, f64, Vec<(Matrix, bool)>) {
        match item {
            Item::Extrusion {
                position, depth, ..
            } => (math::origin(position)[2], *depth, Vec::new()),
            Item::Boolean {
                op: BoolOp::Difference,
                first,
                second,
            } => {
                let (z, d, mut cuts) = prism_and_cuts(first);
                match second.as_ref() {
                    Item::HalfSpace { plane, below, .. } => cuts.push((*plane, *below)),
                    other => panic!("{other:?}"),
                }
                (z, d, cuts)
            }
            other => panic!("{other:?}"),
        }
    }

    /// The top (feet) a wall's cuts leave at a point: the lowest of the
    /// planes whose upper side is cut away.
    fn top_at(cuts: &[(Matrix, bool)], x: f64, y: f64) -> f64 {
        cuts.iter()
            .filter(|(_, below)| !below)
            .map(|(plane, _)| {
                let n = math::column(plane, 2);
                let o = math::dot(n, math::origin(plane));
                (o - n[0] * x * FOOT_MM - n[1] * y * FOOT_MM) / n[2] / FOOT_MM
            })
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn a_wall_attached_to_a_gable_roof_ends_under_both_slopes() {
        // mitcad#348: the house's gable wall runs from its base to above
        // the ridge and loses what lies above either slope's lower surface:
        // its top rises from the eaves to the ridge under both slopes.
        use mitcad_rvt::testfile::building as t;
        let file = mitcad_rvt::RvtFile::parse(t::house()).unwrap();
        let schema = file.schema().unwrap();
        let model = Model::read(&file, &schema).unwrap();
        let wall = model.element(t::WALL_C).unwrap();
        let p = plan(&model, wall).unwrap();
        assert!(
            p.notes
                .iter()
                .any(|n| n == "its top follows roof 44's lower surface (2 slopes)"),
            "{:?}",
            p.notes
        );
        assert_eq!(p.attached, vec!["its top attached to 44"]);
        let (bottom, depth, cuts) = prism_and_cuts(&p.solid);
        assert!(bottom.abs() < 1e-9, "{bottom}");
        assert!(depth > (t::HEIGHT + 6.0 * t::ROOF_PITCH) * FOOT_MM);
        assert_eq!(cuts.len(), 2);
        // Under the first slope, the second, and at the ridge.
        let rise = t::ROOF_PITCH;
        for (y, top) in [
            (0.0, t::HEIGHT),
            (3.0, t::HEIGHT + 3.0 * rise),
            (6.0, t::HEIGHT + 6.0 * rise),
            (9.0, t::HEIGHT + 3.0 * rise),
            (12.0, t::HEIGHT),
        ] {
            let at = top_at(&cuts, 20.0, y);
            assert!((at - top).abs() < 1e-6, "y {y}: {at} (not {top})");
        }
        // The walls that are not attached keep their heights.
        let a = plan(&model, model.element(t::WALL_A).unwrap()).unwrap();
        assert!(a.attached.is_empty());
        let (bottom, depth, cuts) = prism_and_cuts(&a.solid);
        assert!(cuts.is_empty() && bottom.abs() < 1e-9);
        assert!((depth - t::HEIGHT * FOOT_MM).abs() < 1e-6);
    }

    #[test]
    fn an_attachment_that_is_not_applied_says_why() {
        // mitcad#348: a top attached to what the import does not read (a
        // face roof, an element missing) stays at the wall's own height,
        // with the attachment named.
        use mitcad_rvt::testfile::building as t;
        let file = mitcad_rvt::RvtFile::parse(t::house()).unwrap();
        let schema = file.schema().unwrap();
        let mut model = Model::read(&file, &schema).unwrap();
        model
            .building
            .walls
            .get_mut(&t::WALL_C)
            .unwrap()
            .attachments[0]
            .elements = vec![999];
        let p = plan(&model, model.element(t::WALL_C).unwrap()).unwrap();
        assert!(
            p.notes.iter().any(|n| n
                == "its top's attachment to 999 is not applied: element 999 is no roof, floor, \
                    ceiling or reference plane read"),
            "{:?}",
            p.notes
        );
        assert_eq!(p.attached, vec!["its top attached to 999"]);
        let (_, depth, cuts) = prism_and_cuts(&p.solid);
        assert!(cuts.is_empty());
        assert!((depth - t::HEIGHT * FOOT_MM).abs() < 1e-6);
    }

    /// The house's model (mitcad#345, mitcad#348).
    fn house() -> Model {
        use mitcad_rvt::testfile::building as t;
        let file = mitcad_rvt::RvtFile::parse(t::house()).unwrap();
        let schema = file.schema().unwrap();
        Model::read(&file, &schema).unwrap()
    }

    /// A cut's box (feet).
    fn tool_box(item: &Item) -> ([f64; 3], [f64; 3]) {
        let (lo, hi) = crate::ifc::transform::bounds(item).unwrap();
        (lo.map(|v| v / FOOT_MM), hi.map(|v| v / FOOT_MM))
    }

    #[test]
    fn a_door_cuts_its_opening_through_its_wall() {
        // mitcad#345: the house's door, its type's cut loop at its
        // transform, through the gable wall across it.
        use mitcad_rvt::testfile::building as t;
        let model = house();
        let p = plan(&model, model.element(t::WALL_C).unwrap()).unwrap();
        assert_eq!(
            p.openings,
            vec![WallOpening {
                id: t::DOOR,
                label: format!("insert {}", t::DOOR),
                missing: None,
            }]
        );
        let door: Vec<&Item> = p
            .cuts
            .iter()
            .filter(|(what, _)| *what == format!("insert {}", t::DOOR))
            .map(|(_, tool)| tool)
            .collect();
        assert_eq!(door.len(), 1);
        let (lo, hi) = tool_box(door[0]);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(near(lo[1], 1.5) && near(hi[1], 4.5), "{lo:?} {hi:?}");
        assert!(
            near(lo[2], 0.0) && near(hi[2], t::DOOR_HEIGHT),
            "{lo:?} {hi:?}"
        );
        // Through the wall's width (x from 19.75 to 20.25) and beyond.
        assert!(lo[0] < 19.75 && hi[0] > 20.25, "{lo:?} {hi:?}");
    }

    #[test]
    fn an_insert_type_without_cut_loops_cuts_by_its_void_geometry_or_says_why() {
        // mitcad#345: the house's window type has neither cut loops nor
        // void geometry: wall B lists it with the reason. Given a void (a
        // unit cube in the type's coordinates), the window cuts by it.
        use mitcad_rvt::testfile::building as t;
        let mut model = house();
        let wall = model.element(t::WALL_B).unwrap().clone();
        let p = plan(&model, &wall).unwrap();
        let why = "its type has neither cut loops nor void geometry";
        assert_eq!(
            p.openings,
            vec![WallOpening {
                id: t::WINDOW,
                label: format!("insert {}", t::WINDOW),
                missing: Some(why.into()),
            }]
        );
        assert!(
            p.notes
                .iter()
                .any(|n| *n == format!("insert {} is not cut: {why}", t::WINDOW)),
            "{:?}",
            p.notes
        );
        assert!(p.cuts.is_empty());
        model
            .building
            .symbols
            .get_mut(&t::WINDOW_TYPE)
            .unwrap()
            .cut_solids = vec![crate::rvt::geometry::tests::cube()];
        let p = plan(&model, &wall).unwrap();
        assert_eq!(p.openings[0].missing, None);
        assert!(
            p.notes
                .iter()
                .any(|n| *n == format!("insert {} is cut by its type's void geometry", t::WINDOW)),
            "{:?}",
            p.notes
        );
        let [(_, tool)] = p.cuts.as_slice() else {
            panic!("{:?}", p.cuts);
        };
        match tool {
            // (Its six planes as they are.)
            Item::Faceted { faces, closed } => assert!(*closed && faces.len() == 6),
            other => panic!("{other:?}"),
        }
        // The cube at the window's transform: x from -1 to 0 (its y turned
        // onto -x), y from 6 to 7, z from 3 to 4.
        let pts: Vec<[f64; 3]> = match tool {
            Item::Faceted { faces, .. } => faces.iter().flatten().flatten().copied().collect(),
            _ => unreachable!(),
        };
        let lo: Vec<f64> = (0..3)
            .map(|k| pts.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min) / FOOT_MM)
            .collect();
        let hi: Vec<f64> = (0..3)
            .map(|k| pts.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max) / FOOT_MM)
            .collect();
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(near(lo[0], -1.0) && near(hi[0], 0.0), "{lo:?} {hi:?}");
        assert!(near(lo[1], 6.0) && near(hi[1], 7.0), "{lo:?} {hi:?}");
        assert!(near(lo[2], 3.0) && near(hi[2], 4.0), "{lo:?} {hi:?}");
    }

    #[test]
    fn a_rectangular_wall_opening_cuts_through_its_wall() {
        // mitcad#345: an opening element's rectangle between its corners.
        use mitcad_rvt::testfile::building as t;
        let mut model = house();
        model.building.openings.insert(
            77,
            building::Opening {
                id: 77,
                class: "SWallRectOpening".into(),
                host: Some(t::WALL_A),
                loops: Vec::new(),
                corners: Some([[8.0, 0.0, 4.0], [5.0, 0.0, 1.0]]),
                base: None,
                top: None,
            },
        );
        let p = plan(&model, model.element(t::WALL_A).unwrap()).unwrap();
        assert_eq!(
            p.openings,
            vec![WallOpening {
                id: 77,
                label: "opening 77".into(),
                missing: None,
            }]
        );
        let (lo, hi) = tool_box(&p.cuts[0].1);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(near(lo[0], 5.0) && near(hi[0], 8.0), "{lo:?} {hi:?}");
        assert!(near(lo[2], 1.0) && near(hi[2], 4.0), "{lo:?} {hi:?}");
        assert!(lo[1] < -0.25 && hi[1] > 0.25, "{lo:?} {hi:?}");
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
            legs: [0.0; 2],
            corner_angles: [0.0; 2],
            trims: Vec::new(),
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
    fn a_trimmed_mullion_s_end_cut_along_its_stored_end_face() {
        use mitcad_rvt::brep::{Face, GRep, Solid, Surface};
        // (mitcad#385) A border mullion (x from -0.2 to 0) along -z, its
        // start trimmed to a neighbour: its stored body's start face runs
        // through (0, 0, -0.2) and (-0.2, 0, -0.4); its end is square at
        // the path's end; a face without loops (the untrimmed start's
        // plane) and the sides bound nothing at its ends.
        let mut m = rect_mullion([0.1, 0.1], 4);
        m.trims.push(building::MullionTrim {
            end: 0,
            path: line([-1.0, 0.0, 0.03], [0.0, 0.0, 0.0]),
            flags: 3,
            neighbour: Some(5),
            normal: [0.0; 3],
        });
        let (mut model, e) = with_symbol(Symbol {
            id: 1,
            mullion: Some(m),
            ..Symbol::default()
        });
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let face = |origin: P3, x: P3, y: P3, loops: bool| Face {
            surface: Surface::Plane { origin, x, y },
            loops: if loops {
                vec![vec![(0, 0)]]
            } else {
                Vec::new()
            },
            material: None,
        };
        let solid = Solid {
            faces: vec![
                face([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], false),
                face([0.0, 0.0, -0.2], [s, 0.0, s], [0.0, 1.0, 0.0], true),
                face([0.0, 0.0, -8.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], true),
                face([0.0; 3], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], true),
            ],
            edges: Vec::new(),
        };
        model.geometry.insert(
            1,
            GRep {
                solids: vec![solid],
                ..GRep::default()
            },
        );
        let p = plan(&model, &e).unwrap();
        assert!(p.notes.iter().any(|n| n.contains("stored end faces")));
        let (z, depth, cuts) = prism_and_cuts(&p.solid);
        // The prism reaches its widest side (0.4) past the cut start.
        assert!((z - 0.4 * FOOT_MM).abs() < 1e-9, "{z}");
        assert!((depth - 8.4 * FOOT_MM).abs() < 1e-9, "{depth}");
        let [(plane, below)] = cuts.as_slice() else {
            panic!("{cuts:?}")
        };
        assert!(!below);
        // Its normal points out of the mullion, up and to -x.
        let n = math::column(plane, 2);
        assert!((n[0] + s).abs() < 1e-9 && (n[2] - s).abs() < 1e-9, "{n:?}");
        // (Moved 10 ft along x by the instance.)
        let o = math::origin(plane);
        let at = [10.0 * FOOT_MM, 0.0, -0.2 * FOOT_MM];
        assert!(math::dot(math::sub(o, at), n).abs() < 1e-6);
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
            spiral: false,
            layout: None,
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
                ..StairParts::default()
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

    /// A profile type (71, of family 70) whose family's document draws a
    /// 0.2 × 0.1 rectangle while the type's own loops are 0.4 × 0.1.
    fn profile_type(model: &mut Model) {
        let rect = |w: f64| {
            polyline(&[
                [0.0, 0.0, 0.0],
                [w, 0.0, 0.0],
                [w, 0.1, 0.0],
                [0.0, 0.1, 0.0],
            ])
        };
        model.profiles.insert(70, rect(0.2));
        model.elements.push(Element {
            id: 71,
            family_id: Some(70),
            is_type: true,
            ..Element::default()
        });
        model.building.symbols.insert(
            71,
            Symbol {
                id: 71,
                profile_loops: vec![rect(0.4)],
                ..Symbol::default()
            },
        );
    }

    fn sweep_of(paths: Vec<Vec<Curve>>, sides: Vec<u32>, flipped: bool) -> building::Sweep {
        building::Sweep {
            id: 72,
            class: if sides.is_empty() {
                "SweepHost"
            } else {
                "WallSweep"
            }
            .into(),
            paths,
            sides,
            path3d: None,
            profile: Some(71),
            type_id: None,
            up: Some([0.0, 0.0, 1.0]),
            offsets: [0.0, 0.0],
            flipped,
            host: None,
            cut_by: Vec::new(),
            returns: Vec::new(),
            joined: Vec::new(),
        }
    }

    fn line(start: P3, end: P3) -> Curve {
        Curve::Line { start, end }
    }

    #[test]
    fn a_profile_along_a_bent_path() {
        // The type's 0.4 × 0.1 profile along an L of two lines: one sweep
        // (mitcad#340), the profile's x across the first line, the mirror
        // image that fits the stored box.
        let mut model = Model::default();
        profile_type(&mut model);
        let s = sweep_of(
            vec![vec![
                line([0.0, 0.0, 0.0], [5.0, 0.0, 0.0]),
                line([5.0, 0.0, 0.0], [5.0, 5.0, 0.0]),
            ]],
            Vec::new(),
            false,
        );
        model.building.sweeps.insert(72, s);
        let e = Element {
            id: 72,
            bbox: Some([-0.4, -0.4, 0.0, 5.4, 5.0, 0.1]),
            ..Element::default()
        };
        assert_eq!(kind_of(&model, &e), Some("sweep"));
        let p = plan(&model, &e).unwrap();
        assert!(p.more.is_empty());
        let Item::Sweep {
            profile,
            directrix,
            reference,
            ..
        } = &p.solid
        else {
            panic!("{:?}", p.solid);
        };
        assert_eq!(directrix.segments.len(), 2);
        let r = reference.unwrap();
        assert!(
            r[0].abs() < 1e-9 && (r[1].abs() - 1.0).abs() < 1e-9,
            "{r:?}"
        );
        let pts: Vec<[f64; 3]> = profile.areas[0]
            .outer
            .segments
            .iter()
            .map(|s| s.start())
            .collect();
        let width = pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max)
            - pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        assert!((width - 0.4 * FOOT_MM).abs() < 1e-6, "{width}");
    }

    #[test]
    fn a_wall_sweep_lies_on_its_steps_sides() {
        // Two straight paths along +x, one on side 0 (the profile's x to
        // the left, +y), one on side 1 (mirrored, -y), flipped: below.
        let mut model = Model::default();
        profile_type(&mut model);
        let s = sweep_of(
            vec![
                vec![line([0.0, 0.0, 1.0], [5.0, 0.0, 1.0])],
                vec![line([0.0, 10.0, 1.0], [5.0, 10.0, 1.0])],
            ],
            vec![0, 1],
            true,
        );
        model.building.sweeps.insert(72, s);
        let e = Element {
            id: 72,
            ..Element::default()
        };
        let p = plan(&model, &e).unwrap();
        assert_eq!(p.more.len(), 1);
        let ft = |v: f64| v / FOOT_MM;
        let (lo, hi) = crate::ifc::transform::bounds(&p.solid).unwrap();
        assert!(ft(lo[1]).abs() < 1e-9 && (ft(hi[1]) - 0.4).abs() < 1e-9);
        assert!((ft(lo[2]) - 0.9).abs() < 1e-9 && (ft(hi[2]) - 1.0).abs() < 1e-9);
        let (lo, hi) = crate::ifc::transform::bounds(&p.more[0]).unwrap();
        assert!((ft(lo[1]) - 9.6).abs() < 1e-9 && (ft(hi[1]) - 10.0).abs() < 1e-9);
        assert!((ft(lo[2]) - 0.9).abs() < 1e-9);
    }

    #[test]
    fn a_wall_sweep_s_return_of_no_length_closes_its_corner() {
        // (mitcad#385) A path along +x, the profile (0.4 wide) on its left
        // (+y); a return at its end of a quarter turn and no length turns
        // away from the profile (-y): the sweep runs on 0.8 past the end and
        // is cut at the mitre x - 5 = y, the return's piece (along -y, from
        // 0.8 behind the end) keeps the other side.
        let mut model = Model::default();
        profile_type(&mut model);
        let mut s = sweep_of(
            vec![vec![line([0.0, 0.0, 1.0], [5.0, 0.0, 1.0])]],
            vec![0],
            false,
        );
        s.returns.push(building::SweepReturn {
            path: 0,
            end: 1,
            at_opening: false,
            angle: std::f64::consts::FRAC_PI_2,
            length: 0.0,
        });
        model.building.sweeps.insert(72, s);
        let e = Element {
            id: 72,
            ..Element::default()
        };
        let p = plan(&model, &e).unwrap();
        let Item::Boolean {
            op: BoolOp::Union,
            first,
            second,
        } = &p.solid
        else {
            panic!("{:?}", p.solid)
        };
        let ft = |v: f64| v / FOOT_MM;
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let (_, depth, cuts) = prism_and_cuts(first);
        assert!((ft(depth) - 5.8).abs() < 1e-9, "{depth}");
        let [(plane, false)] = cuts.as_slice() else {
            panic!("{cuts:?}")
        };
        let n = math::column(plane, 2);
        assert!((n[0] - s).abs() < 1e-9 && (n[1] + s).abs() < 1e-9, "{n:?}");
        let (_, depth, cuts) = prism_and_cuts(second);
        assert!((ft(depth) - 0.8).abs() < 1e-9, "{depth}");
        assert!(matches!(cuts.as_slice(), [(_, true)]), "{cuts:?}");
        let (lo, hi) = crate::ifc::transform::bounds(second).unwrap();
        assert!((ft(lo[0]) - 5.0).abs() < 1e-9 && (ft(hi[0]) - 5.4).abs() < 1e-9);
        assert!(ft(lo[1]).abs() < 1e-9 && (ft(hi[1]) - 0.8).abs() < 1e-9);
    }

    #[test]
    fn a_sweep_s_ends_meet_another_sweep_s_in_a_mitre() {
        // (mitcad#385) Two sweeps' paths meet at (5, 0) at right angles,
        // the first's end joined to the second: it is cut along the
        // bisector, no piece added.
        let mut model = Model::default();
        profile_type(&mut model);
        let mut s = sweep_of(
            vec![vec![line([0.0, 0.0, 1.0], [5.0, 0.0, 1.0])]],
            vec![0],
            false,
        );
        s.joined.push((0, 1, 73));
        model.building.sweeps.insert(72, s);
        let mut other = sweep_of(
            vec![vec![line([5.0, 0.0, 1.0], [5.0, 4.0, 1.0])]],
            vec![0],
            false,
        );
        other.id = 73;
        model.building.sweeps.insert(73, other);
        let e = Element {
            id: 72,
            ..Element::default()
        };
        let p = plan(&model, &e).unwrap();
        let (_, depth, cuts) = prism_and_cuts(&p.solid);
        assert!((depth / FOOT_MM - 5.8).abs() < 1e-9, "{depth}");
        let [(plane, false)] = cuts.as_slice() else {
            panic!("{cuts:?}")
        };
        // Out of the first along x + y.
        let n = math::column(plane, 2);
        let s = std::f64::consts::FRAC_1_SQRT_2;
        assert!((n[0] - s).abs() < 1e-9 && (n[1] - s).abs() < 1e-9, "{n:?}");
    }

    #[test]
    fn lines_straight_on_are_one_line_and_openings_break_paths() {
        let joined = joined_lines(vec![
            line([0.0; 3], [1.0, 0.0, 0.0]),
            line([1.0, 0.0, 0.0], [3.0, 0.0, 0.0]),
            line([3.0, 0.0, 0.0], [3.0, 1.0, 0.0]),
        ]);
        assert_eq!(
            joined,
            vec![
                line([0.0; 3], [3.0, 0.0, 0.0]),
                line([3.0, 0.0, 0.0], [3.0, 1.0, 0.0])
            ]
        );
        // Two pieces with a gap from 2 to 5, and a curve of no length at
        // 9 (an opening's far side past the second piece's end).
        let path = vec![
            line([5.0, 0.0, 0.0], [7.0, 0.0, 0.0]),
            line([0.0; 3], [2.0, 0.0, 0.0]),
            line([9.0, 0.0, 0.0], [9.0, 0.0, 0.0]),
        ];
        assert_eq!(opening_ends(&path, 0), vec![[5.0, 0.0, 0.0]]);
        assert_eq!(
            opening_ends(&path, 1),
            vec![[7.0, 0.0, 0.0], [2.0, 0.0, 0.0]]
        );
    }

    fn rect_mullion(widths: [f64; 2], flags: u64) -> MullionSym {
        MullionSym {
            profile: "SysRectMullionProfile".into(),
            depths: [0.0; 2],
            profile_id: None,
            widths,
            thickness: 0.4,
            offset: 0.0,
            path: Some(line([0.0, 0.0, 0.0], [0.0, 0.0, -8.0])),
            normal: [0.0, 1.0, 0.0],
            flags,
            legs: [0.0; 2],
            corner_angles: [0.0; 2],
            trims: Vec::new(),
        }
    }

    #[test]
    fn an_off_centre_mullion_s_widths() {
        // (-1007300 towards normal × path: here -x.)
        let (model, e) = with_symbol(Symbol {
            id: 1,
            mullion: Some(rect_mullion([0.1, 0.3], 0)),
            ..Symbol::default()
        });
        let p = plan(&model, &e).unwrap();
        let (lo, hi) = crate::ifc::transform::bounds(&p.solid).unwrap();
        assert!((lo[0] / FOOT_MM - 9.9).abs() < 1e-9, "{lo:?}");
        assert!((hi[0] / FOOT_MM - 10.3).abs() < 1e-9, "{hi:?}");
    }

    #[test]
    fn a_v_mullion_as_an_l() {
        // Legs of 5" and a thickness of 2.5" at right angles (the corner
        // angles π/2 and π from normal × path), as the geometry the file
        // keeps of such a mullion: x and y from -3.75" to 2.5".
        let mut m = rect_mullion([0.0; 2], 174);
        m.profile = "SysVMullionProfile".into();
        m.thickness = 2.5 / 12.0;
        m.legs = [5.0 / 12.0; 2];
        m.corner_angles = [std::f64::consts::FRAC_PI_2, std::f64::consts::PI];
        let (model, e) = with_symbol(Symbol {
            id: 1,
            mullion: Some(m),
            ..Symbol::default()
        });
        let p = plan(&model, &e).unwrap();
        let inch = FOOT_MM / 12.0;
        let (lo, hi) = crate::ifc::transform::bounds(&p.solid).unwrap();
        assert!(
            (lo[0] - 10.0 * FOOT_MM + 3.75 * inch).abs() < 1e-6,
            "{lo:?}"
        );
        assert!((hi[0] - 10.0 * FOOT_MM - 2.5 * inch).abs() < 1e-6, "{hi:?}");
        assert!((lo[1] + 3.75 * inch).abs() < 1e-6 && (hi[1] - 2.5 * inch).abs() < 1e-6);
        let Item::Extrusion { profile, .. } = &p.solid else {
            panic!("{:?}", p.solid)
        };
        assert_eq!(profile.areas[0].outer.segments.len(), 6);
        // Legs at another angle are not read.
        let mut m = rect_mullion([0.0; 2], 0);
        m.profile = "SysVMullionProfile".into();
        m.thickness = 0.2;
        m.legs = [0.4; 2];
        m.corner_angles = [0.0, 2.0];
        let (model, e) = with_symbol(Symbol {
            id: 1,
            mullion: Some(m),
            ..Symbol::default()
        });
        assert!(plan(&model, &e).unwrap_err().contains("right angles"));
    }

    #[test]
    fn a_monolithic_run_s_leaning_risers_and_underside() {
        // Three risers of 1 ft, two treads of 2 ft, a nosing of 0.25 ft and
        // a support 0.5 ft deep under the line through the risers' bases.
        let mut model = Model::default();
        let run = building::StairRun {
            id: 5,
            bottom: 0.0,
            top: 3.0,
            risers: 3,
            treads: vec![2.0, 2.0],
            start: Some(line([0.0, 0.0, 0.0], [0.0, 4.0, 0.0])),
            end: Some(line([4.0, 0.0, 0.0], [4.0, 4.0, 0.0])),
            // Along the nosings: (0, 1) to (4, 3).
            support: Some(line([0.0, 0.0, 1.0], [4.0, 0.0, 3.0])),
            spiral: false,
            layout: None,
        };
        model.building.stair_runs.insert(5, run);
        model.building.run_types.insert(9, (true, 0.5));
        model.building.stair_parts.insert(
            9,
            StairParts {
                monolithic: true,
                nosing: 0.25,
                ..StairParts::default()
            },
        );
        let e = Element {
            id: 5,
            type_id: Some(9),
            ..Element::default()
        };
        let p = plan(&model, &e).unwrap();
        let Item::Extrusion { profile, .. } = &p.solid else {
            panic!("{:?}", p.solid)
        };
        let pts: Vec<[f64; 2]> = profile.areas[0]
            .outer
            .segments
            .iter()
            .map(|s| s.start())
            .map(|q| [q[0] / FOOT_MM, q[1] / FOOT_MM])
            .collect();
        let has = |x: f64, y: f64| {
            pts.iter()
                .any(|q| (q[0] - x).abs() < 1e-9 && (q[1] - y).abs() < 1e-9)
        };
        // The first riser leans from (0.25, 0) to (0, 1); a tread reaches
        // the next riser's base; the run ends at 4 ft, 3 ft up.
        assert!(has(0.25, 0.0) && has(0.0, 1.0) && has(2.25, 1.0) && has(2.0, 2.0));
        assert!(has(4.0, 2.0) && has(4.0, 3.0), "{pts:?}");
        assert!(!pts.iter().any(|q| q[0] > 4.0 + 1e-9), "{pts:?}");
        // The underside: 0.5 × √1.25 below the line through (0.25, 0) of
        // slope 0.5, from 4 ft back to the floor.
        let drop = 0.5 * 1.25f64.sqrt();
        assert!(has(4.0, 3.75 * 0.5 - drop), "{pts:?}");
        assert!(has(0.25 + drop / 0.5, 0.0), "{pts:?}");
        // A spiral run is not built straight.
        let mut spiral = model.building.stair_runs[&5].clone();
        spiral.spiral = true;
        model.building.stair_runs.insert(5, spiral);
        assert!(plan(&model, &e).unwrap_err().contains("spiral"));
    }

    #[test]
    fn a_wall_embedded_in_another_cuts_its_extent_through_it() {
        // A curtain wall in a basic wall (`WallEmbedWallGStep`), its
        // location line a little off the host's.
        use mitcad_rvt::testfile::building as t;
        let mut model = house();
        let mut e = model.building.walls[&t::WALL_A].clone();
        e.id = 88;
        e.curve = Curve::Line {
            start: [2.0, 0.1, 0.5],
            end: [6.0, 0.1, 0.5],
        };
        e.base = Some(0.5);
        e.height = Some(2.0);
        e.cutouts.clear();
        model.building.walls.insert(88, e);
        model
            .building
            .walls
            .get_mut(&t::WALL_A)
            .unwrap()
            .cutouts
            .push(88);
        let p = plan(&model, model.element(t::WALL_A).unwrap()).unwrap();
        let o = p.openings.iter().find(|o| o.id == 88).unwrap();
        assert_eq!((o.label.as_str(), &o.missing), ("embedded wall 88", &None));
        let tool = &p.cuts.iter().find(|c| c.0 == "embedded wall 88").unwrap().1;
        let (lo, hi) = tool_box(tool);
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(near(lo[0], 2.0) && near(hi[0], 6.0), "{lo:?} {hi:?}");
        assert!(near(lo[2], 0.5) && near(hi[2], 2.5), "{lo:?} {hi:?}");
        // Through the host, about its location line.
        assert!(lo[1] < -t::WIDTH && hi[1] > t::WIDTH, "{lo:?} {hi:?}");
        assert!((lo[1] + hi[1]).abs() < 1e-6, "{lo:?} {hi:?}");
    }

    #[test]
    fn a_ceiling_runs_down_by_its_type_from_its_top() {
        // Its reference faces are its top and its core's: its finish
        // layers lie below.
        use mitcad_rvt::testfile::building as t;
        let model = house();
        let mut s = model.building.slabs[&t::FLOOR].clone();
        s.bottom = None;
        s.class = "Ceiling".into();
        assert!((below_core(&s, 10.0, 9.9, Some(0.4)) - 9.6).abs() < 1e-12);
        assert!((below_core(&s, 10.0, 9.5, Some(0.4)) - 9.5).abs() < 1e-12);
        s.class = "Floor".into();
        assert!((below_core(&s, 10.0, 9.9, Some(0.4)) - 9.9).abs() < 1e-12);
    }

    #[test]
    fn an_extrusion_roof_spans_the_side_its_box_lies_on() {
        use mitcad_rvt::testfile::building as t;
        let mut model = house();
        let mut s = model.building.slabs[&t::FLOOR].clone();
        // A profile along +y at x = 0, extents 1 and 10 square to it: from
        // x = -10 to 1 as drawn, from -1 to 10 when its box lies there.
        s.extrusion = Some([-1.0, 10.0]);
        s.loops = vec![vec![Curve::Line {
            start: [0.0, 0.0, 3.0],
            end: [0.0, 5.0, 4.0],
        }]];
        let span_x = |model: &Model, s: &Slab| {
            let a = extrusion_span(model, s).unwrap();
            let xs: Vec<f64> = a[0]
                .outer
                .segments
                .iter()
                .flat_map(|g| g.sample(1))
                .map(|p| p[0])
                .collect();
            (
                xs.iter().copied().fold(f64::INFINITY, f64::min),
                xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            )
        };
        let e = model
            .elements
            .iter_mut()
            .find(|e| e.id == t::FLOOR)
            .unwrap();
        e.bbox = Some([-10.0, 0.0, 3.0, 1.0, 5.0, 4.0]);
        let (lo, hi) = span_x(&model, &s);
        assert!(
            (lo + 10.0).abs() < 1e-9 && (hi - 1.0).abs() < 1e-9,
            "{lo} {hi}"
        );
        let e = model
            .elements
            .iter_mut()
            .find(|e| e.id == t::FLOOR)
            .unwrap();
        e.bbox = Some([-1.0, 0.0, 3.0, 10.0, 5.0, 4.0]);
        let (lo, hi) = span_x(&model, &s);
        assert!(
            (lo + 1.0).abs() < 1e-9 && (hi - 10.0).abs() < 1e-9,
            "{lo} {hi}"
        );
    }

    #[test]
    fn a_wall_under_its_roof_in_part_follows_it_there_only() {
        // The house's gable wall attached to a roof whose outline covers
        // half of it: within the outline the roof's slope cuts it (or it
        // runs on up to under the slope), elsewhere it keeps its own top.
        use mitcad_rvt::testfile::building as t;
        let mut model = house();
        let roof = model.building.slabs.get_mut(&44).unwrap();
        let z = roof.loops[0][0].start()[2];
        let corners = [[-1.0, -1.0], [21.0, -1.0], [21.0, 6.0], [-1.0, 6.0]];
        roof.loops = vec![
            (0..4)
                .map(|k| {
                    let (a, b) = (corners[k], corners[(k + 1) % 4]);
                    Curve::Line {
                        start: [a[0], a[1], z],
                        end: [b[0], b[1], z],
                    }
                })
                .collect(),
        ];
        let p = plan(&model, model.element(t::WALL_C).unwrap()).unwrap();
        assert!(
            p.notes.iter().any(|n| n
                == "its top follows roof 44's lower surface where it is over it, elsewhere its own top"),
            "{:?}",
            p.notes
        );
        // Its prism to its own top, carried on up within the outline to
        // under the slopes; no slope lies below its own top, so no half
        // space bounded to the outline cuts it.
        fn walk(item: &Item, unions: &mut usize, bounded: &mut usize) {
            if let Item::Boolean { op, first, second } = item {
                if *op == BoolOp::Union {
                    *unions += 1;
                }
                if let Item::HalfSpace {
                    boundary: Some(_), ..
                } = second.as_ref()
                {
                    *bounded += 1;
                }
                walk(first, unions, bounded);
                walk(second, unions, bounded);
            }
        }
        let (mut unions, mut bounded) = (0, 0);
        walk(&p.solid, &mut unions, &mut bounded);
        assert_eq!((unions, bounded), (1, 0), "{:?}", p.solid);
    }
}
