// SPDX-License-Identifier: MIT
//! An item's definitions evaluated ahead, in parallel (mitcad#95).
//!
//! The importer tries an item's candidate definitions one after another
//! and takes the first that gives the history's state; most of an
//! import's time goes to the definitions it rejects. With
//! [`crate::Options::threads`] above one, while the importer evaluates
//! candidate *k* on its document (from the second candidate of a list on:
//! the first is usually the one taken), workers evaluate the candidates
//! after it, each on its own copy of the document as it is before the item
//! ([`Document::fork`]: the same definition and cached results, shared; a
//! kernel of its own, [`Kernel::fork`]). The importer still decides in
//! rank order, exactly as without workers: when it comes to a candidate a
//! worker has evaluated (or waits for it), it takes the worker's result
//! instead of evaluating it again, and the worker's results go into the
//! document's cache ([`Document::adopt_results`]) as if evaluated there.
//! A rejected candidate leaves the document as it is (it was undone
//! anyway); the one accepted is added to the document again and reuses
//! the worker's results, so its features, uids, topological names and
//! report notes are those of the run without workers. Once the importer
//! leaves the candidates (one was accepted, or none is left), the workers
//! still evaluating are cancelled through their monitors, as a stop
//! cancels a definition.
//!
//! A worker's result stands in only where the run without workers would
//! have had the same one: the importer gives it the first round's share
//! of the item's time ([`SHARE`]) by waiting for it no longer than that
//! from when it began, and takes it as given way then. The worker itself
//! has the item's time (mitcad#874): a definition that gave way goes on
//! there, and the second round takes its result from there instead of
//! evaluating it again from the start. So a definition given a share in
//! the first round goes to a worker of its own too, the first of a list
//! included, when one is free (a pattern that takes 37 s spent its share
//! of 20 s and then 37 s again in the second round, 58 s of its item's
//! 60). The decisions stay those of the rounds; only the time already
//! run is not spent twice. Results that depend on the time (definitions
//! cut short by the item's time) can differ, as they do between two runs
//! without workers.
//!
//! Only the plain candidate lists are evaluated ahead
//! ([`Importer::try_state`], and each set of bodies of an item without a
//! state of its own): the closer definition after a loose match, the
//! edits of a pattern's extrusions and the definitions on bodies the
//! history holds already stay on the importer's thread, and so does the
//! second round of a definition that gave way on the document (no worker
//! was free).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::history::Sig;
use crate::{
    CANDIDATE_OUT_OF_TIME, Candidate, GAVE_WAY, Importer, LOW_MEMORY, OUT_OF_TIME, Progress, SHARE,
    STOPPED, Ticking, candidate_seconds, seconds, summary, trace_clock, tracing,
};
use mitcad_import_base::progress::{AHEAD_MEMORY, Helper};
use mitcad_model::{
    BodyUid, ComponentUid, Document, FeatureUid, Kernel, KernelError, RecomputeMonitor,
};

/// A kernel the import can use: its documents may go to the threads that
/// evaluate definitions ahead (their shapes are read from several threads
/// at once; [`Kernel::fork`] says whether it can work on another thread).
pub trait ImportKernel: Kernel<Shape: Send + Sync> + Send + 'static {}

impl<K> ImportKernel for K
where
    K: Kernel + Send + 'static,
    K::Shape: Send + Sync,
{
}

/// A job for a new thread.
pub type WorkerJob = Box<dyn FnOnce() + Send + 'static>;

/// Starts the threads that evaluate definitions ahead
/// ([`crate::Options::spawn`]): runs the job on a new thread, false when
/// none could be started (the definition is then evaluated on the import's
/// thread). `mitcad-ffi` installs the geometry kernel's crash handlers on
/// them first.
#[derive(Clone)]
pub struct Spawner(pub Arc<dyn Fn(WorkerJob) -> bool + Send + Sync>);

impl std::fmt::Debug for Spawner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Spawner")
    }
}

/// The stack of a worker: 64 MiB. The import's own thread has 256 MiB
/// (`mitcad-ffi`: the geometry kernel recurses deeply, and an overflow
/// fails a definition), but a worker's stack counts against the memory
/// limits the import measures itself against (an address-space or data
/// limit counts reserved stacks whole), and seven of 256 MiB would take a
/// corpus run's whole limit. The kernel's own worker threads (its parallel
/// booleans) run on the system's default stack of a few MiB.
pub const WORKER_STACK: usize = 64 << 20;

impl Spawner {
    /// Plain threads with [`WORKER_STACK`].
    pub fn threads() -> Self {
        Spawner(Arc::new(|job: WorkerJob| {
            std::thread::Builder::new()
                .name("mitcad-import-worker".to_owned())
                .stack_size(WORKER_STACK)
                .spawn(job)
                .is_ok()
        }))
    }
}

/// Worker threads of all imports that still run: also those whose result is
/// no longer wanted, whose kernel call has not returned yet. A process
/// must not run its static destructors under them (`mitcad-ffi`,
/// `abandoned_imports`).
static RUNNING: AtomicUsize = AtomicUsize::new(0);

/// How many threads that evaluate definitions ahead still run, in all
/// imports of the process.
pub fn running_workers() -> usize {
    RUNNING.load(Ordering::SeqCst)
}

/// Counts a running worker until it is dropped (its thread's last act).
struct Running(Arc<AtomicUsize>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
        RUNNING.fetch_sub(1, Ordering::SeqCst);
    }
}

/// How often the importer looks for a stop while it waits for a worker.
const WAIT_STEP: Duration = Duration::from_millis(20);

/// What a worker found for a candidate.
pub(crate) struct Done<K: Kernel> {
    added: Result<Vec<FeatureUid>, String>,
    /// The solids afterwards (when added).
    bodies: Vec<(BodyUid, Sig)>,
    /// Seconds it evaluated, on the import's clock ([`crate::now`]) and on
    /// the wall clock (for the trace), and when it ended (the import's).
    ran: f64,
    wall: Duration,
    ended: Instant,
    /// Which of its deadlines had passed when it ended: the item's, its
    /// own ([`candidate_seconds`]).
    past_item: bool,
    past_own: bool,
    /// The worker's document, whose results an accepted candidate adopts.
    doc: Document<K>,
}

/// A candidate given to a worker. The worker is given the item's time
/// (and the definition's own), not a share of it: the importer gives it
/// its share by waiting for it no longer, and a definition that gave way
/// so goes on on its worker for the second round (mitcad#874), instead of
/// starting there again from nothing.
struct Job<K: Kernel> {
    /// Cancels it (no longer wanted).
    cancel: Arc<RecomputeMonitor>,
    done: mpsc::Receiver<Done<K>>,
    /// Its result, once it came after the importer had given it way.
    finished: Option<Done<K>>,
    /// When it was given to the worker, and the end of the time it is
    /// given of its own ([`candidate_seconds`]); on the import's clock.
    started: Instant,
    own: Option<Instant>,
}

impl<K: Kernel> Job<K> {
    /// When the importer stops waiting for the worker: `grace` seconds
    /// after the end of the time the worker was given (the item's
    /// `deadline`, or its own); None without any. A kernel call that does
    /// not check for a request to stop runs on past the end of its time
    /// (one ran for hours, mitcad#434); the importer does not wait for it.
    fn wait_limit(&self, deadline: Option<Instant>, grace: f64) -> Option<Instant> {
        [deadline, self.own]
            .into_iter()
            .flatten()
            .min()
            .map(|end| end + seconds(grace))
    }

    /// Whether it has a result or is still evaluating (it has not ended
    /// without one).
    fn alive(&mut self) -> bool {
        if self.finished.is_some() {
            return true;
        }
        match self.done.try_recv() {
            Ok(done) => {
                self.finished = Some(done);
                true
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        }
    }
}

/// The candidates of one list being evaluated ahead; dropping it cancels
/// the workers still evaluating.
pub(crate) struct Ahead<K: Kernel> {
    jobs: BTreeMap<usize, Job<K>>,
    /// The next candidate to give a worker.
    next: usize,
    /// No more workers (the kernel has no fork).
    off: bool,
    /// A definition of the list was evaluated (and not taken).
    tried: bool,
}

impl<K: Kernel> Ahead<K> {
    /// A candidate the importer passes over: its worker is not needed.
    pub(crate) fn forget(&mut self, k: usize) {
        if let Some(job) = self.jobs.remove(&k) {
            job.cancel.cancel();
        }
    }

    /// Whether candidate `k` gave way at its share and goes on on its
    /// worker (or has ended there with a result): the second round takes
    /// it from there, however long it ran already.
    pub(crate) fn continuing(&mut self, k: usize) -> bool {
        self.jobs.get_mut(&k).is_some_and(Job::alive)
    }
}

impl<K: Kernel> Drop for Ahead<K> {
    fn drop(&mut self) {
        for job in self.jobs.values() {
            job.cancel.cancel();
        }
    }
}

/// A candidate evaluated, on the document or by a worker.
pub(crate) struct Evaluated {
    pub(crate) result: Result<Vec<FeatureUid>, String>,
    /// A worker's: the solids its features left (when added), which the
    /// document does not hold yet ([`Importer::take_over`]).
    pub(crate) bodies: Option<Vec<(BodyUid, Sig)>>,
    /// A worker's: the seconds it evaluated (the import's clock).
    pub(crate) ran: Option<f64>,
}

impl Evaluated {
    /// Whether the candidate's features are only in a worker's document.
    pub(crate) fn ahead(&self) -> bool {
        self.bodies.is_some()
    }
}

/// What adding a candidate's features needs besides the document: the
/// item's component, the monitor its evaluation runs under (None: the
/// document's own), and the import's progress.
pub struct Adding<'p> {
    pub component: ComponentUid,
    pub monitor: Option<Arc<RecomputeMonitor>>,
    pub progress: Option<&'p Progress>,
    /// Whether an operation that runs out of memory tells the import that
    /// it is low on memory ([`Progress::low_memory`]; not a worker's,
    /// which is evaluated again on the import's thread).
    pub memory: bool,
}

/// Evaluates a candidate on a worker's document, within the item's time
/// (`item_deadline`) and its own (`own`).
#[allow(clippy::too_many_arguments)]
fn evaluate_on<K: Kernel>(
    mut doc: Document<K>,
    scope: crate::body_scope::Scope,
    candidate: &Candidate,
    name: Option<&str>,
    component: ComponentUid,
    parents: Vec<Arc<RecomputeMonitor>>,
    item_deadline: Option<Instant>,
    own: Option<Instant>,
    progress: Option<Arc<Progress>>,
) -> Done<K> {
    let _scope = scope.enter(&doc);
    // Its ticks are the import's (the watchdog's), its kernel calls too.
    let _ticking = Ticking::new(progress.clone());
    let clock = crate::now();
    let wall = Instant::now();
    let deadline = [item_deadline, own].into_iter().flatten().min();
    let adding = Adding {
        component,
        monitor: Some(Arc::new(RecomputeMonitor::within(parents, deadline))),
        progress: progress.as_deref(),
        memory: false,
    };
    let added = crate::add_all(&mut doc, candidate, name, &adding);
    let bodies = if added.is_ok() {
        crate::bodies_of(&doc)
    } else {
        Vec::new()
    };
    let ended = crate::now();
    let past = |d: Option<Instant>| d.is_some_and(|d| ended >= d);
    Done {
        added,
        bodies,
        ran: ended.saturating_duration_since(clock).as_secs_f64(),
        wall: wall.elapsed(),
        ended,
        past_item: past(item_deadline),
        past_own: past(own),
        doc,
    }
}

impl<K: crate::ImportKernel> Importer<'_, K> {
    /// Workers for a list of candidates ([`crate::Options::threads`]);
    /// None without.
    pub(crate) fn ahead(&self) -> Option<Ahead<K>> {
        (self.options.threads > 1).then(|| Ahead {
            jobs: BTreeMap::new(),
            next: 0,
            off: false,
            tried: false,
        })
    }

    /// Whether another worker may start: fewer run than the threads allow
    /// (the importer's own is one of them), and the process has memory to
    /// spare.
    fn worker_free(&self) -> bool {
        if self.workers.load(Ordering::SeqCst) + 1 >= self.options.threads {
            return false;
        }
        self.options
            .progress
            .as_ref()
            .is_none_or(|p| !p.is_memory_low() && p.memory_share() < AHEAD_MEMORY)
    }

    /// A thread of the import's budget for a worker (shared with the
    /// bodies built ahead, mitcad#103): None when there is no room; `Some(None)`
    /// without a progress to share it (then [`Importer::worker_free`]
    /// counts the workers alone).
    fn worker_helper(&self) -> Option<Option<Helper>> {
        match &self.options.progress {
            Some(p) => p.helper().map(Some),
            None => Some(None),
        }
    }

    /// Gives the candidates after `k` (up to `until`) to free workers.
    fn launch(
        &mut self,
        ahead: &mut Ahead<K>,
        candidates: &[Candidate],
        k: usize,
        until: usize,
        name: Option<&str>,
    ) {
        ahead.next = ahead.next.max(k + 1);
        while !ahead.off && ahead.next < until.min(candidates.len()) && self.worker_free() {
            let j = ahead.next;
            if !self.launch_one(ahead, candidates, j, name) {
                break;
            }
            trace!(
                "{}: definition {} evaluated ahead",
                name.unwrap_or("?"),
                j + 1
            );
            ahead.next += 1;
        }
    }

    /// Gives candidate `j` to a worker: whether one took it.
    fn launch_one(
        &mut self,
        ahead: &mut Ahead<K>,
        candidates: &[Candidate],
        j: usize,
        name: Option<&str>,
    ) -> bool {
        let Some(helper) = self.worker_helper() else {
            return false;
        };
        let Some(doc) = self.doc.fork() else {
            ahead.off = true;
            return false;
        };
        let cancel = Arc::new(RecomputeMonitor::new());
        let mut parents = self.try_parents();
        parents.push(Arc::clone(&cancel));
        parents.extend(
            self.options
                .progress
                .as_ref()
                .and_then(|p| p.ahead_monitor()),
        );
        let (sender, done) = mpsc::channel();
        let candidate = candidates[j].clone();
        let scope = self.body_scope.scope();
        let started = crate::now();
        let own = candidate_seconds(&candidate, self.options).map(|s| started + seconds(s));
        let owned_name = name.map(str::to_owned);
        let component = self.component;
        let deadline = self.deadline;
        let tolerance = self.options.geometric_tolerance;
        let progress = self.options.progress.clone();
        self.workers.fetch_add(1, Ordering::SeqCst);
        RUNNING.fetch_add(1, Ordering::SeqCst);
        let running = Running(Arc::clone(&self.workers));
        let job: WorkerJob = Box::new(move || {
            let _running = running;
            let _helper = helper;
            let _tolerance = crate::refs::GeometricTolerance::set(tolerance);
            let done = evaluate_on(
                doc,
                scope,
                &candidate,
                owned_name.as_deref(),
                component,
                parents,
                deadline,
                own,
                progress,
            );
            // (Nobody waits once the importer has moved on.)
            let _ = sender.send(done);
        });
        let spawner = self.options.spawn.clone().unwrap_or_else(Spawner::threads);
        // A job that does not start is dropped, and with it its count.
        if !(spawner.0)(job) {
            ahead.off = true;
            return false;
        }
        ahead.jobs.insert(
            j,
            Job {
                cancel,
                done,
                finished: None,
                started,
                own,
            },
        );
        true
    }

    /// Evaluates candidate `k` of `candidates` (the modelling item `index`)
    /// for at most `share` seconds ([`Importer::try_candidate_for`]): with
    /// `ahead`, the candidates after it, up to `until`, go to free workers
    /// first, and a worker's result for it stands in where it is the one
    /// the document would give. A worker's result that is accepted must
    /// then be adopted ([`Importer::take_over`]).
    ///
    /// A definition given a share in the first round goes to a worker of
    /// its own when one is free (mitcad#874): cut short at its share it
    /// goes on there, and the second round (`again`) takes its result from
    /// there instead of evaluating it again from the start (a pattern of
    /// threads that takes 37 s in a debug build spent its share of 20 s
    /// and then the 37 s again, 58 s of its item's 60). The decisions are
    /// those of the rounds without workers; only the time a definition cut
    /// short at its share already ran is not lost.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate(
        &mut self,
        ahead: Option<&mut Ahead<K>>,
        candidates: &[Candidate],
        k: usize,
        until: usize,
        index: i64,
        name: Option<&str>,
        share: Option<f64>,
        again: bool,
    ) -> Evaluated {
        if let Some(ahead) = ahead {
            if !again {
                // The definition itself on a worker, so that it can go on
                // past its share.
                if share.is_some()
                    && !ahead.jobs.contains_key(&k)
                    && !ahead.off
                    && self.worker_free()
                    && self.launch_one(ahead, candidates, k, name)
                {
                    trace!(
                        "{}: definition {} evaluated on a worker, to go on past its share",
                        name.unwrap_or("?"),
                        k + 1
                    );
                }
                // The ones after it not before the first definition was
                // rejected: it is usually the one taken, and workers
                // evaluating the next ones would only take processors and
                // memory from it.
                if ahead.tried {
                    self.launch(ahead, candidates, k, until, name);
                }
                ahead.tried = true;
            }
            if let Some(evaluated) = self.take(ahead, candidates, k, index, name, share) {
                return evaluated;
            }
        }
        Evaluated {
            result: self.try_candidate_for(index, &candidates[k], name, share),
            bodies: None,
            ran: None,
        }
    }

    /// A worker's result for candidate `k`, once it is there; None when
    /// there is no worker's or the document must evaluate it (the worker
    /// was cancelled otherwise, or ended without a result). With `share`,
    /// the importer waits for it until `share` seconds after it began: then
    /// it gives way, as the document's would (also when its result came
    /// later than that), and goes on on its worker for the second round.
    fn take(
        &mut self,
        ahead: &mut Ahead<K>,
        candidates: &[Candidate],
        k: usize,
        index: i64,
        name: Option<&str>,
        share: Option<f64>,
    ) -> Option<Evaluated> {
        let mut job = ahead.jobs.remove(&k)?;
        let halted = |result: String| Evaluated {
            result: Err(result),
            bodies: None,
            ran: None,
        };
        // (Another share of the item's time past the end of its time.)
        let grace = SHARE * self.options.item_seconds;
        let limit = job.wait_limit(self.deadline, grace);
        let give_way = share.map(|s| job.started + seconds(s));
        let gave_way = |job: Job<K>, ahead: &mut Ahead<K>| {
            trace!(
                "{}: definition {} gives way at its share and goes on on its worker",
                name.unwrap_or("?"),
                k + 1
            );
            ahead.jobs.insert(k, job);
            Some(Evaluated {
                result: Err(GAVE_WAY.to_owned()),
                bodies: None,
                ran: share,
            })
        };
        let done = loop {
            if self.abandoned() || self.stopped(index) {
                job.cancel.cancel();
                return Some(halted(STOPPED.to_owned()));
            }
            if self.memory_low() {
                job.cancel.cancel();
                return Some(halted(LOW_MEMORY.to_owned()));
            }
            let received = match job.finished.take() {
                Some(done) => Some(done),
                None => match job.done.recv_timeout(WAIT_STEP) {
                    Ok(done) => Some(done),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    // (Its thread ended without a result.)
                    Err(mpsc::RecvTimeoutError::Disconnected) => return None,
                },
            };
            if let Some(done) = received {
                // Ended past its share: it gave way there all the same,
                // its result kept for the second round.
                if give_way.is_some_and(|at| done.ended > at) {
                    job.finished = Some(done);
                    return gave_way(job, ahead);
                }
                break done;
            }
            let now = crate::now();
            if give_way.is_some_and(|at| now >= at) {
                return gave_way(job, ahead);
            }
            if let Some(limit) = limit
                && now >= limit
            {
                // The worker's kernel call did not stop at the end of the
                // definition's time and has run on for another share since:
                // the definition is cut short, as if it had stopped then,
                // and the worker is left to end on its own (its thread
                // counts in `running_workers`).
                job.cancel.cancel();
                let ran = crate::since(job.started).as_secs_f64();
                let past = |d: Option<Instant>| d.is_some_and(|d| now >= d);
                let reason = if past(self.deadline) {
                    OUT_OF_TIME
                } else if past(job.own) {
                    CANDIDATE_OUT_OF_TIME
                } else {
                    GAVE_WAY
                };
                trace!(
                    "{}: definition {} evaluated ahead did not stop at the end of its time; \
                     given up after {ran:.2} s",
                    name.unwrap_or("?"),
                    k + 1
                );
                return Some(Evaluated {
                    result: Err(reason.to_owned()),
                    bodies: None,
                    ran: Some(ran),
                });
            }
        };
        let cancelled = |e: &str| e == mitcad_model::Cancelled::MESSAGE;
        let result = match done.added {
            Err(_) if self.abandoned() || self.stopped(index) => Err(STOPPED.to_owned()),
            Err(_) if self.memory_low() => Err(LOW_MEMORY.to_owned()),
            Err(e) if cancelled(&e) && done.past_item => Err(OUT_OF_TIME.to_owned()),
            Err(e) if cancelled(&e) && done.past_own => Err(CANDIDATE_OUT_OF_TIME.to_owned()),
            // Cancelled otherwise (the memory was short for workers), or
            // out of memory beside the other workers: evaluated on the
            // document.
            Err(e) if cancelled(&e) || KernelError::is_out_of_memory(&e) => return None,
            added => added,
        };
        trace!(
            "{}: definition {} evaluated ahead in {}",
            name.unwrap_or("?"),
            k + 1,
            crate::paced(Duration::from_secs_f64(done.ran), done.wall)
        );
        if let Ok(uids) = &result {
            // As the document would keep them in its cache.
            self.doc.adopt_results(&done.doc, uids);
            if tracing() {
                trace!(
                    "{}: {} ahead -> {} solids",
                    name.unwrap_or("?"),
                    summary(&candidates[k]),
                    done.bodies.len()
                );
            }
        }
        let bodies = result.is_ok().then_some(done.bodies);
        Some(Evaluated {
            result,
            bodies,
            ran: Some(done.ran),
        })
    }

    /// Brings an accepted candidate that a worker evaluated into the
    /// document: its definitions are added again, reusing the worker's
    /// results (the same uids, shapes and names). The uids of the
    /// candidate's features.
    pub(crate) fn take_over(
        &mut self,
        index: i64,
        candidate: &Candidate,
        name: Option<&str>,
        evaluated: &Evaluated,
    ) -> Result<Vec<FeatureUid>, String> {
        let uids = evaluated.result.clone()?;
        if !evaluated.ahead() {
            return Ok(uids);
        }
        let added = self.try_candidate_for(index, candidate, name, None)?;
        if added != uids {
            trace!(
                "{}: {} added as {added:?}, ahead as {uids:?}",
                name.unwrap_or("?"),
                summary(candidate)
            );
        }
        Ok(added)
    }
}

/// Runs `f` on `0..count` on up to `threads` threads and hands the results
/// to `consume` in order, as `for k in 0..count { consume(k, f(k)) }` would
/// (mitcad#103): the threads work at most `2 × threads` ahead of `consume`,
/// so that the results waiting take little memory. With one thread (or one
/// item) everything runs on the calling thread. A panic in `f` or
/// `consume` ends the others and goes on on the calling thread.
pub fn in_order<T: Send>(
    count: usize,
    threads: usize,
    f: impl Fn(usize) -> T + Sync,
    consume: impl FnMut(usize, T),
) {
    in_order_sharing(count, threads, None, f, consume);
}

/// Threads that [`in_order_sharing`]'s workers share with work beside
/// them (mitcad#245): the work takes one while it runs ([`Share::take`]),
/// and the workers start no item while as many of them work as there are
/// threads left, so that the two together stay within the threads.
#[derive(Debug)]
pub struct Share {
    left: AtomicUsize,
}

/// A thread taken from a [`Share`], given back when dropped.
#[derive(Debug)]
pub struct Taken(Arc<Share>);

impl Share {
    pub fn new(threads: usize) -> Arc<Self> {
        Arc::new(Self {
            left: AtomicUsize::new(threads.max(1)),
        })
    }

    /// One of the threads while more than `keep` are left (and at least
    /// one): the workers keep those.
    pub fn take(self: &Arc<Self>, keep: usize) -> Option<Taken> {
        let keep = keep.max(1);
        let mut n = self.left.load(Ordering::SeqCst);
        loop {
            if n <= keep {
                return None;
            }
            match self
                .left
                .compare_exchange(n, n - 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return Some(Taken(Arc::clone(self))),
                Err(now) => n = now,
            }
        }
    }

    /// The threads left for the workers.
    pub fn left(&self) -> usize {
        self.left.load(Ordering::SeqCst)
    }
}

impl Drop for Taken {
    fn drop(&mut self) {
        self.0.left.fetch_add(1, Ordering::SeqCst);
    }
}

/// [`in_order`] with its threads shared with work beside it (`share`):
/// while that work holds threads, fewer workers run (at least one). Items
/// are claimed only by a worker that may run, so no item waits for a
/// thread while the others go on without it.
pub fn in_order_sharing<T: Send>(
    count: usize,
    threads: usize,
    share: Option<&Share>,
    f: impl Fn(usize) -> T + Sync,
    mut consume: impl FnMut(usize, T),
) {
    let threads = threads.min(count);
    if threads <= 1 {
        for k in 0..count {
            consume(k, f(k));
        }
        return;
    }
    let window = 2 * threads;
    // The next item to start, the items consumed, and the workers working
    // on an item.
    let progress = std::sync::Mutex::new((0usize, 0usize, 0usize));
    let moved = std::sync::Condvar::new();
    let lock = || {
        progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    };
    // Ends the threads when the caller's side ends (also on a panic): no
    // more items start, and none waits for the window.
    struct Ending<'a>(&'a dyn Fn());
    impl Drop for Ending<'_> {
        fn drop(&mut self) {
            (self.0)();
        }
    }
    let end = || {
        lock().0 = count;
        moved.notify_all();
    };
    type Outcome<T> = (usize, std::thread::Result<T>);
    let (sender, results) = std::sync::mpsc::channel::<Outcome<T>>();
    let work = |sender: std::sync::mpsc::Sender<Outcome<T>>| {
        loop {
            let k = {
                let mut p = lock();
                loop {
                    if p.0 >= count {
                        return;
                    }
                    if p.0 >= p.1 + window {
                        p = moved
                            .wait(p)
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                    } else if share.is_some_and(|s| p.2 >= s.left()) {
                        // (A thread given back to the share is seen here.)
                        p = moved
                            .wait_timeout(p, Duration::from_millis(10))
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .0;
                    } else {
                        break;
                    }
                }
                p.0 += 1;
                p.2 += 1;
                p.0 - 1
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(k)));
            lock().2 -= 1;
            if share.is_some() {
                moved.notify_all();
            }
            if sender.send((k, result)).is_err() {
                return;
            }
        }
    };
    std::thread::scope(|scope| {
        let _ending = Ending(&end);
        for _ in 0..threads {
            let (work, sender) = (&work, sender.clone());
            // (One that does not start leaves the work to the others.)
            let _ = std::thread::Builder::new()
                .name("mitcad-import-helper".to_owned())
                .stack_size(WORKER_STACK)
                .spawn_scoped(scope, move || work(sender));
        }
        // Once every thread has ended, receiving fails.
        drop(sender);
        let mut waiting = std::collections::BTreeMap::new();
        for k in 0..count {
            let result = loop {
                if let Some(result) = waiting.remove(&k) {
                    break result;
                }
                match results.recv() {
                    Ok((j, Ok(result))) => {
                        waiting.insert(j, result);
                    }
                    Ok((_, Err(panic))) => std::panic::resume_unwind(panic),
                    // No thread runs (none started): here.
                    Err(_) => {
                        let mut p = lock();
                        p.0 = p.0.max(k + 1);
                        drop(p);
                        break f(k);
                    }
                }
            };
            consume(k, result);
            lock().1 = k + 1;
            moved.notify_all();
        }
    });
}
