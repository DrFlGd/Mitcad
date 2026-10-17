# SPDX-License-Identifier: MIT
# The UTF-8 code page for Mitcad's Windows executables. Without it a
# program's narrow strings (main's argv, getenv, std::filesystem::path and
# the file streams made from a std::string) are in the system's ANSI code
# page, and a path with a letter outside it, or even inside it (a-umlaut,
# the degree sign), reached the bridge as bytes that are not UTF-8: "data
# for rust::Str is not utf-8". The manifest makes the process's ANSI code
# page UTF-8 (Windows 10 1903 and later), so a path stays UTF-8 from the
# command line to the Rust model, to OCCT (which takes narrow file names
# as UTF-8) and to the C and C++ runtimes. Qt's own strings are wide and
# unaffected. Elsewhere narrow strings are UTF-8 already and it does nothing.
set(MITCAD_UTF8_CODE_PAGE_MANIFEST "${CMAKE_CURRENT_LIST_DIR}/utf8-code-page.manifest")

function(mitcad_utf8_code_page target)
  if(WIN32)
    target_sources(${target} PRIVATE "${MITCAD_UTF8_CODE_PAGE_MANIFEST}")
  endif()
endfunction()
