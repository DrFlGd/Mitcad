// SPDX-License-Identifier: MIT
#pragma once

// How modelled threads build and apply their tools (mitcad#843), for
// comparisons and tests. Internal to the geometry library.
//
// A thread of several faces reads every face from the body it is given.
// The tools of a face whose reach (the box round all its tools can change)
// meets no other face's depend only on its cylinder and the spec: they are
// built at once on all cores (OCCT's thread pool), and the tools of one
// stage (bands, chamfers, what lies beyond the crest, grooves) of all such
// faces go into one local boolean where that can be done. The faces whose
// reaches meet are threaded after them one by one, each read from the body
// the one before left, as all faces were before. The result is the same.

#include <cstddef>

namespace mitcad::geometry::detail {

struct ThreadOptions {
  // The faces planned from the body as given (above); false: face by face
  // as before mitcad#843 (MITCAD_THREAD_SEQUENTIAL=1).
  bool planned = true;
  // The planned tools built on all cores; false: one after another on the
  // calling thread (MITCAD_THREAD_PARALLEL=0).
  bool parallel = true;
  // The tools of one stage in one local boolean where that can be done;
  // false: one boolean per tool (MITCAD_THREAD_MERGE=0).
  bool merged = true;
};

// This thread's options: those of the innermost ThreadOptionsScope, else
// the environment's, read at each call.
ThreadOptions thread_options();

// While it lives, the threads made on this thread use `options` (tests).
class ThreadOptionsScope {
public:
  explicit ThreadOptionsScope(const ThreadOptions& options);
  ~ThreadOptionsScope();
  ThreadOptionsScope(const ThreadOptionsScope&) = delete;
  ThreadOptionsScope& operator=(const ThreadOptionsScope&) = delete;

private:
  const ThreadOptions* m_previous;
  ThreadOptions m_options;
};

// Counts over the process so far (tests and traces).
struct ThreadCounts {
  // Faces whose tools were built ahead, and faces of planned threads
  // threaded one by one because their reach met another face's.
  std::size_t planned_faces = 0;
  std::size_t ordered_faces = 0;
  // Local booleans that took the tools of several faces in one, and the
  // tools they took.
  std::size_t merged_booleans = 0;
  std::size_t merged_tools = 0;
};
ThreadCounts thread_counts();

} // namespace mitcad::geometry::detail
