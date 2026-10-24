# SPDX-License-Identifier: MIT
if(APPLE AND BUILD_TESTING)
  add_executable(test_mac_command_payload
    ${PROJECT_SOURCE_DIR}/app/tests/mac_command_payload.cpp
    ${PROJECT_SOURCE_DIR}/app/render/MacCommandPayload.cpp)
  target_include_directories(test_mac_command_payload PRIVATE ${PROJECT_SOURCE_DIR}/app)
  target_link_libraries(test_mac_command_payload PRIVATE Qt6::Core mitcad_warnings)
  add_test(NAME app.command_payload_macos COMMAND test_mac_command_payload)
  set_tests_properties(app.command_payload_macos PROPERTIES TIMEOUT 30)
endif()
