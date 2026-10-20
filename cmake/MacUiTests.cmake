# SPDX-License-Identifier: MIT
# macOS UI tests (ctest): the counterpart of the Windows tests registered in
# MesaRuntime.cmake. app.ui-macos runs the application with --demo and
# --open, saves its screenshots (the app renders into an offscreen
# framebuffer; nothing is clicked, so no Accessibility or Screen Recording
# permission is needed) and checks them with tools/ui-image-stats.py.
# The window still opens, so the test needs a window server: in a session
# without one (a daemon, a CI runner without a display) it exits with 77 and
# ctest reports it as skipped. Run it in the macOS VM, not on the host.
option(MITCAD_MACOS_UI_TESTS "macOS: register the UI test app.ui-macos (opens the app's window briefly)" ON)

if(APPLE AND BUILD_TESTING AND MITCAD_MACOS_UI_TESTS AND TARGET mitcad)
  find_package(Python3 COMPONENTS Interpreter QUIET)
  if(Python3_Interpreter_FOUND AND TARGET mitcad-cli)
    set(mitcad_mcp_test_ui qt)
    if(MITCAD_MACOS_NATIVE_UI)
      add_test(NAME app.ui-macos-cut-components
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-cut-components-test.sh"
                "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>" "${CMAKE_BINARY_DIR}/ui-macos-cut-components")
      set_tests_properties(app.ui-macos-cut-components PROPERTIES TIMEOUT 780 SKIP_RETURN_CODE 77 RUN_SERIAL TRUE)
      set(mitcad_mcp_test_ui native)
    endif()
    add_test(NAME app.ui-macos-mcp
      COMMAND "${Python3_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-mcp-test.py"
              "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>" "${CMAKE_BINARY_DIR}/ui-macos-mcp"
              "${mitcad_mcp_test_ui}")
    # With the live debugging flow (mitcad#265, tools/ui-mcp-live.py).
    set_tests_properties(app.ui-macos-mcp PROPERTIES TIMEOUT 360 SKIP_RETURN_CODE 77 RUN_SERIAL TRUE
      ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
  endif()
  find_program(BASH_EXECUTABLE bash)
  if(BASH_EXECUTABLE)
    if(TARGET mitcad-cli AND MITCAD_MACOS_NATIVE_UI)
      add_test(NAME app.ui-macos-section
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-section-test.sh"
                "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>" "${CMAKE_BINARY_DIR}/ui-macos-section")
      set_tests_properties(app.ui-macos-section PROPERTIES TIMEOUT 420 SKIP_RETURN_CODE 77 RUN_SERIAL TRUE)
    endif()
    # $<TARGET_FILE:mitcad> is .../mitcad.app/Contents/MacOS/mitcad for a
    # bundle.
    add_test(NAME app.ui-macos
      COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-test.sh"
              "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos")
    # Five runs of up to 120 s each at the most; 77: no window server.
    set_tests_properties(app.ui-macos PROPERTIES TIMEOUT 600 SKIP_RETURN_CODE 77)
    # The workflow (sketch, extrude, fillet, autosave and recovery, save, open,
    # export, import, version history) driven from inside the application by
    # tools/ui-scripts/*.mitcad-ui (app/framework/TestDriver.hpp): synthesised
    # Qt events, so no Accessibility permission; three runs of the application.
    add_test(NAME app.ui-macos-workflow
      COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-workflow-test.sh"
              "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-workflow")
    set_tests_properties(app.ui-macos app.ui-macos-workflow PROPERTIES
      TIMEOUT 600 SKIP_RETURN_CODE 77 RUN_SERIAL TRUE)
    if(MITCAD_MACOS_NATIVE_UI)
      add_test(NAME app.ui-macos-native
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native")
      add_test(NAME app.ui-macos-native-cancel
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-cancel" cancel)
      add_test(NAME app.ui-macos-native-secondary
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-secondary" secondary)
      add_test(NAME app.ui-macos-native-sketch
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-sketch" sketch)
      add_test(NAME app.ui-macos-native-panels
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-panels" panels)
      add_test(NAME app.ui-macos-native-gestures
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-gestures" gestures)
      add_test(NAME app.ui-macos-native-history
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-history" history)
      add_test(NAME app.ui-macos-native-context-menus
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-context-menus" context-menus)
      add_test(NAME app.ui-macos-native-replacement
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-replacement" replacement)
      add_test(NAME app.ui-macos-native-autoselect
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-autoselect" autoselect)
      add_test(NAME app.ui-macos-native-point-pattern
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-point-pattern" point-pattern)
      add_test(NAME app.ui-macos-native-toolbar
        COMMAND "${BASH_EXECUTABLE}" "${CMAKE_SOURCE_DIR}/tools/ui-macos-native-test.sh"
                "$<TARGET_FILE:mitcad>" "${CMAKE_BINARY_DIR}/ui-macos-native-toolbar" toolbar)
      set_tests_properties(app.ui-macos-native app.ui-macos-native-cancel app.ui-macos-native-secondary app.ui-macos-native-sketch app.ui-macos-native-panels app.ui-macos-native-gestures app.ui-macos-native-history app.ui-macos-native-context-menus app.ui-macos-native-replacement app.ui-macos-native-autoselect app.ui-macos-native-point-pattern app.ui-macos-native-toolbar
        PROPERTIES TIMEOUT 300 SKIP_RETURN_CODE 77 RUN_SERIAL TRUE)
    endif()
  endif()
endif()
