# mitcad-import: .f3d and FreeCAD designs with their history

`core/import` replays a design's history as Mitcad features, checks each
step against the bodies stored in the file, and brings in what it cannot
replay as the stored bodies. These imports are described here:

- **.f3d** (`src/*.rs`): the design dump IR
  (`mitcad_f3d::design::ir::Dump`, [SCHEMA.md](SCHEMA.md)), checked
  against the file's ASM history.
- **FreeCAD .FCStd** (`src/freecad/`), checked against every feature's
  stored result.
- **.ipt** and **.iam**: read by `core/ipt` and replayed on the same dump
  IR as the `.f3d` import.
- **.solidcraft** (`src/solidcraft/`): built again from the file's
  definitions; the file keeps no bodies to check against.

The imports are generic over the [`Kernel`](../model/src/kernel.rs); the
tests run them on the model's mock kernel. The JSON commands and report
fields are in `core/model/src/api/commands.md` (*.f3d import with the
timeline*, *FreeCAD import (.FCStd)*).

## Documents

This README is the index. The notes are in `docs/`, one document per
format and, for the `.f3d` import, per topic:

| Document | What it holds |
|---|---|
| [docs/f3d.md](docs/f3d.md) | The `.f3d` import: entry points, modules, crashes, limits and the known failure classes. |
| [docs/f3d-history.md](docs/f3d-history.md) | The history as the reference, named inputs, sketch patterns, threads, sweeps, path patterns, what the history settles. |
| [docs/f3d-fallbacks.md](docs/f3d-fallbacks.md) | Fallbacks to the file's stored bodies, the outcomes on the timeline, items suppressed in the file, retries on the file's exact state, the geometric tolerance. |
| [docs/f3d-performance.md](docs/f3d-performance.md) | Time limit, stop and hangs, memory, definitions evaluated in parallel, work beside the replay. |
| [docs/f3d-components.md](docs/f3d-components.md) | Components, components of other documents, derived designs, joints, assembly relationships, items without a translation. |
| [docs/f3d-sheet-metal.md](docs/f3d-sheet-metal.md) | Sheet metal. |
| [docs/f3d-appearances.md](docs/f3d-appearances.md) | Appearances and materials, designs with many face appearances, light bulbs. |
| [docs/f3d-learning.md](docs/f3d-learning.md) | Learning the undecoded inputs: the learning dump and the input decoding audits. |
| [docs/f3d-undecoded.md](docs/f3d-undecoded.md) | What the file does not settle yet, the published format descriptions against the decoder. |
| [docs/f3d-rounds-1.md](docs/f3d-rounds-1.md), [-2](docs/f3d-rounds-2.md), [-3](docs/f3d-rounds-3.md), [-4](docs/f3d-rounds-4.md) | Notes on single rounds of work on the corpus: the censuses, model by model, and the fillet and chamfer rounds. |
| [docs/freecad.md](docs/freecad.md) | The FreeCAD import. |
| [docs/ipt.md](docs/ipt.md) | The `.ipt` import. |
| [docs/iam.md](docs/iam.md) | The `.iam` import. |
| [docs/solidcraft.md](docs/solidcraft.md) | The `.solidcraft` import. |

The documents cite each other's sections by their titles in italics, with
a link where the section is in another document. [SCHEMA.md](SCHEMA.md)
is the dump format.

## Where new text goes

- A change to an import adds its notes to the document of its topic, as a
  section of its own at the end of the topic's part, so that parallel
  changes touch different places. Not here.
- A note on one round of work on the corpus goes into the newest
  `docs/f3d-rounds-*.md`; a new numbered one is started when that file
  nears 2 000 lines.
- A new topic gets a document of its own in `docs/` and a row in the table
  above. A document that grows past about 2 000 lines is split by topic.
- Cite a section by its document and title (`docs/f3d-history.md`, *Named
  inputs*); when a section moves, update the links to it
  (`grep -rn 'core/import/docs\|core/import/README' .`).
- Unit tests go into the topic module of `src/tests/`; see below.

## Tests and coverage runs

The crate's unit tests run on the model's mock kernel (`cargo test -p
mitcad-import`, the ctest `core.import`). Those of the `.f3d` import are
in `src/tests/`: `src/tests.rs` only declares the modules, `support.rs`
holds the test kernel and the builders several of them share, and every
other module holds the tests of one topic. The corpus tests of the other
formats are described with them: [FreeCAD](docs/freecad.md#tests),
[.ipt](docs/ipt.md#corpus-test), [.iam](docs/iam.md#iam-corpus-test).

### Corpus test

Real designs are kept outside the repository (`MITCAD_F3D_CORPUS`, default
`~/f3d-corpus`). `test_f3d_import` (`core/tests/test_f3d_import.cpp`):

```bash
test_f3d_import --corpus [dir] [--every N] [--max-items N] \
    [--time-limit S] [--hang-limit S] [--reports DIR] [--threads N] \
    [--jobs N] [--memory SIZE] [--file-timeout S]       # build/rel/core
mitcad-cli import-f3d part.f3d --time-limit 300 --report report.json
```

- `--corpus` imports every design of every `.f3d`/`.f3z` under the
  directory, requires each import to finish without error, and prints
  per feature type how items came in (parametric, partial, fallback,
  skipped) and how the final bodies agree with the stored ones. Exits 77
  (skipped) without a corpus. Files are named by position (f01, …), as in
  `core/f3d/CORPUS_REPORT.md`.
- `--reports DIR` keeps each JSON report; `--hang-limit` defaults to 600 s.
- `--threads N` imports with N threads (*Definitions evaluated in
  parallel*, *Work beside the replay*; default 1). Keep `--jobs` ×
  `--threads` within the cores; the reports are those of `--threads 1`
  except where the time decides.
- `--models [dir]` replays the reference models' external dumps
  (`MITCAD_F3D_MODELS`, default `~/f3d-models`) of lofts and sweeps,
  compares with the stored volumes, and imports each model from its own
  streams too.
- The ctest `f3d.corpus_timeline` (`core/CMakeLists.txt`) runs `--every 4
  --max-items 30 --time-limit 60` to stay quick in debug builds. Run the
  full corpus with the release build, e.g. `--time-limit 300 --hang-limit
  300` per design.
- Each file (each model with `--models`) is imported in a child process of
  its own, `--jobs N` at a time (default: the cores / 4, bounded by the
  memory available / `--memory`, 4G per child by default), the largest
  first; a crash, a hang past `--file-timeout` (3600 s) or a file over its
  memory ends only that file, which counts as failed. The lines come in
  the files' order and the totals are those of `--jobs 1` (every file in
  the one process, as before); [docs/development.md](../../docs/development.md#f3d-corpus-tests)
  has the options.

#### Coverage runs in parallel

A full coverage run, for numbers before and after a change: build both
states with the release preset (keep the binaries of the first), then run
each with every file and compare. The lines are those of a run in one
process, so only the times differ:

```bash
B=build/rel/core   # the binaries of one state
$B/test_f3d_import --corpus --time-limit 300 --hang-limit 300 --jobs 8 \
    --reports before-reports > before.txt
$B/test_brep_import --corpus --jobs 8 > before-bodies.txt
$B/test_exchange --corpus --jobs 8 > before-import.txt
$B/test_f3d_import --models --jobs 8 > before-models.txt
# ... the same with the other state's binaries into after*.txt, then:
diff <(sed -E 's/[0-9.]+ m?s$//' before.txt) <(sed -E 's/[0-9.]+ m?s$//' after.txt)
```

The children's memory adds up: keep `--jobs` × `--memory` within what
the machine (or `systemd-run --user --scope -p MemoryMax=...`) allows. The
line on standard error at the end gives the largest peak memory of a
child and the longest child, e.g. to choose `--memory`.

