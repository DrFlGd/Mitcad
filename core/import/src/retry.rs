// SPDX-License-Identifier: MIT
//! Retries on the file's exact state (mitcad#283).
//!
//! An item that does not give its state on the replay's bodies is tried
//! again on the history's state before it exactly: where the bodies carry
//! an earlier item's approximation, or where the geometry kernel failed a
//! fillet or chamfer on them. The replay's bodies that are the state's
//! bodies already, exactly or within the import's geometric tolerance
//! (`Options::geometric_tolerance`, mitcad#224), were imported correctly:
//! they stay, parametric. Only the bodies that differ from the state are
//! brought to it, by a base feature; where nothing differs, there is no
//! retry, and an item that still fails takes the file's bodies as any item
//! that fails.
//!
//! Replacing the matching bodies too ([`RetryBodies::Replace`], as the
//! import did before) is the user's choice: the command line takes it as
//! an option, and the application asks the first time a retry would
//! replace a body that matches ([`RetryChoice::ask`]); the answer holds
//! for the rest of the import, its earlier tries and the documents it
//! refers to. The report names the bodies each retry kept and replaced
//! ([`crate::RetryReport`]).
//!
//! Every other base feature that brings the replay's bodies to a state of
//! the history or to the stored design keeps the bodies that match it the
//! same way (mitcad#285, [`Stay::Matching`]): an item's fallback, the
//! pending items' bodies, the state after an approximate item or after an
//! item that failed in the file, and the stored bodies at the end. Only the
//! bodies that differ from the state by more than the tolerance are
//! replaced.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use mitcad_model::{BodyUid, ComponentUid, FeatureUid};
use serde::{Deserialize, Serialize};

use crate::history::{Sig, StoredBody};
use crate::report::{RetryQuestionReport, RetryReport};
use crate::{ImportKernel, Importer, geometric, trace_clock, tracing};

/// What a retry on the file's exact state does with the replay's bodies
/// that match the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryBodies {
    /// They stay; only the bodies that differ are replaced (the default).
    #[default]
    Keep,
    /// The state's stored bodies replace them too.
    Replace,
}

impl RetryBodies {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Replace => "replace",
        }
    }

    /// `keep` or `replace`.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "keep" => Some(Self::Keep),
            "replace" => Some(Self::Replace),
            _ => None,
        }
    }
}

/// Puts the question of [`RetryChoice::ask`] to the user (the
/// application's import process, `mitcad-ffi`).
pub trait AskUser: Send + Sync {
    /// Asks `question` (one line).
    fn ask(&self, question: &str);
    /// The answer, `keep` or `replace`, once given; None while the
    /// question waits.
    fn answer(&self) -> Option<String>;
}

/// The choice of [`RetryBodies`] for an import: given, or asked once.
pub struct RetryChoice {
    chosen: Mutex<Option<RetryBodies>>,
    ask: Option<Box<dyn AskUser>>,
    /// Where it was asked, and the answer.
    asked: Mutex<Option<RetryQuestionReport>>,
}

impl fmt::Debug for RetryChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetryChoice")
            .field("chosen", &self.chosen())
            .field("asks", &self.ask.is_some())
            .finish()
    }
}

/// How often a question waiting for its answer looks for it.
const ANSWER_POLL: Duration = Duration::from_millis(50);

impl RetryChoice {
    /// The choice given.
    pub fn new(choice: RetryBodies) -> Self {
        Self {
            chosen: Mutex::new(Some(choice)),
            ask: None,
            asked: Mutex::new(None),
        }
    }

    /// Asked the first time a retry would replace a body that matches.
    pub fn ask(ask: Box<dyn AskUser>) -> Self {
        Self {
            chosen: Mutex::new(None),
            ask: Some(ask),
            asked: Mutex::new(None),
        }
    }

    /// The choice, once it is made (None: it is to be asked).
    pub fn chosen(&self) -> Option<RetryBodies> {
        *self.chosen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Where the user was asked, and the answer.
    pub fn asked(&self) -> Option<RetryQuestionReport> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The choice: made before, else asked (`question`, at item `item`)
    /// and waited for while `waiting` says so (the import's ticks its
    /// watchdog: the user may take long); not answered, the bodies are
    /// kept and the question stays open. (The import asks; `mitcad-ffi`
    /// asks a test question at the start for the application's UI test.)
    pub fn decide(
        &self,
        item: &str,
        question: &str,
        mut waiting: impl FnMut() -> bool,
    ) -> RetryBodies {
        if let Some(choice) = self.chosen() {
            return choice;
        }
        let Some(ask) = &self.ask else {
            return RetryBodies::Keep;
        };
        ask.ask(question);
        loop {
            if let Some(answer) = ask.answer() {
                let choice = RetryBodies::parse(&answer).unwrap_or_default();
                *self.chosen.lock().unwrap_or_else(PoisonError::into_inner) = Some(choice);
                *self.asked.lock().unwrap_or_else(PoisonError::into_inner) =
                    Some(RetryQuestionReport {
                        item: item.to_owned(),
                        answer: choice,
                    });
                return choice;
            }
            if !waiting() {
                return RetryBodies::Keep;
            }
            std::thread::sleep(ANSWER_POLL);
        }
    }
}

/// A body of the replay that matches a body of a history state: it stays
/// when the item is retried on the state ([`Stay::Paired`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pair {
    /// The state's body (its index in the state).
    pub target: usize,
    pub body: BodyUid,
    /// The same solid ([`Sig::same`]); else within the geometric tolerance.
    pub exact: bool,
}

/// Which of the replay's solids stay when the bodies are brought to a
/// history state (`Importer::replace_bodies_as`).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Stay<'p> {
    /// Those that match a body of the state, exactly or within the
    /// geometric tolerance ([`Importer::pair_within`]): every fallback's
    /// (mitcad#285). A body imported correctly is never replaced; only the
    /// bodies that differ (those of the items that failed among them) are.
    Matching,
    /// Those that measure as a body of the state ([`Sig::same`]): a retry
    /// whose user chose to replace the bodies within the tolerance.
    Same,
    /// None: every solid is replaced.
    Nothing,
    /// These, each for its body of the state.
    Paired(&'p [Pair]),
}

/// How a retry on the file's exact state treats the replay's bodies that
/// match the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Retry {
    /// They stay ([`Importer::exact_pairs`]); the others are replaced.
    Keeping,
    /// They are replaced too, as the user chose: every solid where `all`
    /// (a fillet or chamfer the kernel failed on bodies that are the
    /// state's), else those not the state's exactly.
    Replacing { all: bool },
    /// As [`Retry::Replacing`] once the user, asked now, chose so; else no
    /// retry.
    Asking { all: bool },
}

/// The most bodies a question names.
const NAMED: usize = 5;

impl<K: ImportKernel> Importer<'_, K> {
    /// The retries an item that failed has on the state before it, in order
    /// (`all`: where the import replaced every solid before, see
    /// [`Retry::Replacing`]). The bodies that match the state are found
    /// when a retry is made ([`Importer::exact_pairs`]).
    pub(crate) fn exact_retries(&mut self, all: bool) -> Vec<Retry> {
        self.exact_pairs = None;
        let chosen = match &self.options.retry_bodies {
            Some(choice) => choice.chosen(),
            None => Some(RetryBodies::Keep),
        };
        match chosen {
            Some(RetryBodies::Keep) => vec![Retry::Keeping],
            Some(RetryBodies::Replace) => vec![Retry::Replacing { all }],
            None => vec![Retry::Keeping, Retry::Asking { all }],
        }
    }

    /// The replay's bodies that match state `q` ([`Importer::exact_pairs`],
    /// found once for the item's retries).
    fn pairs_for(&mut self, q: usize) -> Vec<Pair> {
        if let Some(pairs) = &self.exact_pairs {
            return pairs.clone();
        }
        let pairs = self.match_state(q).unwrap_or_else(|e| {
            trace!("exact retry: state {q} not matched: {e}");
            Vec::new()
        });
        self.exact_pairs = Some(pairs.clone());
        pairs
    }

    /// The replay's solids that match a solid of state `q` in their
    /// component: the same solid ([`Sig::same`]), else one near it
    /// ([`Sig::matches`]) whose faces lie on its faces within the geometric
    /// tolerance ([`geometric::body_distance`]).
    fn match_state(&mut self, q: usize) -> Result<Vec<Pair>, String> {
        let kernel = self.doc.kernel();
        let state: Vec<(StoredBody<K::Shape>, Sig)> = self.oracle.state(kernel, q)?.to_vec();
        let current: Vec<(BodyUid, Sig, ComponentUid)> = self
            .current_bodies()
            .into_iter()
            .map(|(uid, sig)| {
                let c = self.doc.body_component(uid).unwrap_or(ComponentUid::ROOT);
                (uid, sig, c)
            })
            .collect();
        Ok(self.pair_within(&state, &current, &format!("state {q}")))
    }

    /// The stored body's signature of each body of the replay that matches a
    /// body of the state the replay is at within the geometric tolerance
    /// but is not the same solid (a body kept as the replay made it,
    /// mitcad#285): what finds bodies of later states by their measures
    /// compares the stored body with them, which a later state that leaves
    /// it alone holds exactly.
    pub(crate) fn stored_counterparts(&mut self) -> HashMap<BodyUid, Sig> {
        let Some(q) = self.oracle.cursor else {
            return HashMap::new();
        };
        let kernel = self.doc.kernel();
        let Ok(state) = self.oracle.state(kernel, q).map(<[_]>::to_vec) else {
            return HashMap::new();
        };
        let current: Vec<(BodyUid, Sig, ComponentUid)> = self
            .current_bodies()
            .into_iter()
            .map(|(uid, sig)| {
                let c = self.doc.body_component(uid).unwrap_or(ComponentUid::ROOT);
                (uid, sig, c)
            })
            .collect();
        self.pair_within(&state, &current, &format!("state {q}"))
            .into_iter()
            .filter(|p| !p.exact)
            .map(|p| (p.body, state[p.target].1))
            .collect()
    }

    /// The replay's solids (`current`: uid, signature, component) that
    /// match a body of `target` (named `what` in traces) in their
    /// component: the same solid ([`Sig::same`]), else one near it
    /// ([`Sig::matches`]) whose faces lie on its faces within the geometric
    /// tolerance ([`geometric::body_distance`]). Such a body was imported
    /// correctly: no fallback or retry replaces it (mitcad#283, mitcad#285).
    pub(crate) fn pair_within(
        &self,
        target: &[(StoredBody<K::Shape>, Sig)],
        current: &[(BodyUid, Sig, ComponentUid)],
        what: &str,
    ) -> Vec<Pair> {
        let kernel = self.doc.kernel();
        let components: Vec<ComponentUid> = target
            .iter()
            .map(|(b, _)| self.components.of_body(b.component))
            .collect();
        let mut used = vec![false; current.len()];
        let mut pairs = Vec::new();
        for (j, (_, sig)) in target.iter().enumerate() {
            if let Some(i) = (0..current.len())
                .find(|&i| !used[i] && current[i].2 == components[j] && current[i].1.same(sig))
            {
                used[i] = true;
                pairs.push(Pair {
                    target: j,
                    body: current[i].0,
                    exact: true,
                });
            }
        }
        let tolerance = geometric::tolerance();
        for (j, (body, sig)) in target.iter().enumerate() {
            if pairs.iter().any(|p| p.target == j) {
                continue;
            }
            let mut near: Vec<usize> = (0..current.len())
                .filter(|&i| !used[i] && current[i].2 == components[j] && current[i].1.matches(sig))
                .collect();
            near.sort_by(|&a, &b| {
                sig.distance(&current[a].1)
                    .total_cmp(&sig.distance(&current[b].1))
            });
            for i in near {
                let uid = current[i].0;
                let Some(shape) = self.doc.body_shape(uid) else {
                    continue;
                };
                match geometric::body_distance(kernel, shape, &body.shape) {
                    Ok(d) if d <= tolerance => {
                        trace!("{what}: {uid} is its body {j} within {d:.1e} mm");
                        used[i] = true;
                        pairs.push(Pair {
                            target: j,
                            body: uid,
                            exact: false,
                        });
                        break;
                    }
                    Ok(d) => trace!("{what}: {uid} is {d:.1e} mm off its body {j}"),
                    Err(e) => trace!("{what}: {uid} against its body {j}: {e}"),
                }
            }
        }
        pairs
    }

    /// Brings the bodies to state `q` for the retry of the item at `at`
    /// (named `name`) as `retry` says, and notes the bodies kept and
    /// replaced in its report. Ok(None): no retry (nothing differs, or the
    /// user chose to keep the bodies that match).
    pub(crate) fn bring_for_retry(
        &mut self,
        at: usize,
        q: usize,
        name: &str,
        retry: Retry,
    ) -> Result<Option<FeatureUid>, String> {
        let pairs = self.pairs_for(q);
        let (bodies, all) = match retry {
            Retry::Keeping => (RetryBodies::Keep, false),
            Retry::Replacing { all } => (RetryBodies::Replace, all),
            Retry::Asking { all } => {
                // Nothing to ask where replacing takes no body that matches
                // (the retry keeping them was the same).
                let matching: Vec<BodyUid> = pairs
                    .iter()
                    .filter(|p| all || !p.exact)
                    .map(|p| p.body)
                    .collect();
                if matching.is_empty()
                    || self.ask_retry_bodies(name, &matching) == RetryBodies::Keep
                {
                    return Ok(None);
                }
                (RetryBodies::Replace, all)
            }
        };
        // The replay's solids, and those that stay: the matching ones, or
        // (replaced) those that are the state's exactly, or none. (A body
        // a base feature replaces keeps its uid: they are told apart here.)
        let solids: Vec<(BodyUid, String)> = {
            let kernel = self.doc.kernel();
            self.doc
                .bodies()
                .iter()
                .filter(|b| Sig::of(kernel, b.shape).is_some())
                .map(|b| (b.uid, b.name.clone()))
                .collect()
        };
        let (stay, staying): (Stay<'_>, Vec<BodyUid>) = match (bodies, all) {
            (RetryBodies::Keep, _) => {
                (Stay::Paired(&pairs), pairs.iter().map(|p| p.body).collect())
            }
            (RetryBodies::Replace, true) => (Stay::Nothing, Vec::new()),
            (RetryBodies::Replace, false) => (
                Stay::Same,
                pairs.iter().filter(|p| p.exact).map(|p| p.body).collect(),
            ),
        };
        let brought = self.fallback_to_as(q, None, stay);
        let (kept, replaced): (Vec<_>, Vec<_>) = solids
            .into_iter()
            .partition(|(uid, _)| staying.contains(uid) || !matches!(brought, Ok(Some(_))));
        let names = |list: Vec<(BodyUid, String)>| -> Vec<String> {
            list.into_iter()
                .map(|(uid, n)| format!("{n} ({uid})"))
                .collect()
        };
        let report = RetryReport {
            state: q,
            bodies,
            kept: names(kept),
            replaced: names(replaced),
            matched: false,
        };
        trace!(
            "exact retry: {name} on state {q} ({}): kept {:?}, replaced {:?}",
            bodies.as_str(),
            report.kept,
            report.replaced
        );
        self.report.items[at].exact_retry = Some(report);
        brought
    }

    /// The item at `at` gave its state on the retry.
    pub(crate) fn retry_matched(&mut self, at: usize) {
        if let Some(retry) = &mut self.report.items[at].exact_retry {
            retry.matched = true;
        }
    }

    /// Whether the bodies that match the state, `matching`, are replaced
    /// for the retry of `name`: as chosen, else asked now.
    fn ask_retry_bodies(&mut self, name: &str, matching: &[BodyUid]) -> RetryBodies {
        let Some(choice) = self.options.retry_bodies.clone() else {
            return RetryBodies::Keep;
        };
        let matching: Vec<String> = matching.iter().map(|b| self.doc.body_name(*b)).collect();
        let mut named = matching
            .iter()
            .take(NAMED)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if matching.len() > NAMED {
            named = format!("{named} and {} more", matching.len() - NAMED);
        }
        let which = match matching.len() {
            1 => "an imported body that already matches them".to_owned(),
            n => format!("{n} imported bodies that already match them"),
        };
        let question = format!(
            "{name} does not give the file's result on the imported bodies. It can be tried \
             again on the file's stored bodies before it, which replace {which} (within {} \
             mm): {named}. Keep the imported bodies (they stay parametric; {name} comes in as \
             the file's bodies if it fails) or replace them with the stored bodies (the \
             features before {name} no longer shape them)? The answer applies to the rest of \
             this import.",
            geometric::tolerance()
        );
        trace!("exact retry: asked at {name}");
        let started = Instant::now();
        let choice = choice.decide(name, &question, || {
            self.tick();
            !self.abandoned() && !self.stop_requested()
        });
        // The wait is not the item's time nor the import's.
        let waited = started.elapsed();
        self.waited.fetch_add(
            u64::try_from(waited.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        if let Some(deadline) = self.deadline.as_mut() {
            *deadline += waited;
        }
        self.started += waited;
        trace!(
            "exact retry: the answer at {name} after {:.1} s: {}",
            waited.as_secs_f64(),
            choice.as_str()
        );
        choice
    }

    /// The retry of the item at `at` (named `name`) on state `s` exactly
    /// ([`Importer::verified_at`]; `all`: see [`Importer::exact_retries`]):
    /// `attempt` tries it on the bodies brought to the state. The first
    /// retry that gives its state stays; None when no retry was made.
    pub(crate) fn retry_on_state(
        &mut self,
        at: usize,
        s: usize,
        name: &str,
        all: bool,
        attempt: &mut dyn FnMut(&mut Self) -> Result<(), String>,
    ) -> Option<Result<(), String>> {
        let mut result = None;
        for retry in self.exact_retries(all) {
            let depth = self.doc.undo_depth();
            match self.bring_for_retry(at, s, name, retry) {
                Ok(Some(uid)) => {
                    trace!("{name}: tried again on state {s} exactly");
                    let tried = attempt(self);
                    if tried.is_ok() {
                        self.exact_before(uid, name);
                        self.retry_matched(at);
                        return Some(tried);
                    }
                    self.undo_to(depth);
                    result = Some(tried);
                }
                Ok(None) => self.undo_to(depth),
                Err(e) => {
                    trace!("{name}: the state before it did not come in exactly: {e}");
                    self.undo_to(depth);
                }
            }
        }
        result
    }
}
