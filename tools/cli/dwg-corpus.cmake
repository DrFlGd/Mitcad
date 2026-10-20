# SPDX-License-Identifier: MIT
# The .dwg import on the trial models (mitcad#453): every drawing an
# index.tsv under MITCAD_DWG_CORPUS lists (the folder of each index and
# below) is imported with mitcad-cli import-dwg --check, which fails when
# a body is not built or not valid, the bodies lie outside the drawing's
# $EXTMIN/$EXTMAX, or a data sheet value of the row disagrees.
#
# index.tsv: a header row naming the columns, then a row per drawing. The
# column `file` (the drawing's path relative to the index) is required;
# these are checked when a row gives them (the import report's `checks`):
#
#   length_mm          the side of the bodies' box nearest to it (within
#                      1 mm or 0.5 %)
#   chord_distance_mm  with tube_diameter_mm, the nearest of the other
#                      sides (within 1 mm or 1 %)
#   tube_diameter_mm   a cylindrical face of the diameter (0.1 mm)
#   tube_wall_mm       and one of the inner diameter
#   mass_kg            the solids' volume times density_kg_m3 (default
#                      2700, aluminium), within 5 %
#   density_kg_m3
#   volume_mm3         the solids' volume (within 1 %), e.g. of a
#                      reference model of the product
#   solids, components their counts
#
# Other columns (the product, its source, the date) are notes. Drawings
# are named by their content id (the first 8 hex digits of their SHA-256).
# Skipped without the corpus.
#
#   cmake -DCLI=<mitcad-cli> -DWORK=<dir> -P dwg-corpus.cmake

if(NOT DEFINED ENV{MITCAD_DWG_CORPUS} OR NOT IS_DIRECTORY "$ENV{MITCAD_DWG_CORPUS}")
  message("dwg corpus: skipped (MITCAD_DWG_CORPUS not set)")
  return()
endif()
file(GLOB_RECURSE indexes "$ENV{MITCAD_DWG_CORPUS}/index.tsv")
if(NOT indexes)
  message("dwg corpus: skipped (no index.tsv under MITCAD_DWG_CORPUS)")
  return()
endif()
file(MAKE_DIRECTORY "${WORK}")
set(checked_columns length_mm chord_distance_mm tube_diameter_mm tube_wall_mm mass_kg density_kg_m3 volume_mm3 solids
  components)
set(count 0)
set(failed "")
foreach(index IN LISTS indexes)
  get_filename_component(dir "${index}" DIRECTORY)
  file(STRINGS "${index}" rows)
  set(header "")
  foreach(row IN LISTS rows)
    if(row MATCHES "^[ \t]*(#|$)")
      continue()
    endif()
    # Tabs separate the columns (empty columns kept).
    string(REPLACE "\t" ";" cells "${row}")
    if(header STREQUAL "")
      set(header "${cells}")
      list(FIND header file file_column)
      if(file_column LESS 0)
        message(FATAL_ERROR "${index}: the header has no column 'file'")
      endif()
      continue()
    endif()
    list(LENGTH cells n)
    if(n LESS_EQUAL file_column)
      continue()
    endif()
    list(GET cells ${file_column} relative)
    set(drawing "${dir}/${relative}")
    if(NOT EXISTS "${drawing}")
      list(APPEND failed "a listed drawing is missing (row ${count})")
      continue()
    endif()
    file(SHA256 "${drawing}" hash)
    string(SUBSTRING "${hash}" 0 8 id)
    set(expect "")
    foreach(column IN LISTS checked_columns)
      list(FIND header ${column} at)
      if(at GREATER_EQUAL 0 AND at LESS n)
        list(GET cells ${at} value)
        string(STRIP "${value}" value)
        if(value MATCHES "^[0-9]+(\\.[0-9]+)?$")
          if(expect STREQUAL "")
            set(expect "${column}=${value}")
          else()
            set(expect "${expect},${column}=${value}")
          endif()
        endif()
      endif()
    endforeach()
    set(expect_args "")
    if(NOT expect STREQUAL "")
      set(expect_args --expect "${expect}")
    endif()
    math(EXPR count "${count} + 1")
    execute_process(
      COMMAND "${CLI}" import-dwg "${drawing}" --check --report "${WORK}/${id}.json" --save "${WORK}/${id}.mitcad"
              ${expect_args}
      OUTPUT_VARIABLE out ERROR_VARIABLE err RESULT_VARIABLE status)
    message("== ${id}\n${out}")
    if(NOT status EQUAL 0)
      list(APPEND failed "${id}: ${err}")
    endif()
  endforeach()
endforeach()
list(LENGTH failed bad)
if(count EQUAL 0)
  message("dwg corpus: skipped (the indexes list no drawings)")
  return()
endif()
if(bad GREATER 0)
  string(REPLACE ";" "\n" lines "${failed}")
  message(FATAL_ERROR "dwg corpus: ${bad} of ${count} drawings fail their checks:\n${lines}")
endif()
message("dwg corpus: ${count} drawings pass their checks")
