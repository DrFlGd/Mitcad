// SPDX-License-Identifier: MIT
//! Feature inputs found in the ASM history.
//!
//! The streams name a feature's face, edge and body inputs by the names the
//! bodies carry ([`super::recipe`], [`crate::names`]). The entities exist in
//! the history state before the item: the `.smbh` blobs rolled back to just
//! before the item's own state (its `_f3d.result_no`). There they are found
//! by their names (some inputs name faces as they were a few states
//! earlier: those states are searched next, up to [`EARLIER`] back), and
//! their geometry goes into the fingerprints, as an
//! external dump gives it (cm): edges get `mid_point`, `length`,
//! `start_point` and `end_point` (and `_f3d.direction`, their own
//! direction at the middle); a thread's cylindrical faces their
//! `geometry` (`Cylinder`: origin, axis, radius) and `point_on_face`. A
//! fillet's or chamfer's face selections become the edges of the faces
//! (mitcad#96). `_f3d.found` says where the entity was found or why not.

use std::collections::HashMap;

use crate::asm::AsmFile;
use crate::asm::history::History;
use crate::container::F3dFile;
use crate::names::{EdgeGeometry, NamedState};

use serde_json::{Map, Value};

use super::ir::{Detail, Dump, EntityName, Fingerprint, Geometry, Reference, TimelineItem};

/// A history state as [`History::view`] gives it.
type View = HashMap<usize, Option<usize>>;

/// A history blob of the document.
struct Blob {
    entry: String,
    file: AsmFile,
    history: History,
}

/// The history's blobs, read when an item first needs them (by one
/// thread; the others wait for them).
#[derive(Default)]
struct Blobs(std::sync::OnceLock<Vec<Blob>>);

impl Blobs {
    fn get(&self, doc: &F3dFile) -> &[Blob] {
        self.0.get_or_init(|| history_blobs(doc))
    }
}

// Inputs on other components' bodies (mitcad#161).

/// A timeline item's ASM state and owner, for finding inputs in other
/// components' histories.
#[derive(Clone, Copy, Debug, Default)]
struct Point {
    /// Its `_f3d.result_no`, when it has a state.
    result: Option<i64>,
    /// The component that owns it (`_f3d.component`).
    owner: Option<u64>,
}

/// The timeline's items as [`Point`]s, in order, and the history blobs'
/// components (worked out once a lookup needs them).
struct Timeline {
    points: Vec<Point>,
    links: std::sync::OnceLock<Vec<Option<u64>>>,
}

impl Timeline {
    fn new(items: &[TimelineItem]) -> Self {
        let points = items
            .iter()
            .map(|item| {
                let f = item.f3d.as_ref();
                Point {
                    result: f
                        .and_then(|f| f.result_no)
                        .filter(|&r| r > 0)
                        .map(i64::from),
                    owner: f.and_then(|f| f.component),
                }
            })
            .collect();
        Timeline {
            points,
            links: std::sync::OnceLock::new(),
        }
    }

    /// The component (object id) of each history blob: the one whose
    /// items name most of the blob's operations, where that blob is also
    /// the one whose operations its items name most (the numbers count
    /// per component, so components' items name other blobs' numbers too).
    fn links(&self, blobs: &[Blob]) -> &[Option<u64>] {
        self.links.get_or_init(|| {
            let counts: Vec<HashMap<u64, usize>> = blobs
                .iter()
                .map(|b| {
                    let ops: std::collections::HashSet<i64> =
                        b.history.states.iter().map(|s| s.id).collect();
                    let mut count: HashMap<u64, usize> = HashMap::new();
                    for p in &self.points {
                        if let (Some(r), Some(o)) = (p.result, p.owner)
                            && ops.contains(&r)
                        {
                            *count.entry(o).or_default() += 1;
                        }
                    }
                    count
                })
                .collect();
            let best = |count: &HashMap<u64, usize>| {
                let most = count.values().copied().max()?;
                let mut top = count.iter().filter(|(_, n)| **n == most).map(|(c, _)| *c);
                match (top.next(), top.next()) {
                    (Some(c), None) => Some((c, most)),
                    _ => None,
                }
            };
            counts
                .iter()
                .enumerate()
                .map(|(b, count)| {
                    let (c, n) = best(count)?;
                    // No other blob whose operations the component's items
                    // name as often.
                    let rival = counts
                        .iter()
                        .enumerate()
                        .any(|(o, other)| o != b && other.get(&c).is_some_and(|&m| m >= n));
                    (!rival).then_some(c)
                })
                .collect()
        })
    }

    /// The state of blob `b` at the item at `position` of the timeline:
    /// its position in the blob's states (newest first; the state after
    /// it is `view(k)`) of the last item before it whose operation the
    /// blob holds, of the blob's component. None when the blob has no
    /// component or none of its items comes before.
    fn state_at(&self, blobs: &[Blob], b: usize, position: usize) -> Option<usize> {
        let component = self.links(blobs).get(b).copied().flatten()?;
        let blob = &blobs[b];
        self.points[..position.min(self.points.len())]
            .iter()
            .rev()
            .filter(|p| p.owner == Some(component))
            .find_map(|p| {
                let r = p.result?;
                blob.history.states.iter().position(|s| s.id == r)
            })
    }
}

/// How many inputs were found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Resolved {
    pub found: usize,
    pub not_found: usize,
}

/// The edge and face fingerprints of an item's detail that carry names and
/// no geometry yet: fillet and chamfer edges, a thread's faces, the face an
/// extrusion goes up to.
fn named_entities(detail: &mut Detail) -> Vec<&mut Fingerprint> {
    // (A fillet's or chamfer's faces left after [`expand_faces`] were not
    // found.) A thread's faces and an extrusion's object are faces.
    let faces = matches!(
        detail,
        Detail::Thread(_) | Detail::Extrude(_) | Detail::Shell(_) | Detail::OffsetFaces(_)
    );
    let refs: Vec<&mut Reference> = match detail {
        Detail::Fillet(f) => f
            .edge_sets
            .iter_mut()
            .flatten()
            .flat_map(|s| s.edges.iter_mut().flatten())
            .collect(),
        Detail::Chamfer(c) => c
            .edge_sets
            .iter_mut()
            .flatten()
            .flat_map(|s| s.edges.iter_mut().flatten())
            .collect(),
        Detail::Thread(t) => t.input_cylindrical_faces.iter_mut().flatten().collect(),
        // The face an extrusion's extent goes up to (mitcad#96), and the
        // faces its profile selects (mitcad#169).
        Detail::Extrude(x) => [&mut x.extent_one, &mut x.extent_two]
            .into_iter()
            .flatten()
            .filter_map(|d| d.entity.as_mut())
            .chain(x.profile.iter_mut().flatten())
            .collect(),
        // A shell's removed faces and offset faces (mitcad#154).
        Detail::Shell(s) => s.input_entities.iter_mut().flatten().collect(),
        Detail::OffsetFaces(o) => o.input_faces.iter_mut().flatten().collect(),
        _ => Vec::new(),
    };
    refs.into_iter()
        .filter_map(|r| match r {
            Reference::Edge(fp) if fp.mid_point.is_none() && fp.f3d.is_some() => Some(&mut **fp),
            Reference::Face(fp) if faces && fp.point_on_face.is_none() && fp.f3d.is_some() => {
                Some(&mut **fp)
            }
            _ => None,
        })
        .collect()
}

/// Finds a named edge or face in a history state and writes its geometry
/// into its fingerprint (cm).
///
/// `twin` (k, n): the edge input is the k-th of n of the item with the
/// same names; where exactly n edges fit them, it is the k-th of those
/// (mitcad#96: a set of n edges between the same two faces, such as
/// the edges where a patterned boss meets a plate, names each the same
/// way).
fn locate(
    state: &NamedState,
    fp: &mut Fingerprint,
    entities: &[Vec<EntityName>],
    twin: (usize, usize),
) -> Result<(), String> {
    let cm = |p: [f64; 3]| p.map(|x| x / 10.0);
    if fp.object_type.as_deref() == Some("BRepFace") {
        let i = state.find_face(entities)?;
        // A planar face (a mirror plane, a split tool; mitcad#67).
        if let (Err(_), Ok(p)) = (state.cylinder(i), state.plane(i)) {
            fp.geometry = Some(Geometry {
                geometry_type: Some("Plane".to_owned()),
                origin: Some(cm(p.point)),
                normal: Some(p.normal),
                ..Geometry::default()
            });
            fp.point_on_face = Some(cm(p.point));
            fp.normal_at_point = Some(p.normal);
            return Ok(());
        }
        let g = match state.cylinder(i) {
            Ok(g) => g,
            // Any other surface (mitcad#161: a thread's flank, a spline).
            Err(_) => {
                let s = state.surface_point(i)?;
                fp.geometry = Some(Geometry {
                    geometry_type: Some(s.kind.to_owned()),
                    origin: s.origin.map(cm),
                    axis: s.axis,
                    radius: s.radius.map(|r| r / 10.0),
                    major_radius: s.minor.and(s.radius).map(|r| r / 10.0),
                    minor_radius: s.minor.map(|r| r / 10.0),
                    half_angle: s.half_angle,
                    ..Geometry::default()
                });
                if s.minor.is_some() {
                    // A torus has major and minor radii only.
                    if let Some(g) = fp.geometry.as_mut() {
                        g.radius = None;
                    }
                }
                fp.point_on_face = Some(cm(s.point));
                fp.normal_at_point = Some(s.normal);
                if !s.spread.is_empty()
                    && let Some(f) = fp.f3d.as_mut()
                {
                    f.face_points = Some(s.spread.into_iter().map(cm).collect());
                }
                return Ok(());
            }
        };
        fp.geometry = Some(Geometry {
            geometry_type: Some("Cylinder".to_owned()),
            origin: Some(cm(g.origin)),
            axis: Some(g.axis),
            radius: Some(g.radius / 10.0),
            ..Geometry::default()
        });
        fp.point_on_face = Some(cm(g.point));
        fp.normal_at_point = Some(g.normal);
        return Ok(());
    }
    let fits = state.edges_fitting(entities)?;
    let i = match (&fits[..], twin) {
        ([i], _) => *i,
        (top, (k, n)) if top.len() == n => top[k],
        (top, _) => return Err(format!("{} edges fit the names", top.len())),
    };
    let g = state.edge(i)?;
    edge_geometry(fp, &g);
    Ok(())
}

/// An edge's geometry into its fingerprint (cm).
fn edge_geometry(fp: &mut Fingerprint, g: &EdgeGeometry) {
    let cm = |p: [f64; 3]| p.map(|x| x / 10.0);
    fp.mid_point = Some(cm(g.mid));
    fp.length = Some(g.length / 10.0);
    fp.start_point = Some(Some(cm(g.start)));
    fp.end_point = Some(Some(cm(g.end)));
    if let Some(f) = fp.f3d.as_mut() {
        f.direction = Some(g.direction);
    }
}

/// The edges of a face a fillet or chamfer selects (mitcad#96: a
/// `bounded_face` input: every edge between the face and another), found
/// as an edge input's are, each a fingerprint with the face's names.
fn face_edges(
    state: &NamedState,
    face: &Fingerprint,
    entities: &[Vec<EntityName>],
) -> Result<Vec<Fingerprint>, String> {
    let i = state.find_face(entities)?;
    let edges = state.edges_of_face(i);
    if edges.is_empty() {
        return Err("the face has no edges to other faces".to_owned());
    }
    edges
        .into_iter()
        .map(|e| {
            let g = state.edge(e)?;
            let mut fp = Fingerprint {
                object_type: Some("BRepEdge".to_owned()),
                f3d: face.f3d.clone(),
                ..Fingerprint::default()
            };
            edge_geometry(&mut fp, &g);
            Ok(fp)
        })
        .collect()
}

/// A fillet's or chamfer's face selections (face references in its edge
/// sets, mitcad#96) replaced by the edges of the faces, found in the
/// state before the item, else up to [`EARLIER`] states before it, as
/// [`find_named`] finds edges. A face not found stays a face reference
/// with the reason.
fn expand_faces(blobs: &[Blob], result: Option<i32>, detail: &mut Detail, out: &mut Resolved) {
    let sets = match detail {
        Detail::Fillet(f) => f.edge_sets.as_mut(),
        Detail::Chamfer(c) => c.edge_sets.as_mut(),
        _ => None,
    };
    let Some(sets) = sets else { return };
    let is_face = |r: &Reference| matches!(r, Reference::Face(fp) if fp.f3d.is_some());
    // (set, position) of each face selection.
    let mut left: Vec<(usize, usize)> = Vec::new();
    for (k, s) in sets.iter().enumerate() {
        for (j, r) in s.edges.iter().flatten().enumerate() {
            if is_face(r) {
                left.push((k, j));
            }
        }
    }
    if left.is_empty() {
        return;
    }
    let mut found: HashMap<(usize, usize), Vec<Fingerprint>> = HashMap::new();
    let mut why: HashMap<(usize, usize), String> = HashMap::new();
    let at: Vec<(&Blob, usize)> = match result.filter(|&r| r > 0) {
        Some(result) => blobs
            .iter()
            .filter_map(|b| {
                let pos = b
                    .history
                    .states
                    .iter()
                    .position(|s| s.id == i64::from(result))?;
                Some((b, pos))
            })
            .collect(),
        None => Vec::new(),
    };
    if result.is_none_or(|r| r <= 0) {
        for &key in &left {
            why.insert(key, "the item has no ASM state".to_owned());
        }
        left.clear();
    } else if at.is_empty() {
        for &key in &left {
            why.insert(
                key,
                format!("no ASM history holds state {}", result.unwrap_or(0)),
            );
        }
        left.clear();
    }
    let result = result.unwrap_or(0);
    for earlier in [0..1, 1..EARLIER + 1] {
        if left.is_empty() {
            break;
        }
        let views: Vec<(&Blob, usize, View)> = earlier
            .flat_map(|e| at.iter().map(move |&(b, pos)| (b, e, pos + 1 + e)))
            .filter(|(b, _, k)| *k <= b.history.states.len())
            .map(|(b, e, k)| (b, e, b.history.view(k)))
            .collect();
        let named: Vec<(&Blob, usize, NamedState)> = views
            .iter()
            .map(|(b, e, view)| (*b, *e, NamedState::new(&b.file, Some(view))))
            .collect();
        let mut still = Vec::new();
        for (k, j) in left {
            let Some(Reference::Face(face)) = sets[k].edges.as_ref().and_then(|e| e.get(j)) else {
                continue;
            };
            let entities = face
                .f3d
                .as_ref()
                .and_then(|f| f.entities.clone())
                .unwrap_or_default();
            let mut hit = None;
            for (b, e, state) in &named {
                match face_edges(state, face, &entities) {
                    Ok(edges) => {
                        hit = Some((edges, b.entry.as_str(), *e));
                        break;
                    }
                    Err(err) => {
                        why.entry((k, j)).or_insert(err);
                    }
                }
            }
            match hit {
                Some((mut edges, entry, e)) => {
                    let text = match e {
                        0 => format!("{entry} before state {result} (an edge of a face)"),
                        e => {
                            format!("{entry} {e} states before state {result} (an edge of a face)")
                        }
                    };
                    for fp in &mut edges {
                        if let Some(f) = fp.f3d.as_mut() {
                            f.found = Some(text.clone());
                        }
                    }
                    found.insert((k, j), edges);
                }
                None => still.push((k, j)),
            }
        }
        left = still;
    }
    for (k, s) in sets.iter_mut().enumerate() {
        let Some(refs) = s.edges.take() else { continue };
        let mut edges = Vec::new();
        for (j, r) in refs.into_iter().enumerate() {
            if let Some(list) = found.remove(&(k, j)) {
                out.found += list.len();
                edges.extend(list.into_iter().map(|fp| Reference::Edge(Box::new(fp))));
                continue;
            }
            match r {
                Reference::Face(mut fp) if fp.f3d.is_some() => {
                    out.not_found += 1;
                    if let Some(f) = fp.f3d.as_mut() {
                        f.found = why.remove(&(k, j));
                    }
                    edges.push(Reference::Face(fp));
                }
                r => edges.push(r),
            }
        }
        s.edges = Some(edges);
    }
}

/// Finds the named inputs of the dump's items in the document's ASM
/// history and writes their geometry into the dump.
pub fn resolve(dump: &mut Dump, doc: &F3dFile) -> Resolved {
    let mut out = Resolved::default();
    let Some(items) = dump.timeline.as_mut().and_then(|t| t.items.as_mut()) else {
        return out;
    };
    let blobs = Blobs::default();
    let timeline = Timeline::new(items);
    for (position, item) in items.iter_mut().enumerate() {
        let at = At {
            blobs: &blobs,
            doc,
            timeline: &timeline,
            position,
        };
        resolve_item(item, &at, &mut out);
    }
    out
}

/// Where an item's inputs are looked for: the history's blobs (read once
/// one needs them), the timeline and the item's position in it.
struct At<'a> {
    blobs: &'a Blobs,
    doc: &'a F3dFile,
    timeline: &'a Timeline,
    position: usize,
}

/// [`resolve`] for one item.
fn resolve_item(item: &mut TimelineItem, at: &At, out: &mut Resolved) {
    let (blobs, doc) = (at.blobs, at.doc);
    let result = item.f3d.as_ref().and_then(|f| f.result_no);
    resolve_bodies(item, blobs, doc, out);
    resolve_path_edges(item, at, out);
    let Some(detail) = item.detail.as_mut() else {
        return;
    };
    if matches!(detail, Detail::Fillet(_) | Detail::Chamfer(_)) {
        let blobs = blobs.get(doc);
        expand_faces(blobs, result, detail, out);
    }
    // Faces of other components only where an item may take them (an
    // extrusion's extent or profile): a fillet's, chamfer's, thread's or
    // shell's inputs are on the bodies it changes.
    let others = matches!(detail, Detail::Extrude(_));
    let edges = named_entities(detail);
    if edges.is_empty() {
        return;
    }
    find_named(at, result, edges, true, others, out);
}

// Inputs found on several threads (mitcad#103).

/// [`resolve`] on up to `threads` threads: the items are independent of
/// each other (each reads the history's blobs, read once for all), so the
/// dump comes out the same; on large designs finding the inputs took over
/// a minute on one thread.
pub fn resolve_on(dump: &mut Dump, doc: &F3dFile, threads: usize) -> Resolved {
    if threads <= 1 {
        return resolve(dump, doc);
    }
    let Some(items) = dump.timeline.as_mut().and_then(|t| t.items.as_mut()) else {
        return Resolved::default();
    };
    let blobs = Blobs::default();
    let timeline = Timeline::new(items);
    let queue = std::sync::Mutex::new(items.iter_mut().enumerate());
    let found = std::sync::Mutex::new(Resolved::default());
    let work = || {
        let mut out = Resolved::default();
        loop {
            let next = queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .next();
            let Some((position, item)) = next else {
                break;
            };
            let at = At {
                blobs: &blobs,
                doc,
                timeline: &timeline,
                position,
            };
            resolve_item(item, &at, &mut out);
        }
        let mut found = found
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        found.found += out.found;
        found.not_found += out.not_found;
    };
    std::thread::scope(|scope| {
        for _ in 1..threads {
            // (One that does not start leaves its items to the others.)
            let _ = std::thread::Builder::new()
                .name("mitcad-import-inputs".to_owned())
                .stack_size(64 << 20)
                .spawn_scoped(scope, work);
        }
        work();
    });
    found
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// An input [`find_named`] looks for: the first state it was found in
/// (blob index, states before the state the pass starts from), else why
/// not.
struct Input<'f> {
    fp: &'f mut Fingerprint,
    twin: (usize, usize),
    entities: Vec<Vec<EntityName>>,
    why: String,
    hit: Option<(usize, usize)>,
}

/// An input not found yet, with its place among the item's inputs of the
/// same names ([`locate`]).
type Left<'f> = Vec<(&'f mut Fingerprint, (usize, usize))>;

/// Finds named edges and faces in the history state before the item
/// whose state is `result`, else up to [`EARLIER`] states before it; what
/// is in none of them, in the other components' histories at the item's
/// point of the timeline (mitcad#161: a sweep along an edge of another
/// component's body), else up to [`EARLIER`] states before that.
///
/// With `twins`, inputs of the same names take the edges that fit them
/// one each ([`locate`]; fillets and chamfers).
fn find_named(
    at: &At,
    result: Option<i32>,
    edges: Vec<&mut Fingerprint>,
    twins: bool,
    others: bool,
    out: &mut Resolved,
) {
    let blobs = at.blobs.get(at.doc);
    let result = result.filter(|&r| r > 0);
    // The blobs that hold the item's state, with its position (newest
    // first).
    let own: Vec<(usize, usize)> = match result {
        Some(result) => blobs
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                let pos = b
                    .history
                    .states
                    .iter()
                    .position(|s| s.id == i64::from(result))?;
                Some((i, pos))
            })
            .collect(),
        None => Vec::new(),
    };
    // Inputs with the same names: each the k-th of n.
    let keys: Vec<String> = edges
        .iter()
        .map(|fp| {
            let f = fp.f3d.as_ref();
            serde_json::to_string(&(
                fp.object_type.as_deref(),
                f.and_then(|f| f.entities.as_ref()),
            ))
            .unwrap_or_default()
        })
        .collect();
    let twins: Vec<(usize, usize)> = (0..keys.len())
        .map(|i| {
            if !twins {
                return (0, 1);
            }
            let k = keys[..i].iter().filter(|x| **x == keys[i]).count();
            (k, keys.iter().filter(|x| **x == keys[i]).count())
        })
        .collect();
    let mut left: Left = edges.into_iter().zip(twins).collect();
    match result {
        None => note_left(&mut left, "the item has no ASM state"),
        Some(r) if own.is_empty() => {
            note_left(&mut left, &format!("no ASM history holds state {r}"))
        }
        Some(r) => {
            // The state just before the item; for what is not there, the
            // earlier ones (some inputs name faces as an earlier state had
            // them).
            for earlier in [0..1, 1..EARLIER + 1] {
                if left.is_empty() {
                    break;
                }
                let views = earlier
                    .flat_map(|e| own.iter().map(move |&(b, pos)| (b, e, pos + 1 + e)))
                    .filter(|&(b, _, k)| k <= blobs[b].history.states.len());
                left = search(blobs, left, views, out, &|b, e| {
                    let entry = &blobs[b].entry;
                    match e {
                        0 => format!("{entry} before state {r}"),
                        e => format!("{entry} {e} states before state {r}"),
                    }
                });
            }
        }
    }
    if left.is_empty() || !others {
        out.not_found += left.len();
        return;
    }
    // Other components' histories at the item's point of the timeline:
    // the state after the last item of the blob's component before it.
    let links = at.timeline.links(blobs);
    let other_blobs: Vec<(usize, usize)> = (0..blobs.len())
        .filter(|b| !own.iter().any(|(o, _)| o == b))
        .filter_map(|b| Some((b, at.timeline.state_at(blobs, b, at.position)?)))
        .collect();
    let views = (0..=EARLIER)
        .flat_map(|e| other_blobs.iter().map(move |&(b, k)| (b, e, k + e)))
        .filter(|&(b, _, k)| k <= blobs[b].history.states.len());
    let position = at.position;
    let mut found = Vec::new();
    left = search_marked(blobs, left, views, &mut found, &|b, e| {
        let entry = &blobs[b].entry;
        match e {
            0 => format!("{entry} (another component's history) at item {position}"),
            e => format!("{entry} (another component's history) {e} states before item {position}"),
        }
    });
    for (fp, b) in found {
        if let Some(f) = fp.f3d.as_mut() {
            f.component = links[b];
        }
        out.found += 1;
    }
    out.not_found += left.len();
}

/// One pass of [`find_named`]: each input looks for its entity in the
/// states `views` give (blob, states before the pass's first state,
/// steps back from the blob's last state), in that order, and takes the
/// first it is in; `found` says where. Returns the inputs not found,
/// with the first reason.
fn search<'f>(
    blobs: &[Blob],
    left: Left<'f>,
    views: impl Iterator<Item = (usize, usize, usize)>,
    out: &mut Resolved,
    found: &dyn Fn(usize, usize) -> String,
) -> Left<'f> {
    let mut hits = Vec::new();
    let left = search_marked(blobs, left, views, &mut hits, found);
    out.found += hits.len();
    left
}

/// [`search`], giving each input found with its blob.
fn search_marked<'f>(
    blobs: &[Blob],
    left: Left<'f>,
    views: impl Iterator<Item = (usize, usize, usize)>,
    hits: &mut Vec<(&'f mut Fingerprint, usize)>,
    found: &dyn Fn(usize, usize) -> String,
) -> Left<'f> {
    // Each input takes the first state (in this order) it is found in, and
    // the first reason it is not. The states are named one at a time: all
    // of them at once (up to EARLIER per blob, on each of the threads of
    // `resolve_on`) took gigabytes on large designs and ran the process out
    // of memory (mitcad#132).
    let mut inputs: Vec<Input> = left
        .into_iter()
        .filter_map(|(fp, twin)| {
            let f = fp.f3d.as_mut()?;
            let entities = f.entities.clone().unwrap_or_default();
            let why = f.found.take().unwrap_or_default();
            Some(Input {
                fp,
                twin,
                entities,
                why,
                hit: None,
            })
        })
        .collect();
    for (b, e, k) in views {
        if inputs.iter().all(|i| i.hit.is_some()) {
            break;
        }
        let blob = &blobs[b];
        let view: View = blob.history.view(k);
        let state = NamedState::new(&blob.file, Some(&view));
        for input in inputs.iter_mut().filter(|i| i.hit.is_none()) {
            match locate(&state, input.fp, &input.entities, input.twin) {
                Ok(()) => input.hit = Some((b, e)),
                Err(err) if input.why.is_empty() => input.why = err,
                Err(_) => {}
            }
        }
    }
    let mut still = Vec::new();
    for Input {
        fp, twin, why, hit, ..
    } in inputs
    {
        let f = fp.f3d.as_mut().expect("named");
        match hit {
            Some((b, e)) => {
                f.found = Some(found(b, e));
                hits.push((fp, b));
            }
            None => {
                f.found = Some(why);
                still.push((fp, twin));
            }
        }
    }
    still
}

/// Notes why the inputs left were not found (the first reason each
/// keeps).
fn note_left(left: &mut Left, why: &str) {
    for (fp, _) in left.iter_mut() {
        if let Some(f) = fp.f3d.as_mut()
            && f.found.is_none()
        {
            f.found = Some(why.to_owned());
        }
    }
}

/// JSON pointers to the edge references with names and no geometry yet in
/// a value.
fn named_edges(v: &Value, at: &str, out: &mut Vec<String>) {
    named_refs(v, at, "edge", out);
}

/// JSON pointers to the references of a kind (`edge`, `face`) with names
/// and no geometry yet in a value.
fn named_refs(v: &Value, at: &str, kind: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            let named = m.get("kind").and_then(Value::as_str) == Some(kind)
                && m.contains_key("_f3d")
                && !m.contains_key("mid_point")
                && !m.contains_key("point_on_face");
            if named {
                out.push(at.to_owned());
                return;
            }
            for (k, x) in m {
                let k = k.replace('~', "~0").replace('/', "~1");
                named_refs(x, &format!("{at}/{k}"), kind, out);
            }
        }
        Value::Array(a) => {
            for (i, x) in a.iter().enumerate() {
                named_refs(x, &format!("{at}/{i}"), kind, out);
            }
        }
        _ => {}
    }
}

/// The edges of sweeps' and pipes' paths and guide rails and of lofts'
/// sections, centre lines and rails (mitcad#34), and of patterns' axes and
/// directions (mitcad#67), found as fillet edges are.
fn resolve_path_edges(item: &mut TimelineItem, at: &At, out: &mut Resolved) {
    let object_type = item.object_type().map(str::to_owned);
    if !matches!(
        object_type.as_deref(),
        Some(
            "SweepFeature"
                | "PipeFeature"
                | "LoftFeature"
                | "CircularPatternFeature"
                | "RectangularPatternFeature"
                | "MirrorFeature"
                | "SplitBodyFeature"
        )
    ) {
        return;
    }
    let Some(detail) = item.detail.as_ref() else {
        return;
    };
    let mut v = serde_json::to_value(detail).expect("serializes");
    let mut pointers = Vec::new();
    named_edges(&v, "", &mut pointers);
    // Faces of patterns, mirrors and splits: objects, axes, planes and
    // tools (mitcad#67).
    if !matches!(
        object_type.as_deref(),
        Some("SweepFeature" | "PipeFeature" | "LoftFeature")
    ) {
        named_refs(&v, "", "face", &mut pointers);
    }
    // A sweep's guide surfaces (mitcad#161).
    if object_type.as_deref() == Some("SweepFeature")
        && let Some(faces) = v.get("guideSurfaces")
    {
        named_refs(faces, "/guideSurfaces", "face", &mut pointers);
    }
    if pointers.is_empty() {
        return;
    }
    let mut fps: Vec<Fingerprint> = pointers
        .iter()
        .map(|p| {
            let map = v.pointer(p).and_then(Value::as_object).cloned();
            Fingerprint::from_map(map.unwrap_or_default())
        })
        .collect();
    let result = item.f3d.as_ref().and_then(|f| f.result_no);
    find_named(at, result, fps.iter_mut().collect(), false, true, out);
    for (p, fp) in pointers.iter().zip(&fps) {
        if let Some(slot) = v.pointer_mut(p) {
            *slot = serde_json::to_value(fp).expect("serializes");
        }
    }
    if let Value::Object(map) = v {
        item.detail = Some(Detail::typed(object_type.as_deref(), map));
    }
}

/// How many states before an item's own the names of its inputs are
/// looked for when the state just before it lacks them.
const EARLIER: usize = 24;

/// Calls `f` on every body reference with names and no points yet in a
/// JSON value (a body named only by the item that made it, whose recipe
/// does not decode, has no names: mitcad#96).
fn named_bodies(v: &mut Value, f: &mut dyn FnMut(&mut Map<String, Value>)) {
    match v {
        Value::Object(m) => {
            let named = m.get("kind").and_then(Value::as_str) == Some("body")
                && m.get("_f3d")
                    .is_some_and(|x| x.get("edge_points").is_none() && x.get("entities").is_some());
            if named {
                f(m);
            } else {
                for x in m.values_mut() {
                    named_bodies(x, f);
                }
            }
        }
        Value::Array(a) => {
            for x in a {
                named_bodies(x, f);
            }
        }
        _ => {}
    }
}

/// The bodies an item's detail names (moved, split, patterned, mirrored
/// bodies), found in the state just before the item: some of their edges'
/// middle points go into `_f3d.edge_points`.
fn resolve_bodies(item: &mut TimelineItem, blobs: &Blobs, doc: &F3dFile, out: &mut Resolved) {
    let Some(detail) = item.detail.as_ref() else {
        return;
    };
    let mut v = serde_json::to_value(detail).expect("serializes");
    let mut count = 0;
    named_bodies(&mut v, &mut |_| count += 1);
    if count == 0 {
        return;
    }
    let result = item.f3d.as_ref().and_then(|f| f.result_no).unwrap_or(-1);
    let blobs = blobs.get(doc);
    let views: Vec<(&Blob, View)> = blobs
        .iter()
        .filter_map(|b| {
            let pos = b
                .history
                .states
                .iter()
                .position(|s| s.id == i64::from(result))?;
            Some((b, b.history.view(pos + 1)))
        })
        .collect();
    let named: Vec<(&Blob, NamedState)> = views
        .iter()
        .map(|(b, view)| (*b, NamedState::new(&b.file, Some(view))))
        .collect();
    named_bodies(&mut v, &mut |m| {
        let mut fp = Fingerprint::from_map(std::mem::take(m));
        let f = fp.f3d.get_or_insert_with(Default::default);
        let entities = f.entities.clone().unwrap_or_default();
        let mut why = match result {
            r if r <= 0 => "the item has no ASM state".to_owned(),
            r if named.is_empty() => format!("no ASM history holds state {r}"),
            _ => String::new(),
        };
        let mut hit = None;
        for (b, state) in &named {
            match state.find_body(&entities) {
                Ok(i) => {
                    let points = state.body_points(i, 8);
                    if !points.is_empty() {
                        hit = Some((b.entry.as_str(), points));
                        break;
                    }
                    why = "no edge of the body has a middle point".to_owned();
                }
                Err(e) => why = e,
            }
        }
        match hit {
            Some((entry, points)) => {
                f.found = Some(format!("{entry} before state {result}"));
                f.edge_points = Some(points.iter().map(|p| p.map(|x| x / 10.0)).collect());
                out.found += 1;
            }
            None => {
                f.found = Some(why);
                out.not_found += 1;
            }
        }
        if let Value::Object(map) = serde_json::to_value(&fp).expect("serializes") {
            *m = map;
        }
    });
    if let Value::Object(map) = v {
        let object_type = item.object_type().map(str::to_owned);
        item.detail = Some(Detail::typed(object_type.as_deref(), map));
    }
}

/// The `.smbh` blobs of a document that parse and have a history.
fn history_blobs(doc: &F3dFile) -> Vec<Blob> {
    doc.body_blobs()
        .into_iter()
        .filter(|b| b.history)
        .filter_map(|b| {
            let data = doc.read(&b.entry).ok()?;
            let file = AsmFile::parse(&data).ok()?;
            let history = History::parse(&file).ok().flatten()?;
            let entry = b.entry.rsplit('/').next().unwrap_or(&b.entry).to_owned();
            Some(Blob {
                entry,
                file,
                history,
            })
        })
        .collect()
}
