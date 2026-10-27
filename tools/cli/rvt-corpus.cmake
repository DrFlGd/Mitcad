# SPDX-License-Identifier: MIT
# The .rvt import on the corpus (mitcad#274, mitcad#275), the files found
# under MITCAD_RVT_CORPUS (and MITCAD_IFC_CORPUS) by their content ids (the
# first 8 hex digits of their SHA-256, the corpus indexes'):
#
# - the MIT-licensed 2024 project (c805df44) imported with mitcad-cli
#   import-rvt against its IFC export (bfdf36ff): every building element
#   on its level's component, its curtain panels and mullions as features
#   and its other family elements as meshes of the
#   geometry the file keeps and no box (mitcad#333), and every wall, slab
#   and column of the
#   export agreed with by a feature (its volume within 1 %, its box within
#   5 mm, by GlobalId); without the IFC corpus, only the first part;
# - a 2024 structural sample project (aab838f8) next to the 2024
#   architectural sample (bb62c161) under the name it links it by: the
#   link found and its elements imported into its component.
#
# Skipped without the corpus.
#
#   cmake -DCLI=<mitcad-cli> -DWORK=<dir> -P rvt-corpus.cmake

set(id c805df44)
if(NOT DEFINED ENV{MITCAD_RVT_CORPUS} OR NOT EXISTS "$ENV{MITCAD_RVT_CORPUS}/index.tsv")
  message("rvt corpus: skipped (MITCAD_RVT_CORPUS not set)")
  return()
endif()
set(corpus "$ENV{MITCAD_RVT_CORPUS}")

# The path of a corpus file by its content id ("" when not there).
function(corpus_file dir cid out)
  set(${out} "" PARENT_SCOPE)
  if(NOT EXISTS "${dir}/index.tsv")
    return()
  endif()
  file(READ "${dir}/index.tsv" index)
  # The row whose second column (the SHA-256) starts with the id: its path.
  if(index MATCHES "(^|\n)([^\t\n]+)\t${cid}[0-9a-f]*\t" AND EXISTS "${dir}/${CMAKE_MATCH_2}")
    set(${out} "${dir}/${CMAKE_MATCH_2}" PARENT_SCOPE)
  endif()
endfunction()

corpus_file("${corpus}" ${id} project)
if(project STREQUAL "")
  message("rvt corpus: skipped (no file ${id} in the corpus)")
  return()
endif()
set(export "")
if(DEFINED ENV{MITCAD_IFC_CORPUS})
  corpus_file("$ENV{MITCAD_IFC_CORPUS}" bfdf36ff export)
endif()
file(MAKE_DIRECTORY "${WORK}")
set(reference_args "")
if(NOT export STREQUAL "")
  set(reference_args --reference-ifc "${export}")
endif()
execute_process(
  COMMAND "${CLI}" import-rvt "${project}" --report "${WORK}/${id}.json" --save "${WORK}/${id}.mitcad" ${reference_args}
  OUTPUT_VARIABLE out ERROR_VARIABLE err RESULT_VARIABLE status)
message("${out}")
if(NOT status EQUAL 0)
  message(FATAL_ERROR "${id}: import-rvt failed (${status}): ${err}")
endif()
file(READ "${WORK}/${id}.json" report)
string(JSON features GET "${report}" import features)
string(JSON bodies GET "${report}" import bodies)
string(JSON not_built LENGTH "${report}" import not_built)
string(JSON levels LENGTH "${report}" import levels)
string(JSON memory GET "${report}" import memory_low)
string(JSON meshes GET "${report}" import geometry_meshes)
string(JSON panels GET "${report}" import built panel)
string(JSON mullions GET "${report}" import built mullion)
string(JSON boxes GET "${report}" import boxes)
string(JSON oriented GET "${report}" import oriented_boxes)
# The curtain panels and mullions as features, the doors, windows and
# furniture as meshes of the geometry the file keeps, no box left
# (mitcad#333).
if(panels LESS 5000 OR mullions LESS 4000 OR meshes LESS 200 OR NOT boxes EQUAL 0 OR NOT oriented EQUAL 0)
  message(FATAL_ERROR "${id}: ${panels} panels and ${mullions} mullions as features, ${meshes} meshes of its geometry, ${boxes} and ${oriented} boxes")
endif()
# The project's 854 building elements the reference reader lists (its 970
# elements but 116 rooms, which are spaces) and its other elements of
# building categories: every one placed, as features or a body
# (core/import-rvt/tests/rvt_corpus.rs checks them one by one).
math(EXPR placed_total "${features} + ${bodies}")
if(placed_total LESS 854 OR NOT not_built EQUAL 0 OR NOT memory EQUAL 0)
  message(FATAL_ERROR "${id}: ${features} features and ${bodies} bodies, ${not_built} not built, ${memory} left out for memory")
endif()
set(placed 0)
math(EXPR last "${levels} - 1")
foreach(i RANGE ${last})
  string(JSON n GET "${report}" import levels ${i} bodies)
  string(JSON f GET "${report}" import levels ${i} features)
  string(JSON component GET "${report}" import levels ${i} component)
  if(component STREQUAL "")
    message(FATAL_ERROR "${id}: level ${i} has no component")
  endif()
  math(EXPR placed "${placed} + ${n} + ${f}")
endforeach()
if(NOT placed EQUAL placed_total)
  message(FATAL_ERROR "${id}: ${placed} elements on level components of ${placed_total}")
endif()
string(JSON planes GET "${report}" import level_planes)
string(JSON axes GET "${report}" import grid_axes)
if(planes LESS 1 OR NOT axes EQUAL 10)
  message(FATAL_ERROR "${id}: ${planes} level planes and ${axes} grid axes (10 grids)")
endif()
if(NOT export STREQUAL "")
  # Every wall, slab and column of the export agreed with by a feature.
  string(JSON unmatched LENGTH "${report}" import reference_unmatched)
  string(JSON walls GET "${report}" import reference_agree IFCWALL)
  string(JSON slabs GET "${report}" import reference_agree IFCSLAB)
  string(JSON columns GET "${report}" import reference_agree IFCCOLUMN)
  if(NOT unmatched EQUAL 0 OR NOT walls EQUAL 360 OR NOT slabs EQUAL 80 OR NOT columns EQUAL 256)
    message(FATAL_ERROR "${id}: ${walls} walls, ${slabs} slabs, ${columns} columns of the export agreed with, ${unmatched} not")
  endif()
  message("rvt corpus: ${id}: ${walls} walls, ${slabs} slabs and ${columns} columns agree with the IFC export")
endif()
message("rvt corpus: ${id}: ${features} features and ${bodies} bodies on ${levels} levels")

# The link: the structural sample next to the architectural one it links.
corpus_file("${corpus}" aab838f8 host)
corpus_file("${corpus}" bb62c161 linked)
if(host STREQUAL "" OR linked STREQUAL "")
  message("rvt corpus: the link's files are not in the corpus: not checked")
  return()
endif()
set(links "${WORK}/links")
file(MAKE_DIRECTORY "${links}")
file(COPY_FILE "${host}" "${links}/host.rvt")
file(COPY_FILE "${linked}" "${links}/rac_basic_sample_project.rvt")
execute_process(
  COMMAND "${CLI}" import-rvt "${links}/host.rvt" --report "${links}/host.json"
  OUTPUT_VARIABLE out ERROR_VARIABLE err RESULT_VARIABLE status)
if(NOT status EQUAL 0)
  message(FATAL_ERROR "aab838f8: import-rvt failed (${status}): ${err}")
endif()
file(READ "${links}/host.json" report)
string(JSON found GET "${report}" import links 0 found)
string(JSON elements GET "${report}" import links 0 elements)
string(JSON component GET "${report}" import links 0 component)
if(NOT found STREQUAL "rac_basic_sample_project.rvt" OR elements LESS 400 OR component STREQUAL "")
  message(FATAL_ERROR "aab838f8: its link ${found}: ${elements} elements in ${component}")
endif()
message("rvt corpus: aab838f8: its link's ${elements} elements in its component ${component}")
