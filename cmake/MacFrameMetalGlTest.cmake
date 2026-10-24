# SPDX-License-Identifier: MIT
# Included from app/CMakeLists.txt after renderer dependencies are configured.
if(NOT APPLE OR NOT BUILD_TESTING)
  return()
endif()

add_executable(test_mac_frame_metal_gl
  "${PROJECT_SOURCE_DIR}/app/tests/mac_frame_metal_gl.mm"
  "${PROJECT_SOURCE_DIR}/app/render/MacFrameMetal.mm"
  "${PROJECT_SOURCE_DIR}/app/render/MacFrameSurface.cpp"
  "${PROJECT_SOURCE_DIR}/app/render/MacFrameDisplay.cpp"
  "${PROJECT_SOURCE_DIR}/app/render/FrameMemory.cpp"
  "${PROJECT_SOURCE_DIR}/app/render/RenderImage.cpp")
target_include_directories(test_mac_frame_metal_gl PRIVATE "${PROJECT_SOURCE_DIR}/app")
target_compile_definitions(test_mac_frame_metal_gl PRIVATE GL_SILENCE_DEPRECATION)
target_compile_options(test_mac_frame_metal_gl PRIVATE "$<$<COMPILE_LANGUAGE:OBJCXX>:-fblocks>")
target_link_libraries(test_mac_frame_metal_gl PRIVATE Qt6::Gui Qt6::OpenGL TKOpenGl
  "-framework Metal" "-framework IOSurface" "-framework OpenGL" "-framework Foundation" mitcad_warnings)
add_test(NAME app.render_surface_metal_gl_macos COMMAND test_mac_frame_metal_gl)
set_tests_properties(app.render_surface_metal_gl_macos PROPERTIES
  TIMEOUT 90 RUN_SERIAL TRUE SKIP_RETURN_CODE 77)
