# SPDX-License-Identifier: MIT
# IFC files kept outside the repository (never committed): every .ifc that
# the index of MITCAD_IFC_CORPUS lists (index.tsv: path, sha256, ...) is
# imported with mitcad-cli import-ifc (mitcad#273). Each must import, with
# a line in the report for every product of the file and none whose shape
# failed to build. A file with a faceted twin (NAME.ifc and NAME.facet.ifc)
# is compared with it by the bodies' totals: within 1e-3 relative by
# volume where the file's shapes are swept solids (the twin's facets
# approximate them; `coarse_facets` below lists the twins too coarse for
# that), within 1e-2 by area for breps and surfaces (printed). Files
# are named by the first 8 hex digits of their SHA-256. Prints a line per
# file; skipped without the corpus.
#
# cmake -DCLI=<mitcad-cli> -DWORK=<folder> -P ifc-corpus.cmake
cmake_minimum_required(VERSION 3.24)

# The limit of swept solids against their faceted twins (parts per million).
set(swept_limit 1000)
# Swept-solid files whose faceted twin is itself a coarse approximation:
# its chords cut inside the curved profile or path, so that the twin's
# volume is short of the exact solid by more than the limit above. Each is
# compared within 1e-2 instead, and its exact volume is held by other
# checks (mitcad#273):
# - 1bdaa6cb (a profile swept along a B-spline path): 1380369.6 mm3; the
#   same solid by Pappus's theorem from the profile's area and the path's
#   centroid 1380370.0 (3e-7); the twin 1368954.1 (8.3e-3 short).
# - c83544bc (a B-spline profile in a CSG solid): within 4.5e-5 of the
#   file's own NetVolume (the report's quantity check); the twin 1.2e-3
#   short.
set(coarse_facets 1bdaa6cb c83544bc)

foreach(var CLI WORK)
  if(NOT DEFINED ${var})
    message(FATAL_ERROR "ifc-corpus.cmake needs -D${var}=...")
  endif()
endforeach()

set(corpus "$ENV{MITCAD_IFC_CORPUS}")
if(corpus STREQUAL "" OR NOT EXISTS "${corpus}/index.tsv")
  message("ifc corpus: skipped (MITCAD_IFC_CORPUS is not set or has no index.tsv)")
  return()
endif()
file(MAKE_DIRECTORY "${WORK}")

# The index's columns.
# Semicolons (in the notes) would split a row as CMake lists do.
file(READ "${corpus}/index.tsv" index)
string(REPLACE ";" "," index "${index}")
string(REGEX MATCHALL "[^\r\n]+" rows "${index}")
list(POP_FRONT rows header)
string(REPLACE "\t" ";" header "${header}")
list(FIND header path path_column)
list(FIND header sha256 sha_column)
if(path_column LESS 0 OR sha_column LESS 0)
  message(FATAL_ERROR "index.tsv has no path and sha256 columns")
endif()

set(failures "")
set(files 0)
set(products_total 0)
foreach(row IN LISTS rows)
  if(row STREQUAL "")
    continue()
  endif()
  # Paths hold no tabs or semicolons; spaces stay.
  string(REPLACE "\t" ";" fields "${row}")
  list(GET fields ${path_column} path)
  list(GET fields ${sha_column} sha)
  if(NOT path MATCHES "\\.ifc$")
    continue()
  endif()
  string(SUBSTRING "${sha}" 0 8 id)
  set(report "${WORK}/${id}.json")
  file(REMOVE "${report}")
  execute_process(
    COMMAND "${CLI}" import-ifc "${corpus}/${path}" --report "${report}"
    RESULT_VARIABLE status
    OUTPUT_FILE "${WORK}/${id}.txt"
    ERROR_VARIABLE errors
    TIMEOUT 1800)
  math(EXPR files "${files} + 1")
  if(NOT status EQUAL 0 OR NOT EXISTS "${report}")
    string(REGEX REPLACE "\n.*" "" first_error "${errors}")
    list(APPEND failures "${id}: the import failed (${status}): ${first_error}")
    message("${id} FAILED (${status})")
    continue()
  endif()
  file(READ "${report}" json)
  string(JSON products LENGTH "${json}" products)
  string(JSON schema GET "${json}" schema)
  string(JSON volume GET "${json}" total_volume)
  string(JSON area GET "${json}" total_area)
  string(JSON outcomes GET "${json}" outcomes)
  string(JSON count LENGTH "${outcomes}")
  set(summary "")
  set(failed 0)
  set(partial 0)
  if(count GREATER 0)
    math(EXPR last "${count} - 1")
    foreach(i RANGE ${last})
      string(JSON key MEMBER "${outcomes}" ${i})
      string(JSON n GET "${outcomes}" "${key}")
      string(APPEND summary " ${n} ${key}")
      if(key STREQUAL "failed")
        set(failed ${n})
      elseif(key STREQUAL "partial")
        set(partial ${n})
      endif()
    endforeach()
  endif()
  string(JSON agree GET "${json}" quantities agree)
  string(JSON compared GET "${json}" quantities compared)
  math(EXPR products_total "${products_total} + ${products}")
  message("${id} ${schema} ${products} products:${summary}; volume ${volume} mm3; quantities ${agree} of ${compared}")
  if(failed GREATER 0 OR partial GREATER 0)
    # The products that did not build, with why.
    file(STRINGS "${WORK}/${id}.txt" lines REGEX "(: failed|: partial|not built:)")
    list(LENGTH lines n)
    if(n GREATER 12)
      list(SUBLIST lines 0 12 lines)
    endif()
    string(REPLACE ";" "\n    " lines "${lines}")
    list(APPEND failures "${id}: ${failed} failed, ${partial} partial\n    ${lines}")
  endif()
  set(volume_${id} "${volume}")
  set(area_${id} "${area}")
  set(all_parametric_${id} TRUE)
  if(summary MATCHES " (stored|mesh|mixed)")
    set(all_parametric_${id} FALSE)
  endif()
  set(id_of_${path} "${id}")
endforeach()

# relative_ppm(a b out): |a - b| / b in parts per million, from the
# numbers' integer parts (mm³ and mm²: plenty of digits; CMake has no
# floating point); empty when b has none.
function(relative_ppm a b out)
  string(REGEX REPLACE "\\..*" "" ia "${a}")
  string(REGEX REPLACE "\\..*" "" ib "${b}")
  if(ia STREQUAL "" OR ia STREQUAL "-" OR ib STREQUAL "" OR ib STREQUAL "-" OR ib EQUAL 0)
    set(${out} "" PARENT_SCOPE)
    return()
  endif()
  math(EXPR diff "${ia} - ${ib}")
  if(diff LESS 0)
    math(EXPR diff "0 - ${diff}")
  endif()
  # Scaled down first, so that the product stays within 64 bits.
  while(ib GREATER 1000000000)
    math(EXPR diff "${diff} / 10")
    math(EXPR ib "${ib} / 10")
  endwhile()
  math(EXPR ppm "${diff} * 1000000 / ${ib}")
  set(${out} "${ppm}" PARENT_SCOPE)
endfunction()

# Faceted twins: NAME.ifc against NAME.facet.ifc: swept solids by volume,
# breps and surfaces by area.
foreach(row IN LISTS rows)
  string(REPLACE "\t" ";" fields "${row}")
  list(GET fields ${path_column} path)
  if(NOT path MATCHES "\\.facet\\.ifc$")
    continue()
  endif()
  string(REGEX REPLACE "\\.facet\\.ifc$" ".ifc" base "${path}")
  if(NOT DEFINED id_of_${base} OR NOT DEFINED id_of_${path})
    continue()
  endif()
  set(a "${id_of_${base}}")
  set(b "${id_of_${path}}")
  if(NOT DEFINED volume_${a} OR NOT DEFINED volume_${b})
    continue()
  endif()
  relative_ppm("${volume_${a}}" "${volume_${b}}" ppm)
  set(measure volume)
  set(va "${volume_${a}}")
  set(vb "${volume_${b}}")
  string(REGEX REPLACE "\\..*" "" ia "${va}")
  # Breps by area: a coarse tessellation of a curved surface falls short
  # of its area by less than of the volume it bounds.
  if(ia STREQUAL "" OR ia STREQUAL "-" OR ia EQUAL 0 OR NOT all_parametric_${a})
    relative_ppm("${area_${a}}" "${area_${b}}" ppm)
    set(measure area)
    set(va "${area_${a}}")
    set(vb "${area_${b}}")
  endif()
  if(ppm STREQUAL "")
    list(APPEND failures "${a}: no ${measure} to compare with its facets ${b}")
    continue()
  endif()
  if(all_parametric_${a} AND a IN_LIST coarse_facets)
    set(limit 10000)
    set(kind "swept solids, coarse facets")
  elseif(all_parametric_${a})
    set(limit ${swept_limit})
    set(kind "swept solids")
  else()
    set(limit 10000)
    set(kind "breps")
  endif()
  message("twin ${a} (${kind}) against its facets ${b}: ${measure} ${va} against ${vb}, ${ppm} ppm (limit ${limit})")
  if(ppm GREATER ${limit})
    list(APPEND failures "${a}: ${ppm} ppm from its facets ${b} by ${measure} (limit ${limit} for ${kind})")
  endif()
endforeach()

message("${files} files, ${products_total} products")
if(failures)
  string(REPLACE ";" "\n  " failures "${failures}")
  message(FATAL_ERROR "ifc corpus:\n  ${failures}")
endif()
