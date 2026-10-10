# SPDX-License-Identifier: MIT
# The render worker's prebuilt libraries (docs/rendering.md "Building"):
# Cycles and Open Image Denoise, which vcpkg has no port for, come from
# the prefix of tools/dev-env/build-cycles.sh (build-cycles.ps1 on
# Windows, build-cycles-macos.sh on Apple Silicon), MITCAD_RENDER_DEPS.
# The render worker is part of every build, so a configure without them
# stops here, before project() and the toolchain's vcpkg install, with
# the script to run; it never leaves the renderer out. No Mitcad build
# ever builds them: they stay outside the build folders, shared by all.
# cmake/Render.cmake (from app/CMakeLists.txt) links them.

if(CMAKE_HOST_WIN32)
  set(mitcad_render_script build-cycles.ps1)
  set(mitcad_render_default "C:/dev/mitcad-render-deps")
elseif(CMAKE_HOST_APPLE)
  set(mitcad_render_script build-cycles-macos.sh)
  set(mitcad_render_default "$ENV{HOME}/mitcad-render-deps")
else()
  set(mitcad_render_script build-cycles.sh)
  set(mitcad_render_default "$ENV{HOME}/mitcad-render-deps")
endif()
set(MITCAD_RENDER_DEPS "$ENV{MITCAD_RENDER_DEPS}" CACHE PATH
    "Prefix of tools/dev-env/${mitcad_render_script} (default ${mitcad_render_default})")
if(NOT MITCAD_RENDER_DEPS)
  set(MITCAD_RENDER_DEPS "${mitcad_render_default}" CACHE PATH
      "Prefix of tools/dev-env/${mitcad_render_script} (default ${mitcad_render_default})" FORCE)
endif()
set(MITCAD_CYCLES_CMAKE "${MITCAD_RENDER_DEPS}/install/cycles/mitcad-cycles.cmake")
if(NOT EXISTS "${MITCAD_CYCLES_CMAKE}")
  message(FATAL_ERROR
    "The render worker needs Cycles and Open Image Denoise, built once outside the build folders: "
    "run tools/dev-env/${mitcad_render_script} (docs/rendering.md, \"Building\"), or set "
    "MITCAD_RENDER_DEPS (CMake or environment variable) to the prefix it built. "
    "Not found: ${MITCAD_CYCLES_CMAKE}")
endif()
# The option MITCAD_RENDER of older build folders (the worker was optional
# before mitcad#141) means nothing now; out of the cache, it does not set
# them apart from new ones (tools/check-all.sh fingerprints the options).
unset(MITCAD_RENDER CACHE)
