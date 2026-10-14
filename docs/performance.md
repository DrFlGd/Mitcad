<!-- SPDX-License-Identifier: MIT -->
# Import performance of large designs (mitcad#119)

This page records where the `.f3d` import of a large design spends its time
and memory, which ideas were measured, which were implemented, and which are
left for tickets of their own. The reference is one design of the private
backup corpus, content id `0b758b01` (256 timeline items, a 25 MB file,
stored bodies of 14 662 and 23 430 faces, 149 history states). All numbers
are from the release build (`RelWithDebInfo`) with `--time-limit 300` on a
32-core Linux workstation shared with other jobs (load average 5–19 during
the runs, so single timings vary by about ±15 %).

## Result on the reference design

| | before | after |
|---|---|---|
| 8 threads, wall (paired runs) | 149 s | 82 s |
| 6 threads, wall (traced runs) | 167 s | 89 s |
| 1 thread, wall (paired runs) | 301 s | 169 s |
| 1 thread, wall (quieter machine) | 267 s | 159 s |
| peak resident memory, 8 / 6 / 1 threads | 3.6 / 3.6 / 2.7 GB | 3.6 / 3.7 / 2.6 GB |
| items parametric / fallback / skipped | 218 / 16 / 22 | 218 / 16 / 22 |

The reports with one thread are byte for byte the same before and after;
with six and eight threads the outcomes, inputs read and final bodies are
the same. (One thread and several differ, before as after, in the notes of
two sketches and a body's volume in the 11th digit: see *Not the same with
threads* below.)

Time per item type, six threads (seconds of the replay; reading the file
comes before it):

| | before | after |
|---|---|---|
| reading: inputs found / bodies converted (in parallel) | 20.9 / 15.0 | 13.4 / 13.4 |
| sketches (75) | 35.2 | 9.3 |
| fillets (45) | 33.7 | 21.9 |
| extrusions (71) | 30.7 | 16.3 |
| base features (4; the first builds the two large bodies) | 30.2 | 13.8 |
| rectangular patterns (3) | 5.5 | 6.7 |
| replay in all | 142.7 | 71.7 |

## Where the time went (profile before the changes)

Stack samples of every thread (gdb, every one or two seconds) and flat
`perf` samples, with the import's own trace (`MITCAD_IMPORT_TRACE`):

- **The first item, 30 s:** building and healing the two large stored
  bodies (ShapeFix, 6 and 7 s on two threads), then OCCT's checker on the
  23 430-face body for 21 s on one thread: its shell orientation check
  searched each face for each of its edges (9.6 s for a shell whose two
  plates hold 23 428 edges each) and the wire classification of the two
  plates compared 8.6 million pairs of their 2 929 holes (3.5 s each).
- **Lookups of a face through a point, 18 % of the replay:** an extrusion up
  to a face, a sketch plane, a face reference measured the point's distance
  from every face of every body (`BRepExtrema_DistShapeShape` per face).
- **Sketches with linked curves, 35 s:** every linked curve was fitted to
  the projection of every edge near the sketch (115 000 edges, 74 000
  projected per sketch), and every edge was projected again for each sketch.
- **Fillet edge lookups:** the edges of every body were measured again for
  each rounding, because the import kept the edges of 16 bodies and each
  lookup read more bodies than that.
- **Counting a body's faces** mapped all faces of the shape again (10 % of
  the replay thread on the large bodies).
- **Reading:** a third of the time of finding the inputs and converting the
  bodies was the standard library's keyed hash of entity indices; the seams
  of sketch circles named history states one extrusion at a time at the end.
- **Fallbacks:** a base feature writes its bodies as B-rep data and reads
  them again (brep data, import_brep and naming 40 000 faces, about 2 s for
  the two large bodies), also when a retry on the exact state replaces bodies
  the replay already has (`fallback_to_as` with `all`): about 10 s in all.
  (Since mitcad#244 the base feature adopts the stored shape instead of
  reading its data back.)
- Booleans on the large bodies: about 7 s; OCCT's optimal box (particle
  swarm on torus faces) a fifth of the CPU, but on idle cores.

## Changes (implemented)

- OCCT port patches **0120** (wire boxes of circles and ellipses whose edge
  ranges pass their parameters by a rounding error) and **0121** (the
  shell's orientation check lists each large face's edge orientations once):
  the checker on the 23 430-face body 14.4 s → 2.1 s, statuses unchanged
  (`geometry/tests/test_large_bodies.cpp`, and the
  [port README](../third_party/vcpkg-ports/README.md)).
- A shape keeps its edges' middles and curves and a box per face
  (`Kernel::face_boxes`); lookups through a point pass over faces whose box
  lies farther than the tolerance.
- The import keeps body edges within a budget of edges (2 million) instead
  of 16 bodies.
- Sketch snaps and the projection rule's sources project only edges whose
  projected reach comes near a linked curve, each once
  (`core/import/src/snap.rs`, `Projector`).
- A body's face count comes from the shape's own face map.
- The `.f3d` reader keys views, seen sets and entity maps with a quick
  integer hash (`core/f3d/src/ids.rs`), finds a record's base type once, and
  works out circle seams for several extrusions at a time with threads.

## The ticket's ideas

| Idea | Status |
|---|---|
| Per-phase and per-item profile, OCCT share | Done (above). `perf` call graphs do not unwind on this machine's perf; flat `perf` plus gdb stack samples. |
| Memory: copies of bodies | Measured: peak 3.6 GB at 8 threads (the ticket's 9.4 GB predates #121's changes); history states keep distinct bodies only. Base features copied stored bodies through B-rep data; since mitcad#244 they share the stored body's B-rep (peak 3.5 → 2.9 GB at 8 threads, 2.5 → 1.9 GB at 1). |
| Faces grouped by surface | Planes per body version existed (#103); sketch planes are no longer a hotspot. Not needed. |
| Spatial index of face boxes | Implemented as per-shape face boxes for point lookups (extrusions 30.7 → 16.3 s). Edge matching by middle is not a hotspot now. |
| Per-group mass properties, divergence theorem | Measuring a body is on all cores and once per shape; under 3 % of wall. Not applicable now. |
| Incremental properties and comparison | The checker already checks only changed faces after booleans (#121); comparisons are cheap. Not applicable. |
| Lazy history bodies, cheap invariants first | Exist (bodies built on demand and ahead; Sig distances; extrusions that cannot change the volume are skipped). |
| Skip candidates by invariants | Exists; not a hotspot. |
| Local operations on a region | Exists for modelled threads. Booleans: examined in mitcad#247, not applicable (*Booleans* below). |
| Reuse between items by geometric hash | Results are cached by definition; nothing measured worth more. |
| Quadratic OCCT algorithms | Found and patched: wire classification boxes (0120) and shell orientation (0121); avoided per-face `BRepExtrema_DistShapeShape` by boxes. Upstream reports: follow-up. |
| Boolean options (OBB, fuzzy, glue, parallel) | Examined in mitcad#247: nothing to gain without changing results (*Booleans* below). |
| Validity and healing cached per shape | Exist (`checked_valid`, healed faces reused between states). |
| Adopt the file's bodies | Fallbacks do. Since mitcad#244 without a B-rep round trip: a base feature adopts the stored shape with its measures (`Kernel::adopt_shape`), the same reports, 170 → 156 s with 1 thread and 86 → 76 s with 8. The retry that replaces bodies the replay already has is left (it would change the report's names). |
| Stream the decoder and the replay | Reading (13 s at 6 threads) and the first bodies' healing (13 s) were sequential; since mitcad#245 the first state's large bodies are healed while the rest is read (*Healing while reading* below). |
| Process-level parallelism | Not applicable: one component, one dependent chain (#103's analysis). |
| GPU or SIMD | Not applicable: numeric integrals are not where the time goes. |
| Exact formulas for analytic faces | Not applicable now (see mass properties); a plane's centroid of a face with thousands of holes costs 3.5 s once per body version (`face_plane`): minor follow-up. |
| Matching faces by invariant signatures | Healed faces are matched by content hashes (#121); replay and history by signatures. Nothing to add. |

## Booleans (mitcad#247)

`MITCAD_BOOLEAN_TRACE=1` prints a line per boolean on standard error: the
operands' face counts and the time of each phase of `run_operation`
(`geometry/src/boolean.cpp`: copy, far features, OCCT's boolean, face
merge, naming, checker, holes put back) and of the whole call. On the
reference design with one thread (release build, after #243):

- 216 boolean calls (230 operations with the reruns without the face
  merge) take 18.7 s of a 161 s import; OCCT's boolean itself 11.1 s,
  the checker on the changed faces 1.7 s, the face merge 1.2 s, copies,
  far features and naming under 0.6 s.
- **No boolean takes a large body.** The largest target has 1 638 faces;
  the stored bodies of 14 662 and 23 430 faces are built, healed and
  compared, but no cut or join of the replay works on them any more
  (#119's "booleans on the large bodies, ~7 s" came before its own and
  #243's changes). Most cuts are of tools of 6–27 faces on bodies of
  about 1 300 faces with 170–320 holes taken out (far features already
  in place), 0.1–0.2 s each.
- The four slowest are a rectangular pattern's joins: 765 to 5 346 tool
  faces (up to 1 071 copies) into a body of 29 faces, 5.7 s together
  (2.3 s for the largest in OCCT's boolean alone).

What was tried, on the operands of the slowest operations written out
and run again alone (results compared as BRep text):

- **Oriented boxes** (`SetUseOBB`): the same result here, but no
  quicker (2.51 s against 2.35 s on the largest join; the copies and
  faces are axis-aligned, so the oriented boxes reject nothing the plain
  ones do not). Already known to change a corpus result (a hole cut kept
  an extra edge): not used.
- **Parallel mode** (`SetRunParallel`): already on; the same result as
  serial (3.9 s serial against 2.3 s on the largest join).
- **Glue options**: they assume the operands only share coinciding faces
  and never cross; the cuts and joins here cross, so a glue option would
  not be exact. Not used.
- **Local booleans** (only the faces near the tool, as for modelled
  threads, `local_boolean.hpp`): its shell is the far faces in the
  body's order followed by the kept pieces, not the order OCCT's boolean
  assembles. A body's volume, centre and reference point are sums over
  its faces and vertices in their order, so another order changes the
  volumes in the last digits, and the reports print the differences to
  the history in full. Making the order OCCT's would mean rebuilding its
  shell assembly. On the design's cuts the local path does not even
  apply (tools of several solids), and the ordinary boolean it falls
  back to, without the face merge, already gives other faces (1 390
  faces with 2 477 edges against 2 475, 6 at the same index). With no
  boolean on a large body on this design there is nothing for it to win
  either: kept global.
- The reruns without the face merge (after the merge broke a result)
  cost 0.6 s; reusing the first run's unmerged result is not safe (the
  merge writes into the sub-shapes it shares).

## Healing while reading (mitcad#245)

The first item of the reference design is a base feature with the two
large stored bodies; its history state is the large blob's oldest state
with bodies. Reading took 15.5 s with eight threads (four finding the
inputs, four converting the bodies), and the first item then waited
21 s for the two bodies' healing. Now the conversion of each history blob
converts its oldest state with bodies first, and heals its bodies of
1 000 faces or more on threads taken from the converting ones (at most
half of them; `mitcad_import::Share`, `in_order_sharing`) while the rest
of the file is read; the bodies go to the replay as bodies built ahead
(`core/import/README.md`, *Work beside the replay*). The two bodies were
healed in 12.5 and 5.4 s beside the conversion (14.8 s), and the first
item took 1.1 s instead of 22.6 s.

| `0b758b01` | before | after |
|---|---|---|
| 8 threads, wall | 88.5 s | 61.6 s |
| 8 threads, peak resident memory | 3.02 GB | 3.12 GB |
| 1 thread, wall | 178 s | 136 s |
| 1 thread, peak resident memory | 1.97 GB | 2.01 GB |
| reports | | byte for byte the same, before and after, with 1 and 8 threads |

One paired run each (the load average 12–15 during the 8-thread runs; 14
and 4 during the 1-thread ones, whose code is the same before and after:
with one converting thread nothing is healed while reading, so that
difference is the machine's load).

## History states named once (mitcad#246)

Finding the inputs names history states (`NamedState`: the bodies, faces
and edges of a blob at a state, with their names). On the reference design
it named 135 states 234 times: an item looks in its own state for its
bodies, its fillets' faces and its edges separately, an input not found
there is looked for up to 24 states back (the states of the items before
it), and the circle seams named each extrusion's state again after the
items. A state is a function of the blob and the position alone, so a
naming can be shared without changing what is found: `NamedState` owns
its view, and the inputs keep the states last used within a size budget
(`KEPT_STATES` in `core/f3d/src/design/inputs.rs`), the circle seams take
the whole circles when the items name the state, and a fillet's face
selections name the earlier states one at a time instead of all 24 at
once.

| Budget | Namings | Peak memory, finding the inputs alone |
|---|---|---|
| before | 234 | 0.61 GB |
| none kept (only shared while in use) | 188 | 0.63 GB |
| two or three states (chosen) | 184 | 0.72 GB |
| about twelve states | 155 | 1.09 GB |
| all | 135 | 5.7 GB |

Each naming takes 0.13–0.15 s on that design (about 160 000 faces, edges
and changed entities). The 24-state look-backs make the remaining
repeats: naming each state once needs about twelve states kept, and with
threads finding the inputs runs beside the conversion of the bodies,
where the import's memory peaks: twelve states raised that peak from 3.6
to 4.1 GB, so two or three are kept. The import on the reference design
(same reports, byte for byte, with one and with eight threads):

| | before | after |
|---|---|---|
| 1 thread, wall / peak memory | 154 s / 2.59 GB | 152 s / 2.52 GB |
| 8 threads, run back to back (load average 21–27) | 110 s / 3.69 GB | 92 s / 3.48 GB |
| 8 threads, first pair (load average 6–15 before, 11–17 after) | 68 s / 3.56 GB | 88 s / 3.47 GB |

The wall times follow the machine's load (other jobs ran beside them);
the namings saved are about 6 s of one thread's work. Memory is a little
lower, as each state's lists no longer keep spare room and the fillets'
look-backs hold one state at a time.

## Not the same with threads

With threads the history's bodies are built ahead, and a body built after
another state's build of the same blob took the faces it shares with it as
healed there (#121). Which builds existed first depended on timing, so a
body could be healed as a whole in one run and from shared faces in
another, and differ by an edge (113 568 against 113 569 replay edges on the
reference design): two sketches then kept one or two more linked curves as
fixed geometry, and a final body's volume differed in the 11th digit.
Since #243 each build's base is fixed by the file (the same body in the
blob's state before), waited for or built when missing, so the reports with
one thread and with several are the same.
