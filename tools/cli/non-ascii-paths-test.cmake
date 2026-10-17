# SPDX-License-Identifier: MIT
# Paths with letters outside ASCII (cmake/Utf8CodePage.cmake): on Windows a
# STEP file named with an a-umlaut or a degree sign failed with "data for
# rust::Str is not utf-8". A design is saved, exported to STEP, imported
# and opened again, in a folder and under names that have U+00E4 and
# U+00B0, and the folder must then hold exactly those names. The names are
# made from their UTF-8 bytes, as the sources are ASCII.
#
# cmake -DCLI=<mitcad-cli> -DSCRIPT=<script.json> -DDIR=<work folder>
#       -P non-ascii-paths-test.cmake

foreach(var CLI SCRIPT DIR)
  if(NOT DEFINED ${var})
    message(FATAL_ERROR "non-ascii-paths-test.cmake needs -D${var}=...")
  endif()
endforeach()

string(ASCII 195 164 a_umlaut)
string(ASCII 194 176 degree)
set(folder "${DIR}/Schr${a_umlaut}g 45${degree}")
set(design_name "Schr${a_umlaut}gkugellager_10x30x9.mitcad")
set(step_name "Schr${a_umlaut}gkugellager_10x30x9_45${degree}.stp")
set(imported_name "Winkel 45${degree} ${a_umlaut}.mitcad")
file(REMOVE_RECURSE "${DIR}")
file(MAKE_DIRECTORY "${folder}")

# Runs mitcad-cli with the arguments after `pattern`, which its output
# must match; it must succeed.
function(cli pattern)
  execute_process(COMMAND "${CLI}" ${ARGN} RESULT_VARIABLE result OUTPUT_VARIABLE out ERROR_VARIABLE err)
  if(NOT result EQUAL 0)
    message(FATAL_ERROR "mitcad-cli ${ARGN}: exit status '${result}':\n${out}${err}")
  endif()
  if(NOT "${out}${err}" MATCHES "${pattern}")
    message(FATAL_ERROR "mitcad-cli ${ARGN}: no '${pattern}' in:\n${out}${err}")
  endif()
endfunction()

cli("volume" run "${SCRIPT}" --save "${folder}/${design_name}")
cli("\"format\":\"step\"" export "${folder}/${design_name}" "${folder}/${step_name}")
cli("volume" import "${folder}/${step_name}" --save "${folder}/${imported_name}")
cli("volume" info "${folder}/${imported_name}")

# The files have the names given, not their bytes in another code page.
file(GLOB names RELATIVE "${folder}" "${folder}/*")
list(SORT names)
set(expected "${design_name}" "${step_name}" "${imported_name}")
list(SORT expected)
if(NOT "${names}" STREQUAL "${expected}")
  message(FATAL_ERROR "the folder holds '${names}', '${expected}' expected")
endif()
message(STATUS "saved, exported, imported and opened under non-ASCII names")
