# Failed allocations, threads and memory errors: 0030–0034, 0090, 0091

Patches that make OCCT handle a failed allocation, give the checker's
parallel mode the sequential mode's results, keep a Windows signal from
locking a mutex twice, and fix a heap overflow of the chamfer and the
fillet. Back to the [port's README](../README.md).

## Overview

- **0030 to 0032** make an allocation that fails throw instead of
  crashing, let a caller learn of it and leave the collections valid
  (mitcad#132).
- **0033** makes the checker's parallel mode give the sequential mode's
  results (mitcad#203).
- **0034** lets a signal raised on Windows reach OCCT's exception instead
  of locking its mutex twice.
- **0090 and 0091** fix a heap overflow of OCCT's chamfer and fillet
  (mitcad#160).

## 0030 to 0032: failed allocations

The patches 0030 to 0032 make OCCT handle an allocation that fails
(mitcad#132).

### 0030-standard-failed-allocation-throws

`Standard::AllocateOptimal`, under the collections' allocators and the
strings, returned null when an allocation failed with the native (the
default), TBB and jemalloc memory managers, and its callers wrote to the
block at once: under a memory limit the shape checker crashed in
`CSLib_Class2d`. It throws `Standard_OutOfMemory` now, as
`Standard::Allocate` and the flexible memory manager do;
`Standard_Failure`'s message, the one caller that checked for null, goes
without it as before. `Standard_OutOfMemory::NbRaised` counts the exceptions
made, so that a caller learns of a failure that the checker or the healing
caught inside and went on from.

### 0031-osd-threadpool-failed-allocations-to-launching-thread

`Standard_OutOfMemory::NbRaisedInThread` counts the failures of the calling
thread, and `OSD_ThreadPool::Launcher` hands those of the jobs its threads
ran over to the thread that launched them (the checker and the booleans run
on the pool), so a caller knows whether memory ran out under its own call
rather than under another thread's at the same time.

### 0032-ncollection-containers-intact-when-allocation-fails

The collections stay valid when an allocation in them throws.
`NCollection_Array1::Resize` and `NCollection_LocalArray` freed their block
before allocating the new one and kept pointing to it (freed again by their
destructors); `NCollection_IndexedMap` and
`NCollection_IndexedDataMap::ReSize` moved their nodes to the new buckets
before reallocating their index array, and when that threw their destructors
freed nodes twice (a boolean ended the process so under a memory limit).
`NCollection_BaseMap::BeginResize` leaked its first bucket array when the
second failed.

## 0033: the checker's parallel mode

### 0033-brepcheck-edge-curve-adaptor-per-context

In the checker's parallel mode (`BRepCheck_Analyzer`'s `theIsParallel`,
which MitCad's checks use since mitcad#181) the faces are checked on several
threads, and an edge in the contexts of its two faces at the same time
(`BRepCheck_Edge::InContext`). Both evaluated the edge's one curve adaptor,
which caches the span of a B-spline it evaluated last, so a thread could
take another's span, find the edge off its face
(`BRepCheck_InvalidCurveOnSurface`) and call the face unorientable: a valid
shape came out valid or invalid from run to run (a round groove swept along
a helix and cut from a nut, mitcad#203). Each context now evaluates its own
copy of the adaptor (`ShallowCopy`); the sequential mode, the checks and
their statuses are unchanged.

## 0034: the Windows signal handler

### 0034-osd-signal-handler-recursive-mutex

On Windows the handler `OSD::SetSignal` installs for SIGSEGV, SIGFPE and
SIGILL (`SIGWntHandler`) locked the signal mutex and called `CallHandler`,
which locked it again on the same thread; MSVC's runtime threw
`std::system_error` ("resource deadlock would occur") in place of
`OSD_Exception_ACCESS_VIOLATION`, which the geometry's guard missed (a crash
in a section's booleans was reported on the body instead of the section).
The mutex is recursive now.

## 0090 and 0091: memory errors of the chamfer and the fillet

The patches 0090 and later fix memory errors (mitcad#160).

### 0090-chfi3d-double-interference-at-vertex-kind

Where two consecutive pieces of a chamfer or rounding meet on an edge they
reach with opposite transitions, `ChFi3d_FilDS` adds a second interference
on that edge. When the pieces meet at a vertex of the edge, the index it
used is the vertex's, but the interference was made of kind point (the other
calls there pass the kind); `TopOpeBRepBuild_Builder::BuildEdges` then
counted that point past the end of its array of points and overwrote the
heap (`free(): invalid next size` later; a debug build of OCCT throws
`Standard_OutOfRange`). The interference now takes the index's kind, and the
existing one is looked up with it. The case: a counterbore's circle on its
floor chamfered by the floor's width, the circle in two arcs whose radii
differ by rounding, so that the chamfer runs a hair inside the hole along
one arc and on the floor along the other, the two meeting at the hole
circle's vertex.

### 0091-topopebrepbuild-point-uses-within-array

`BuildEdges` counts the uses of a point only for indices within its array,
as one of its four counts already did: an interference with a wrong index
can no longer write outside it.

## Reports and tests

- 0030 to 0032: mitcad#132 and `geometry/tests/test_allocation.cpp`.
- 0033: mitcad#203 and the helix cases of `geometry/tests/test_sweeps.cpp`.
- 0034: the ctests `cli.occt_crash` and `cli.occt_crash_section` on
  Windows.
- 0090 and 0091: mitcad#160 and `geometry/tests/test_chamfer_kernel.cpp`,
  which aborts without them.
