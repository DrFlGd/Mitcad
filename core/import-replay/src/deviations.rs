// SPDX-License-Identifier: MIT
//! Results that deviate from the file's, kept at the user's choice
//! (mitcad#379).
//!
//! An item whose decoded definition builds but does not give the file's
//! result (its change of volume or its faces are off, see
//! [`crate::geometric`]) takes the file's bodies, as before. The import
//! records how far off the result is at that place
//! ([`DeviationReport`]): the volume it adds or removes against the
//! file's, the largest distance of its new faces from the file's and the
//! share of their area beyond the geometric tolerance, and the changed
//! bodies' volumes against the stored ones. The report lists every such
//! candidate.
//!
//! With [`AcceptDeviations`] the user keeps a candidate parametric
//! instead: the import adds its definition again and goes on from its
//! result. Its bodies then differ from the file's states by what the item
//! changed otherwise; for the comparisons with the history each such body
//! counts with that difference taken off ([`Offset`]), so that the items
//! after it are checked by what they change themselves. A later item that
//! does not give its state there takes the file's bodies as any item that
//! fails, and the stored body takes the deviated one's place from then on.

use std::collections::{BTreeSet, HashMap};

use mitcad_model::{BodyUid, ComponentUid, FeatureUid, Kernel};
use serde::Serialize;

use crate::history::Sig;
use crate::{Candidate, ImportKernel, Importer, geometric, trace_clock, tracing};

/// Which deviating results the import keeps parametric (mitcad#379).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AcceptDeviations {
    /// None: each takes the file's bodies (the default).
    #[default]
    None,
    /// Every one.
    All,
    /// The items with these timeline indices.
    Items(BTreeSet<i64>),
}

impl AcceptDeviations {
    /// Whether the deviating result of the item with timeline index
    /// `index` is kept.
    pub fn accepts(&self, index: i64) -> bool {
        match self {
            Self::None => false,
            Self::All => true,
            Self::Items(items) => items.contains(&index),
        }
    }

    /// `all`, `none`, or timeline indices separated by commas (`88,93`).
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "all" => Some(Self::All),
            "none" | "" => Some(Self::None),
            list => list
                .split(',')
                .map(|s| s.trim().parse::<i64>().ok())
                .collect::<Option<BTreeSet<i64>>>()
                .map(Self::Items),
        }
    }
}

/// A face of the design where a deviating result differs from the file's:
/// of the replay's result where it was kept, else of the file's bodies
/// that stand in for it. For the application to select.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviatingFace {
    /// The body's uid (`F12.b0`).
    pub body: String,
    /// The face's name.
    pub face: String,
}

/// How far an item's result is from the file's where it deviates
/// (mitcad#379): what the import's dialog shows per item.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DeviationReport {
    /// The volume the item adds (negative: removes), mm³.
    pub volume_change: f64,
    /// The volume the file's operation adds or removes, mm³.
    pub file_volume_change: f64,
    /// `volume_change - file_volume_change`, mm³.
    pub volume_difference: f64,
    /// That difference relative to the file's change; None when the file
    /// changes no volume.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_difference: Option<f64>,
    /// The largest distance (mm) of the result's new faces from the
    /// file's state after the item; None when the faces were not measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub largest_distance: Option<f64>,
    /// The share (0–1) of the new faces' area farther than the tolerance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area_beyond: Option<f64>,
    /// The same of the file's new faces against the result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_largest_distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_area_beyond: Option<f64>,
    /// The volume of the bodies the item changed, after it, mm³.
    pub body_volume: f64,
    /// The volume of the file's bodies its operation changed, after it.
    pub stored_body_volume: f64,
    /// The geometric tolerance the faces were measured with (mm).
    pub tolerance: f64,
    /// Why the result is not the file's.
    pub reason: String,
    /// Kept parametric at the user's choice.
    pub accepted: bool,
    /// A rounding the geometry kernel failed with all of its edges, built
    /// without these (their names in the replay).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_edges: Vec<String>,
    /// Faces of the design at the deviation (filled at the end of the
    /// import).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<DeviatingFace>,
}

impl DeviationReport {
    /// One line for reports and notes.
    pub fn summary(&self) -> String {
        let relative = self
            .relative_difference
            .map_or(String::new(), |r| format!(" ({:+.1} %)", 100.0 * r));
        let mut text = format!(
            "volume change {:.4} mm³ against the file's {:.4}{relative}",
            self.volume_change, self.file_volume_change
        );
        if !self.missing_edges.is_empty() {
            text = format!(
                "without {} of its edges ({}); {text}",
                self.missing_edges.len(),
                self.missing_edges.join(", ")
            );
        }
        if let Some(d) = self.largest_distance {
            text.push_str(&format!(
                "; its new faces up to {d:.3} mm off ({:.1} % of their area beyond {} mm)",
                100.0 * self.area_beyond.unwrap_or(0.0),
                self.tolerance
            ));
        }
        if let Some(d) = self.file_largest_distance {
            text.push_str(&format!(
                "; the file's new faces up to {d:.3} mm off ({:.1} % beyond)",
                100.0 * self.file_area_beyond.unwrap_or(0.0)
            ));
        }
        text.push_str(&format!(
            "; body volume {:.4} mm³ against the stored {:.4}",
            self.body_volume, self.stored_body_volume
        ));
        text
    }
}

/// What a body that carries an accepted deviation differs by from the
/// file's body it stands for: taken off its measures in the comparisons
/// with the history.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offset {
    volume: f64,
    area: f64,
    /// The first moment of volume (centre times volume).
    moment: [f64; 3],
    faces: i64,
}

impl Offset {
    /// From the replay's body `ours` to the file's `file`.
    fn between(ours: &Sig, file: &Sig) -> Self {
        Offset {
            volume: file.volume - ours.volume,
            area: file.area - ours.area,
            moment: [0, 1, 2].map(|i| file.center[i] * file.volume - ours.center[i] * ours.volume),
            faces: file.faces as i64 - ours.faces as i64,
        }
    }

    fn plus(&self, other: &Offset) -> Self {
        Offset {
            volume: self.volume + other.volume,
            area: self.area + other.area,
            moment: [0, 1, 2].map(|i| self.moment[i] + other.moment[i]),
            faces: self.faces + other.faces,
        }
    }

    fn minus(&self, other: &Offset) -> Self {
        Offset {
            volume: self.volume - other.volume,
            area: self.area - other.area,
            moment: [0, 1, 2].map(|i| self.moment[i] - other.moment[i]),
            faces: self.faces - other.faces,
        }
    }
}

/// The body of `among` nearest to `sig` that is not `used`.
fn nearest(sig: &Sig, among: &[Sig], used: &[bool]) -> Option<usize> {
    (0..among.len())
        .filter(|&j| !used[j])
        .min_by(|&a, &b| sig.distance(&among[a]).total_cmp(&sig.distance(&among[b])))
}

impl Offset {
    /// The measures of a body with the difference taken off.
    pub(crate) fn apply(&self, sig: &Sig) -> Sig {
        let volume = sig.volume + self.volume;
        if volume.is_nan() || volume <= 1e-9 {
            return *sig;
        }
        Sig {
            volume,
            area: sig.area + self.area,
            center: [0, 1, 2].map(|i| (sig.center[i] * sig.volume + self.moment[i]) / volume),
            faces: (sig.faces as i64 + self.faces).max(0) as usize,
        }
    }
}

/// The decoded definition of the item being replayed that built a result
/// the file's history did not take, recorded while its definitions were
/// tried: measured only if the item takes the file's bodies in the end.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// The document's undo depth before its features: it applies to the
    /// bodies as they were then.
    pub depth: usize,
    pub candidate: Candidate,
    /// The edges of the decoded rounding it leaves out (the kernel failed
    /// the rounding with them).
    pub missing: Vec<String>,
}

/// A deviating result measured.
#[derive(Debug, Clone)]
pub struct Measured {
    pub report: DeviationReport,
    /// Points where the result's new faces are off the file's (the
    /// farthest of each such face), and where the file's are off the
    /// result.
    pub ours: Vec<[f64; 3]>,
    pub theirs: Vec<[f64; 3]>,
}

/// Points measured on each new face, and in all for a side (a distance
/// from a large body's faces takes the kernel milliseconds a point).
const POINTS: usize = 8;
const ALL_POINTS: usize = 600;
/// The points of the faces off kept per side, for the faces to select.
const KEPT_POINTS: usize = 24;

/// One side's new faces measured against the other side: their area, the
/// area off, the largest distance and the farthest point of each face off.
#[derive(Debug, Default)]
struct Side {
    area: f64,
    off_area: f64,
    largest: Option<f64>,
    points: Vec<[f64; 3]>,
}

/// The new faces of `changed` (that the shapes `old` do not have), each
/// against the nearest of `other` by its farthest point.
fn side<K: Kernel>(
    kernel: &K,
    old: &[&K::Shape],
    changed: &[&K::Shape],
    other: &[&K::Shape],
) -> Result<Side, mitcad_model::KernelError> {
    let mut faces = Vec::new();
    for shape in changed {
        faces.extend(kernel.new_faces(old, shape, POINTS)?);
        crate::tick();
    }
    let each = (ALL_POINTS / faces.len().max(1)).clamp(4, POINTS);
    let tolerance = geometric::tolerance();
    let mut out = Side::default();
    let mut offs: Vec<(f64, [f64; 3])> = Vec::new();
    for f in &faces {
        let n = f.points.len();
        if n == 0 {
            continue;
        }
        let take = n.min(each);
        let points: Vec<[f64; 3]> = (0..take).map(|k| f.points[k * n / take]).collect();
        // Per point, the distance from the nearest body of the other side.
        let mut nearest = vec![f64::INFINITY; points.len()];
        for o in other {
            let d = kernel.boundary_distances(o, &points)?;
            crate::tick();
            for (a, b) in nearest.iter_mut().zip(d) {
                if !b.is_nan() {
                    *a = a.min(b);
                }
            }
        }
        let (k, worst) = nearest
            .iter()
            .copied()
            .enumerate()
            .fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
        // (No distance measured: the face is left out.)
        if !worst.is_finite() {
            continue;
        }
        out.area += f.area;
        out.largest = Some(out.largest.unwrap_or(0.0).max(worst));
        if worst > tolerance {
            out.off_area += f.area;
            offs.push((worst, points[k]));
        }
    }
    offs.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.points = offs.into_iter().take(KEPT_POINTS).map(|(_, p)| p).collect();
    Ok(out)
}

/// The bodies of `a` that `b` does not have as they are (the same solid,
/// [`Sig::same`]).
fn changed<'s, S>(a: &'s [(Sig, S)], b: &[(Sig, S)]) -> Vec<&'s (Sig, S)> {
    let mut used = vec![false; b.len()];
    a.iter()
        .filter(
            |(sig, _)| match (0..b.len()).find(|&j| !used[j] && b[j].0.same(sig)) {
                Some(j) => {
                    used[j] = true;
                    false
                }
                None => true,
            },
        )
        .collect()
}

fn volume<S>(bodies: &[&(Sig, S)]) -> f64 {
    bodies.iter().map(|(s, _)| s.volume).sum()
}

impl<K: ImportKernel> Importer<'_, K> {
    /// Records candidate `c`, which built a result the file's history did
    /// not take on the bodies at undo depth `depth`: the first decoded
    /// definition there (not a guess), or a rounding of some of the decoded
    /// rounding's edges where the kernel failed it with all (`missing`: the
    /// others).
    pub(crate) fn record_deviation(&mut self, c: &Candidate, depth: usize, missing: Vec<String>) {
        if (c.guess && missing.is_empty()) || self.deviations.iter().any(|r| r.depth == depth) {
            return;
        }
        self.deviations.push(Recorded {
            depth,
            candidate: c.clone(),
            missing,
        });
    }

    /// How far the result in the document now (made on the bodies
    /// `before`) is from the file's state `states.after` (`reason`: why it
    /// is not the file's); None when it changed no body or a state does not
    /// rebuild.
    fn measure_deviation(
        &mut self,
        name: &str,
        before: &[(BodyUid, K::Shape)],
        states: geometric::States,
        reason: &str,
    ) -> Option<Measured> {
        let clock = std::time::Instant::now();
        let kernel = self.doc.kernel();
        let measured = |shapes: &mut dyn Iterator<Item = K::Shape>| -> Vec<(Sig, K::Shape)> {
            shapes
                .filter_map(|s| Some((Sig::of(kernel, &s)?, s)))
                .collect()
        };
        let ours_before = measured(&mut before.iter().map(|(_, s)| s.clone()));
        let ours_after = measured(
            &mut crate::body_scope::bodies(self.doc)
                .iter()
                .map(|b| b.shape.clone()),
        );
        let mut state = |q: usize| -> Option<Vec<(Sig, K::Shape)>> {
            let bodies = self.oracle.state(kernel, q).ok()?;
            Some(
                bodies
                    .iter()
                    .map(|(b, sig)| (*sig, b.shape.clone()))
                    .collect(),
            )
        };
        let reference = match states.before {
            Some(s) => state(s),
            None => Some(Vec::new()),
        };
        let (Some(reference), Some(target)) = (reference, state(states.after)) else {
            return None;
        };
        let ours = changed(&ours_after, &ours_before);
        if ours.is_empty() {
            return None;
        }
        let gone = changed(&ours_before, &ours_after);
        let theirs = changed(&target, &reference);
        let replaced = changed(&reference, &target);
        let volume_change = volume(&ours) - volume(&gone);
        let file_volume_change = volume(&theirs) - volume(&replaced);
        let shapes = |bodies: &[(Sig, K::Shape)]| -> Vec<K::Shape> {
            bodies.iter().map(|(_, s)| s.clone()).collect()
        };
        let all_before = shapes(&ours_before);
        let all_after = shapes(&ours_after);
        let all_reference = shapes(&reference);
        let all_target = shapes(&target);
        fn as_refs<S>(v: &[S]) -> Vec<&S> {
            v.iter().collect()
        }
        let changed_ours: Vec<&K::Shape> = ours.iter().map(|(_, s)| s).collect();
        let changed_theirs: Vec<&K::Shape> = theirs.iter().map(|(_, s)| s).collect();
        // Each side against the bodies the other changed (else all of its
        // bodies): a distance from a large body's faces takes the kernel
        // milliseconds a point.
        let our_side = side(
            kernel,
            &as_refs(&all_before),
            &changed_ours,
            &if changed_theirs.is_empty() {
                as_refs(&all_target)
            } else {
                changed_theirs.clone()
            },
        );
        let their_side = side(
            kernel,
            &as_refs(&all_reference),
            &changed_theirs,
            &if changed_ours.is_empty() {
                as_refs(&all_after)
            } else {
                changed_ours.clone()
            },
        );
        let share = |s: &Side| (s.area > 0.0).then(|| s.off_area / s.area);
        let difference = volume_change - file_volume_change;
        let report = DeviationReport {
            volume_change,
            file_volume_change,
            volume_difference: difference,
            relative_difference: (file_volume_change.abs() > 1e-9)
                .then(|| difference / file_volume_change.abs()),
            largest_distance: our_side.as_ref().ok().and_then(|s| s.largest),
            area_beyond: our_side.as_ref().ok().and_then(share),
            file_largest_distance: their_side.as_ref().ok().and_then(|s| s.largest),
            file_area_beyond: their_side.as_ref().ok().and_then(share),
            body_volume: volume(&ours),
            stored_body_volume: volume(&theirs),
            tolerance: geometric::tolerance(),
            reason: reason.to_owned(),
            accepted: false,
            missing_edges: Vec::new(),
            faces: Vec::new(),
        };
        trace!(
            "{name}: deviating result measured in {:.2} s: {}",
            clock.elapsed().as_secs_f64(),
            report.summary()
        );
        Some(Measured {
            report,
            ours: our_side.map(|s| s.points).unwrap_or_default(),
            theirs: their_side.map(|s| s.points).unwrap_or_default(),
        })
    }

    /// The deviating result recorded on the bodies as they are now, if
    /// any.
    pub(crate) fn recorded_deviation(&self) -> Option<&Recorded> {
        let depth = self.doc.undo_depth();
        self.deviations.iter().rev().find(|r| r.depth == depth)
    }

    /// The recorded definition `c` added again, with a time of its own
    /// (the item's may be over: its definition built once already).
    fn again(
        &mut self,
        index: i64,
        c: &Candidate,
        item_name: Option<&str>,
    ) -> Result<Vec<FeatureUid>, String> {
        let budget = self.options.item_seconds;
        self.within(budget, |s| s.try_candidate(index, c, item_name))
    }

    /// The deviating result recorded on the bodies as they are now,
    /// built again and measured (then taken back): for the item named
    /// `name` (timeline index `index`) that takes the file's bodies.
    pub(crate) fn deviation_now(
        &mut self,
        index: i64,
        item_name: Option<&str>,
        states: geometric::States,
        reason: &str,
    ) -> Option<Measured> {
        let recorded = self.recorded_deviation()?.clone();
        let depth = self.doc.undo_depth();
        let name = item_name.unwrap_or("?").to_owned();
        let before = self.body_shapes();
        let measured = match self.again(index, &recorded.candidate, item_name) {
            Ok(_) => self
                .measure_deviation(&name, &before, states, reason)
                .map(|mut m| {
                    m.report.missing_edges = recorded.missing.clone();
                    m
                }),
            Err(e) => {
                trace!("{name}: its deviating result did not build again: {e}");
                None
            }
        };
        self.undo_to(depth);
        measured
    }

    /// Keeps the item at report entry `at` (timeline index `index`, state
    /// `state`) parametric with its recorded deviating result, when the
    /// options accept it: its definition is added again, the bodies it
    /// changed carry their difference from the file's state as an
    /// [`Offset`], and the report and the timeline say so. False when
    /// there is none to keep, it is not accepted or it does not build
    /// again (the item then takes the file's bodies).
    pub(crate) fn accept_deviation(
        &mut self,
        at: usize,
        index: i64,
        item_name: Option<&str>,
        states: geometric::States,
        reason: &str,
    ) -> bool {
        if !self.options.accept_deviations.accepts(index) {
            return false;
        }
        let state = states.after;
        let depth = self.doc.undo_depth();
        let Some(recorded) = self.recorded_deviation().cloned() else {
            return false;
        };
        let name = self.report.items[at].name.clone();
        let before = self.current_bodies();
        let before_shapes = self.body_shapes();
        let uids = match self.again(index, &recorded.candidate, item_name) {
            Ok(uids) => uids,
            Err(e) => {
                trace!("{name}: its deviating result did not build again: {e}");
                self.undo_to(depth);
                return false;
            }
        };
        let Some(measured) = self.measure_deviation(&name, &before_shapes, states, reason) else {
            self.undo_to(depth);
            return false;
        };
        let after = self.current_bodies();
        let kernel = self.doc.kernel();
        let Some(target) = self.oracle.try_sigs(kernel, state) else {
            self.undo_to(depth);
            return false;
        };
        let reference = match states.before {
            Some(q) => self.oracle.try_sigs(kernel, q).unwrap_or_default(),
            None => Vec::new(),
        };
        // The bodies it left as they were pair with the state's first; each
        // body it changed with the nearest of the state's others.
        let unchanged =
            |uid: &BodyUid, sig: &Sig| before.iter().any(|(u, s)| u == uid && s.same(sig));
        let mut used = vec![false; target.len()];
        for (uid, sig) in &after {
            if unchanged(uid, sig)
                && let Some(j) = nearest(sig, &target, &used)
            {
                used[j] = true;
            }
        }
        let mut offsets = self.deviated.clone();
        for (uid, sig) in &after {
            if unchanged(uid, sig) {
                continue;
            }
            let Some(j) = nearest(sig, &target, &used) else {
                continue;
            };
            used[j] = true;
            // What it changed of the body against what the file's
            // operation changed of it: a difference the body carried
            // before (an approximation of an earlier item's) stays as the
            // replay has it.
            let mut offset = Offset::between(sig, &target[j]);
            let was = before.iter().find(|(u, _)| u == uid).map(|(_, s)| s);
            if let Some(was) = was
                && let Some(k) = nearest(was, &reference, &vec![false; reference.len()])
            {
                offset = offset.minus(&Offset::between(was, &reference[k]));
            }
            let total = match offsets.get(uid) {
                Some(old) => old.plus(&offset),
                None => offset,
            };
            trace!(
                "{name}: {uid} carries its accepted deviation: {:+.4} mm³ from the file's body",
                -total.volume
            );
            offsets.insert(*uid, total);
        }
        self.set_deviated(offsets);
        self.oracle.cursor = Some(state);
        self.accepted(at, index, &uids, &recorded.candidate, Some(false));
        let mut report = measured.report;
        report.missing_edges = recorded.missing.clone();
        report.accepted = true;
        let note = format!(
            "its result deviates from the file's; kept parametric at the user's choice: {}",
            report.summary()
        );
        self.note(at, crate::Outcome::Parametric, note);
        self.report.items[at].accepted_deviation = Some(report);
        self.deviation_points
            .push((at, measured.ours, self.component));
        trace!("{name}: its deviating result is kept parametric");
        true
    }

    /// The item at report entry `at` took the file's bodies: its deviating
    /// result `measured` is listed with it.
    pub(crate) fn list_deviation(&mut self, at: usize, measured: Measured) {
        self.report.items[at].deviation = Some(measured.report);
        self.deviation_points
            .push((at, measured.theirs, self.component));
    }

    /// The bodies that carry accepted deviations become `offsets` (kept to
    /// be undone with the document, [`Importer::undo_deviated`]).
    pub(crate) fn set_deviated(&mut self, offsets: HashMap<BodyUid, Offset>) {
        if offsets == self.deviated {
            return;
        }
        let old = std::mem::replace(&mut self.deviated, offsets);
        self.deviated_log.push((self.doc.undo_depth(), old));
    }

    /// Bodies a base feature replaced carry no deviation any more.
    pub(crate) fn not_deviated(&mut self, replaced: &[BodyUid]) {
        if !replaced.iter().any(|u| self.deviated.contains_key(u)) {
            return;
        }
        let mut offsets = self.deviated.clone();
        for uid in replaced {
            offsets.remove(uid);
        }
        self.set_deviated(offsets);
    }

    /// The deviations as they were at undo depth `depth`.
    pub(crate) fn undo_deviated(&mut self, depth: usize) {
        while let Some((d, _)) = self.deviated_log.last() {
            if *d <= depth {
                break;
            }
            let (_, old) = self.deviated_log.pop().expect("an entry");
            self.deviated = old;
        }
    }

    /// The measures of the bodies `bodies` with the accepted deviations
    /// they carry taken off.
    pub(crate) fn corrected(&self, bodies: &mut [(BodyUid, Sig)]) {
        if self.deviated.is_empty() {
            return;
        }
        for (uid, sig) in bodies {
            if let Some(o) = self.deviated.get(uid) {
                *sig = o.apply(sig);
            }
        }
    }

    /// The faces of the design at each deviation (the end of the import):
    /// those of the item's component whose surface passes through a point
    /// recorded off the other side.
    pub(crate) fn deviation_faces(&mut self) {
        let points = std::mem::take(&mut self.deviation_points);
        for (at, points, component) in points {
            let faces = self.faces_at(&points, component);
            let item = &mut self.report.items[at];
            if let Some(d) = item.accepted_deviation.as_mut() {
                d.faces = faces;
            } else if let Some(d) = item.deviation.as_mut() {
                d.faces = faces;
            }
        }
    }

    fn faces_at(&self, points: &[[f64; 3]], component: ComponentUid) -> Vec<DeviatingFace> {
        // (On the face within the exact tolerance of the lookups.)
        const ON: f64 = 1e-3;
        let kernel = self.doc.kernel();
        let mut out: Vec<DeviatingFace> = Vec::new();
        for body in crate::body_scope::bodies(self.doc) {
            if self
                .doc
                .body_component(body.uid)
                .unwrap_or(ComponentUid::ROOT)
                != component
            {
                continue;
            }
            let Ok(faces) = kernel.faces(body.shape) else {
                continue;
            };
            for face in faces {
                let Some(name) = face
                    .names
                    .first()
                    .and_then(|n| n.parse::<mitcad_model::FaceName>().ok())
                else {
                    continue;
                };
                crate::tick();
                let on = points.iter().any(|p| {
                    kernel
                        .face_point_normal(body.shape, &name, *p)
                        .is_ok_and(|near| crate::geom::distance(near.point, *p) <= ON)
                });
                if on {
                    out.push(DeviatingFace {
                        body: body.uid.to_string(),
                        face: name.to_string(),
                    });
                }
            }
        }
        out
    }

    /// The deviations of the import, for the timeline's notes: the
    /// features of each item kept parametric with one.
    pub(crate) fn deviation_notes(&self) -> Vec<(FeatureUid, String)> {
        let mut out = Vec::new();
        for item in &self.report.items {
            let Some(d) = &item.accepted_deviation else {
                continue;
            };
            let note = format!(
                "Deviation from the file accepted at import: {}",
                d.summary()
            );
            for uid in item.features.iter().filter_map(|f| f.parse().ok()) {
                out.push((uid, note.clone()));
            }
        }
        out
    }
}
