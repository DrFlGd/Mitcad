# SPDX-License-Identifier: MIT
# Keep release optimisation separate from the dependency CPU target: the supported
# AppleClang and Rust toolchains already default to Apple M1 on Darwin arm64.
if(APPLE)
  set(mitcad_macos_thin_lto_default OFF)
  if(CMAKE_OSX_ARCHITECTURES STREQUAL "arm64")
    set(mitcad_macos_thin_lto_default ON)
  endif()
  option(MITCAD_MACOS_THIN_LTO "Use ThinLTO in macOS release configurations" ${mitcad_macos_thin_lto_default})
  if(MITCAD_MACOS_THIN_LTO)
    if(NOT CMAKE_OSX_ARCHITECTURES STREQUAL "arm64")
      message(FATAL_ERROR "MITCAD_MACOS_THIN_LTO requires an arm64-only macOS build")
    endif()
    include(CheckIPOSupported)
    check_ipo_supported(RESULT mitcad_ipo_supported OUTPUT mitcad_ipo_error LANGUAGES CXX)
    if(NOT mitcad_ipo_supported)
      message(FATAL_ERROR "MITCAD_MACOS_THIN_LTO is unavailable: ${mitcad_ipo_error}")
    endif()
    # Refuse a full-LTO substitute. The compiler modules know both languages'
    # ThinLTO flags, although CMake's IPO property only instruments C++ here.
    foreach(language CXX OBJCXX)
      if(NOT "-flto=thin" IN_LIST CMAKE_${language}_COMPILE_OPTIONS_IPO)
        message(FATAL_ERROR "MITCAD_MACOS_THIN_LTO needs ${language} ThinLTO support")
      endif()
    endforeach()
    set(CMAKE_INTERPROCEDURAL_OPTIMIZATION_RELEASE ON)
    set(CMAKE_INTERPROCEDURAL_OPTIMIZATION_RELWITHDEBINFO ON)
    # CMake 3.31 does not emit its IPO flags for Objective-C++ objects even
    # with IPO enabled on their target. Cover those compiles explicitly and
    # also any executable whose selected linker language is Objective-C++.
    add_compile_options(
      "$<$<AND:$<COMPILE_LANGUAGE:OBJCXX>,$<CONFIG:Release,RelWithDebInfo>>:-flto=thin>")
    add_link_options(
      "$<$<AND:$<LINK_LANGUAGE:OBJCXX>,$<CONFIG:Release,RelWithDebInfo>>:-flto=thin>")
    message(STATUS "macOS release optimisation: C++/Objective-C++ ThinLTO (Rust profiles unchanged)")
  endif()
endif()
