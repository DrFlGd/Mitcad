# SPDX-License-Identifier: MIT
# One source per operation family (see include/mitcad/geometry/geometry.hpp),
# so parallel work on different families touches different files.
add_library(mitcad_geometry STATIC
  src/boolean.cpp
  src/brep_import.cpp
  src/dressup.cpp
  src/extrude.cpp
  src/history.cpp
  src/history.hpp
  src/import.cpp
  src/naming.cpp
  src/profile.cpp
  src/query.cpp
  src/shape.cpp
  src/util.cpp
  src/util.hpp
  src/datum.cpp
  include/mitcad/geometry/boolean.hpp
  include/mitcad/geometry/brep_import.hpp
  include/mitcad/geometry/dressup.hpp
  include/mitcad/geometry/extrude.hpp
  include/mitcad/geometry/geometry.hpp
  include/mitcad/geometry/import.hpp
  include/mitcad/geometry/naming.hpp
  include/mitcad/geometry/profile.hpp
  include/mitcad/geometry/query.hpp
  include/mitcad/geometry/shape.hpp
  include/mitcad/geometry/datum.hpp
  # Face operations: shell, draft, offset, delete, replace, split (F2).
  src/face_select.cpp
  src/face_select.hpp
  src/faceops.cpp
  src/replace.cpp
  src/replace.hpp
  src/split.cpp
  src/tool.cpp
  include/mitcad/geometry/faceops.hpp
  include/mitcad/geometry/split.hpp
  include/mitcad/geometry/tool.hpp)
# Transforms, patterns, mirrors, combine and primitives (F4).
target_sources(mitcad_geometry PRIVATE
  src/pattern.cpp
  src/primitive.cpp
  src/transform.cpp
  include/mitcad/geometry/pattern.hpp
  include/mitcad/geometry/primitive.hpp
  include/mitcad/geometry/transform.hpp)
# Profile features (F1): extrude options, revolve, hole, thread.
target_sources(mitcad_geometry PRIVATE
  src/hole.cpp
  src/reference.cpp
  src/revolve.cpp
  src/sweep.cpp
  src/sweep.hpp
  src/thread.cpp
  src/local_boolean.cpp
  src/local_boolean.hpp
  include/mitcad/geometry/hole.hpp
  include/mitcad/geometry/reference.hpp
  include/mitcad/geometry/revolve.hpp
  include/mitcad/geometry/target.hpp
  include/mitcad/geometry/thread.hpp)
# Sketch planes and projected geometry (S1).
target_sources(mitcad_geometry PRIVATE
  src/sketch_ref.cpp
  include/mitcad/geometry/sketch_ref.hpp)
# Crashes inside OCCT as errors (.f3d import, T1).
target_sources(mitcad_geometry PRIVATE
  src/guard.cpp
  include/mitcad/geometry/guard.hpp)
# Sweeps, lofts, pipes, coils, ribs and webs (F3).
target_sources(mitcad_geometry PRIVATE
  src/loft.cpp
  src/path_sweep.cpp
  src/rib.cpp
  src/spine.cpp
  src/spine.hpp
  include/mitcad/geometry/loft.hpp
  include/mitcad/geometry/path_sweep.hpp
  include/mitcad/geometry/rib.hpp)
# Lofts with end conditions and rails (P2), and the curve and surface
# fitting they share with other features.
target_sources(mitcad_geometry PRIVATE
  src/loft_skin.cpp
  src/loft_skin.hpp
  src/skin.cpp
  src/skin.hpp)
# Fillet and chamfer options (P6): asymmetric and G2 fillets from a chamfer
# whose bevels become blend surfaces; setback and blend corner patches.
target_sources(mitcad_geometry PRIVATE
  src/blend.cpp
  src/blend.hpp
  src/corner.cpp
  src/corner.hpp)
# Sketch text (P3): glyph outlines through OCCT's fonts, with the bundled
# font (third_party/fonts/droid-sans) embedded in the library.
find_package(Freetype REQUIRED)
set(mitcad_font_dir "${PROJECT_SOURCE_DIR}/third_party/fonts/droid-sans")
add_custom_command(
  OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/embedded_fonts.cpp"
  COMMAND "${CMAKE_COMMAND}" "-DOUTPUT=${CMAKE_CURRENT_BINARY_DIR}/embedded_fonts.cpp"
          "-DNAMESPACE=mitcad::geometry::fonts" "-DNAMES=droid_sans,droid_sans_bold"
          "-DPATHS=${mitcad_font_dir}/DroidSans.ttf,${mitcad_font_dir}/DroidSans-Bold.ttf"
          -P "${CMAKE_CURRENT_SOURCE_DIR}/cmake/embed.cmake"
  DEPENDS cmake/embed.cmake "${mitcad_font_dir}/DroidSans.ttf" "${mitcad_font_dir}/DroidSans-Bold.ttf"
  COMMENT "Embedding the bundled font"
  VERBATIM)
target_sources(mitcad_geometry PRIVATE
  src/text.cpp
  include/mitcad/geometry/text.hpp
  "${CMAKE_CURRENT_BINARY_DIR}/embedded_fonts.cpp")
target_link_libraries(mitcad_geometry PUBLIC TKService PRIVATE Freetype::Freetype)
# Shapes with their names as bytes, for the store of computed results (P7d).
target_sources(mitcad_geometry PRIVATE
  src/persist.cpp
  include/mitcad/geometry/persist.hpp)
set_property(SOURCE src/persist.cpp APPEND PROPERTY COMPILE_DEFINITIONS "MITCAD_VERSION=\"${PROJECT_VERSION}\"")
# Long operations stopped on request (P7e).
target_sources(mitcad_geometry PRIVATE
  src/cancel.cpp
  include/mitcad/geometry/cancel.hpp)
# Operations leave their inputs as they are: B-rep fingerprints and the
# input check (T0e).
target_sources(mitcad_geometry PRIVATE
  src/input_check.cpp
  include/mitcad/geometry/input_check.hpp)
# Booleans on perforated bodies: holes away from the tool sit out (mitcad#72).
target_sources(mitcad_geometry PRIVATE
  src/far_features.cpp
  src/far_features.hpp)
# The material a near copy of a body lacks, cut only where the two differ
# (the import's history-based guesses, mitcad#85).
target_sources(mitcad_geometry PRIVATE
  src/removed.cpp
  include/mitcad/geometry/removed.hpp)
# Roundings that run over cut-outs of their faces: the cut-outs filled and
# cut out again (mitcad#311).
target_sources(mitcad_geometry PRIVATE
  src/cutouts.cpp
  src/cutouts.hpp)
# Fillets and chamfers of whole circles that run over a neighbouring face,
# built as rings (mitcad#60).
target_sources(mitcad_geometry PRIVATE
  src/ring_dressup.cpp
  src/ring_dressup.hpp)
# Sheet material construction (mitcad#206).
target_sources(mitcad_geometry PRIVATE
  src/sheet.cpp
  src/sheet_chart.cpp
  src/sheet_curve.cpp
  src/sheet_curve.hpp
  src/sheet_trim.cpp
  include/mitcad/geometry/sheet_domain.hpp
  include/mitcad/geometry/sheet_trim.hpp
  include/mitcad/geometry/sheet.hpp)
# The faces a run of features made, as solids a pattern copies (its
# optimized compute option).
target_sources(mitcad_geometry PRIVATE
  src/face_copy.cpp
  include/mitcad/geometry/face_copy.hpp)
# The tangent chains of a body's edges and faces, for the hover (mitcad#290).
target_sources(mitcad_geometry PRIVATE
  src/tangent_chain.cpp
  include/mitcad/geometry/tangent_chain.hpp)
# How a new feature's tool lies against bodies: Extrude's automatic
# operation (mitcad#289).
target_sources(mitcad_geometry PRIVATE
  src/contact.cpp
  include/mitcad/geometry/contact.hpp)
# The analytic surfaces and curves of a dressup's walked B-spline faces
# (mitcad#328).
target_sources(mitcad_geometry PRIVATE
  src/canonical.cpp
  src/canonical.hpp)
# BUILD: the face names of derived parts (mitcad#375).
target_sources(mitcad_geometry PRIVATE
  src/build.cpp
  include/mitcad/geometry/build.hpp)
# Helices built as B-reps whose faces run all the turns, and threads whose
# turns meet in one edge (mitcad#235).
target_sources(mitcad_geometry PRIVATE
  src/helix_brep.cpp
  src/helix_brep.hpp)
# Contact sets: bodies that stop each other while joints move (mitcad#454).
target_sources(mitcad_geometry PRIVATE
  src/motion_contact.cpp
  include/mitcad/geometry/motion_contact.hpp)
# Emboss: profiles wrapped onto cylinders (mitcad#458).
target_sources(mitcad_geometry PRIVATE
  src/emboss.cpp
  include/mitcad/geometry/emboss.hpp)
# Boundary Fill: the cells tool bodies enclose (mitcad#457).
target_sources(mitcad_geometry PRIVATE
  src/boundary_fill.cpp
  include/mitcad/geometry/boundary_fill.hpp)
# Patch, Stitch and Thicken: surface bodies filled, sewn and made solid (mitcad#456).
target_sources(mitcad_geometry PRIVATE
  src/surfaces.cpp
  include/mitcad/geometry/surfaces.hpp)
# Surface modelling: surface bodies and their features (mitcad#455).
target_sources(mitcad_geometry PRIVATE
  src/surface_sweeps.cpp
  include/mitcad/geometry/surface_sweeps.hpp)
# Surface modelling: Offset Surface, Ruled Surface and Extend (mitcad#455).
target_sources(mitcad_geometry PRIVATE
  src/free_loops.hpp
  src/surface_offsets.cpp
  include/mitcad/geometry/surface_offsets.hpp)
# Surface modelling: Trim, Untrim, Unstitch and Reverse Normal (mitcad#455).
target_sources(mitcad_geometry PRIVATE
  src/surface_edits.cpp
  include/mitcad/geometry/surface_edits.hpp)
# Plastic part features: the bands of lips and grooves (mitcad#459).
target_sources(mitcad_geometry PRIVATE
  src/plastic.cpp
  include/mitcad/geometry/plastic.hpp)
# Surface modelling: Blend, Boundary, Match and Fair Surface (mitcad#455).
target_sources(mitcad_geometry PRIVATE
  src/sheet_edges.hpp
  src/surface_fills.cpp
  include/mitcad/geometry/surface_fills.hpp)
# Drawings: views with hidden lines removed and round holes (mitcad#471).
target_sources(mitcad_geometry PRIVATE
  src/drawing.cpp
  include/mitcad/geometry/drawing.hpp)
target_link_libraries(mitcad_geometry PRIVATE TKHLR TKMesh)

# Gears: toothed profiles swept along their axis or ruled to an apex (mitcad#472).
target_sources(mitcad_geometry PRIVATE
  src/gear.cpp
  include/mitcad/geometry/gear.hpp)
target_include_directories(mitcad_geometry PUBLIC include)
# Volumes and areas come from the analysis library's integrals, so that
# every reported volume agrees (query.cpp, brep_import.cpp).
target_link_libraries(mitcad_geometry
  PUBLIC TKernel TKMath TKG2d TKG3d TKGeomBase TKBRep TKTopAlgo TKPrim TKBO TKFillet TKGeomAlgo TKShHealing
         TKBool TKOffset TKFeat
  PRIVATE mitcad_geometry_analysis mitcad_warnings)

if(BUILD_TESTING)
  add_executable(test_geometry
    tests/check.hpp
    tests/test_boolean.cpp
    tests/test_dressup.cpp
    tests/test_extrude.cpp
    tests/test_faceops.cpp
    tests/test_split.cpp
    tests/test_main.cpp
    tests/test_naming.cpp
    tests/test_profile.cpp
    tests/test_query.cpp
    tests/test_datum.cpp)
  # Transforms, patterns, mirrors, combine and primitives (F4).
  target_sources(test_geometry PRIVATE tests/test_transform.cpp)
  target_sources(test_geometry PRIVATE
    tests/test_extrude_feature.cpp
    tests/test_hole.cpp
    tests/test_revolve.cpp
    tests/test_thread.cpp)
  # Sweeps, lofts, pipes, coils, ribs and webs (F3).
  target_sources(test_geometry PRIVATE tests/test_sweeps.cpp)
  # Sketch text (P3).
  target_sources(test_geometry PRIVATE tests/test_text.cpp)
  # Lofts with end conditions and rails, curve and surface fitting (P2).
  target_sources(test_geometry PRIVATE tests/test_skin.cpp)
  # Shapes with their names as bytes (P7d).
  target_sources(test_geometry PRIVATE tests/test_persist.cpp)
  # Long operations stopped on request (P7e).
  target_sources(test_geometry PRIVATE tests/test_cancel.cpp)
  # Operations leave their inputs as they are (T0e), the analyses too.
  target_sources(test_geometry PRIVATE tests/test_input_check.cpp)
  # Booleans on perforated bodies (mitcad#72).
  target_sources(test_geometry PRIVATE tests/test_far_features.cpp)
  # The material a near copy of a body lacks (mitcad#85).
  target_sources(test_geometry PRIVATE tests/test_removed.cpp)
  # A mirror image joined only where it differs from the body (mitcad#88).
  target_sources(test_geometry PRIVATE tests/test_mirror_join.cpp)
  # Roundings wider than a neighbouring face, OCCT's edge overflow (mitcad#121).
  target_sources(test_geometry PRIVATE tests/test_fillet_overflow.cpp)
  # Roundings OCCT's fillet failed on, with the port's patches (mitcad#122).
  target_sources(test_geometry PRIVATE tests/test_fillet_kernel.cpp)
  # Ends, seams and corner caps of OCCT's roundings (mitcad#133).
  target_sources(test_geometry PRIVATE tests/test_fillet_ends.cpp)
  # Allocations that fail inside OCCT, with the port's patch (mitcad#132).
  target_sources(test_geometry PRIVATE tests/test_allocation.cpp)
  # ShapeFix on faces with many holes, with the port's patch 0060 (mitcad#60).
  target_sources(test_geometry PRIVATE tests/test_shapefix.cpp)
  # A chamfer whose pieces meet at a vertex of the edge they reach, with the
  # port's patches 0090 and 0091 (mitcad#160).
  target_sources(test_geometry PRIVATE tests/test_chamfer_kernel.cpp)
  # Shells whose wall makes a narrow face vanish, with the port's patch 0100
  # (mitcad#172).
  target_sources(test_geometry PRIVATE tests/test_shell_vanishing.cpp)
  # Walked roundings that are true rolling balls, with the port's patches
  # 0075-0077 (mitcad#121).
  target_sources(test_geometry PRIVATE tests/test_fillet_rolling_ball.cpp)
  # The faces a run of features made, copied by a pattern's optimized
  # compute option.
  target_sources(test_geometry PRIVATE tests/test_face_copy.cpp)
  # Operations on bodies of thousands of faces, with the port's patch 0110
  # (mitcad#121).
  target_sources(test_geometry PRIVATE tests/test_large_bodies.cpp)
  # Corners where a rounding ends, with the port's patches 0130 and later
  # (mitcad#236).
  target_sources(test_geometry PRIVATE tests/test_fillet_corners.cpp)
  # Chamfers wider than a face beside them, with the port's patches 0140 and
  # later (mitcad#240).
  target_sources(test_geometry PRIVATE tests/test_chamfer_overflow.cpp)
  # Roundings whose results were invalid or whose computation failed, with
  # the port's patches 0150 and later (mitcad#239).
  target_sources(test_geometry PRIVATE tests/test_fillet_results.cpp)
  # Fillet contours that stop where their edges turn, with the port's patch
  # 0180 (mitcad#241).
  target_sources(test_geometry PRIVATE tests/test_fillet_chains.cpp)
  # The tangent chains of a body's edges and faces (mitcad#290).
  target_sources(test_geometry PRIVATE tests/test_tangent_chain.cpp)
  # Roundings that run over cut-outs of their faces (mitcad#311).
  target_sources(test_geometry PRIVATE tests/test_fillet_cutouts.cpp)
  # Section curves of sketches (mitcad#389).
  target_sources(test_geometry PRIVATE tests/test_section_curves.cpp)
  # Roundings cut by a face of the solid they pass through, with the port's
  # patch 0700 (mitcad#343).
  target_sources(test_geometry PRIVATE tests/test_fillet_pierced.cpp)
  target_link_libraries(test_geometry PRIVATE mitcad_geometry_analysis)
  target_link_libraries(test_geometry PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry COMMAND test_geometry)
  # Planar material domains and sheet thickness/reference sides (mitcad#206).
  add_executable(test_sheet tests/test_sheet.cpp)
  target_link_libraries(test_sheet PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.sheet COMMAND test_sheet)
  add_executable(test_sheet_trim tests/test_sheet_trim.cpp)
  target_link_libraries(test_sheet_trim PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.sheet_trim COMMAND test_sheet_trim)
  add_executable(test_sheet_bend tests/test_sheet_bend.cpp)
  target_link_libraries(test_sheet_bend PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.sheet_bend COMMAND test_sheet_bend)
  add_executable(test_sheet_bend_graph tests/test_sheet_bend_graph.cpp)
  target_link_libraries(test_sheet_bend_graph PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.sheet_bend_graph COMMAND test_sheet_bend_graph)
  # Booleans on stored faces that run many turns (threaded holes, mitcad#349).
  add_executable(test_multi_turn_faces tests/test_multi_turn_faces.cpp)
  target_link_libraries(test_multi_turn_faces PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.multi_turn_faces COMMAND test_multi_turn_faces)
  # Joins of treads and helical stringers on shared cylinders (mitcad#400).
  add_executable(test_helical_joins tests/test_helical_joins.cpp)
  target_link_libraries(test_helical_joins PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.helical_joins COMMAND test_helical_joins)
  # Corners and ends of roundings, with the port's patches 0720 and later
  # (mitcad#343).
  add_executable(test_fillet_corner_cases tests/test_fillet_corner_cases.cpp)
  target_link_libraries(test_fillet_corner_cases PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_corner_cases COMMAND test_fillet_corner_cases)
  # A thread's crest narrower than the roundings of both its edges, with the
  # port's patch 0750 (mitcad#343).
  add_executable(test_fillet_thread_crests tests/test_fillet_thread_crests.cpp)
  target_link_libraries(test_fillet_thread_crests PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_thread_crests COMMAND test_fillet_thread_crests)
  # Roundings vanishing inside their chains, rolling over narrow faces and
  # lenses, with the port's patches 0770 and later (mitcad#343).
  add_executable(test_fillet_lens_overflow tests/test_fillet_lens_overflow.cpp)
  target_link_libraries(test_fillet_lens_overflow PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_lens_overflow COMMAND test_fillet_lens_overflow)
  # Roundings round a thread's chamfer and up to a groove's floor on bodies
  # with the traits of stored ones, with the port's patches 0800 and later
  # (mitcad#343).
  add_executable(test_fillet_crest_corners tests/test_fillet_crest_corners.cpp)
  target_link_libraries(test_fillet_crest_corners PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_crest_corners COMMAND test_fillet_crest_corners)
  # A fillet contour round a closed chain that stops at a turn short of its
  # start ends, with the port's patch 0840 (mitcad#434).
  add_executable(test_fillet_chain_loops tests/test_fillet_chain_loops.cpp)
  target_link_libraries(test_fillet_chain_loops PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_chain_loops COMMAND test_fillet_chain_loops)
  # A fillet contour that runs on into an edge whose faces are nearly
  # tangent, with the port's patches 0860 and 0861 (mitcad#441).
  add_executable(test_fillet_near_tangent_contour tests/test_fillet_near_tangent_contour.cpp)
  target_link_libraries(test_fillet_near_tangent_contour PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_near_tangent_contour COMMAND test_fillet_near_tangent_contour)
  # A rounding along an edge of a strip the ball cannot touch, with the
  # port's patch 0880 (mitcad#343).
  add_executable(test_fillet_strip_start tests/test_fillet_strip_start.cpp)
  target_link_libraries(test_fillet_strip_start PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_strip_start COMMAND test_fillet_strip_start)
  # Roundings whose contacts run off a narrow face: up to a sliver at the
  # edge's end, and over a hole's rounded mouth, with the port's patches 0890
  # and 0891 (mitcad#343).
  add_executable(test_fillet_end_overflow tests/test_fillet_end_overflow.cpp)
  target_link_libraries(test_fillet_end_overflow PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_end_overflow COMMAND test_fillet_end_overflow)
  # A rounding whose contact runs off a narrow face across its tangent edge
  # with a rounded rim keeps to that face's surface, cut by the rim, with
  # the port's patch 0870 (mitcad#343).
  add_executable(test_fillet_tangent_overflow tests/test_fillet_tangent_overflow.cpp)
  target_link_libraries(test_fillet_tangent_overflow PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.fillet_tangent_overflow COMMAND test_fillet_tangent_overflow)
  # Boundary Fill: the cells tool bodies enclose (mitcad#457).
  add_executable(test_boundary_fill tests/test_boundary_fill.cpp)
  target_link_libraries(test_boundary_fill PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.boundary_fill COMMAND test_boundary_fill)
  # Patch, Stitch and Thicken: surface bodies filled, sewn and made solid (mitcad#456).
  add_executable(test_surfaces tests/test_surfaces.cpp)
  target_link_libraries(test_surfaces PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.surfaces COMMAND test_surfaces)
  # Surface modelling: surface bodies and their features (mitcad#455).
  add_executable(test_surface_modelling tests/test_surface_modelling.cpp)
  target_link_libraries(test_surface_modelling PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.surface_modelling COMMAND test_surface_modelling)
  # OCCT's crash handlers next to the application's crash report (mitcad#62).
  if(NOT WIN32)
    add_executable(test_guard tests/test_guard.cpp)
    target_link_libraries(test_guard PRIVATE mitcad_geometry mitcad_warnings)
    add_test(NAME geometry.guard_inside COMMAND test_guard inside)
    set_tests_properties(geometry.guard_inside PROPERTIES PASS_REGULAR_EXPRESSION "converted: SIGSEGV")
    add_test(NAME geometry.guard_outside COMMAND test_guard outside)
    set_tests_properties(geometry.guard_outside PROPERTIES PASS_REGULAR_EXPRESSION "earlier handler: signal 11")
    # The wrapper of the analysis bridge's operations (mitcad#392).
    add_test(NAME geometry.guard_operation COMMAND test_guard operation)
    set_tests_properties(geometry.guard_operation PROPERTIES ENVIRONMENT "MITCAD_TEST_OCCT_CRASH=clip"
      PASS_REGULAR_EXPRESSION "error: clip: SIGSEGV")
  endif()
  # Emboss: profiles wrapped onto cylinders (mitcad#458).
  add_executable(test_emboss tests/test_emboss.cpp)
  target_link_libraries(test_emboss PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.emboss COMMAND test_emboss)
  # Drawings: views with hidden lines removed and round holes (mitcad#471).
  add_executable(test_drawing tests/test_drawing.cpp)
  target_link_libraries(test_drawing PRIVATE mitcad_geometry mitcad_warnings)
  add_test(NAME geometry.drawing COMMAND test_drawing)
endif()

# Data exchange and analysis services, separate libraries on top of OCCT.
add_subdirectory(io)
add_subdirectory(analysis)
