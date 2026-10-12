# SPDX-License-Identifier: MIT
# Windows development builds only: copies Mesa's software OpenGL (llvmpipe)
# from MITCAD_MESA_DIR next to mitcad.exe. Windows loads opengl32.dll from
# the application's directory first, so Qt and OCCT then render with Mesa,
# e.g. in a VM without a GPU that offers only OpenGL 1.1. The DLLs come from
# tools/dev-env/setup-mesa-windows.ps1. Off by default; never in a release.
set(MITCAD_MESA_DIR "" CACHE PATH
  "Windows development builds: copy Mesa's opengl32.dll and libgallium_wgl.dll from this directory next to mitcad.exe")

set(mesa_runtime_dlls opengl32.dll libgallium_wgl.dll)

if(MITCAD_MESA_DIR)
  if(NOT WIN32)
    message(FATAL_ERROR "MITCAD_MESA_DIR is only for Windows builds")
  endif()
  set(mesa_runtime_files "")
  foreach(dll IN LISTS mesa_runtime_dlls)
    if(NOT EXISTS "${MITCAD_MESA_DIR}/${dll}")
      message(FATAL_ERROR "MITCAD_MESA_DIR: ${MITCAD_MESA_DIR}/${dll} not found; "
                          "build it with tools/dev-env/setup-mesa-windows.ps1")
    endif()
    list(APPEND mesa_runtime_files "${MITCAD_MESA_DIR}/${dll}")
  endforeach()
  message(STATUS "Mesa software OpenGL from ${MITCAD_MESA_DIR} is copied next to mitcad.exe")
  # A target of its own: add_custom_command(TARGET mitcad) only works in
  # app/CMakeLists.txt.
  add_custom_target(mitcad_mesa_runtime ALL
    COMMAND "${CMAKE_COMMAND}" -E copy_if_different ${mesa_runtime_files} "$<TARGET_FILE_DIR:mitcad>"
    VERBATIM)
  add_dependencies(mitcad_mesa_runtime mitcad)
  if(BUILD_TESTING)
    # With a renderer the app can run in ctest: screenshots of a demo block
    # and of a project file opened from the command line.
    add_test(NAME app.ui-windows
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    # Input-driven: sketch, extrude, fillet, save, open, STEP export and
    # import with mouse and keyboard messages posted to the window.
    add_test(NAME app.ui-windows-workflow
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-workflow-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-workflow"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-workflow PROPERTIES TIMEOUT 300)
    # An .f3d design of components of other documents (mitcad#196): the
    # file dialog for each document not found, skipped or chosen.
    add_test(NAME app.ui-windows-references
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-references-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-references"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-references PROPERTIES TIMEOUT 300)
    # Sheet metal (mitcad#210): Face, Flange, rule edits, Flat Pattern, its
    # DXF export, an edit from the timeline, the Floating layout.
    add_test(NAME app.ui-windows-sheet
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-sheet-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-sheet"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-sheet PROPERTIES TIMEOUT 300)
    # Sheet-metal operations (mitcad#125): Fold, Bend, Corner Round and
    # Chamfer, Corner Relief, Sheet Cut, Unfold and Refold, each with its
    # picks, preview, a timeline edit and Undo.
    add_test(NAME app.ui-windows-sheet-ops
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-sheet-ops-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-sheet-ops"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-sheet-ops PROPERTIES TIMEOUT 600)
  endif()
elseif(WIN32 AND CMAKE_RUNTIME_OUTPUT_DIRECTORY)
  # Turned off again: remove earlier copies, or they would stay in use.
  foreach(dll IN LISTS mesa_runtime_dlls)
    file(REMOVE "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/${dll}")
  endforeach()
endif()
