# SPDX-License-Identifier: MIT
# The .rvt import on the corpus's MIT-licensed 2024 project (mitcad#274):
# found under MITCAD_RVT_CORPUS by its content id (the first 8 hex digits
# of its SHA-256, the corpus index's), imported with mitcad-cli import-rvt;
# every building element must come in, each on its level's component.
# Skipped without the corpus.
#
#   cmake -DCLI=<mitcad-cli> -DWORK=<dir> -P rvt-corpus.cmake

set(id c805df44)
if(NOT DEFINED ENV{MITCAD_RVT_CORPUS} OR NOT EXISTS "$ENV{MITCAD_RVT_CORPUS}/index.tsv")
  message("rvt corpus: skipped (MITCAD_RVT_CORPUS not set)")
  return()
endif()
set(corpus "$ENV{MITCAD_RVT_CORPUS}")
file(READ "${corpus}/index.tsv" index)
set(project "")
# The row whose second column (the SHA-256) starts with the id: its path.
if(index MATCHES "(^|\n)([^\t\n]+)\t${id}[0-9a-f]*\t" AND EXISTS "${corpus}/${CMAKE_MATCH_2}")
  set(project "${corpus}/${CMAKE_MATCH_2}")
endif()
if(project STREQUAL "")
  message("rvt corpus: skipped (no file ${id} in the corpus)")
  return()
endif()
file(MAKE_DIRECTORY "${WORK}")
execute_process(
  COMMAND "${CLI}" import-rvt "${project}" --report "${WORK}/${id}.json" --save "${WORK}/${id}.mitcad"
  OUTPUT_VARIABLE out ERROR_VARIABLE err RESULT_VARIABLE status)
message("${out}")
if(NOT status EQUAL 0)
  message(FATAL_ERROR "${id}: import-rvt failed (${status}): ${err}")
endif()
file(READ "${WORK}/${id}.json" report)
string(JSON bodies GET "${report}" import bodies)
string(JSON not_built LENGTH "${report}" import not_built)
string(JSON levels LENGTH "${report}" import levels)
string(JSON memory GET "${report}" import memory_low)
# The project's 854 building elements the reference reader lists (its 970
# elements but 116 rooms, which are spaces) and its other elements of
# building categories: every one placed (core/import/tests/rvt_corpus.rs
# checks them one by one).
if(bodies LESS 854 OR NOT not_built EQUAL 0 OR NOT memory EQUAL 0)
  message(FATAL_ERROR "${id}: ${bodies} bodies, ${not_built} not built, ${memory} left out for memory")
endif()
set(placed 0)
math(EXPR last "${levels} - 1")
foreach(i RANGE ${last})
  string(JSON n GET "${report}" import levels ${i} bodies)
  string(JSON component GET "${report}" import levels ${i} component)
  if(n GREATER 0 AND component STREQUAL "")
    message(FATAL_ERROR "${id}: level ${i} has bodies but no component")
  endif()
  math(EXPR placed "${placed} + ${n}")
endforeach()
if(NOT placed EQUAL bodies)
  message(FATAL_ERROR "${id}: ${placed} bodies on level components of ${bodies}")
endif()
message("rvt corpus: ${id}: ${bodies} bodies on ${levels} levels")
