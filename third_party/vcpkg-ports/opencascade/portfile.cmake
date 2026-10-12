# SPDX-License-Identifier: MIT
# vcpkg's opencascade port (microsoft/vcpkg at the baseline in vcpkg.json)
# with Mitcad's patches 0006 and later (see ../README.md).
string(REPLACE "." "_" VERSION_STR "V${VERSION}")
vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO Open-Cascade-SAS/OCCT
    REF "${VERSION_STR}"
    SHA512 bbe7099071cbf5397940ebc6e66ec05f8023d5e5aae6142870e14b93aa6f8f94c30980ef421e717f0fbfbc23b3520c3ccfe8a939c4caba3ccbf325060e26eb52
    HEAD_REF master
    PATCHES
        0001-cmake-keep-build-use-vcpkg-explicit.patch
        0002-cmake-load-exported-package-dependencies.patch
        0003-image-remove-freeimage-msvc-autolink.patch
        0004-cmake-add-additional-path-extraction-for-OpenCASCADE.patch
        0005-drop-bin-letter.patch
        0006-shapeupgrade-unifysamedomain-seam-search-by-vertex.patch
        0007-brepcheck-analyzer-no-search-in-face.patch
        0008-brepcheck-wire-seam-edges-once-per-face.patch
        0009-bopalgo-face-face-adaptors-once-per-face.patch
        0010-brepblend-surfrst-line-on-both-restrictions.patch
        0011-chfi3d-fillet-edge-overflow.patch
        0012-chfi3d-closed-stripe-end-on-vertex.patch
        0013-chfi3d-overflow-across-faces-and-overlapping-roundings.patch
        0014-chfi3d-contact-along-face-edge.patch
        0015-brepblend-surfrst-smaller-step-on-failed-reframing.patch
        0016-chfi3d-rounding-rolls-on-anothers-contact-line.patch
        0017-chfi3d-contact-along-face-edge-ends-at-vertices.patch
        0018-chfi3d-corner-where-the-end-face-is-split-in-one-plane.patch
        0019-chfi3d-undone-moresurfdata-corner-fails.patch
        0020-chfi3d-crash-fixes.patch
        0021-chfi3d-corner-of-contour-closed-at-sharp-vertex.patch
        0022-chfi3d-round-edges-asked-for-near-tangent.patch
        0023-chfi3d-meeting-point-of-periodic-contact-lines.patch
        0024-chfi3d-order-of-periodic-kpart-surfaces.patch
        0025-chfi3d-end-cut-by-faces-around-a-cylinder-corner.patch
        0026-chfi3d-onsame-end-cut-by-several-faces.patch
        0030-standard-failed-allocation-throws.patch
        0031-osd-threadpool-failed-allocations-to-launching-thread.patch
        0032-ncollection-containers-intact-when-allocation-fails.patch
        0033-brepcheck-edge-curve-adaptor-per-context.patch
        0050-chfi3d-kpart-end-runs-into-end-face.patch
        0051-chfi3d-rolling-on-across-surfdatas.patch
        0052-chfi3d-rolling-start-beside-tangent-face.patch
        0053-chfi3d-corner-after-overflow-onto-face-beyond.patch
        0054-chfi3d-contact-along-a-part-of-a-face-edge.patch
        0055-chfi3d-kpart-blend-cut-by-face-beyond.patch
        0056-chfi3d-corner-cutting-the-last-two-walked-pieces.patch
        0057-chfi3d-kpart-gaps-in-order-across-closed-lines.patch
        0058-chfi3d-rounding-rolls-on-a-convex-far-edge.patch
        0059-chfi3d-kpart-blend-cut-by-the-face-at-the-end.patch
        0060-shapefix-intersecting-edges-once-per-face.patch
        0070-chfi3d-kpart-cut-to-end-where-the-narrow-face-vanishes.patch
        0071-chfi3d-kpart-cut-by-several-faces-beyond.patch
        0072-chfi3d-kpart-cuts-on-closed-spines.patch
        0073-chfi3d-kpart-pieces-over-a-hole-are-one-face.patch
        0074-chfi3d-walked-blend-cut-by-the-face-at-the-end.patch
        0075-blendfunc-constrad-rolling-ball-section.patch
        0076-chfi3d-extension-rolls-on-the-faces-own-surface.patch
        0077-chfi3d-walk-past-the-spine-ends-before-a-singular-section.patch
        0090-chfi3d-double-interference-at-vertex-kind.patch
        0091-topopebrepbuild-point-uses-within-array.patch
        0100-brepoffset-faces-vanishing-within-the-offset.patch
)

if (VCPKG_LIBRARY_LINKAGE STREQUAL "dynamic")
    set(BUILD_TYPE "Shared")
else()
    set(BUILD_TYPE "Static")
endif()

vcpkg_check_features(OUT_FEATURE_OPTIONS FEATURE_OPTIONS
    FEATURES
        freeimage   USE_FREEIMAGE
        freetype    USE_FREETYPE
        rapidjson   USE_RAPIDJSON
        tbb         USE_TBB
        vtk         USE_VTK
)

# We turn off BUILD_MODULE_Draw as it requires TCL 8.6 and TK 8.6 specifically which conflicts with vcpkg only having TCL 9.0 
# And pre-built ActiveTCL binaries are behind a marketing wall :(
# We use the Unix install layout for Windows as it matches vcpkg
vcpkg_cmake_configure(
    SOURCE_PATH "${SOURCE_PATH}"
    OPTIONS
        ${FEATURE_OPTIONS}
        -DBUILD_LIBRARY_TYPE=${BUILD_TYPE}
        -DBUILD_MODULE_Draw=OFF
        -DBUILD_DOC_Overview=OFF
        -DINSTALL_DIR_LAYOUT=Unix
        -DINSTALL_DIR_DOC=share/trash
        -DINSTALL_DIR_SCRIPT=share/trash # not relocatable
        -DINSTALL_TEST_CASES=OFF
        -DUSE_TK=OFF
)

vcpkg_cmake_install()
vcpkg_copy_pdbs()
vcpkg_cmake_config_fixup(CONFIG_PATH lib/cmake/opencascade)

#make occt includes relative to source_file
file(GLOB extra_headers
    LIST_DIRECTORIES false
    RELATIVE "${CURRENT_PACKAGES_DIR}/include/opencascade"
    "${CURRENT_PACKAGES_DIR}/include/opencascade/*.h"
)
list(JOIN extra_headers "|" extra_headers)
file(GLOB files "${CURRENT_PACKAGES_DIR}/include/opencascade/*.[hgl]xx")
foreach(file_name IN LISTS files)
    vcpkg_replace_string("${file_name}" "(# *include) <([a-zA-Z0-9_]*[.][hgl]xx|${extra_headers})>" [[\1 "\2"]] REGEX IGNORE_UNCHANGED)
endforeach()

if(VCPKG_LIBRARY_LINKAGE STREQUAL "static")
    vcpkg_replace_string("${CURRENT_PACKAGES_DIR}/include/opencascade/Standard_Macro.hxx" "defined(OCCT_STATIC_BUILD)" "(1)")
endif()

file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")
file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/share")
file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/share/opencascade/samples/qt")
file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/share/trash")

vcpkg_install_copyright(
    FILE_LIST
        "${SOURCE_PATH}/LICENSE_LGPL_21.txt"
        "${SOURCE_PATH}/OCCT_LGPL_EXCEPTION.txt"
)
