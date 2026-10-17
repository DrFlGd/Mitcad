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
    # Design policy (mitcad#220): the new design's Part1, a root that
    # refuses sketches, the policy dialog and its presets, undo, and
    # sketch planes of other components left out of picking.
    add_test(NAME app.ui-windows-policy
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-policy-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-policy"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-policy PROPERTIES TIMEOUT 300)
    # Clean Up Projections (mitcad#232): a face projected twice into a
    # sketch on it, the repeats listed by the preview and removed; a
    # coincident that replaces a Horizontal and a refused snap (mitcad#287).
    add_test(NAME app.ui-windows-projections
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-projections-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-projections"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-projections PROPERTIES TIMEOUT 420)
    # Picks on clipped bodies (mitcad#233): Project with Hide Above Sketch
    # on, Fillet under Section Analysis, by the bodies' own names.
    add_test(NAME app.ui-windows-section-pick
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-section-pick-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-section-pick"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-section-pick PROPERTIES TIMEOUT 300)
    # Dimension names (mitcad#231): the tooltip, the name in the value
    # editor, a dimension picked into it, a rename, Dimension text, and a
    # feature panel's field with its parameter's name.
    add_test(NAME app.ui-windows-dimension-names
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-dimension-names-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-dimension-names"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-dimension-names PROPERTIES TIMEOUT 300)
    # Sketch mode's view: fully constrained lines recoloured when the
    # background turns dark or light (mitcad#253), the sketch origin's place
    # and the pointer's sketch coordinates (mitcad#254).
    add_test(NAME app.ui-windows-sketch-view
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-sketch-view-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-sketch-view"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-sketch-view PROPERTIES TIMEOUT 300)
    # Extrude's preview in components placed off the origin (mitcad#266):
    # hidden bodies and occurrences stay hidden, shown and new bodies are
    # where their occurrences put them, a linked component's too.
    add_test(NAME app.ui-windows-preview-placement
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-preview-placement-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-preview-placement"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-preview-placement PROPERTIES TIMEOUT 300)

    # Live MCP for debugging (mitcad#265): the actual application and
    # mitcad-cli.exe mcp --connect; queries and sketch commands with a
    # sketch open, ui_state, ui_select, ui_screenshot, ui_command, Save,
    # ui_log.
    add_test(NAME app.ui-windows-mcp
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-mcp-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-mcp"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-mcp PROPERTIES TIMEOUT 300)
    # The debug log (mitcad#269): --debug-log, its header and lines,
    # Preferences' switch on and off without a restart, Open Log Folder;
    # Create Sketch by its default Alt+S (mitcad#267).
    add_test(NAME app.ui-windows-debug-log
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-debug-log-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-debug-log"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-debug-log PROPERTIES TIMEOUT 300)

    # Body edges and vertices in the sketch plane (mitcad#256, mitcad#258):
    # Sketch Dimension from a block edge in one undo step and its projection
    # reused; the Point tool on a corner and at an edge's middle, the points
    # following the block.
    add_test(NAME app.ui-windows-body-picks
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-body-picks-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-body-picks"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-body-picks PROPERTIES TIMEOUT 300)

    # Light bulbs in every mode (mitcad#295): a body hidden while Create
    # Sketch asks for a plane lets the face behind it be picked; during
    # Extrude a body hidden stays so on Cancel and is no part of OK's undo
    # step; an edit's roll-back keeps it too. Create Sketch's origin planes
    # only without components, else from the Origin folder (mitcad#294).
    add_test(NAME app.ui-windows-visibility
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-visibility-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-visibility"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-visibility PROPERTIES TIMEOUT 300)

    # Extrude's automatic operation (mitcad#289): Join on a body's face,
    # Cut into it and Join out again, New Body away from bodies, a hand
    # picked operation that stays, and an edit that keeps its operation.
    add_test(NAME app.ui-windows-auto-operation
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-auto-operation-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-auto-operation"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-auto-operation PROPERTIES TIMEOUT 300)

    # The browser's scrolling with a selection (mitcad#318): a pick in the
    # view scrolls to its row out of sight; a Ctrl+click on a row below the
    # first selected one, scrolled away, keeps the scroll position.
    add_test(NAME app.ui-windows-browser
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-browser-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-browser"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-browser PROPERTIES TIMEOUT 300)

    # The library browser's Update (mitcad#317): empty until Update, which
    # fetches every enabled source (a local git repository) and lists its
    # parts; a broken source reported in one warning, the selection kept.
    add_test(NAME app.ui-windows-library
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-library-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-library"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-library PROPERTIES TIMEOUT 300)

    # The rendered view from below the ground (mitcad#320): orbited under
    # the ground and back, orthographic and perspective, the box rendered
    # in every view, the ground left out below and built again above.
    add_test(NAME app.ui-windows-render-ground
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-render-ground-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-render-ground"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-render-ground PROPERTIES TIMEOUT 600)

    # The BUILD tab (mitcad#375): a library of synthetic parts fetched from
    # a local git repository; a frame of four profiles (a mitred end, a
    # bound length), a connector, a panel, a rail with a carriage; saved,
    # reopened, the cut list's CSV and PDF.
    add_test(NAME app.ui-windows-build
      COMMAND powershell -NoProfile -ExecutionPolicy Bypass
              -File "${CMAKE_SOURCE_DIR}/tools/ui-windows-build-test.ps1"
              -App "$<TARGET_FILE:mitcad>" -Out "${CMAKE_BINARY_DIR}/ui-windows-build"
              -QtBin "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties(app.ui-windows-build PROPERTIES TIMEOUT 600)
  endif()
elseif(WIN32 AND CMAKE_RUNTIME_OUTPUT_DIRECTORY)
  # Turned off again: remove earlier copies, or they would stay in use.
  foreach(dll IN LISTS mesa_runtime_dlls)
    file(REMOVE "${CMAKE_RUNTIME_OUTPUT_DIRECTORY}/${dll}")
  endforeach()
endif()
