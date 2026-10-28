# SPDX-License-Identifier: MIT
# The same visible Qt selection-card assertions on every desktop platform.
# Linux needs its own hidden X server; Windows needs the configured Mesa VM.
if(BUILD_TESTING AND TARGET mitcad AND TARGET mitcad-cli)
  find_package(Python3 COMPONENTS Interpreter QUIET)
  set(selection_ui_platform "")
  # On macOS and Windows these tests run alone (RUN_SERIAL): one Aqua
  # session, and the Windows VM's session 0. On Linux each has a hidden
  # X server of its own (xvfb-run) and its own settings, autosave, project
  # and discovery folders (and MCP's port is the system's choice), so they
  # run beside the other tests; the test runner holds them to the
  # machine's UI entries at once and reruns one that failed beside others
  # alone (tools/testrun/README.md, mitcad#832).
  set(selection_ui_alone RUN_SERIAL TRUE)
  if(APPLE AND MITCAD_MACOS_UI_TESTS)
    set(selection_ui_platform macos)
  elseif(WIN32 AND MITCAD_MESA_DIR)
    set(selection_ui_platform windows)
  elseif(UNIX AND NOT APPLE)
    find_program(MITCAD_XVFB_RUN_EXECUTABLE xvfb-run)
    if(MITCAD_XVFB_RUN_EXECUTABLE)
      set(selection_ui_platform linux)
      set(selection_ui_alone "")
    endif()
  endif()
  # selection_ui_command_prefix(<display>): selection_ui_prefix, the
  # command's prefix; on Linux xvfb-run looking for a free display from a
  # number of the test's own. xvfb-run -a
  # takes the first number without a lock file and only then starts the
  # server, so tests starting at the same moment from one number could
  # pick the same one; the shell UI tests use 90-189 (tools/ui-test-lib.sh).
  function(selection_ui_command_prefix display)
    if(selection_ui_platform STREQUAL "linux")
      set(selection_ui_prefix "${MITCAD_XVFB_RUN_EXECUTABLE}" -a -n ${display} -s "-screen 0 1600x1000x24"
          PARENT_SCOPE)
    else()
      set(selection_ui_prefix "" PARENT_SCOPE)
    endif()
  endfunction()
  if(Python3_Interpreter_FOUND AND selection_ui_platform)
    set(selection_ui_test "app.ui-${selection_ui_platform}-selection-owners")
    selection_ui_command_prefix(200)
    add_test(NAME "${selection_ui_test}"
      COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
              "${CMAKE_SOURCE_DIR}/tools/ui-selection-owners-test.py"
              "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>"
              "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-selection-owners"
              "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties("${selection_ui_test}" PROPERTIES TIMEOUT 240 ${selection_ui_alone}
      ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
    if(selection_ui_platform STREQUAL "linux")
      set_property(TEST "${selection_ui_test}" APPEND PROPERTY ENVIRONMENT "LIBGL_ALWAYS_SOFTWARE=1")
    endif()
  endif()
endif()

# The same text-based formula editor and limits acceptance on every platform.
if(BUILD_TESTING AND TARGET mitcad AND TARGET mitcad-cli AND Python3_Interpreter_FOUND
   AND selection_ui_platform)
  set(formula_ui_test "app.ui-${selection_ui_platform}-formula-builder")
  selection_ui_command_prefix(220)
  add_test(NAME "${formula_ui_test}"
    COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
            "${CMAKE_SOURCE_DIR}/tools/ui-formula-builder-test.py"
            "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>"
            "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-formula-builder"
            "$<TARGET_FILE_DIR:Qt6::Core>")
  set_tests_properties("${formula_ui_test}" PROPERTIES TIMEOUT 420 ${selection_ui_alone}
    ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
  if(selection_ui_platform STREQUAL "linux")
    set_property(TEST "${formula_ui_test}" APPEND PROPERTY ENVIRONMENT "LIBGL_ALWAYS_SOFTWARE=1")
  endif()
endif()

# Assistant setup uses private config homes and a compiled stub on each platform.
if(BUILD_TESTING AND TARGET mitcad AND TARGET mitcad-cli AND TARGET test_assistant_client
   AND Python3_Interpreter_FOUND AND selection_ui_platform)
  set(assistant_ui_test "app.ui-${selection_ui_platform}-assistant-setup")
  selection_ui_command_prefix(240)
  add_test(NAME "${assistant_ui_test}"
    COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
            "${CMAKE_SOURCE_DIR}/tools/ui-assistant-setup-test.py"
            "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>" "$<TARGET_FILE:test_assistant_client>"
            "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-assistant-setup"
            "$<TARGET_FILE_DIR:Qt6::Core>")
  set_tests_properties("${assistant_ui_test}" PROPERTIES TIMEOUT 600 ${selection_ui_alone}
    ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
  if(selection_ui_platform STREQUAL "linux")
    set_property(TEST "${assistant_ui_test}" APPEND PROPERTY ENVIRONMENT "LIBGL_ALWAYS_SOFTWARE=1")
  endif()
endif()

# Portable timeline inspection also runs on Linux and the Windows Mesa VM.
# macOS uses its platform wrapper in MacUiTests.cmake.
if(BUILD_TESTING AND TARGET mitcad AND TARGET mitcad-cli AND Python3_Interpreter_FOUND
   AND selection_ui_platform AND NOT APPLE)
  set(timeline_ui_test "app.ui-${selection_ui_platform}-timeline-inspection")
  selection_ui_command_prefix(260)
  add_test(NAME "${timeline_ui_test}"
    COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
            "${CMAKE_SOURCE_DIR}/tools/ui-timeline-inspection-test.py"
            "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>"
            "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-timeline-inspection"
            "$<TARGET_FILE_DIR:Qt6::Core>")
  set_tests_properties("${timeline_ui_test}" PROPERTIES TIMEOUT 360 ${selection_ui_alone}
    ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
  if(selection_ui_platform STREQUAL "linux")
    set_property(TEST "${timeline_ui_test}" APPEND PROPERTY ENVIRONMENT "LIBGL_ALWAYS_SOFTWARE=1")
  endif()
endif()
