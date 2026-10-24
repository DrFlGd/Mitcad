# The .f3d import: time, memory and parallel work

The time limit, the stop and hangs, the memory guard, definitions evaluated
in parallel and the work beside the replay. Part of the [.f3d
import](f3d.md) notes; the [import documents](../README.md#documents) list
the other topics.

## Time limit, stop and hangs

**Time limit.** `time_limit` (`--time-limit`) seconds after the start, the
remaining items take their history states without trying definitions, and
the final bodies are compared by volume only. The application sets none:
its imports run to the end unless the user stops them.

**Item time.** An item's definitions are tried for `item_seconds` (60 s;
the part of it on bodies brought to the state before it exactly gets half
again). The definitions are evaluated with a monitor whose deadline is the
end of that time (`RecomputeMonitor::within`, mitcad#69), so that the
kernel's long operations stop inside one definition (a rectangular pattern
of 35 × 35 copies took 113 s): the definition fails ("the item's time was
over before its definition was evaluated") and the item gives up like one
out of time.

**A definition's share** (mitcad#78). One slow definition must not use up
the item's time before those ranked after it are tried (a join onto a plate
of a thousand holes took a minute where the next definitions took a second
or two). Checked against the history (an item with a state of its own, and
each set of bodies of one without), the definitions are tried in two
rounds (`Turns` in `lib.rs`):

1. In rank order, each with a third of the item's time (`SHARE`); the
   last one, when none waits, with the rest. One cut short at its share
   gives way and waits; the same definition limited to participants waits
   behind it untried (the boolean on fewer bodies takes as long).
2. Those that gave way, in rank order, with the rest of the item's time,
   each only while more time is left than it already ran (it cannot finish
   in less).

Definitions that give a state take seconds (up to about 15 s on bodies of
thousands of faces), so one that runs longer is mostly a wrong guess (on a
plate of a thousand holes the wrong ones took one to two minutes each); one
that would give the state within the item's time still does when those
after it are quick. A smaller share would reach more definitions, but each
one cut short and tried again costs its share. When the item gives no
state, its note names the definition cut short after the longest time
("cut short after 20.0 s: definition 3 of 12 (extrude, join, 1 profile)
(and 2 others)"; the trace has the whole definition).

**Stop.** `Options::stop` is a `RecomputeMonitor` that another thread
cancels. `mitcad-ffi` passes the document's job (`import_f3d` under
`attach_job`); the application's import worker cancels it on Stop and Keep
What Is Imported (`app/files/F3dImport.cpp`). Once cancelled:

- the item being replayed and those after it are given up as after the
  time limit (note "the import was stopped"): with a history they take
  their states, without one the stored bodies stand in; sketches and
  construction geometry still come in;
- while an item's definitions are tried, the request is the document's
  monitor, so the kernel's long operations stop inside evaluation and the
  definition fails at once; nothing else (fallbacks, undo) runs with it;
- the final bodies are compared by volume only;
- the report says where it stopped (`stopped`: the item's `index` and
  `name`; no `item` when the stop came after the last one) and warns.

The monitor's test delay slows each definition tried
(`tools/ui-import-test.sh`). A try that hung after a stop is rerun with
`Options::stop_at`: items before the one it stopped at
(`Progress::stopped`, or the one it hung on if it had not seen the stop
yet) are replayed, the rest given up.

**Hangs.** A kernel call cannot be interrupted. With `hang_limit`
(`--hang-limit`; `test_f3d_import --corpus` defaults to 600 s) the import
runs on its own thread (`core/ffi/src/f3d_import.rs`) with a 256 MiB stack
(OCCT recurses deeply; on a default 2 MiB stack features failed from
overflows) and shares its progress (`Options::progress`: a step counter and
the current item). The step moves between kernel calls, also in the
import's own long loops, which tick it after each call
(`mitcad_import::tick`, for the import running on the thread): building
and measuring bodies (a call per body; `Sig::of` ticks), finding faces
and edges by their fingerprints or for a joint's frame, comparing
bodies. Building a body from the file heals it with OCCT's ShapeFix,
which reports its progress face by face (`brep::BuildOptions::progress`,
`f3d_build_body`), and that moves the step too, as does the progress of
OCCT's long algorithms in the definitions tried (booleans, fillets,
sweeps: those a request to stop stops,
`geometry::CancelSource::progressed`). So a long item is no hang; a
kernel call that does not return is (mitcad#82: a large design's first
item, a base feature, healed two bodies of 15 000 and 23 000 faces for
110 s and looked hung under a limit of 60 s). A call can still take
longer than the limit and return: healing one face of that body took
74 s. The tries share such builds (`StoredFile::builds`): a try waits
while another builds a body, and when the watchdog gave the builder up
meanwhile, the builder hands its body over as B-rep data and the next
try reads it instead of building it again (which would take as long and
look hung again; waiting shows no progress either, so a build that does
not return still does). `MITCAD_IMPORT_STALL=slow` makes the file's
first body take 1.5 s to build, for the tests.

When the step does not move for that many seconds, the
thread is abandoned and the import reruns on a new document with the item
in `Options::hung_items`: it takes its history state without trying
definitions (note "the geometry kernel did not finish a definition of
it"), and the report warns. With the import's trace on
(`MITCAD_IMPORT_TRACE`), each definition is named before it is evaluated
("evaluating […]"), so the trace of a try given up ends with the
definition the kernel did not finish (mitcad#495). The items before it
replay as before, so those
whose definitions gave no state (`Progress::failed_items`) take their
states at once in the next try, with the same note
(`Options::failed_items`): a pattern of a thousand copies took minutes in
every try. A hang while comparing the final bodies makes
the next try compare volumes only; a hang elsewhere brings in the stored
bodies without the timeline. The try given up is abandoned
(`Progress::abandon`, mitcad#71): when its kernel call returns it stops
before its next item or definition (a definition's long kernel operations
stop inside, as on a stop), compares nothing and its result never reaches
the report or the document, so it does not compete with the next try for
the processors and memory. Until then the thread keeps its stack and
memory, so under an address-space limit
(`ulimit -v`) the next thread's 256 MiB stack may not fit: it then starts
on the largest of 64, 16 or 8 MiB that does (a smaller stack can overflow
in OCCT's deepest recursions, which fails that feature, and the report
warns), and when none fits the import ends with an error saying so (it
used to fail with "Resource temporarily unavailable", mitcad#58).
A process that ends while such a thread still runs must not run its
static destructors: on Linux the shared libraries' finalizers destroyed
OCCT's statics under a thread healing a body it built, which crashed
(`mitcad-cli` exited with a segmentation fault after the error that the
import hung, mitcad#82). `abandoned_imports()` (`mitcad-ffi`) counts the
threads given up that still run; `mitcad-cli` and the application's
import worker then end with `std::_Exit` once their output is written.
`MITCAD_IMPORT_STALL=compare` simulates a hang for the tests
(`core/tests/test_f3d_import.cpp`, also under a tight `RLIMIT_AS` on
Linux); `MITCAD_IMPORT_STALL=busy` makes every try build the file's
bodies over and over without progress, so that the import ends hung
while the threads given up are in the kernel
(`cli.import_f3d_hang_exit`, `tools/cli/hang-exit-test.cmake`). A
tapered extrusion whose arcs shrink past their apex, on
which OCCT's draft loops, is refused before the draft ("the taper closes
the profile", `geometry/src/sweep.cpp`). The file is read once, before
the first try, on an import thread of its own and without the watchdog
(reading and converting a large design takes minutes and calls no
kernel): the tries share its decoded design and its bodies converted to
the neutral model (`StoredFile`), and each builds the OCCT shapes it asks
for (`F3dGeometry`), so a try run again neither reads the file again nor
holds a second copy of it beside the abandoned one (mitcad#80).

## Memory

An allocation that fails ends the process: Rust's cannot be caught, and
the import used to lose everything, also the items done (mitcad#80). So
the import guards its memory (`core/ffi/src/f3d_import.rs`,
`MemoryGuard`), on the watchdog's loop or, without one, on a thread of
its own, ten times a second: it measures the process against the
tightest limit it runs under (`core/ffi/src/memory.rs`,
`core/cpp/bridge/memory.cpp`): an address-space (`ulimit -v`) or data
limit, or the memory the system has left (available memory and free
swap) on Linux; a job's memory limit or the commit the system has left
on Windows; the physical memory on macOS; and the `memory_limit` option
(MiB, against the resident memory). Near a Linux limit the memory the C
library's allocator holds free (`mallinfo2`) does not count: it is used
again before more is mapped.

- At 70 % of the limit (and each 5 % more) memory is *tight*
  (`Progress::tight_on_memory`): before its next item the import drops
  the cached results of the definitions it tried and the bodies of the
  history states behind the replay (`Oracle::release_behind`,
  `StoredGeometry::release_before`), which are built again if they are
  asked for; what it imports stays the same.
- At 85 % it is *low* (`Progress::low_memory`): the definition being
  evaluated is cut short as on a stop and its item takes its history
  state (note "the import ran low on memory"), as do the modelling
  items after it until the memory is back under 70 %
  (`Progress::memory_recovered`); the import drops what it can as above,
  and from then on caches at most 256 MiB of results
  (`CACHE_AFTER_LOW_MEMORY`). A definition's own memory goes when it is
  cut short, so the import usually goes on with the next item (a
  rectangular pattern of a thousand copies took 8 GB in one definition).
  The final bodies are compared by volume only if the memory is still
  low then. The report says where it first ran low, how much was in use
  and how many items it gave up (`low_memory`, and a warning).
- A kernel operation whose allocation fails (OCCT's
  `Standard_OutOfMemory`, `std::bad_alloc`) fails with "<operation>:
  out of memory" (`geometry/src/util.hpp`, `KernelError::OUT_OF_MEMORY`)
  instead of ending the process; the import takes it as low memory.
  `MITCAD_TEST_OCCT_OUT_OF_MEMORY=<operation>` makes one fail for the
  tests.
- OCCT's allocator throws `Standard_OutOfMemory` instead of returning
  null, and counts its failures, per process and per thread with the
  jobs of OCCT's thread pool it ran (patches 0030 and 0031 of the port,
  mitcad#132):
  OCCT's collections wrote to the null block, and a large design under
  the corpus runner's 4 GB limit crashed in the checker
  (`CSLib_Class2d`) while one of the file's bodies was healed. The
  checker and the healing catch OCCT's failures inside and go on (the
  checker then calls a sub-shape invalid), so a moved count is what
  tells: a kernel operation and the checks whose thread's count moved
  (`geometry::failed_allocations_in_thread`) fail "out of memory", a
  body of the file whose build moved it does not build ("building a
  body of the file: out of memory", the import is low on memory), and
  the memory guard takes a process count that moved since its last
  measure (`geometry::failed_allocations`) as low memory ("an
  allocation in the geometry kernel failed"; when the operation that
  failed reports it later, as it does when the guard polls first, the
  report tells the operation's words), also for failures on threads of
  no import. An operation on another thread at the same time is not
  failed by them.
  `MITCAD_TEST_OCCT_ALLOCATION_FAILS=<operation>` (or `heal`) makes one
  fail and be caught inside for the tests.

The history's bodies are converted keeping only the distinct ones: each
state holds the bodies that differ from the state after it, not a copy of
every body (a design of 256 items took 5.8 GB before the first item that
way, and the memory the allocator kept afterwards left 40 MB of a 12 GB
address-space limit once OCCT's threads started).

## Definitions evaluated in parallel

Most of an import's time goes to definitions the history rejects: each
candidate ([*What the history settles*](f3d-history.md#what-the-history-settles)) is a full geometry evaluation. With
`Options::threads` above one (`--threads`, the `threads` option of
`import_f3d`; `mitcad-cli` and the application use the logical cores, at
most 8; `test_f3d_import` one unless `--threads`), the definitions ranked
after the one being evaluated are evaluated ahead by workers (`ahead.rs`,
mitcad#95), and the import is the same as with one thread, only faster:

- **Rank order.** The import still goes through the candidates one by
  one and decides as before; it evaluates candidate *k* on its document
  while workers evaluate *k+1*, *k+2*, … (up to `threads` − 1 at once, as
  far as the item's attempts and a loose match's closer tries reach), from
  the second candidate of a list on: the first is usually the one taken,
  and workers beside it would only take processors and memory from it.
  When
  it comes to a candidate a worker has evaluated (or waits for it), it
  takes the worker's result instead of evaluating it again. So candidate
  *k* is taken only when every one ranked before it was rejected,
  whichever finished first, and everything that depends on earlier
  outcomes (participants that give what the definition without them gave,
  edges of a fillet that failed, `Turns`) is decided as before. Once the
  import leaves the list (one was taken, or none is left) the workers
  still evaluating are cut short through their monitors, as on a stop; the
  import does not wait for them.
- **Copies of the document.** A worker evaluates on its own copy of the
  document as it is before the item (`Document::fork`: the definition, the
  cached results and the last recompute, shared, with a kernel of its own,
  `Kernel::fork`; no undo history). A feature added there gets the uid it
  would get in the document, and its result the same version. A rejected
  candidate leaves the document as it is (it was undone anyway); its
  results go into the document's cache, as if it had been evaluated there.
  The candidate taken is added to the document again and finds the
  worker's results in the cache (`Document::adopt_results`): the same
  shapes, topological names, uids and notes.
- **Shared shapes.** The workers read the same bodies (the results before
  the item) at once. The geometry's operations never modify their inputs
  (T0e): those whose OCCT algorithms write into their inputs' sub-shapes
  (booleans, fillets, chamfers, shells, healing) work on copies
  (`InputCopy`), a `Shape` keeps what it measured of itself behind a lock,
  and OCCT's handles count references atomically. The cancellation scope
  of the geometry is per thread, and each worker installs the kernel's
  crash handlers and ticks the import's progress (`mitcad-ffi`).
- **Time.** A worker's definition is given the first round's share of the
  item's time (`SHARE`) from when it starts, and the item's deadline. One
  that gave way at its share is evaluated again on the document when the
  import would give it the rest of the item's time (the last one);
  otherwise its result stands. Results that depend on the time (an item
  out of time, an import past its time limit) can differ, as between two
  runs with one thread: with workers more definitions fit into the time.
- **Memory.** Each worker holds its definition's memory and a stack of
  64 MiB (the import's own thread has 256 MiB). No worker starts while the
  process uses half of its tightest memory limit or more (*Memory*;
  `Progress::set_memory_share`), and at 60 % the definitions being
  evaluated ahead are cut short and evaluated on the import's thread
  instead when the import comes to them; the import's own definition keeps
  the memory, as with one thread. A worker's definition that runs out of
  memory is evaluated again on the import's thread too.
- **Stop and hangs.** While the import waits for a worker it looks for a
  stop, the watchdog's give-up and low memory every 20 ms: a worker that
  does not return does not keep the stop from stopping. The watchdog
  counts the workers' progress as the import's; a worker the import waits
  for that hangs leaves no progress once the others are done, so the item
  is given up as before (`Options::hung_items`). Workers whose result is
  no longer wanted may still be in a kernel call when the import ends:
  `running_workers()` counts them, and `mitcad-ffi`'s
  `abandoned_imports()` includes them, so that the process ends with
  `std::_Exit` as after a hang.
- **A worker that does not stop at the end of its time** (mitcad#434). A
  kernel call that does not check for a request to stop runs on past the
  definition's time. The import waits for such a worker until another
  share of the item's time (the first round's) has passed after the end of
  the time it would give the definition (its share, its own time or the
  item's). Then the definition is cut short as if it had stopped there
  (gave way, or out of time), and the import goes on. The worker is left to
  end on its own and counts in `running_workers()`. The case behind it was
  `db73863f` item 88, a fillet whose definition named some edges of a
  closed tangent chain. Its contour's walk inside
  `BRepFilletAPI_MakeFillet::Add`, which no progress range reaches, went
  round the chain for hours, and the import, waiting for the worker, never
  finished. The OCCT port's patch 0840 ends that walk.
- **What stays on the import's thread.** The second round of definitions
  that gave way, the approximate definition taken after a loose match,
  the next history state's fillet and chamfer edge guesses (`Guesses`,
  found between the definitions), the edits of a pattern's extrusions
  (*Profiles a pattern or mirror needs*), and the definitions tried on
  bodies the history holds already (*Bodies the history already holds*).

**Where the time goes.** Workers help only where an item has several
definitions queued and the first ones are rejected; the rest of an import
runs on its own thread as before. On a 199-item design of the private
corpus (time limit 300 s) 46 % of the time went to evaluating definitions
on one thread, 8 % to translating items and the rest to finding fillet
and chamfer edges between the definitions, the edits of a pattern's
extrusions, sketches, the history's bodies and the final comparison;
on a 76-item design 52 % went to definitions and 36 % to finding fillet
edges. Most items take their first definition (sketches, construction
geometry, fillets of edges found by their names, extrusions whose first
profile set is right), and a fillet's or chamfer's edge guesses come one
history state at a time, so its list has one or two definitions when the
workers could start. So an import with eight threads keeps about 1.5 to
2.5 cores busy where a single-threaded one keeps 1 to 2 (the geometry
kernel's own parallel booleans), and designs whose time goes to rejected
extrusions, combines and patterns gain most (up to about twice as fast).
A worker's definition also slows the import's own one where the
processors are all busy: on a machine loaded already, an import with
eight threads can take longer than one with one. *Work beside the
replay* below runs more of an import in parallel.

**The time limit on large designs** (mitcad#181). The five designs of
the private corpora that lost the most items to the 300 s limit (one
thread, as the coverage runs import) spent it, by their traces and
stack samples, on: probing the next states for an extrusion's region
sets while its decoded profiles gave the state (10–30 s an extrusion on
bodies of thousands of faces), the same combine built with each of its
bodies as the target on bodies carrying an approximation (15 s each),
a fillet's edge guesses against later states that gave the same edges
(5 s a state), OCCT's checker on large bodies (37 s for a stored body of
23 000 faces, half of each body built from the file), finding faces by
name on such bodies (each lookup read every face's names) and the
pattern edits of extrusions' other definitions. Each is cut
([*What the history settles*](f3d-history.md#what-the-history-settles), `docs/architecture.md`); the items the
limit took went from 81 to none on a 199-item design (in both corpora),
from 48 to none on a 142-item one (the import 385 → 252 s and
348 → 246 s), from 50 to 45 and from 123 to 86–117 (the load of the
machine decides) on the other two, with no item's outcome changed but
those the limit took. What the two keep (one thread):

- healing the stored bodies of every history state (OCCT's ShapeFix,
  mostly its wires' self-intersection checks: 6–7 s a state for a body
  of 5 000 faces, about 80 such states in one design), which threads
  build ahead of the replay (*Work beside the replay*);
- definitions that do not give the state and use the item's whole time:
  guessed patterns (classifying their copies' points against a body of
  4 500 faces took 10 s a guess), through-all cuts of every body that
  reach stored bodies of 14 000–23 000 faces (OCCT rebuilding the cut
  solid, 30 s a definition), a decoded join that OCCT leaves invalid;
- the first base feature's bodies of 14 662 and 23 430 faces (45 s).

A second round on the six designs that still hit the limit (traces of
each import, stack samples of the slowest items) cut, in the same way:

- **Patterns that are not searched.** A circular or rectangular pattern
  whose decoded definition has read inputs, or whose copied feature did not
  come in, is no longer given the history's suggestions (the other features,
  axes and counts of `guessed_copies`): of the settled patterns with read
  inputs none took a suggestion, while on the rejected ones twenty
  definitions of a large body's copies used the item's whole time (125 s
  and more a pattern, and the rest of the import with it). Two designs went
  from 71 and 37 items lost to the limit to 6 and none (110 → 168 and
  100 → 127 parametric items) with this change alone.
- **Sweeps** take the decoded profiles' regions first (`_f3d_profile_loops`,
  as extrusions do) and no longer probe the next history states for other
  region sets: a sweep along a thread took 12 s the first and 30 s the
  second time to translate, now 0.3 s.
- **Boxes that hold a body.** A pattern decides which of its copies reach a
  body by their boxes, and the name lookups skip bodies by theirs; both
  used the optimal box (`BRepBndLib::AddOptimal`, a particle swarm on each
  spline face: a third of the active time of a pattern over a body of 4 400
  faces). `Kernel::enclosing_box` gives the box of the control points and
  tolerances instead (the tight one where it was measured already).
- **Fillets and chamfers repeated on copies** are applied in groups of 128
  copies: OCCT compares every pair of the roundings of one operation
  (`OverlappingStripes`, a minute for a thousand copies), and the faces at
  the contours' edges are found once per operation, not once per edge.
- **No other extrusion definitions after the time.** A pattern whose
  candidates used the item's time does not go on to edit the extrusions it
  copies (each edit would evaluate the same long definition again).

What keeps these designs from importing within the limit (not import
mistakes, the geometry kernel's time or failure): booleans on stored bodies
of 4 400–14 700 faces (15–70 s a definition, the pattern's or extrusion's
cut), a guide-surface sweep along a stored thread of 720 spline faces that
the kernel fails after 8 s and the variants of which take 10 s each, the
roundings repeated on a thousand copies (the kernel cannot end the rounding
at a vertex on some copies), and the healing of the stored bodies (45 s for
the 14 662-face base feature). An item that fails after such time leaves the
later items on stored states, and each item still takes the item time while
its definitions fail.

A third round, on the two designs that still went past the limit (a
116-item design whose body grows to 5 000 faces over 60 history states, and
the 256-item design of the 14 662- and 23 430-face base feature), by the
import's traces, stack samples and the reference dumps' bodies after each
item:

- **Healed faces reused between history states.** Each state's stored body
  was built and healed with ShapeFix as a whole (25–45 s a state for 5 000
  faces, built again after the limit for every item). A build of a large
  body (256 faces or more) now keeps its healed faces, edges and vertices by
  the content of their data (`brep::Healed`: two 64-bit hashes of a face's
  surface, its loops' edges, their curves and vertices; a part whose key
  the body has twice is left out), as healing left them (what its
  `ShapeBuild_ReShape` put in their place, an edge it split as the chain of
  its pieces, a seam it added to a closed face with the face). The build
  of the body in the next state that changes it takes the faces it shares
  with that build (its base: the same ASM record in the blob's state
  before, `StoredFile::bases`) as copies of those healed there, builds the
  others from its data,
  heals only those with ShapeFix (their edges made same-parameter and their
  vertex tolerances fixed where they meet the copies) and checks the whole
  body: when fewer than half of the faces are shared or the result is not a
  valid solid, the body is built and healed as a whole as before. The
  states' signatures stay the same (103 of 104 identical in the 116-item
  design, the other within 5 · 10⁻³ mm³ of 600 000), and a state of 4 500–
  5 200 faces takes 2–7 s instead of 25–45 s (4 300–4 900 faces taken).
  The base is fixed by the file, not by which builds exist (mitcad#243):
  a build waits for its base's build on another thread, and builds the
  base itself when no build of it kept anything yet or any more (a state
  the replay skipped, or released); what a build kept goes when its heir
  is built or no later state has it. Before, the next build took the
  blob's last three builds, which with threads depended on the timing of
  the builds ahead: a body could be healed as a whole in one run and from
  shared faces in another, an edge apart.
- **The body the file continues keeps its id.** A base feature that takes a
  history state paired each new body with the nearest old one; stored bodies
  now carry their ASM record (`StoredBody::lineage`, the same in every state
  of the blob), and a new body with the record of an old one replaces it. A
  split that came in as base features gave its first new piece the id of the
  body it split, so a later cut that names the split's first new body (its
  `body_index` 0) took another piece. The base feature that brings the
  bodies to the state before an item from the item's own start
  (`item_start`: the operations no item names right before its own) is now
  counted as the item's (`Importer::start_bases`): a split whose history
  holds two operations made the new bodies of both.
- **Named participants alone.** A join, cut or intersection with decoded
  participants was also tried on all bodies; where the item's history state
  changed only bodies it names, that is not tried (on all bodies the
  operation gives those the same and must leave the others as they are),
  which took 30 s a definition on the 14 000-face bodies.
- **No inverted-solid check in booleans.** OCCT checked each solid of a
  boolean for being inverted by building a point classifier of all its
  faces on one thread; Mitcad's solids are oriented, so the check is off
  (`SetCheckInverted(false)`).
- **A pattern's consecutive cuts in one operation.** A pattern of features
  cut each place (a feature at one of the places a pattern of patterns gives
  it) with its own copies: a pattern of a hole and of a circular pattern of
  21 of it, 28 times along an axis, was 44 cuts of 28 copies each from a
  body of 4 000 faces (more than a minute). With Identical and Optimized,
  consecutive places that cut the same bodies are cut together with all
  their copies (10 s, and the pattern matches its state).
- **Probes in one call.** Probing a sketch's regions against the next
  states ([*What the history settles*](f3d-history.md#what-the-history-settles)) crossed each body with one segment
  per region and side, each call setting up every face the segment reaches
  (its intersector and the classifier of its wires: 1.5 s a probe on a body
  of 5 000 faces whose large faces hold hundreds of holes, 66 probes and
  98 s for one extrusion). `Kernel::segments_crossings` takes all the
  segments and sets each face up once.

What these designs keep: the 256-item design imports within the limit (213
items parametric, from 104–111; 106 inputs read, from 10–14); its fallbacks
included a rectangular pattern of a pin and its two roundings on 14 × 77
copies, whose second rounding the geometry kernel cannot end at a vertex on
copy 146 (its definition with the read axis takes 71 s to fail; the
optimized option's face copy replays it since, see *Pattern and mirror
objects*), and still include a cleanup
cut whose boolean OCCT gets wrong (the cut of its decoded profiles leaves
the body 2 mm³ larger where the file removes 31 mm³), and three splits
whose tool is not decoded. The 116-item design gets further (69 items parametric, from 62;
the pattern above and six joins and roundings after it now replayed) but
still reaches the limit near its 50th item: its joins and roundings on the
5 000-face body take 20–60 s an item (OCCT's general fuse, its checker on
the whole result and the face merge after it), and three joins whose
decoded definitions OCCT leaves invalid try their other definitions.

A fourth round, on the 116-item design (stack samples of the import's
threads, the trace of every item):

- **Faces measured once.** An extrusion up to a face looked the face up on
  each body of the definitions it tried (`refs::resolve_face_where`,
  `face_resolved_uniquely`), and each lookup measured the area of every
  face of the 5 000-face body (`Kernel::faces`, one face after another):
  88 s for one extrusion, which then fell back because its time was over.
  A shape now keeps its faces' kinds and areas once measured, measured on
  all cores (`Shape::face_measures`, geometry); the extrusion takes 6–9 s
  and is parametric.
- **A rounding's edges mapped to faces once.** After OCCT failed a
  rounding as asked, the retries mapped the whole body's edges to faces
  for each selected edge (`faces_at_edge`): 15 s of a rounding of a few
  dozen edges on the large body. The map is made once per try.
- **Checks only where the body changed.** A boolean whose targets the
  checker found valid checks only the faces it changed and those meeting
  them at a vertex, and counts the shells' closure, orientation and
  connection over the whole result (`is_valid_where_changed`,
  `docs/architecture.md`); a join near a few faces of the large body
  checked about 200 of its 5 000 faces. Booleans, roundings, chamfers and
  the bodies built from the file say when the checker found them valid
  (`Shape::checked_valid`).
- **The optimal box on all cores.** An extrusion through all asks for the
  box of the bodies; the box of each new body was the particle search over
  every spline face on one thread (3 s on the large body). The faces'
  boxes are now found on all cores, the same box.
- **A face merge that cannot concatenate a chain keeps it.** OCCT's face
  merge after a boolean (`ShapeUpgrade_UnifySameDomain`) threw "Courbes
  non jointives" when the p-curves or curves of a chain of edges it merges
  did not meet within their vertex's tolerance, and the join or cut whose
  boolean had given a valid result failed (two extrusions of the design,
  one of them its decoded definition). The port's patch 0110 keeps such a
  chain as its edges.

The design goes from 70 to 77 items parametric (27 → 34 inputs read) at
300 s on a loaded machine; its first component (57 items) now finishes
within the limit, and with the limit at 600 s 87 items are parametric (43
read). What it keeps: an extrusion whose decoded profile names curve
pieces the sketch's regions do not give cuts 3 % more than the file
(OCCT's cut of the right region, also without the far features: 2052
against 1985 mm³), a join that OCCT leaves invalid on bodies carrying an
earlier pattern's approximation (1.8 · 10⁻⁵ of the volume) before it is
tried on the history's state, a cut through all whose read side does not
cut (the other side gives the state), and the second component's items
after the limit. A 145-item design of the same round keeps its outcomes
(116 parametric) and no longer reaches the limit (261 s); its fallbacks
are roundings the geometry kernel fails with inputs as the file's
(mitcad#121) and a guide-surface sweep along a 720-face thread that OCCT
fails.

## Work beside the replay

The timeline's items are replayed one after another; with `threads`
above one the work that does not depend on the replay runs beside it
(mitcad#103), and the import is the same as with one thread:

- **Reading the file.** The design streams of every design of the file
  are decoded first (`mitcad_f3d::design::decode_streams`); only the
  chosen design's named inputs are then looked up in the bodies' history
  (`resolve_inputs`), its items on several threads (each item reads the
  history's blobs only, read once for all). That runs at the same time
  as the conversion of the bodies of every history state to the neutral
  model (`StoredFile::new`), whose states are converted on several
  threads and taken in order as before (`mitcad_import::in_order`), so
  that the same bodies are kept once each. The two share the threads
  half and half. On a large design of the private corpus reading took
  about 140 s on one thread (80 s finding the inputs, 60 s converting);
  with eight threads about 20 s. An item's inputs are looked up in one
  named history state at a time (the state before the item, then up to
  24 earlier ones per blob, until each input is found): naming all of
  them at once on every thread ran the largest design of the backup
  corpus out of a 4 GB limit before the import began (an allocation of
  Rust's, which ends the process, mitcad#132). A named state is shared
  (mitcad#246): the items and threads that look in it take one naming
  while it is kept, the two or three states used last are kept, the
  whole circles the circle seams read after the items are taken when
  their states are named, and a fillet's face selections name the
  earlier states one at a time. Naming each state only once would mean
  keeping about twelve states of a large design, half a gigabyte more at
  the import's peak ([History states named once](../../../docs/performance.md#history-states-named-once-mitcad246)).
- **The first state's large bodies healed while reading** (mitcad#245).
  The replay's first item asks for a history's oldest state with bodies,
  and healing its large bodies took as long as reading the file. The
  conversion of a history blob therefore converts that state first, and
  each of its bodies of 1 000 faces or more (largest first, 100 000 faces
  in all) is healed on a thread taken from the converting ones
  (`mitcad_import::Share`; at most half of them, while the conversion
  goes on with the others: `in_order_sharing` starts an item only on a
  thread left) while the rest of the file is read. The healing is the
  replay's build of that body: from the same data, without a base (no
  older state has the body), keeping what its heirs take (which leaves
  the body as it is; what it kept is handed on only when the body has
  heirs), measured as a body built ahead. Once the file's variants and
  bases are known, the body goes to its variant as a build in progress or
  built ahead (`StoredFile::take_early`); a try asking for it waits for it
  and ticks the watchdog with its progress, and its thread counts in
  `abandoned_imports()`. A body that does not build, or whose variant is
  not the converted one, is left to the replay. With one converting
  thread nothing is healed while reading.
- **The history's bodies built ahead.** When the replay asks for a
  history state, the bodies of the next four states that it has not built
  yet are built (healed) and measured on other threads
  (`F3dGeometry::build_ahead`, `mitcad-ffi`): the replay finds them
  built when it comes to them (the same bodies; a shape keeps what it
  measured of itself). A try that asks for a body being built ahead waits
  for it and ticks the watchdog while the build makes progress, so a
  build that does not return still looks hung; the threads count in
  `abandoned_imports()`. Bodies built ahead that no try took are dropped
  with the states behind the replay when memory gets tight. On the large
  design above, building two bodies of 15 000 and 23 000 faces took the
  first item 142 s on one thread.
- **One budget.** The workers that evaluate definitions ahead and the
  threads that build bodies ahead take their threads from the same
  `Options::threads` (`Progress::helper`: the import's own thread is one of
  them), and none starts while the process uses half of its tightest
  memory limit or is low on memory.
- **Planar faces kept per body.** Every sketch looks for its plane among
  the planar faces of all bodies (`refs::planar_faces`); on bodies of
  thousands of faces that took seconds per sketch (12 s on a body of
  23 000 faces, where most of a 256-item design's time went to its 73
  sketches). An import keeps the planar faces it found by the body's
  version (`refs::KeptPlanes`), which names one shape, so a sketch finds
  them again until the body changes. This is quicker with one thread too.

Coverage runs before and after these changes, at the same time with six
threads (time limit 300 s): `MITCAD_F3D_CORPUS` 575 s → 520 s in all
(1.11×, the slowest designs 1.08–1.39×), the backup corpus 4759 s →
3287 s (1.45×; the 256-item design above 1005 s → 341 s, others up to
3×). The reports with one and with eight threads are the same except
where a time limit decides (`MITCAD_F3D_CORPUS` 47 of 48 identical,
the backup corpus 318 of 325, each difference first at an item whose
time ran out or in the time-limited final comparison).

**Why the items are not replayed in parallel.** A profile of the five
slowest designs of each corpus (four and eight threads, time limit
300 s, before the changes above) kept 1.0 to 3.1 cores busy (mostly 1.4
to 2.3). Evaluating definitions took most of the time of the corpus
designs (the workers' share), finding fillet and chamfer edges 10–30 %
of the fillet-heavy ones, sketches up to 70 % of one large design
(the planar faces above), building the history's bodies up to 240 s of
another, and reading the file up to 140 s. Replaying independent items
at the same time would need items that neither read nor change what an
earlier one changes. Most designs are one component (in the slowest
ones 62–100 % of the items with a state change the same component), and
most of a part's items change the same body. Counting only the bodies
each item's state changes (an item reads at least those; a join or cut
with all bodies as participants reads more), the longest chain of
dependent items holds 42–66 % of the items' time on the slowest corpus
designs (a bound of 1.5–2.4× at best); with the items without a state of
their own (sketches, construction geometry, which find their planes on
the bodies) as dependencies of what follows, the chain is 82–100 % of
the time. A speculative item would also have to
run on a copy of the document and the importer's state (report, history
cursor, alternatives, sketches) and be replayed into the document as
the definitions evaluated ahead are, and the features' uids and
topological names of later items depend on every earlier item's. So the
replay stays sequential, and the work beside it runs ahead instead.

Not done: finding the fillet and chamfer edges of several history
states at once (10–30 % of the time of fillet-heavy designs). The search
asks the stored bodies' boundary index (`Kernel::boundary_distances`),
which keeps the projections it set up between calls; asked in another
order on other threads, the same answers are not assured, so it stays on
the import's thread.

