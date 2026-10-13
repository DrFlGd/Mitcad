# SPDX-License-Identifier: MIT
# Designs saved by earlier versions, kept outside the repository (never
# committed): every .mitcad under the folders of MITCAD_DESIGN_CORPUS (a
# path list, like PATH; each design in its project folder, with the
# project's .mitcad/brep store) is opened and recomputed with mitcad-cli
# info, and no sketch of it may fail. A linked projection of a source that
# changed shape since must keep its geometry rather than tear its fixed
# curves apart (mitcad#249). Prints a line per file; skipped without the
# corpus.
#
# cmake -DCLI=<mitcad-cli> -P designs-corpus.cmake

if(NOT DEFINED CLI)
  message(FATAL_ERROR "designs-corpus.cmake needs -DCLI=...")
endif()

set(corpus "$ENV{MITCAD_DESIGN_CORPUS}")
if(NOT CMAKE_HOST_WIN32)
  string(REPLACE ":" ";" corpus "${corpus}")
endif()
set(files)
foreach(dir IN LISTS corpus)
  if(IS_DIRECTORY "${dir}")
    file(GLOB_RECURSE found "${dir}/*.mitcad")
    # A project's own folder (.mitcad/) holds no designs.
    list(FILTER found EXCLUDE REGEX "/\\.mitcad/")
    list(APPEND files ${found})
  endif()
endforeach()
list(REMOVE_DUPLICATES files)
list(SORT files)
if(NOT files)
  message("design corpus: skipped (set MITCAD_DESIGN_CORPUS to folders of Mitcad projects)")
  return()
endif()

set(failed)
foreach(file IN LISTS files)
  get_filename_component(dir "${file}" DIRECTORY)
  execute_process(COMMAND "${CLI}" info "${file}"
    WORKING_DIRECTORY "${dir}"
    RESULT_VARIABLE code OUTPUT_VARIABLE out ERROR_VARIABLE err)
  string(REGEX MATCHALL "[^\n]*\\(sketch\\): error[^\n]*" broken "${out}")
  if(NOT code EQUAL 0)
    message("FAIL ${file}: mitcad-cli info exited with ${code}\n${err}")
    list(APPEND failed "${file}")
  elseif(broken)
    string(REPLACE ";" "\n" broken "${broken}")
    message("FAIL ${file}: sketches fail\n${broken}")
    list(APPEND failed "${file}")
  else()
    message("ok   ${file}")
  endif()
endforeach()
if(failed)
  list(LENGTH failed count)
  message(FATAL_ERROR "design corpus: ${count} file(s) failed")
endif()
