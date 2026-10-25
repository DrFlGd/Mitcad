# SPDX-License-Identifier: MIT
# The same visible Qt selection-card assertions on every desktop platform.
# Linux needs its own hidden X server; Windows needs the configured Mesa VM.
if(BUILD_TESTING AND TARGET mitcad AND TARGET mitcad-cli)
  find_package(Python3 COMPONENTS Interpreter QUIET)
  set(selection_ui_platform "")
  set(selection_ui_prefix "")
  if(APPLE AND MITCAD_MACOS_UI_TESTS)
    set(selection_ui_platform macos)
  elseif(WIN32 AND MITCAD_MESA_DIR)
    set(selection_ui_platform windows)
  elseif(UNIX AND NOT APPLE)
    find_program(MITCAD_XVFB_RUN_EXECUTABLE xvfb-run)
    if(MITCAD_XVFB_RUN_EXECUTABLE)
      set(selection_ui_platform linux)
      set(selection_ui_prefix "${MITCAD_XVFB_RUN_EXECUTABLE}" -a -s "-screen 0 1600x1000x24")
    endif()
  endif()
  if(Python3_Interpreter_FOUND AND selection_ui_platform)
    set(selection_ui_test "app.ui-${selection_ui_platform}-selection-owners")
    add_test(NAME "${selection_ui_test}"
      COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
              "${CMAKE_SOURCE_DIR}/tools/ui-selection-owners-test.py"
              "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>"
              "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-selection-owners"
              "$<TARGET_FILE_DIR:Qt6::Core>")
    set_tests_properties("${selection_ui_test}" PROPERTIES TIMEOUT 240 RUN_SERIAL TRUE
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
  add_test(NAME "${formula_ui_test}"
    COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
            "${CMAKE_SOURCE_DIR}/tools/ui-formula-builder-test.py"
            "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>"
            "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-formula-builder"
            "$<TARGET_FILE_DIR:Qt6::Core>")
  set_tests_properties("${formula_ui_test}" PROPERTIES TIMEOUT 420 RUN_SERIAL TRUE
    ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
  if(selection_ui_platform STREQUAL "linux")
    set_property(TEST "${formula_ui_test}" APPEND PROPERTY ENVIRONMENT "LIBGL_ALWAYS_SOFTWARE=1")
  endif()
endif()

# Assistant setup uses private config homes and a compiled stub on each platform.
if(BUILD_TESTING AND TARGET mitcad AND TARGET mitcad-cli AND TARGET test_assistant_client
   AND Python3_Interpreter_FOUND AND selection_ui_platform)
  set(assistant_ui_test "app.ui-${selection_ui_platform}-assistant-setup")
  add_test(NAME "${assistant_ui_test}"
    COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
            "${CMAKE_SOURCE_DIR}/tools/ui-assistant-setup-test.py"
            "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>" "$<TARGET_FILE:test_assistant_client>"
            "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-assistant-setup"
            "$<TARGET_FILE_DIR:Qt6::Core>")
  set_tests_properties("${assistant_ui_test}" PROPERTIES TIMEOUT 600 RUN_SERIAL TRUE
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
  add_test(NAME "${timeline_ui_test}"
    COMMAND ${selection_ui_prefix} "${Python3_EXECUTABLE}"
            "${CMAKE_SOURCE_DIR}/tools/ui-timeline-inspection-test.py"
            "$<TARGET_FILE:mitcad>" "$<TARGET_FILE:mitcad-cli>"
            "${CMAKE_BINARY_DIR}/ui-${selection_ui_platform}-timeline-inspection"
            "$<TARGET_FILE_DIR:Qt6::Core>")
  set_tests_properties("${timeline_ui_test}" PROPERTIES TIMEOUT 360 RUN_SERIAL TRUE
    ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
  if(selection_ui_platform STREQUAL "linux")
    set_property(TEST "${timeline_ui_test}" APPEND PROPERTY ENVIRONMENT "LIBGL_ALWAYS_SOFTWARE=1")
  endif()
endif()
