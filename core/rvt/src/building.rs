// SPDX-License-Identifier: MIT
//! The building elements' own parameters (mitcad#275): what the import
//! rebuilds walls, floors, roofs, columns, beams and their openings from,
//! with the grids and the linked models. Every value is a field the schema
//! names, read by its name; lengths are in model feet, angles in radians.
//!
//! - **Curves** (`GLine`: `m_origin`, `m_dirVec`, `m_endParams`; `GArc`:
//!   `m_center`, `m_radius`, `m_xVec`, `m_yVec`, `m_endParams`, the angles
//!   from the x vector towards the y vector).
//! - **Walls** (`VWall` and its classes): the location curve
//!   (`m_pCurveDriver` → `m_pCrv`), the base and the height (the location
//!   reference face `m_pRefFaces[0]`, a `Plane` whose envelope spans the
//!   curve's length and the wall's height from the plane's origin), the
//!   type (`m_WallAttributesId`, its `CompoundStructure` layers), the joins
//!   at the two ends (`m_controlJoinsSet`: per end, the joined walls in
//!   order; the first one extends to the far face of the others), the
//!   walls joining its sides (`m_sideJoins`), and the elements cutting it
//!   (`CutPairGStep.m_otherId` of its geometry steps).
//! - **Floors and roofs** (`Floor`, `Roof`, ...): the curve loops of their
//!   extrusion helper cell (`FloorExtrusionHelper`, `RoofExtrusionHelper`:
//!   `m_pCurveLoops`, at the top of the slab), the top and bottom
//!   (`m_top`, `m_bottom`) or a roof's base level and offset and its layer
//!   reference faces (`m_pRefLayerFaces`, the planes of its slopes).
//! - **Family instances**: the symbol and transform (phase 1), the host,
//!   the level references (`FamilyLevelRef`: the base and top levels with
//!   their offsets), the location curve of a curve-driven instance (a
//!   beam's `FamInstCurveDriver`: `m_pCrv`, `m_setbacks`), and the
//!   elements cutting it.
//! - **Family types** (`FamilySymbol`): the parameters (`m_pParams` →
//!   `NamedParam`: id, value), the cut loops an insert cuts its host with
//!   (`m_pCutLoops`, in the type's coordinates) and their direction
//!   (`m_cutDirs`), and the geometry's materials (`m_geomTag2MaterialId`).
//! - **Parameter names**: a family parameter's element
//!   (`ParamElemFamily`, ...) → `m_pParamDef` → `m_caption`.
//! - **Grids** (`Grid`): the name (`m_text`) and the vertical surface its
//!   line lies in (`m_pSurface`: a `Plane` whose envelope's u range is the
//!   line; a cylinder's for an arc).
//! - **Links** (`RvtLinkInstance`: the transform and the link type;
//!   `RvtLinkSymbol`: the linked file's name and stored path).

use std::collections::BTreeMap;

use serde::Serialize;

use crate::schema::Schema;
use crate::walk::{Object, Record, Value};

pub type P3 = [f64; 3];

/// A curve in model feet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Curve {
    Line {
        start: P3,
        end: P3,
    },
    /// A circular arc: `center + radius (cos t x_axis + sin t y_axis)`
    /// for t from `start` to `end` (`end > start`).
    Arc {
        center: P3,
        x_axis: P3,
        y_axis: P3,
        radius: f64,
        start: f64,
        end: f64,
    },
}

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: P3, s: f64) -> P3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// A point moved by a transform (the 3×3 matrix by rows, then the origin).
pub fn apply(t: &[f64; 12], p: P3) -> P3 {
    std::array::from_fn(|r| t[3 * r] * p[0] + t[3 * r + 1] * p[1] + t[3 * r + 2] * p[2] + t[9 + r])
}

/// A direction turned by a transform.
pub fn turn(t: &[f64; 12], v: P3) -> P3 {
    std::array::from_fn(|r| t[3 * r] * v[0] + t[3 * r + 1] * v[1] + t[3 * r + 2] * v[2])
}

impl Curve {
    /// The point at a fraction of the curve (0 its start, 1 its end).
    pub fn at(&self, f: f64) -> P3 {
        match self {
            Curve::Line { start, end } => {
                std::array::from_fn(|k| start[k] + (end[k] - start[k]) * f)
            }
            Curve::Arc {
                center,
                x_axis,
                y_axis,
                radius,
                start,
                end,
            } => {
                let t = start + (end - start) * f;
                add(
                    *center,
                    add(
                        scale(*x_axis, radius * t.cos()),
                        scale(*y_axis, radius * t.sin()),
                    ),
                )
            }
        }
    }

    pub fn start(&self) -> P3 {
        self.at(0.0)
    }

    pub fn end(&self) -> P3 {
        self.at(1.0)
    }

    pub fn length(&self) -> f64 {
        match self {
            Curve::Line { start, end } => ((end[0] - start[0]).powi(2)
                + (end[1] - start[1]).powi(2)
                + (end[2] - start[2]).powi(2))
            .sqrt(),
            Curve::Arc {
                radius, start, end, ..
            } => radius * (end - start),
        }
    }

    /// The curve moved by a transform (rigid).
    pub fn transformed(&self, t: &[f64; 12]) -> Curve {
        match self {
            Curve::Line { start, end } => Curve::Line {
                start: apply(t, *start),
                end: apply(t, *end),
            },
            Curve::Arc {
                center,
                x_axis,
                y_axis,
                radius,
                start,
                end,
            } => Curve::Arc {
                center: apply(t, *center),
                x_axis: turn(t, *x_axis),
                y_axis: turn(t, *y_axis),
                radius: *radius,
                start: *start,
                end: *end,
            },
        }
    }
}

/// A plane with the envelope of what lies in it: points `origin + u x +
/// v y` for u, v in the ranges.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plane {
    pub origin: P3,
    pub x: P3,
    pub y: P3,
    pub u: [f64; 2],
    pub v: [f64; 2],
}

/// Where an element's end joins: the element and its end (0 the start, 1
/// the end, 2 and 3 its sides).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct JoinedEnd {
    pub element: i64,
    pub end: u8,
}

/// A layer of a wall, floor or roof type, from the exterior (or the top).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Layer {
    pub width: f64,
    pub material: Option<i64>,
    pub function: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Wall {
    pub id: i64,
    pub class: String,
    pub type_id: Option<i64>,
    /// The location curve at the wall's base.
    pub curve: Curve,
    /// The base and the height (from the location reference face), feet.
    pub base: Option<f64>,
    pub height: Option<f64>,
    pub top_level: Option<i64>,
    /// Per end (start, end): the walls joined there, in the file's order
    /// (this wall among them), and the join's kind (`m_wallJoinType`).
    pub joins: [Vec<JoinedEnd>; 2],
    pub join_types: [u64; 2],
    /// Joins its ends must not make (`m_noJoinAtEnd`).
    pub no_join: [bool; 2],
    /// The wall ends joining its sides.
    pub side_joins: Vec<JoinedEnd>,
    pub flipped: bool,
    /// The location line (`m_wallKeyRef`) and its offset.
    pub key_ref: u64,
    pub key_offset: f64,
    /// Its geometry steps' classes, in order.
    pub steps: Vec<String>,
    /// The elements whose geometry cuts it (`CutPairGStep`).
    pub cut_by: Vec<i64>,
    /// The inserts (doors, windows, openings) whose cut it takes
    /// (`WallCutoutGStep.m_instId`).
    pub cutouts: Vec<i64>,
    /// Where its faces lie: the least and the greatest offset (feet, to
    /// the left of a line wall's location line) of its vertical reference
    /// faces (`m_pRefFaces`: the location line, the faces, the core's).
    pub faces: Option<[f64; 2]>,
    /// An edited profile: the loops of its extrusion helper cell
    /// (`VWallExtrusionHelper.m_pCurveLoops`), in the wall's vertical plane.
    pub profile: Vec<Vec<Curve>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Slab {
    pub id: i64,
    pub class: String,
    pub type_id: Option<i64>,
    /// The boundary loops at the slab's top (floors) or base (roofs).
    pub loops: Vec<Vec<Curve>>,
    /// Curves of the loops that are no lines or arcs.
    pub unread_curves: Vec<String>,
    pub top: Option<f64>,
    pub bottom: Option<f64>,
    /// A roof's base level and offset.
    pub base_level: Option<i64>,
    pub base_offset: Option<f64>,
    /// A roof's layer reference faces (planes of its slopes).
    pub layer_faces: Vec<Plane>,
    /// Its reference faces' planes (a pad's top and bottom).
    pub ref_faces: Vec<Plane>,
    /// An extrusion roof's extents along its profile plane's normal
    /// (`m_extrusionDepth`).
    pub extrusion: Option<[f64; 2]>,
    /// A shape edit that is in use (`SlabShapeEditGStep` not dormant).
    pub shape_edited: bool,
    pub steps: Vec<String>,
    pub cut_by: Vec<i64>,
    pub cutouts: Vec<i64>,
}

/// A family level reference: the level and the offset from it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct LevelRef {
    pub level: i64,
    pub offset: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Instance {
    pub id: i64,
    /// The symbol it places and its type.
    pub symbol: Option<i64>,
    pub master: Option<i64>,
    pub transform: Option<[f64; 12]>,
    pub host: Option<i64>,
    /// Its level references (`m_refs`' `FamilyLevelRef`s), in order: the
    /// base, then the top.
    pub levels: Vec<LevelRef>,
    /// A curve-driven instance's location curve and the setbacks at its
    /// ends.
    pub curve: Option<Curve>,
    pub setbacks: [f64; 2],
    /// `m_structType` (1 a beam, 2 a brace, 3 a column, ...).
    pub struct_type: u64,
    /// Whether it stands vertical (`m_bVertical`).
    pub vertical: bool,
    pub steps: Vec<String>,
    pub cut_by: Vec<i64>,
}

/// A parameter of a type.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Param {
    pub id: i64,
    pub value: f64,
    pub text: String,
    pub element: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Symbol {
    pub id: i64,
    pub params: Vec<Param>,
    /// The loops an insert cuts its host with, in the type's coordinates,
    /// and the directions it cuts in.
    pub cut_loops: Vec<Vec<Curve>>,
    pub cut_dirs: Vec<P3>,
    /// Its geometry's materials.
    pub materials: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Grid {
    pub id: i64,
    pub name: String,
    /// The grid's line or arc (at its surface's lowest height).
    pub curve: Option<Curve>,
    /// Why there is no curve.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Link {
    /// The link instance and its type.
    pub id: i64,
    pub symbol: Option<i64>,
    pub instance_name: String,
    pub transform: Option<[f64; 12]>,
    /// The linked file's name and the path the file stores.
    pub file: Option<String>,
    pub path: Option<String>,
}

/// A stair run (`StairsRun`): its steps from its bottom to its top, its
/// start and end riser lines (in plan, at the bottom), the depths of its
/// treads and the underside of its monolithic support
/// (`m_oSupportPathCurveLoops`, a sloped line at each side).
#[derive(Debug, Clone, Serialize)]
pub struct StairRun {
    pub id: i64,
    pub bottom: f64,
    pub top: f64,
    /// Risers (`m_topRiserIndex - m_baseRiserIndex`).
    pub risers: u64,
    pub treads: Vec<f64>,
    pub start: Option<Curve>,
    pub end: Option<Curve>,
    pub support: Option<Curve>,
}

/// An opening element (a wall or floor opening, a shaft).
#[derive(Debug, Clone, Serialize)]
pub struct Opening {
    pub id: i64,
    pub class: String,
    pub host: Option<i64>,
    /// Its boundary loops, model coordinates.
    pub loops: Vec<Vec<Curve>>,
    /// A rectangular wall opening's two corners.
    pub corners: Option<[P3; 2]>,
    /// A shaft's base and top.
    pub base: Option<f64>,
    pub top: Option<f64>,
}

/// What one record gives, by its class.
#[derive(Debug, Clone)]
pub(crate) enum Raw {
    Wall(Box<Wall>),
    Slab(Box<Slab>),
    Instance(Box<Instance>),
    Symbol(Box<Symbol>),
    Structure(Vec<Layer>),
    Grid(Grid),
    LinkInstance(Link),
    LinkSymbol {
        name: String,
        path: Option<String>,
    },
    ParamName(String),
    Opening(Box<Opening>),
    StairRun(Box<StairRun>),
    RunType(bool, f64),
    /// A building element of one of these kinds whose parameters did not
    /// read: why.
    Unread(String),
}

/// Everything read of the building elements.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Building {
    pub walls: BTreeMap<i64, Wall>,
    pub slabs: BTreeMap<i64, Slab>,
    pub instances: BTreeMap<i64, Instance>,
    pub symbols: BTreeMap<i64, Symbol>,
    /// The compound structures of wall, floor and roof types.
    pub structures: BTreeMap<i64, Vec<Layer>>,
    pub grids: Vec<Grid>,
    pub links: Vec<Link>,
    pub openings: BTreeMap<i64, Opening>,
    pub stair_runs: BTreeMap<i64, StairRun>,
    /// Stair run types: monolithic (`m_hasMonolithicSupport`), and the
    /// depth of the support under the nosing line (`m_structuralDepth`).
    pub run_types: BTreeMap<i64, (bool, f64)>,
    /// Family parameters' names.
    pub param_names: BTreeMap<i64, String>,
    /// Elements of these kinds whose parameters did not read, and why.
    pub unread: BTreeMap<i64, String>,
}

impl Building {
    pub(crate) fn add(&mut self, id: i64, raw: Raw) {
        match raw {
            Raw::Wall(w) => {
                self.walls.insert(id, *w);
            }
            Raw::Slab(s) => {
                self.slabs.insert(id, *s);
            }
            Raw::Instance(i) => {
                self.instances.insert(id, *i);
            }
            Raw::Symbol(s) => {
                self.symbols.insert(id, *s);
            }
            Raw::Structure(layers) => {
                self.structures.insert(id, layers);
            }
            Raw::Grid(g) => self.grids.push(g),
            Raw::LinkInstance(l) => self.links.push(l),
            Raw::LinkSymbol { .. } => {}
            Raw::ParamName(n) => {
                self.param_names.insert(id, n);
            }
            Raw::RunType(m, d) => {
                self.run_types.insert(id, (m, d));
            }
            Raw::StairRun(r) => {
                self.stair_runs.insert(id, *r);
            }
            Raw::Opening(o) => {
                self.openings.insert(id, *o);
            }
            Raw::Unread(why) => {
                self.unread.insert(id, why);
            }
        }
    }

    /// Fills the links' file names from their types, sorts what is listed.
    pub(crate) fn finish(&mut self, symbols: &BTreeMap<i64, (String, Option<String>)>) {
        for link in &mut self.links {
            if let Some((name, path)) = link.symbol.and_then(|s| symbols.get(&s)) {
                link.file = Some(name.clone());
                link.path = path.clone();
            }
        }
        self.links.sort_by_key(|l| l.id);
        self.grids.sort_by_key(|g| g.id);
    }

    /// A type parameter's value by its id, or by its family parameter's
    /// name (any of `names`, case ignored).
    pub fn param(&self, symbol: i64, ids: &[i64], names: &[&str]) -> Option<f64> {
        let s = self.symbols.get(&symbol)?;
        s.params
            .iter()
            .find(|p| ids.contains(&p.id))
            .or_else(|| {
                s.params.iter().find(|p| {
                    self.param_names
                        .get(&p.id)
                        .is_some_and(|n| names.iter().any(|w| n.eq_ignore_ascii_case(w)))
                })
            })
            .map(|p| p.value)
    }

    /// The total width of a type's layers.
    pub fn width(&self, type_id: i64) -> Option<f64> {
        let layers = self.structures.get(&type_id)?;
        let w: f64 = layers.iter().map(|l| l.width).sum();
        (w > 0.0).then_some(w)
    }
}

/// Reads records' fields by name.
struct Reader<'a> {
    schema: &'a Schema,
    record: &'a Record,
}

impl<'a> Reader<'a> {
    fn get(&self, o: &'a Object, name: &str) -> Option<&'a Value> {
        o.get(self.schema, name)
    }

    /// The object a field holds or points at.
    fn obj(&self, o: &'a Object, name: &str) -> Option<&'a Object> {
        self.record.deref(self.get(o, name)?)
    }

    fn deref(&self, v: &'a Value) -> Option<&'a Object> {
        self.record.deref(v)
    }

    fn id(&self, o: &Object, name: &str) -> Option<i64> {
        self.get(o, name)
            .and_then(Value::as_id)
            .filter(|id| *id != -1)
    }

    fn f64(&self, o: &Object, name: &str) -> Option<f64> {
        self.get(o, name)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
    }

    fn u64(&self, o: &Object, name: &str) -> Option<u64> {
        self.get(o, name).and_then(Value::as_u64)
    }

    fn bool(&self, o: &Object, name: &str) -> Option<bool> {
        self.get(o, name).and_then(Value::as_bool)
    }

    fn floats(&self, o: &Object, name: &str) -> Vec<f64> {
        self.get(o, name).map(Value::floats).unwrap_or_default()
    }

    fn vec3(&self, o: &Object, name: &str) -> Option<P3> {
        let v = self.floats(o, name);
        (v.len() == 3 && v.iter().all(|x| x.is_finite())).then(|| [v[0], v[1], v[2]])
    }

    fn list(&self, o: &'a Object, name: &str) -> &'a [Value] {
        self.get(o, name).and_then(Value::as_list).unwrap_or(&[])
    }

    fn class(&self, o: &Object) -> &'a str {
        self.schema.class_name(o.class)
    }

    fn curve(&self, o: &Object) -> Result<Curve, String> {
        match self.class(o) {
            "GLine" => {
                let origin = self
                    .vec3(o, "m_origin")
                    .ok_or("a line without its origin")?;
                let dir = self
                    .vec3(o, "m_dirVec")
                    .ok_or("a line without its direction")?;
                let t = self.floats(o, "m_endParams");
                if t.len() != 2 {
                    return Err("a line without its end parameters".into());
                }
                Ok(Curve::Line {
                    start: add(origin, scale(dir, t[0])),
                    end: add(origin, scale(dir, t[1])),
                })
            }
            "GArc" => {
                let center = self
                    .vec3(o, "m_center")
                    .ok_or("an arc without its centre")?;
                let radius = self.f64(o, "m_radius").ok_or("an arc without its radius")?;
                let x_axis = self
                    .vec3(o, "m_xVec")
                    .ok_or("an arc without its x vector")?;
                let y_axis = self
                    .vec3(o, "m_yVec")
                    .ok_or("an arc without its y vector")?;
                let t = self.floats(o, "m_endParams");
                if t.len() != 2 || t[1] <= t[0] {
                    return Err("an arc without its end parameters".into());
                }
                Ok(Curve::Arc {
                    center,
                    x_axis,
                    y_axis,
                    radius,
                    start: t[0],
                    end: t[1],
                })
            }
            other => Err(format!("a curve of class {other}")),
        }
    }

    fn plane(&self, o: &Object) -> Option<Plane> {
        if self.class(o) != "Plane" {
            return None;
        }
        let corners = self
            .get(o, "m_Envelope")
            .and_then(Value::as_object)
            .map(|e| self.floats(e, "m_corners"))
            .unwrap_or_default();
        if corners.len() != 4 {
            return None;
        }
        Some(Plane {
            origin: self.vec3(o, "m_origin")?,
            x: self.vec3(o, "m_xVec")?,
            y: self.vec3(o, "m_yVec")?,
            u: [corners[0], corners[2]],
            v: [corners[1], corners[3]],
        })
    }

    /// A face's surface as a plane.
    fn face_plane(&self, face: &Object) -> Option<Plane> {
        self.plane(self.obj(face, "m_pSurf")?)
    }

    /// The curve loops a list of `CurveLoop` pointers holds.
    fn loops(&self, values: &[Value], unread: &mut Vec<String>) -> Vec<Vec<Curve>> {
        let mut out = Vec::new();
        for l in values {
            let Some(l) = self.deref(l) else { continue };
            let mut curves = Vec::new();
            for c in self.list(l, "m_curves") {
                let Some(c) = self.deref(c) else { continue };
                match self.curve(c) {
                    Ok(c) => curves.push(c),
                    Err(e) => unread.push(e),
                }
            }
            if !curves.is_empty() {
                out.push(curves);
            }
        }
        out
    }

    /// Its geometry steps' classes, and the elements cutting it.
    fn steps(&self, root: &'a Object) -> (Vec<String>, Vec<i64>, Vec<i64>) {
        let mut classes = Vec::new();
        let mut cut_by = Vec::new();
        let mut cutouts = Vec::new();
        let Some(list) = self.obj(root, "m_geomSteps") else {
            return (classes, cut_by, cutouts);
        };
        for field in [
            "m_bRepFormGList",
            "m_bRepAdjustGList",
            "m_bRepCutOutGList",
            "m_bRepPostCutOutGList",
            "m_bRepTweakGList",
        ] {
            for s in self.list(list, field) {
                let Some(step) = self.deref(s) else { continue };
                let class = self.class(step);
                classes.push(class.to_string());
                match class {
                    "CutPairGStep" => {
                        if let Some(other) = self.id(step, "m_otherId") {
                            cut_by.push(other);
                        }
                    }
                    "WallCutoutGStep" => {
                        if let Some(inst) = self.id(step, "m_instId") {
                            cutouts.push(inst);
                        }
                    }
                    _ => {}
                }
            }
        }
        (classes, cut_by, cutouts)
    }

    /// A join's ends (`CDJoinInfo.m_info`).
    fn join_info(&self, info: &Object) -> (Vec<JoinedEnd>, u64) {
        let ends = self
            .list(info, "m_info")
            .iter()
            .filter_map(Value::as_object)
            .filter_map(|e| {
                Some(JoinedEnd {
                    element: self.id(e, "m_elemId")?,
                    end: self.u64(e, "m_joinedEnd")? as u8,
                })
            })
            .collect();
        (ends, self.u64(info, "m_wallJoinType").unwrap_or(0))
    }

    fn params(&self, root: &'a Object) -> Vec<(i64, f64)> {
        let Some(set) = self.obj(root, "m_pParamValueSetDouble") else {
            return Vec::new();
        };
        self.list(set, "m_paramSet")
            .iter()
            .filter_map(Value::as_object)
            .filter_map(|p| Some((self.id(p, "m_paramId")?, self.f64(p, "m_value")?)))
            .collect()
    }

    fn wall(&self, id: i64, root: &'a Object) -> Result<Wall, String> {
        let driver = self
            .obj(root, "m_pCurveDriver")
            .ok_or("no location curve driver")?;
        let curve = self
            .curve(self.obj(driver, "m_pCrv").ok_or("no location curve")?)
            .map_err(|e| format!("the location curve: {e}"))?;
        let mut joins: [Vec<JoinedEnd>; 2] = [Vec::new(), Vec::new()];
        let mut join_types = [0, 0];
        for pair in self.list(driver, "m_controlJoinsSet") {
            let Some(pair) = pair.as_object() else {
                continue;
            };
            let index = self.u64(pair, "m_index").unwrap_or(9) as usize;
            let Some(info) = self.obj(pair, "m_JoinInfo") else {
                continue;
            };
            if index < 2 {
                let (ends, kind) = self.join_info(info);
                joins[index] = ends;
                join_types[index] = kind;
            }
        }
        let mut side_joins = Vec::new();
        for s in self.list(driver, "m_sideJoins") {
            let Some(info) = self.deref(s) else { continue };
            let (ends, _) = self.join_info(info);
            side_joins.extend(ends.into_iter().filter(|e| e.element != id));
        }
        let no_join = {
            let v = self.list(driver, "m_noJoinAtEnd");
            [
                v.first().and_then(Value::as_bool).unwrap_or(false),
                v.get(1).and_then(Value::as_bool).unwrap_or(false),
            ]
        };
        // The location reference face: the curve's plane, its envelope's v
        // range the wall's height above the plane's origin.
        let location = self
            .list(root, "m_pRefFaces")
            .first()
            .and_then(|f| self.deref(f))
            .and_then(|f| self.face_plane(f));
        let (mut base, mut height) = (None, None);
        if let Some(p) = &location
            && p.y[2].abs() > 1.0 - 1e-9
            && p.v[1] > p.v[0]
        {
            base = Some(p.origin[2] + p.y[2] * p.v[0]);
            height = Some(p.v[1] - p.v[0]);
        }
        if height.is_none() {
            // An arc wall's faces are no planes: the height parameter
            // (verified against the planes of the straight walls).
            let params = self.params(root);
            height = params
                .iter()
                .find(|(p, _)| *p == -1_001_101)
                .map(|(_, v)| *v);
            base = Some(curve.start()[2]);
        }
        let (steps, cut_by, cutouts) = self.steps(root);
        let mut faces = None;
        if let Curve::Line { start, end } = &curve {
            let (dx, dy) = (end[0] - start[0], end[1] - start[1]);
            let l = dx.hypot(dy);
            if l > 1e-9 {
                let n = [-dy / l, dx / l];
                let offsets: Vec<f64> = self
                    .list(root, "m_pRefFaces")
                    .iter()
                    .filter_map(|f| self.deref(f))
                    .filter_map(|f| self.face_plane(f))
                    .filter(|p| {
                        (p.x[0] * n[0] + p.x[1] * n[1]).abs() < 1e-6 && p.y[2].abs() > 1.0 - 1e-6
                    })
                    .map(|p| (p.origin[0] - start[0]) * n[0] + (p.origin[1] - start[1]) * n[1])
                    .collect();
                if offsets.len() >= 2 {
                    let lo = offsets.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = offsets.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    if hi - lo > 1e-6 {
                        faces = Some([lo, hi]);
                    }
                }
            }
        }
        Ok(Wall {
            faces,
            id,
            class: self.class(root).to_string(),
            type_id: self.id(root, "m_WallAttributesId"),
            curve,
            base,
            height,
            top_level: self.id(root, "m_upToLevelId"),
            joins,
            join_types,
            no_join,
            side_joins,
            flipped: self.bool(root, "m_isFlipped").unwrap_or(false),
            key_ref: self.u64(root, "m_wallKeyRef").unwrap_or(0),
            key_offset: self.f64(root, "m_locLineOffset").unwrap_or(0.0),
            steps,
            cut_by,
            cutouts,
            profile: self.helper_loops(root, &mut Vec::new()).unwrap_or_default(),
        })
    }

    /// The curve loops of an element's extrusion helper cell
    /// (`FloorExtrusionHelper`, `RoofExtrusionHelper`,
    /// `VWallExtrusionHelper`, ...), None without one.
    fn helper_loops(&self, root: &'a Object, unread: &mut Vec<String>) -> Option<Vec<Vec<Curve>>> {
        let cells = self.obj(root, "m_cellList")?;
        self.list(cells, "m_cells").iter().find_map(|c| {
            let cell = self.deref(c)?;
            self.class(cell)
                .ends_with("ExtrusionHelper")
                .then(|| self.loops(self.list(cell, "m_pCurveLoops"), unread))
        })
    }

    fn slab(&self, id: i64, root: &'a Object) -> Result<Slab, String> {
        let mut unread = Vec::new();
        // The loops of its extrusion helper cell, else its own
        // (`ProfileRoof.m_curveLoops`).
        let loops = match self.helper_loops(root, &mut unread) {
            Some(loops) => loops,
            None if root.get(self.schema, "m_curveLoops").is_some() => {
                self.loops(self.list(root, "m_curveLoops"), &mut unread)
            }
            None => return Err("no extrusion helper with its boundary loops".into()),
        };
        let layer_faces = self
            .list(root, "m_pRefLayerFaces")
            .iter()
            .filter_map(|f| self.deref(f))
            .filter_map(|f| self.face_plane(f))
            .collect();
        let (steps, cut_by, cutouts) = self.steps(root);
        let mut shape_edited = false;
        if let Some(list) = self.obj(root, "m_geomSteps") {
            for s in self.list(list, "m_bRepFormGList") {
                if let Some(step) = self.deref(s)
                    && self.class(step) == "SlabShapeEditGStep"
                    && !self.bool(step, "m_dormant").unwrap_or(true)
                {
                    shape_edited = true;
                }
            }
        }
        let type_id = ["m_floorAttributesId", "m_roofAttributesId"]
            .iter()
            .find_map(|f| self.id(root, f));
        Ok(Slab {
            id,
            class: self.class(root).to_string(),
            type_id,
            loops,
            unread_curves: unread,
            top: self.f64(root, "m_top").filter(|v| v.abs() < 1e20),
            bottom: self.f64(root, "m_bottom").filter(|v| v.abs() < 1e20),
            base_level: self.id(root, "m_baseLevelId"),
            base_offset: self.f64(root, "m_baseOffset"),
            extrusion: {
                let d = self.floats(root, "m_extrusionDepth");
                (d.len() == 2).then(|| [d[0], d[1]])
            },
            layer_faces,
            ref_faces: self
                .list(root, "m_pRefFaces")
                .iter()
                .filter_map(|f| self.deref(f))
                .filter_map(|f| self.face_plane(f))
                .collect(),
            shape_edited,
            steps,
            cut_by,
            cutouts,
        })
    }

    fn instance(&self, id: i64, root: &'a Object) -> Instance {
        let mut inst = Instance {
            id,
            master: self.id(root, "m_masterSymbolId"),
            host: self.id(root, "m_hostId"),
            struct_type: self.u64(root, "m_structType").unwrap_or(0),
            vertical: self.bool(root, "m_bVertical").unwrap_or(true),
            ..Instance::default()
        };
        if let Some(info) = self.obj(root, "m_pInstanceInfo") {
            inst.symbol = self.id(info, "m_symbolId");
            inst.transform = info
                .get(self.schema, "m_Trf")
                .and_then(Value::as_object)
                .and_then(|trf| {
                    let mut m = self.floats(trf, "m_3x3");
                    m.extend(self.floats(trf, "m_or"));
                    (m.len() == 12).then(|| std::array::from_fn(|i| m[i]))
                });
        }
        for r in self.list(root, "m_refs") {
            let Some(r) = self.deref(r) else { continue };
            if self.class(r) == "FamilyLevelRef"
                && let Some(level) = self.id(r, "m_elemId")
            {
                inst.levels.push(LevelRef {
                    level,
                    offset: self.f64(r, "m_offset").unwrap_or(0.0),
                });
            }
        }
        if let Some(driver) = self.obj(root, "m_pCurveDriver")
            && let Some(c) = self.obj(driver, "m_pCrv")
        {
            inst.curve = self.curve(c).ok();
            let s = self.floats(driver, "m_setbacks");
            if s.len() == 2 {
                inst.setbacks = [s[0], s[1]];
            }
        }
        let (steps, cut_by, _) = self.steps(root);
        inst.steps = steps;
        inst.cut_by = cut_by;
        inst
    }

    fn symbol(&self, id: i64, root: &'a Object) -> Symbol {
        let mut s = Symbol {
            id,
            ..Symbol::default()
        };
        if let Some(params) = self.obj(root, "m_pParams") {
            for p in self.list(params, "m_params") {
                let Some(p) = p.as_object() else { continue };
                let Some(pid) = self.id(p, "m_paramId") else {
                    continue;
                };
                s.params.push(Param {
                    id: pid,
                    value: self.f64(p, "m_value").unwrap_or(0.0),
                    text: self
                        .get(p, "m_str")
                        .and_then(Value::as_text)
                        .unwrap_or_default()
                        .to_string(),
                    element: self.id(p, "m_elemId"),
                });
            }
        }
        let mut unread = Vec::new();
        s.cut_loops = self.loops(self.list(root, "m_pCutLoops"), &mut unread);
        s.cut_dirs = self
            .list(root, "m_cutDirs")
            .iter()
            .map(Value::floats)
            .filter(|v| v.len() == 3)
            .map(|v| [v[0], v[1], v[2]])
            .collect();
        s.materials = self
            .list(root, "m_geomTag2MaterialId")
            .iter()
            .filter_map(|v| match v {
                Value::Object(o) => o.fields.iter().find_map(|(_, _, v)| v.as_id()),
                other => other.as_id(),
            })
            .filter(|id| *id > 0)
            .collect();
        s
    }

    fn structure(&self, root: &'a Object) -> Option<Vec<Layer>> {
        let cs = self.obj(root, "m_pCompoundStructure")?;
        Some(
            self.list(cs, "m_layers")
                .iter()
                .filter_map(|l| self.deref(l))
                .map(|l| Layer {
                    width: self.f64(l, "m_layerWidth").unwrap_or(0.0),
                    material: self.id(l, "m_materialId"),
                    function: self.u64(l, "m_layerFunction").unwrap_or(0),
                })
                .collect(),
        )
    }

    fn grid(&self, id: i64, root: &'a Object) -> Grid {
        let name = self
            .get(root, "m_text")
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_string();
        let surface = self.obj(root, "m_pSurface");
        let (curve, note) = match surface.map(|s| (self.class(s), s)) {
            Some(("Plane", s)) => match self.plane(s) {
                Some(p) if p.u[1] > p.u[0] => {
                    // The line at the envelope's lowest height.
                    let at = |u: f64| add(add(p.origin, scale(p.x, u)), scale(p.y, p.v[0]));
                    (
                        Some(Curve::Line {
                            start: at(p.u[0]),
                            end: at(p.u[1]),
                        }),
                        None,
                    )
                }
                _ => (None, Some("its plane has no extent".to_string())),
            },
            Some(("CylinderSurface" | "Cylinder", s)) => match self.cylinder_arc(s) {
                Some(c) => (Some(c), None),
                None => (None, Some("its cylinder did not read".to_string())),
            },
            Some((other, _)) => (None, Some(format!("a surface of class {other}"))),
            None => (None, Some("no surface".to_string())),
        };
        Grid {
            id,
            name,
            curve,
            note,
        }
    }

    /// An arc grid's cylinder: its axis frame, radius and envelope.
    fn cylinder_arc(&self, s: &Object) -> Option<Curve> {
        let origin = self.vec3(s, "m_origin")?;
        let x = self.vec3(s, "m_xVec")?;
        let y = self.vec3(s, "m_yVec")?;
        let radius = self
            .f64(s, "m_radius")
            .unwrap_or_else(|| (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt());
        let corners = self
            .get(s, "m_Envelope")
            .and_then(Value::as_object)
            .map(|e| self.floats(e, "m_corners"))
            .unwrap_or_default();
        if corners.len() != 4 || corners[2] <= corners[0] || radius <= 0.0 {
            return None;
        }
        let unit = |v: P3| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            scale(v, 1.0 / l)
        };
        Some(Curve::Arc {
            center: [origin[0], origin[1], origin[2] + corners[1]],
            x_axis: unit(x),
            y_axis: unit(y),
            radius,
            start: corners[0],
            end: corners[2],
        })
    }

    fn link_instance(&self, id: i64, root: &'a Object) -> Link {
        let info = self.obj(root, "m_pInstanceInfo");
        Link {
            id,
            symbol: info.and_then(|i| self.id(i, "m_symbolId")),
            instance_name: self
                .get(root, "m_name")
                .and_then(Value::as_text)
                .unwrap_or_default()
                .to_string(),
            transform: info.and_then(|i| {
                let trf = i.get(self.schema, "m_Trf").and_then(Value::as_object)?;
                let mut m = self.floats(trf, "m_3x3");
                m.extend(self.floats(trf, "m_or"));
                (m.len() == 12).then(|| std::array::from_fn(|k| m[k]))
            }),
            file: None,
            path: None,
        }
    }

    fn link_symbol(&self, root: &'a Object) -> Raw {
        let name = self
            .obj(root, "m_symbolInfo")
            .and_then(|i| self.get(i, "m_name"))
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_string();
        let path = self
            .get(root, "m_otherDocModelLocation")
            .and_then(Value::as_object)
            .and_then(|l| self.get(l, "m_lastKnownModelPath"))
            .and_then(Value::as_object)
            .and_then(|p| self.get(p, "m_path"))
            .and_then(Value::as_text)
            .filter(|p| !p.is_empty())
            .map(str::to_string);
        Raw::LinkSymbol { name, path }
    }

    fn stair_run(&self, id: i64, root: &'a Object) -> StairRun {
        let line = |field: &str| self.obj(root, field).and_then(|c| self.curve(c).ok());
        let treads = self
            .list(root, "m_stairsTreadInfoArr")
            .iter()
            .filter_map(|t| self.deref(t))
            .filter_map(|t| self.f64(t, "m_actDepth"))
            .collect();
        let support = self
            .list(root, "m_oSupportPathCurveLoops")
            .iter()
            .filter_map(|l| self.deref(l))
            .find_map(|l| {
                self.list(l, "m_curves")
                    .first()
                    .and_then(|c| self.deref(c))
                    .and_then(|c| self.curve(c).ok())
            });
        let index = |field: &str| self.u64(root, field).unwrap_or(0);
        StairRun {
            id,
            bottom: self.f64(root, "m_bottomElevation").unwrap_or(0.0),
            top: self.f64(root, "m_topElevation").unwrap_or(0.0),
            risers: index("m_topRiserIndex").saturating_sub(index("m_baseRiserIndex")),
            treads,
            start: line("m_oStartRiser"),
            end: line("m_oEndRiser"),
            support,
        }
    }

    fn opening(&self, id: i64, root: &'a Object) -> Opening {
        let mut unread = Vec::new();
        let mut loops = Vec::new();
        for field in ["m_pCurveLoops", "m_profileLoops", "m_loops"] {
            let l = self.loops(self.list(root, field), &mut unread);
            if !l.is_empty() {
                loops = l;
                break;
            }
        }
        if loops.is_empty()
            && let Some(cells) = self.obj(root, "m_cellList")
        {
            for c in self.list(cells, "m_cells") {
                if let Some(cell) = self.deref(c)
                    && self.class(cell).ends_with("Helper")
                {
                    loops = self.loops(self.list(cell, "m_pCurveLoops"), &mut unread);
                }
            }
        }
        let corners = match (self.vec3(root, "m_pt1"), self.vec3(root, "m_pt2")) {
            (Some(a), Some(b)) => Some([a, b]),
            _ => None,
        };
        Opening {
            id,
            class: self.class(root).to_string(),
            host: self
                .id(root, "m_hostId")
                .or_else(|| self.id(root, "m_hostElemId")),
            loops,
            corners,
            base: None,
            top: None,
        }
    }
}

/// What a record gives the building model, by its class (None for
/// classes it has nothing of).
pub(crate) fn read(schema: &Schema, record: &Record, id: i64) -> Option<Raw> {
    let r = Reader { schema, record };
    let root = &record.root;
    let class = schema.class_name(root.class);
    let is = |name: &str| schema.is_a(root.class, name);
    if is("VWall") {
        return Some(match r.wall(id, root) {
            Ok(w) => Raw::Wall(Box::new(w)),
            Err(e) => Raw::Unread(format!("wall: {e}")),
        });
    }
    if matches!(class, "Floor" | "Roof" | "Ceiling" | "BuildingPad") || is("RoofBase") {
        return Some(match r.slab(id, root) {
            Ok(s) => Raw::Slab(Box::new(s)),
            Err(e) => Raw::Unread(format!("{}: {e}", class.to_lowercase())),
        });
    }
    if is("FamilyInstance") {
        return Some(Raw::Instance(Box::new(r.instance(id, root))));
    }
    if is("FamilySymbol") {
        return Some(Raw::Symbol(Box::new(r.symbol(id, root))));
    }
    if is("Symbol") && root.get(schema, "m_pCompoundStructure").is_some() {
        return r.structure(root).map(Raw::Structure);
    }
    match class {
        "Grid" => return Some(Raw::Grid(r.grid(id, root))),
        "RvtLinkInstance" => return Some(Raw::LinkInstance(r.link_instance(id, root))),
        "RvtLinkSymbol" => return Some(r.link_symbol(root)),
        "StairsRun" => return Some(Raw::StairRun(Box::new(r.stair_run(id, root)))),
        "StairsRunType" => {
            return Some(Raw::RunType(
                r.bool(root, "m_hasMonolithicSupport").unwrap_or(false),
                r.f64(root, "m_structuralDepth").unwrap_or(0.0),
            ));
        }
        "Opening" | "SWallRectOpening" | "ShaftOpening" => {
            return Some(Raw::Opening(Box::new(r.opening(id, root))));
        }
        _ => {}
    }
    if class.starts_with("ParamElem") {
        let caption = r
            .obj(root, "m_pParamDef")
            .and_then(|d| r.get(d, "m_caption"))
            .and_then(Value::as_text)
            .filter(|t| !t.is_empty())?;
        return Some(Raw::ParamName(caption.to_string()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_and_transforms() {
        let arc = Curve::Arc {
            center: [1.0, 1.0, 0.0],
            x_axis: [1.0, 0.0, 0.0],
            y_axis: [0.0, 1.0, 0.0],
            radius: 2.0,
            start: 0.0,
            end: std::f64::consts::FRAC_PI_2,
        };
        let e = arc.end();
        assert!((e[0] - 1.0).abs() < 1e-12 && (e[1] - 3.0).abs() < 1e-12);
        assert!((arc.length() - std::f64::consts::PI).abs() < 1e-12);
        // A quarter turn about z and a move.
        let t = [0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 0.0];
        let line = Curve::Line {
            start: [0.0, 0.0, 0.0],
            end: [1.0, 0.0, 0.0],
        }
        .transformed(&t);
        assert_eq!(line.end(), [10.0, 1.0, 0.0]);
    }

    #[test]
    fn the_building_project_reads() {
        use crate::testfile::building as t;
        let file = crate::RvtFile::parse(t::project()).unwrap();
        let schema = file.schema().unwrap();
        let model = crate::elements::Model::read(&file, &schema).unwrap();
        assert_eq!(model.header_errors + model.object_errors, 0);
        let b = &model.building;
        assert!(b.unread.is_empty(), "{:?}", b.unread);
        let wall = &b.walls[&t::WALL_A];
        assert_eq!(wall.curve.end(), [20.0, 0.0, 0.0]);
        assert_eq!(wall.height, Some(t::HEIGHT));
        assert_eq!(wall.base, Some(0.0));
        assert_eq!(
            wall.joins[0].iter().map(|j| j.element).collect::<Vec<_>>(),
            vec![t::WALL_A, t::WALL_B]
        );
        assert_eq!(b.width(t::WALL_TYPE), Some(t::WIDTH));
        assert_eq!(b.structures[&t::WALL_TYPE][0].material, Some(t::MATERIAL));
        let floor = &b.slabs[&t::FLOOR];
        assert_eq!((floor.top, floor.bottom), (Some(0.0), Some(-0.5)));
        assert_eq!(
            floor.loops.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![4, 4]
        );
        assert_eq!(b.grids.len(), 1);
        assert_eq!(b.grids[0].name, "A");
        assert_eq!(
            b.grids[0].curve,
            Some(Curve::Line {
                start: [-5.0, -2.0, 0.0],
                end: [25.0, -2.0, 0.0]
            })
        );
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.links[0].file.as_deref(), Some(t::LINKED));
        assert_eq!(b.links[0].transform.unwrap()[9], 100.0);
    }
}
