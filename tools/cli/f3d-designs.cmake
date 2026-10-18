# SPDX-License-Identifier: MIT
# Single .f3d designs checked item by item (mitcad#311): each found by its
# content id (the first 8 hex digits of its SHA-256) under the folders of
# MITCAD_F3D_CORPUS and MITCAD_F3D_BACKUP (each one folder or several
# separated by ':' or ';'), imported with mitcad-cli import-f3d with the
# import trace on. Per design:
#
# - the items named parametric come in parametric, and only the items
#   named as falling back fall back (the others of the file's own base
#   features and of placing occurrences aside);
# - no parametric item went through a retry on the file's exact state,
#   and every modelling input was read, none inferred from the history
#   (but those of the items a design names as inferred, a fourth field);
# - reading the file and applying its appearances take less time than
#   replaying its items (the appearances made a design of hundreds of face
#   appearances 16 times slower to open: 25 s reading them and 159 s
#   applying them against 8 s of replay).
#
# Skipped when none of the designs is found.
#
#   cmake -DCLI=<mitcad-cli> -DWORK=<dir> -P f3d-designs.cmake

cmake_minimum_required(VERSION 3.24)

# <id>|<items parametric, comma separated>|<items that fall back>[|<items
# whose inputs may be inferred>]
set(designs
    # A circuit board with a pole round it (150 items, 78 occurrences):
    # its fillet between the pole's chamfer and a band 0.3 mm high where
    # flats cut the pole runs over the flats: built with them filled and cut
    # out again (mitcad#311). Nothing falls back.
    "e3c34193|Revolve1,Revolve2,Revolve3,Fillet5,Extrude8,Extrude11,Sketch12,Plane1,Fillet9,Fillet10|"
    # An assembly of a housing, a plug screwed into it, a nut and a tool
    # (145 items, 7 components; mitcad#379): the plug's thread found its
    # angle in the plug's own bodies, not in the housing's internal thread
    # round it. The revolution cut into the threaded bore is parametric: the
    # bore's cylinders, which the file keeps as cones of 1e-13 rad, are read
    # as cylinders (mitcad#389). A rounding along the thread (item 109) is
    # parametric since the walk past a seam of mitcad#235. Falling back: a
    # sweep that passes over itself, roundings the geometry kernel does not
    # build or builds off the file's, an extrusion whose profile's curve the
    # sketch no longer has.
    "db73863f|Thread1,Revolve4|Sweep1,Fillet4,Fillet1,Fillet2,Fillet6,Extrude3,Fillet1,Fillet2,Fillet7,Fillet5,Fillet5,Fillet6|Axis1,C-Pattern1,Extrude3,Extrude6,Fillet4,GeometricRelationship1,Plane1,Plane2,Plane3,Thread1,Thread2")

# Seconds as the trace gives them (two decimals) in hundredths.
function(hundredths seconds out)
  string(REPLACE "." "" digits "${seconds}")
  string(REGEX REPLACE "^0+([0-9])" "\\1" digits "${digits}")
  set(${out} "${digits}" PARENT_SCOPE)
endfunction()

set(folders "")
foreach(variable MITCAD_F3D_CORPUS MITCAD_F3D_BACKUP)
  if(DEFINED ENV{${variable}})
    string(REPLACE ";" ":" value "$ENV{${variable}}")
    string(REPLACE ":" ";" value "${value}")
    foreach(folder IN LISTS value)
      if(IS_DIRECTORY "${folder}")
        list(APPEND folders "${folder}")
      endif()
    endforeach()
  endif()
endforeach()
if(NOT folders)
  message("f3d designs: skipped (MITCAD_F3D_CORPUS and MITCAD_F3D_BACKUP name no folder)")
  return()
endif()

# The designs' ids, then each file's id until all are found.
set(wanted "")
foreach(design IN LISTS designs)
  string(REPLACE "|" ";" fields "${design}")
  list(GET fields 0 id)
  list(APPEND wanted "${id}")
endforeach()
set(found_ids "")
foreach(folder IN LISTS folders)
  file(GLOB_RECURSE files "${folder}/*.f3d")
  list(SORT files)
  foreach(f IN LISTS files)
    # (A name with a semicolon falls apart in the list: not looked at.)
    if(NOT EXISTS "${f}" OR IS_DIRECTORY "${f}")
      continue()
    endif()
    file(SHA256 "${f}" sum)
    string(SUBSTRING "${sum}" 0 8 id)
    list(FIND wanted "${id}" at)
    list(FIND found_ids "${id}" seen)
    if(at GREATER -1 AND seen EQUAL -1)
      list(APPEND found_ids "${id}")
      set(path_${id} "${f}")
    endif()
  endforeach()
endforeach()
if(NOT found_ids)
  message("f3d designs: skipped (none of ${wanted} under ${folders})")
  return()
endif()

file(MAKE_DIRECTORY "${WORK}")
set(problems "")
foreach(design IN LISTS designs)
  string(REPLACE "|" ";" fields "${design}")
  list(GET fields 0 id)
  list(GET fields 1 parametric)
  list(GET fields 2 falling)
  set(inferred "")
  list(LENGTH fields field_count)
  if(field_count GREATER 3)
    list(GET fields 3 inferred)
  endif()
  string(REPLACE "," ";" parametric "${parametric}")
  string(REPLACE "," ";" falling "${falling}")
  string(REPLACE "," ";" inferred "${inferred}")
  list(FIND found_ids "${id}" at)
  if(at EQUAL -1)
    message("f3d designs: ${id} not found, not checked")
    continue()
  endif()
  execute_process(
    COMMAND "${CMAKE_COMMAND}" -E env MITCAD_IMPORT_TRACE=1
            "${CLI}" import-f3d "${path_${id}}" --report "${WORK}/${id}.json"
    OUTPUT_FILE "${WORK}/${id}.txt" ERROR_FILE "${WORK}/${id}.trace" RESULT_VARIABLE status)
  if(NOT status EQUAL 0)
    list(APPEND problems "${id}: import-f3d failed (${status})")
    continue()
  endif()
  file(READ "${WORK}/${id}.json" report)
  string(JSON count LENGTH "${report}" designs 0 items)
  math(EXPR last "${count} - 1")
  set(fell "")
  foreach(i RANGE ${last})
    string(JSON name GET "${report}" designs 0 items ${i} name)
    string(JSON type GET "${report}" designs 0 items ${i} type)
    string(JSON outcome GET "${report}" designs 0 items ${i} outcome)
    string(JSON retry ERROR_VARIABLE retry_missing GET "${report}" designs 0 items ${i} exact_retry)
    string(JSON provenance ERROR_VARIABLE provenance_missing GET "${report}" designs 0 items ${i}
           input_provenance)
    list(FIND parametric "${name}" named)
    if(named GREATER -1 AND NOT outcome STREQUAL "parametric")
      list(APPEND problems "${id}: ${name} ${outcome}, not parametric")
    endif()
    if(outcome STREQUAL "fallback" AND NOT type STREQUAL "BaseFeature")
      list(APPEND fell "${name}")
    endif()
    if(outcome STREQUAL "parametric" AND NOT retry_missing)
      list(APPEND problems "${id}: ${name} is parametric after a retry on the file's exact state")
    endif()
    list(FIND inferred "${name}" may_infer)
    if(outcome STREQUAL "parametric" AND NOT provenance_missing AND NOT provenance STREQUAL "read"
       AND may_infer EQUAL -1)
      list(APPEND problems "${id}: ${name}'s inputs ${provenance}, not read")
    endif()
  endforeach()
  if(NOT fell STREQUAL falling)
    list(APPEND problems "${id}: falling back: '${fell}', expected '${falling}'")
  endif()
  # The phases' times from the trace.
  file(STRINGS "${WORK}/${id}.trace" reading REGEX "file read: inputs found in [0-9.]+ s")
  file(STRINGS "${WORK}/${id}.trace" items REGEX "^import: \\[[0-9.]+\\] item [0-9]+:")
  file(STRINGS "${WORK}/${id}.trace" faces REGEX "^import: appearance of face .* in [0-9.]+ s$")
  if(NOT reading OR NOT items)
    list(APPEND problems "${id}: the trace has no reading time or no items")
    continue()
  endif()
  list(GET reading 0 reading)
  string(REGEX REPLACE ".* in ([0-9.]+) s.*" "\\1" reading "${reading}")
  list(GET items -1 replay)
  string(REGEX REPLACE "^import: \\[([0-9.]+)\\].*" "\\1" replay "${replay}")
  set(applying 0)
  foreach(line IN LISTS faces)
    string(REGEX REPLACE ".* in ([0-9.]+) s$" "\\1" s "${line}")
    hundredths("${s}" s)
    math(EXPR applying "${applying} + ${s}")
  endforeach()
  hundredths("${reading}" reading_cs)
  hundredths("${replay}" replay_cs)
  math(EXPR beside "${reading_cs} + ${applying}")
  message("f3d designs: ${id}: reading ${reading} s, replay ${replay} s, appearances "
          "${applying} cs; falling back: ${fell}")
  if(beside GREATER replay_cs)
    list(APPEND problems "${id}: reading (${reading} s) and the appearances (${applying} cs) took longer than the replay (${replay} s)")
  endif()
endforeach()
if(problems)
  string(REPLACE ";" "\n  " problems "${problems}")
  message(FATAL_ERROR "f3d designs:\n  ${problems}")
endif()
