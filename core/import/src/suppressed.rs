// SPDX-License-Identifier: MIT
//! Items suppressed in the file (mitcad#180): they come in as suppressed
//! features with their definitions, so that unsuppressing them in Mitcad
//! builds them.
//!
//! - A modelling item suppressed in the file (`isSuppressed`, which the
//!   stream decoder reads from the state bytes of the item's tail,
//!   [`mitcad_f3d::design::decode::item_state`]) is translated as any other
//!   item, and its first definition that builds is taken (the candidates a
//!   proven rule decoded first, no guesses: nothing could check one). The
//!   file keeps no result for it, so no history state checks the
//!   definition; its note says so ([`SUPPRESSED_UNCHECKED`]).
//! - Items suppressed in a row (a suppressed extrusion and the suppressed
//!   fillets of its edges, the suppressed sketch of a suppressed
//!   extrusion) stay built until the next item that is not suppressed, so
//!   that each is translated on the bodies the ones before it make, as
//!   they were made in the file; then they are suppressed together, the
//!   last first, and the bodies are as before them. An item whose
//!   definitions do not build on those bodies is tried again on the bodies
//!   without them (it may have been made while they were suppressed).
//! - Where no definition builds (no translation, only guesses, the
//!   definitions fail, or the import gave up on the item), the suppressed
//!   placeholder of mitcad#170 stays, its note giving the reason.
//! - Suppressed sketches and construction geometry come in as usual and
//!   are suppressed with their run.

use mitcad_f3d::design::decode::Suppression;
use mitcad_f3d::design::ir::TimelineItem;
use mitcad_model::FeatureUid;
use mitcad_model::features::FeatureDef;
use serde::Serialize;

use crate::history::{self, Sig};
use crate::report::Outcome;
use crate::{Candidate, Importer, halts, trace_clock, tracing};

/// A trace line with the time (`MITCAD_IMPORT_TRACE`).
macro_rules! trace {
    ($($arg:tt)*) => {
        if tracing() {
            eprintln!("import: [{:.2}] {}", trace_clock(), format!($($arg)*));
        }
    };
}

/// The note of an item suppressed in the file.
pub(crate) const SUPPRESSED: &str = "suppressed in the file";

/// The note of an item suppressed in the file that came in as a
/// suppressed feature.
pub(crate) const SUPPRESSED_UNCHECKED: &str =
    "suppressed in the file; not checked against the file's history";

/// The note of an item after the timeline marker in the file.
pub(crate) const ROLLED_BACK: &str = "rolled back in the file (after its timeline marker)";

/// The note of an item after the timeline marker in the file that came in
/// as a feature.
pub(crate) const ROLLED_BACK_UNCHECKED: &str =
    "rolled back in the file (after its timeline marker); not checked against the file's history";

/// What a placeholder's note adds for an item suppressed only through an
/// item it depends on: the faces its inputs name may be those of that
/// item's result, which no stored state holds.
const THROUGH_ANOTHER: &str = " (suppressed through an item it depends on)";

/// Why a suppressed item after the fallback to the stored bodies is not
/// translated: those bodies are the end of the timeline.
const AFTER_STORED: &str = "after the fallback to the stored bodies (no history to continue from)";

/// The items suppressed in the file in a row, built until the next item
/// that is not.
#[derive(Debug, Default)]
pub(crate) struct Run {
    /// The bodies before the run's first item.
    before: Option<Vec<Sig>>,
    /// The features the run's items made, in order.
    features: Vec<FeatureUid>,
    /// Why a fillet's or chamfer's edges found by their names did not
    /// build, from its last translation (its other definitions are the
    /// history's guesses).
    pub(crate) named_error: Option<String>,
    /// The timeline position of the first item after the marker in the
    /// file, when the timeline ends with such items only ([`rolled_from`]).
    pub(crate) rolled_from: Option<usize>,
    /// The number of features before that item's.
    marker: Option<usize>,
}

/// The timeline position of the first item after the timeline marker in
/// the file (rolled back), when every item from there on is rolled back
/// too or makes no feature; else none, and the items come in as they would
/// without the marker (an item of a component inserted from another design
/// comes after the root's last item and is not rolled back).
pub(crate) fn rolled_from(items: &[TimelineItem]) -> Option<usize> {
    let first = items.iter().position(|i| i.is_rolled_back == Some(true))?;
    items[first..]
        .iter()
        .all(|i| {
            i.is_rolled_back == Some(true)
                || matches!(crate::classify(i.object_type()), crate::Class::NoGeometry)
        })
        .then_some(first)
}

/// How the modelling items suppressed in the file came in (mitcad#180).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SuppressedReport {
    /// As suppressed features with their definitions.
    pub suppressed_as_features: usize,
    /// As suppressed placeholders (their definitions did not come in).
    pub suppressed_as_placeholders: usize,
    /// Of those two counts, the items suppressed only through an item they
    /// depend on (the state bytes `010000`); the others are
    /// suppressed themselves.
    #[serde(skip_serializing_if = "is_zero")]
    pub suppressed_indirectly: usize,
    /// Modelling items after the timeline marker in the file (rolled back),
    /// as features after the marker with their definitions.
    #[serde(skip_serializing_if = "is_zero")]
    pub rolled_back_as_features: usize,
    /// Those whose definitions did not come in (placeholders).
    #[serde(skip_serializing_if = "is_zero")]
    pub rolled_back_as_placeholders: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl SuppressedReport {
    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

impl<K: crate::ImportKernel> Importer<'_, K> {
    /// An item suppressed in the file is next: a run starts unless one is
    /// open.
    pub(crate) fn begin_suppressed_run(&mut self) {
        if self.suppressed.before.is_none() {
            self.suppressed.before = Some(self.current_sigs());
        }
    }

    /// An item that is not suppressed in the file is next (or the timeline
    /// ends): the open run's features are suppressed, the last first.
    pub(crate) fn close_suppressed_run(&mut self) {
        let Some(before) = self.suppressed.before.take() else {
            return;
        };
        let features = std::mem::take(&mut self.suppressed.features);
        if features.is_empty() {
            return;
        }
        for uid in features.iter().rev() {
            if let Err(e) = self.doc.set_suppressed(*uid, true) {
                let name = self.doc.feature(*uid).map(|f| f.name.clone());
                self.report.warnings.push(format!(
                    "{}, suppressed in the file, could not be suppressed: {e}",
                    name.as_deref().unwrap_or("?")
                ));
            }
        }
        let after = self.current_sigs();
        if !history::bodies_distance(&before, &after).is_some_and(|(_, exact)| exact) {
            self.report.warnings.push(format!(
                "the bodies after {} features suppressed in the file differ from the bodies \
                 before them",
                features.len()
            ));
        }
    }

    /// A sketch or construction feature suppressed in the file came in
    /// (report entry `at`): its features join the run.
    pub(crate) fn suppressed_in_run(&mut self, at: usize) {
        let item = &mut self.report.items[at];
        if item.features.is_empty() {
            return;
        }
        let uids: Vec<FeatureUid> = item
            .features
            .iter()
            .filter_map(|f| f.parse().ok())
            .collect();
        item.note = Some(match item.note.take() {
            Some(n) if !n.is_empty() => format!("{n}; {SUPPRESSED}"),
            _ => SUPPRESSED.to_owned(),
        });
        self.suppressed.features.extend(uids);
        self.note_suppressed(at);
    }

    /// A modelling item suppressed in the file (report entry `at`): a
    /// suppressed feature with its definition, else a placeholder.
    pub(crate) fn suppressed_item(&mut self, at: usize, index: i64, item: &TimelineItem) {
        // (A component made from bodies uses no sketch.)
        let originals = if item.object_type() == Some("ComponentFromBodies") {
            Vec::new()
        } else {
            self.sketches_into_component(at, index, item)
        };
        let mut tried = self.suppressed_definition(index, item);
        if let Err(e) = &tried
            && !halts(e)
            && !self.suppressed.features.is_empty()
        {
            // Perhaps made while the items before it were suppressed: on
            // the bodies without them; when it builds there, they are
            // suppressed and a run starts with it, else they stay built.
            trace!(
                "{}: on the bodies without the suppressed items before it",
                crate::item_name(item)
            );
            let depth = self.doc.undo_depth();
            let run = self.suppressed.features.clone();
            let all = run
                .iter()
                .rev()
                .all(|uid| self.doc.set_suppressed(*uid, true).is_ok());
            match all.then(|| self.suppressed_definition(index, item)) {
                Some(Ok(added)) => {
                    self.suppressed.features.clear();
                    tried = Ok(added);
                }
                _ => self.undo_to(depth),
            }
        }
        self.sketches.extend(originals);
        // Suppressed only through an item it depends on: the
        // item suppressed itself is before it, in this run or an earlier
        // one.
        let indirect = item.suppression() == Some(Suppression::Indirect);
        if indirect {
            self.report.suppressed.suppressed_indirectly += 1;
        }
        match tried {
            Ok((uids, c)) => {
                trace!(
                    "{}: suppressed in the file, came in as {}",
                    crate::item_name(item),
                    crate::summary(&c)
                );
                self.accept_unchecked(at, index, &uids, &c, SUPPRESSED_UNCHECKED);
                self.suppressed.features.extend(uids.iter().copied());
                self.note_suppressed(at);
                self.report.suppressed.suppressed_as_features += 1;
            }
            Err(reason) => {
                let through = if indirect { THROUGH_ANOTHER } else { "" };
                self.note(
                    at,
                    Outcome::Skipped,
                    format!("{SUPPRESSED}; its definition did not come in: {reason}{through}"),
                );
                self.placeholder(at);
                self.report.suppressed.suppressed_as_placeholders += 1;
            }
        }
    }

    /// An item whose definition `c` was taken without the file's history
    /// to check it (suppressed or rolled back in the file): its features
    /// `uids`, the report entry `at` with `note`.
    fn accept_unchecked(
        &mut self,
        at: usize,
        index: i64,
        uids: &[FeatureUid],
        c: &Candidate,
        note: &str,
    ) {
        let own = uids
            .iter()
            .copied()
            .find(|u| {
                !matches!(
                    self.doc.feature(*u).map(|f| &f.def),
                    Some(FeatureDef::Sketch(_))
                )
            })
            .or(uids.first().copied());
        if let Some(own) = own {
            // (Later items suppressed with it may refer to it.)
            self.features.insert(index, own);
            self.adopt(index, own);
        }
        self.learn_accepted(index, c, None);
        let report = &mut self.report.items[at];
        report.features.extend(uids.iter().map(ToString::to_string));
        report.verified = None;
        report.input_provenance = Some(c.input_provenance);
        let note = match c.note.as_deref().filter(|n| !n.is_empty()) {
            Some(n) => format!("{note}; {n}"),
            None => note.to_owned(),
        };
        self.note(at, Outcome::Parametric, note);
    }

    /// A modelling item after the timeline marker in the file (rolled
    /// back), report entry `at`: a feature with its definition
    /// built on the bodies as they are, which the marker is moved before
    /// at the end ([`Self::close_rolled_back`]); else a placeholder. No
    /// state of the file's history checks the definition (the file keeps
    /// none for it).
    pub(crate) fn rolled_back_item(&mut self, at: usize, index: i64, item: &TimelineItem) {
        let originals = if item.object_type() == Some("ComponentFromBodies") {
            Vec::new()
        } else {
            self.sketches_into_component(at, index, item)
        };
        let tried = self.suppressed_definition(index, item);
        self.sketches.extend(originals);
        match tried {
            Ok((uids, c)) => {
                trace!(
                    "{}: rolled back in the file, came in as {}",
                    crate::item_name(item),
                    crate::summary(&c)
                );
                self.accept_unchecked(at, index, &uids, &c, ROLLED_BACK_UNCHECKED);
                self.report.suppressed.rolled_back_as_features += 1;
            }
            Err(reason) => {
                self.note(
                    at,
                    Outcome::Skipped,
                    format!("{ROLLED_BACK}; its definition did not come in: {reason}"),
                );
                self.placeholder(at);
                self.report.suppressed.rolled_back_as_placeholders += 1;
            }
        }
    }

    /// The first item of the timeline's rolled-back end is next: the
    /// marker goes before the features it makes.
    pub(crate) fn begin_rolled_back(&mut self, position: usize) {
        if self.suppressed.rolled_from == Some(position) {
            self.suppressed.marker = Some(self.doc.marker());
        }
    }

    /// The timeline ends: the marker goes before the features of the items
    /// that were after it in the file, which are rolled back. Done before
    /// the final bodies are compared with the file's.
    pub(crate) fn close_rolled_back(&mut self) {
        let Some(marker) = self.suppressed.marker.take() else {
            return;
        };
        if let Err(e) = self.doc.set_marker(marker) {
            self.report
                .warnings
                .push(format!("the timeline marker could not be moved: {e}"));
        }
    }

    /// The first definition of a suppressed item that builds on the bodies
    /// as they are, added; Err with why there is none.
    fn suppressed_definition(
        &mut self,
        index: i64,
        item: &TimelineItem,
    ) -> Result<(Vec<FeatureUid>, Candidate), String> {
        if self.at_final {
            return Err(AFTER_STORED.to_owned());
        }
        if let Some(reason) = self.give_up(index) {
            return Err(reason);
        }
        // A fillet's or chamfer's edges found by their names only: the
        // history's guesses (edges the next states lost) are not worked
        // out, as none of them could be taken (the states past every
        // state already looked at).
        self.suppressed.named_error = None;
        let lazy = self.guesses.replace(crate::Guesses {
            states: usize::MAX,
            ..crate::Guesses::default()
        });
        let translated = match self.translate(index, item) {
            Err(_) if self.suppressed.named_error.is_some() => Err(format!(
                "its edges found by their names: {}",
                self.suppressed.named_error.take().unwrap_or_default()
            )),
            translated => translated,
        };
        self.guesses = lazy;
        let mut candidates: Vec<Candidate> = translated?.into_iter().filter(|c| !c.guess).collect();
        if candidates.is_empty() {
            return Err(match self.suppressed.named_error.take() {
                Some(e) => {
                    format!("its edges found by their names: {e}; no history to check a guess")
                }
                None => "only guesses, which need the file's history to check".to_owned(),
            });
        }
        // Those a proven rule decoded first, the others in their order.
        candidates.sort_by_key(|c| !c.first);
        candidates.truncate(self.options.max_candidates);
        let mut last = "nothing to try".to_owned();
        for c in candidates {
            match self.try_candidate(index, &c, item.name()) {
                Ok(uids) => return Ok((uids, c)),
                Err(e) => {
                    let halt = halts(&e);
                    last = e;
                    if halt {
                        break;
                    }
                }
            }
        }
        Err(last)
    }
}
