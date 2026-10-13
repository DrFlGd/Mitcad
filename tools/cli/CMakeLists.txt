# SPDX-License-Identifier: MIT
# mitcad-cli: the model and the kernel without the user interface.
add_executable(mitcad-cli main.cpp library.cpp library.hpp projects.cpp projects.hpp)
target_link_libraries(mitcad-cli PRIVATE mitcad_ffi mitcad_geometry mitcad_geometry_io mitcad_warnings)

# mitcad-cli render (mitcad#48, docs/rendering.md "Final render"). It makes
# the scene and the job as File > Render Image does (the application's
# render/RenderBatch, with Qt Core and Gui, no widgets) and runs the render
# worker mitcad-render; it links none of the renderer's libraries.
set(app "${PROJECT_SOURCE_DIR}/app")
target_sources(mitcad-cli PRIVATE render.cpp render.hpp
  "${app}/framework/Appearances.cpp" "${app}/framework/Appearances.hpp"
  "${app}/render/RenderBatch.cpp" "${app}/render/RenderBatch.hpp"
  "${app}/render/RenderDevice.cpp" "${app}/render/RenderDevice.hpp"
  "${app}/render/RenderMesh.cpp" "${app}/render/RenderMesh.hpp"
  "${app}/render/RenderScene.cpp" "${app}/render/RenderScene.hpp")
set_target_properties(mitcad-cli PROPERTIES AUTOMOC ON)
target_include_directories(mitcad-cli PRIVATE "${app}")
target_include_directories(mitcad-cli SYSTEM PRIVATE "${MITCAD_CGLTF_INCLUDE_DIR}")
target_link_libraries(mitcad-cli PRIVATE Qt6::Gui TKMesh TKFillet TKPrim TKTopAlgo TKBRep)

# The imports, which the import corpus tests run, apart from the other
# commands (tools/check-all.sh runs the corpus again when they change).
target_sources(mitcad-cli PRIVATE import.cpp import.hpp)

if(BUILD_TESTING)
  set(scripts "${CMAKE_CURRENT_SOURCE_DIR}/tests")
  # Each script builds a part with commands and checks it with expectations.
  foreach(script block_fillet_chamfer join cut_split names_follow_parameters)
    add_test(NAME cli.${script} COMMAND mitcad-cli run "${scripts}/${script}.json")
  endforeach()
  # Profile features (F1), with the volumes of the T0 reference models.
  foreach(script f1_extrude f1_revolve f1_hole f1_thread f1_failures)
    add_test(NAME cli.${script} COMMAND mitcad-cli run "${scripts}/${script}.json")
  endforeach()
  # Assembly cut scope preserves component-local geometry and split ownership.
  set(cut_scope "${CMAKE_CURRENT_BINARY_DIR}/extrude-cut-scope.mitcad")
  add_test(NAME cli.extrude_cut_scope COMMAND mitcad-cli run "${scripts}/extrude_cut_scope.json" --save "${cut_scope}")
  set_tests_properties(cli.extrude_cut_scope PROPERTIES FIXTURES_SETUP cli_cut_scope)
  add_test(NAME cli.extrude_cut_scope_reopened COMMAND mitcad-cli run "${scripts}/extrude_cut_scope_reopened.json" --open "${cut_scope}")
  set_tests_properties(cli.extrude_cut_scope_reopened PROPERTIES FIXTURES_REQUIRED cli_cut_scope)

  # Transforms, patterns, mirrors, combine and primitives (F4): the volumes
  # of the T0 reference models (MITCAD_F3D_MODELS) and analytic ones.
  foreach(script f4_combine f4_mirror f4_mirror_feature f4_move f4_align_scale
                 f4_pattern_bodies f4_pattern_features f4_pattern_circular f4_primitives
                 pattern_at_points f4_pattern_face_copy)
    add_test(NAME cli.${script} COMMAND mitcad-cli run "${scripts}/${script}.json")
  endforeach()

  # Face operations (F2): the T0 models' volumes, names through edits, failures.
  foreach(script f2_fillet_chamfer f2_shell_draft f2_face_ops f2_names_follow_parameters f2_failures)
    add_test(NAME cli.${script} COMMAND mitcad-cli run "${scripts}/${script}.json")
  endforeach()

  # The general sketch (S1): drawing tools, planes, frames and projections
  # extruded, with analytic volumes.
  foreach(script s1_sketch_tools)
    add_test(NAME cli.${script} COMMAND mitcad-cli run "${scripts}/${script}.json")
  endforeach()
  # Sketch text, patterns and offsets (P3, P4): extruded letters against
  # their regions' areas, patterned holes and an ellipse's offset.
  add_test(NAME cli.p34_text_patterns COMMAND mitcad-cli run "${scripts}/p34_text_patterns.json")

  # Sweeps, lofts, pipes, coils, ribs and webs (F3): the T0 models' volumes
  # and analytic ones, names through parameter edits, failures.
  foreach(script f3_sweep f3_loft f3_pipe_coil f3_rib_web f3_names_follow_parameters f3_failures)
    add_test(NAME cli.${script} COMMAND mitcad-cli run "${scripts}/${script}.json")
  endforeach()

  # Components and occurrences (F6): placements, subassemblies, timeline
  # moves, components made by features and copies, with world-space
  # volumes, boxes and centres of mass; then external components: the
  # assembly saved, inserted linked and as a copy, edited, and the link
  # following the edit when the design opens again.
  add_test(NAME cli.f6_components COMMAND mitcad-cli run "${scripts}/f6_components.json")
  # Explicit move destinations keep real OCCT geometry and external support
  # links in place across nested rotations, undo/redo and save/reopen (#201).
  set(move_destination "${CMAKE_CURRENT_BINARY_DIR}/move-destination.mitcad")
  add_test(NAME cli.move_destination COMMAND mitcad-cli run "${scripts}/move_destination.json"
           --save "${move_destination}")
  set_tests_properties(cli.move_destination PROPERTIES FIXTURES_SETUP cli_move_destination)
  add_test(NAME cli.move_destination_reopened
           COMMAND mitcad-cli run "${scripts}/move_destination_reopened.json" --open "${move_destination}")
  set_tests_properties(cli.move_destination_reopened PROPERTIES FIXTURES_REQUIRED cli_move_destination)
  # Moving an occurrence to another parent keeps real OCCT geometry and the
  # links through it in place across turned parents, undo/redo and
  # save/reopen (mitcad#219).
  set(reparent_occurrence "${CMAKE_CURRENT_BINARY_DIR}/reparent-occurrence.mitcad")
  add_test(NAME cli.reparent_occurrence COMMAND mitcad-cli run "${scripts}/reparent_occurrence.json"
           --save "${reparent_occurrence}")
  set_tests_properties(cli.reparent_occurrence PROPERTIES FIXTURES_SETUP cli_reparent_occurrence)
  add_test(NAME cli.reparent_occurrence_reopened
           COMMAND mitcad-cli run "${scripts}/reparent_occurrence_reopened.json" --open "${reparent_occurrence}")
  set_tests_properties(cli.reparent_occurrence_reopened PROPERTIES FIXTURES_REQUIRED cli_reparent_occurrence)
  set(f6 "${CMAKE_CURRENT_BINARY_DIR}/f6")
  file(MAKE_DIRECTORY "${f6}")
  add_test(NAME cli.f6_assembly COMMAND mitcad-cli run "${scripts}/f6_assembly.json" --save "${f6}/plate.mitcad")
  set_tests_properties(cli.f6_assembly PROPERTIES FIXTURES_SETUP cli_f6_part)
  add_test(NAME cli.f6_insert COMMAND mitcad-cli run "${scripts}/f6_insert.json" --save "${f6}/linked.mitcad")
  set_tests_properties(cli.f6_insert PROPERTIES WORKING_DIRECTORY "${f6}"
    FIXTURES_REQUIRED cli_f6_part FIXTURES_SETUP cli_f6_linked)
  add_test(NAME cli.f6_part_edit COMMAND mitcad-cli run "${scripts}/f6_part_edit.json"
                                         --open "${f6}/plate.mitcad" --save "${f6}/plate.mitcad")
  set_tests_properties(cli.f6_part_edit PROPERTIES FIXTURES_REQUIRED cli_f6_linked FIXTURES_SETUP cli_f6_edited)
  add_test(NAME cli.f6_link_update COMMAND mitcad-cli info "${f6}/linked.mitcad")
  set_tests_properties(cli.f6_link_update PROPERTIES FIXTURES_REQUIRED cli_f6_edited PASS_REGULAR_EXPRESSION
    "Warning: plate is updated from plate\\.mitcad.*Placed bodies: 8.*plate:1/Body1 \\(F1\\.b0\\): volume 8000\\.000 mm3")

  # A version 1 file of V0 opens and recomputes to the same geometry.
  add_test(NAME cli.info_v1 COMMAND mitcad-cli info "${scripts}/v1_boss.mitcad")
  set_tests_properties(cli.info_v1 PROPERTIES PASS_REGULAR_EXPRESSION
    "F6 Fillet1 \\(fillet\\): ok.*Body1 \\(F2\\.b0\\): volume 48403\\.451 mm3")
  # OCCT's crash handlers (catch_occt_crashes): the same file with its
  # fillet crashing inside the geometry kernel (a test hook) reports the
  # fillet failed and the bodies before it, instead of crashing. On Linux
  # OCCT's handler names the signal (on macOS it is SIGSEGV or SIGBUS, as a
  # fault can arrive as either). On Windows a crash outside OCCT's own
  # code (compiled with /EHa) reaches OCCT's handler through the C
  # runtime's SIGSEGV, where OCCT 8 locks its mutex twice: the error is
  # "resource deadlock would occur", an error all the same.
  if(WIN32)
    set(crash_error "error: ")
  elseif(APPLE)
    set(crash_error "error: fillet: SIG(SEGV|BUS)")
  else()
    set(crash_error "error: fillet: SIGSEGV")
  endif()
  add_test(NAME cli.occt_crash COMMAND mitcad-cli info "${scripts}/v1_boss.mitcad")
  set_tests_properties(cli.occt_crash PROPERTIES ENVIRONMENT "MITCAD_TEST_OCCT_CRASH=fillet"
    PASS_REGULAR_EXPRESSION "F6 Fillet1 \\(fillet\\): ${crash_error}.*Body1 \\(F2\\.b0\\): volume")

  # Saving the result of a script, then reading it back.
  set(saved "${CMAKE_CURRENT_BINARY_DIR}/join.mitcad")
  add_test(NAME cli.save COMMAND mitcad-cli run "${scripts}/join.json" --save "${saved}")
  set_tests_properties(cli.save PROPERTIES FIXTURES_SETUP cli_saved)
  add_test(NAME cli.info_saved COMMAND mitcad-cli info "${saved}" --json)
  set_tests_properties(cli.info_saved PROPERTIES FIXTURES_REQUIRED cli_saved
    PASS_REGULAR_EXPRESSION "\"name\": \"Body1\"")

  add_test(NAME cli.failed_expectation COMMAND mitcad-cli run "${scripts}/failing.json")
  set_tests_properties(cli.failed_expectation PROPERTIES WILL_FAIL TRUE)

  # Construction geometry and analysis (F5): datums checked through queries,
  # physical properties, measurement and comparison with a STEP file.
  foreach(script construction analysis holed_block)
    add_test(NAME cli.${script} COMMAND mitcad-cli run "${scripts}/${script}.json")
  endforeach()
  add_test(NAME cli.properties COMMAND mitcad-cli run "${scripts}/analysis.json" --properties)
  set_tests_properties(cli.properties PROPERTIES PASS_REGULAR_EXPRESSION
    "Physical properties:.*Body1 \\(F2\\.b0\\): Steel, density 7\\.85 g/cm3.*mass 0\\.376800 kg.*Body2 \\(F4\\.b0\\): Aluminum")
  add_test(NAME cli.properties_json
    COMMAND mitcad-cli info "${scripts}/v1_boss.mitcad" --json --properties --density 1)
  set_tests_properties(cli.properties_json PROPERTIES PASS_REGULAR_EXPRESSION
    "\"properties\": {.*\"density\": 1\\.0")
  set(top "F2:end(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})")
  set(bottom "F2:start(r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]})")
  add_test(NAME cli.measure
    COMMAND mitcad-cli measure "${scripts}/holed_block.json" "Body1/${top}" "Body1/${bottom}")
  set_tests_properties(cli.measure PROPERTIES PASS_REGULAR_EXPRESSION
    "area 1386\\.902.*Distance: 20\\.000000 mm.*Angle: 180\\.000000 deg")
  set(step "${CMAKE_CURRENT_BINARY_DIR}/holed-block.step")
  add_test(NAME cli.compare_step_file COMMAND mitcad-geomtool make holed-block "${step}")
  set_tests_properties(cli.compare_step_file PROPERTIES FIXTURES_SETUP cli_step)
  add_test(NAME cli.compare_step COMMAND mitcad-cli compare-step "${scripts}/holed_block.json"
    "${step}" --max-deviation 1e-4 --max-relative 1e-6)
  set_tests_properties(cli.compare_step PROPERTIES FIXTURES_REQUIRED cli_step
    PASS_REGULAR_EXPRESSION "Within the limits")
  # A different body exceeds the limit: exit status 1.
  add_test(NAME cli.compare_step_differs COMMAND mitcad-cli compare-step
    "${scripts}/construction.json" "${step}" --body F2.b0 --step-body 0 --max-deviation 0.01 --json)
  set_tests_properties(cli.compare_step_differs PROPERTIES FIXTURES_REQUIRED cli_step WILL_FAIL TRUE)
  # Data exchange: round trips through every format inside a script (files
  # in its working directory), then the export, import and import-f3d
  # commands on its design and on a small .f3d file.
  set(exchange "${CMAKE_CURRENT_BINARY_DIR}/exchange")
  file(MAKE_DIRECTORY "${exchange}")
  add_test(NAME cli.exchange
    COMMAND mitcad-cli run "${scripts}/exchange.json" --save "${exchange}/exchange.mitcad")
  set_tests_properties(cli.exchange PROPERTIES WORKING_DIRECTORY "${exchange}"
    FIXTURES_SETUP cli_exchange)
  add_test(NAME cli.export
    COMMAND mitcad-cli export "${exchange}/exchange.mitcad" "${exchange}/out.step"
            --bodies Bracket,F4.b0 --schema ap242 --unit in)
  set_tests_properties(cli.export PROPERTIES FIXTURES_REQUIRED cli_exchange
    FIXTURES_SETUP cli_exported PASS_REGULAR_EXPRESSION "\"bodies\":\\[\"F2\\.b0\",\"F4\\.b0\"\\]")
  add_test(NAME cli.import COMMAND mitcad-cli import "${exchange}/out.step"
                                   --save "${exchange}/imported.mitcad")
  set_tests_properties(cli.import PROPERTIES FIXTURES_REQUIRED cli_exported PASS_REGULAR_EXPRESSION
    "Bracket \\(F1\\.b0\\): volume 23946\\.350 mm3.*Bracket \\(2\\) \\(F1\\.b1\\): volume 47892\\.699 mm3")
  add_test(NAME cli.export_mesh
    COMMAND mitcad-cli export "${exchange}/exchange.mitcad" "${exchange}/out.stl"
            --refinement low --ascii)
  set_tests_properties(cli.export_mesh PROPERTIES FIXTURES_REQUIRED cli_exchange
    PASS_REGULAR_EXPRESSION "\"format\":\"stl\"")
  # 3MF for slicers (mitcad#13): the bodies as the parts of one object; the
  # Bracket's mesh (23946.35 mm3 after the script's change) within the
  # refinement's deviation.
  add_test(NAME cli.export_3mf
    COMMAND mitcad-cli export "${exchange}/exchange.mitcad" "${exchange}/out.3mf" --refinement high)
  set_tests_properties(cli.export_3mf PROPERTIES FIXTURES_REQUIRED cli_exchange PASS_REGULAR_EXPRESSION
    "\"format\":\"3mf\",\"meshes\":\\[{\"body\":\"F2\\.b0\",\"name\":\"Bracket\",\"triangles\":[0-9]+,\"volume\":2394[56]\\.")
  # USDZ for AR viewers (mitcad#123): the exchange design's shown bodies
  # as shaded meshes with their materials.
  add_test(NAME cli.export_usdz
    COMMAND mitcad-cli export "${exchange}/exchange.mitcad" "${exchange}/out.usdz" --refinement low)
  set_tests_properties(cli.export_usdz PROPERTIES FIXTURES_REQUIRED cli_exchange PASS_REGULAR_EXPRESSION
    "\"format\":\"usdz\",\"materials\":\\[[^]]+\\].*\"warnings\":\\[\\]")
  # The bodies in their components' coordinates (mitcad#19).
  add_test(NAME cli.export_component
    COMMAND mitcad-cli export "${exchange}/exchange.mitcad" "${exchange}/local-out.step"
            --bodies Bracket --coordinates component)
  set_tests_properties(cli.export_component PROPERTIES FIXTURES_REQUIRED cli_exchange
    PASS_REGULAR_EXPRESSION "\"bodies\":\\[\"F2\\.b0\"\\],\"error\":null,\"format\":\"step\"")
  add_test(NAME cli.export_sketch
    COMMAND mitcad-cli export-sketch "${exchange}/exchange.mitcad" Sketch1 "${exchange}/sketch.dxf" --r12)
  set_tests_properties(cli.export_sketch PROPERTIES FIXTURES_REQUIRED cli_exchange
    PASS_REGULAR_EXPRESSION "\"entities\":4")
  # Design files of version 3 (P12a): the exchange design's base features
  # converted into a project's B-rep store and back.
  add_test(NAME cli.project_v3
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DSOURCE=${exchange}/exchange.mitcad"
            "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/project-v3" -P "${CMAKE_CURRENT_SOURCE_DIR}/project-test.cmake")
  set_tests_properties(cli.project_v3 PROPERTIES FIXTURES_REQUIRED cli_exchange)
  # Version history (P12b): versions of the exchange design in a project
  # with history, an older version opened and restored; checked with the
  # system's git where it is installed.
  find_package(Git QUIET)
  add_test(NAME cli.version_history
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DSOURCE=${exchange}/exchange.mitcad"
            "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/version-history" "-DGIT=${GIT_EXECUTABLE}"
            -P "${CMAKE_CURRENT_SOURCE_DIR}/history-test.cmake")
  set_tests_properties(cli.version_history PROPERTIES FIXTURES_REQUIRED cli_exchange)
  # Comparison of versions (P12c): two versions of a block, a version and
  # the saved file, and two files, as text and JSON, with volumes and areas.
  add_test(NAME cli.diff
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/diff"
            -P "${CMAKE_CURRENT_SOURCE_DIR}/diff-test.cmake")
  # Remote repositories (P12 remote): the exchange design's project connected to a
  # bare repository, opened from it, pushed and fetched both ways; needs the
  # system's git.
  if(GIT_FOUND)
    add_test(NAME cli.remote
      COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DGIT=${GIT_EXECUTABLE}"
              "-DSOURCE=${exchange}/exchange.mitcad" "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/remote"
              -P "${CMAKE_CURRENT_SOURCE_DIR}/remote-test.cmake")
    set_tests_properties(cli.remote PROPERTIES FIXTURES_REQUIRED cli_exchange)
  endif()
  # A project's folders and designs (mitcad#144): made, listed, moved and
  # deleted as versions, every refusal, and with git a Cloud project's push.
  add_test(NAME cli.project_folders
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DGIT=${GIT_EXECUTABLE}"
            "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/project-folders" -P "${CMAKE_CURRENT_SOURCE_DIR}/folders-test.cmake")

  # .f3d bodies without history: Mitcad's own ASM test bodies in an .f3d.
  add_test(NAME cli.f3d_file COMMAND test_exchange --write-f3d "${exchange}/bodies.f3d")
  set_tests_properties(cli.f3d_file PROPERTIES FIXTURES_SETUP cli_f3d)
  add_test(NAME cli.import_f3d COMMAND mitcad-cli import-f3d "${exchange}/bodies.f3d" --bodies-only
                                       --save "${exchange}/bodies.mitcad")
  set_tests_properties(cli.import_f3d PROPERTIES FIXTURES_REQUIRED cli_f3d FIXTURES_SETUP cli_f3d_saved
    PASS_REGULAR_EXPRESSION
    "Imported 2 bodies of bodies\\.f3d.*Body1 \\(F1\\.b0\\): volume 1000\\.000 mm3.*Body2 \\(F2\\.b0\\): volume 6283\\.185 mm3")
  add_test(NAME cli.info_f3d COMMAND mitcad-cli info "${exchange}/bodies.mitcad")
  set_tests_properties(cli.info_f3d PROPERTIES FIXTURES_REQUIRED cli_f3d_saved PASS_REGULAR_EXPRESSION
    "F1 Base1 \\(base\\): ok.*Body1 \\(F1\\.b0\\): volume 1000\\.000 mm3.*Body2 \\(F2\\.b0\\): volume 6283\\.185 mm3")
  # Without --bodies-only the timeline import (T1): this file has no
  # timeline, so its stored bodies come in as one base feature, with the
  # report's warning about it as its import note (mitcad#170).
  add_test(NAME cli.import_f3d_timeline COMMAND mitcad-cli import-f3d "${exchange}/bodies.f3d")
  set_tests_properties(cli.import_f3d_timeline PROPERTIES FIXTURES_REQUIRED cli_f3d PASS_REGULAR_EXPRESSION
    "bodies\\.f3d: 0 timeline items.*F1 Stored bodies \\(base\\): warning: Nothing replayed: the file's bodies come in as they are \\(F1\\).*Body1 \\(F1\\.b0\\): volume 1000\\.000 mm3")
  # After an import whose geometry kernel hung (mitcad#82): the exit status
  # of the import (1 when every try hung, 0 when one finished) while the
  # threads the hang watchdog gave up still run, not a crash at exit.
  add_test(NAME cli.import_f3d_hang_exit
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DFILE=${exchange}/bodies.f3d"
            -P "${CMAKE_CURRENT_SOURCE_DIR}/hang-exit-test.cmake")
  set_tests_properties(cli.import_f3d_hang_exit PROPERTIES FIXTURES_REQUIRED cli_f3d)
  # Components of other documents (mitcad#196): a synthetic assembly that
  # places a part once and a sub-assembly (which places the part) twice,
  # with its reference folder next to it; then the same with the folder
  # named otherwise and without references.json: the documents are
  # missing (their components stay empty, the report names them) until the
  # folder is given with --references, where the files' GUIDs tell them.
  add_test(NAME cli.f3d_references_files
    COMMAND test_exchange --write-f3d-references "${exchange}/references")
  add_test(NAME cli.f3d_references_moved_files
    COMMAND test_exchange --write-f3d-references "${exchange}/references-moved" parts)
  set_tests_properties(cli.f3d_references_files cli.f3d_references_moved_files PROPERTIES
    FIXTURES_SETUP cli_f3d_references)
  add_test(NAME cli.import_f3d_references
    COMMAND mitcad-cli import-f3d "${exchange}/references/Assembly v1.f3d")
  set_tests_properties(cli.import_f3d_references PROPERTIES FIXTURES_REQUIRED cli_f3d_references
    PASS_REGULAR_EXPRESSION
    "3 of other documents \\(3 filled from their files\\).*documents referred to: 2 linked from their files, 0 not.*Part \\(version 2\\): linked, occurrences: 1, from [^\n]*Part v2\\.f3d \\(reference folder\\).*Holder \\(version 1\\): linked, occurrences: 2.*Placed bodies: 3.*center of mass \\[55\\.000, 5\\.000, 5\\.000\\].*center of mass \\[5\\.000, 55\\.000, 25\\.000\\].*center of mass \\[5\\.000, 105\\.000, 25\\.000\\]")
  add_test(NAME cli.import_f3d_references_missing
    COMMAND mitcad-cli import-f3d "${exchange}/references-moved/Assembly v1.f3d")
  set_tests_properties(cli.import_f3d_references_missing PROPERTIES FIXTURES_REQUIRED cli_f3d_references
    PASS_REGULAR_EXPRESSION
    "documents referred to: 0 linked from their files, 2 not.*the document of Inserted component \\(version 2\\): missing.*warning: components of other documents stay empty.*Placed bodies: 0")
  add_test(NAME cli.import_f3d_references_folder
    COMMAND mitcad-cli import-f3d "${exchange}/references-moved/Assembly v1.f3d"
            --references "${exchange}/references-moved/parts")
  set_tests_properties(cli.import_f3d_references_folder PROPERTIES FIXTURES_REQUIRED cli_f3d_references
    PASS_REGULAR_EXPRESSION
    "documents referred to: 2 linked from their files, 0 not.*\\(by its GUID\\).*Placed bodies: 3")
  # The same assembly in an .f3z package with the documents it refers to
  # (mitcad#229), whose manifests list the GUIDs of the documents they refer
  # to as well as their own, and whose root sorts first: each reference
  # comes from its own design of the package (by DesignDescription.json,
  # named from it; without it by the GUIDs, never the root or a design
  # being imported), the root is never imported for a reference, and the
  # part (referred to by the assembly and the holder) is imported once.
  add_test(NAME cli.f3d_references_package_files
    COMMAND test_exchange --write-f3d-package "${exchange}/package")
  set_tests_properties(cli.f3d_references_package_files PROPERTIES FIXTURES_SETUP cli_f3d_package)
  add_test(NAME cli.import_f3d_references_package
    COMMAND mitcad-cli import-f3d "${exchange}/package/Package.f3z")
  set_tests_properties(cli.import_f3d_references_package PROPERTIES FIXTURES_REQUIRED cli_f3d_package
    ENVIRONMENT "MITCAD_IMPORT_TRACE=1"
    PASS_REGULAR_EXPRESSION
    "import: the document urn:mitcad\\.test:fs\\.file:vf\\.part\\?version=2 again, imported before.*3 of other documents \\(3 filled from their files\\).*documents referred to: 2 linked from their files, 0 not.*Packaged part \\(version 2\\): linked, occurrences: 1, from [^\n]*Package\\.f3z \\(package\\).*Packaged holder \\(version 1\\): linked, occurrences: 2, from [^\n]*Package\\.f3z \\(package\\).*Placed bodies: 3.*center of mass \\[55\\.000, 5\\.000, 5\\.000\\].*center of mass \\[5\\.000, 55\\.000, 25\\.000\\].*center of mass \\[5\\.000, 105\\.000, 25\\.000\\]"
    FAIL_REGULAR_EXPRESSION
    "from [^\n]*!0a5e3b10\\);vf\\.part\\?version=2 from.*vf\\.part\\?version=2 from;refers back")
  add_test(NAME cli.import_f3d_references_package_plain
    COMMAND mitcad-cli import-f3d "${exchange}/package/Package-plain.f3z" --design 0a5e3b10)
  set_tests_properties(cli.import_f3d_references_package_plain PROPERTIES FIXTURES_REQUIRED cli_f3d_package
    ENVIRONMENT "MITCAD_IMPORT_TRACE=1"
    PASS_REGULAR_EXPRESSION
    "documents referred to: 2 linked from their files, 0 not.*occurrences: 1, from [^\n]*Package-plain\\.f3z \\(package\\).*Placed bodies: 3"
    FAIL_REGULAR_EXPRESSION
    "from [^\n]*!0a5e3b10\\);vf\\.part\\?version=2 from.*vf\\.part\\?version=2 from;refers back")

  # The .ipt import (mitcad#60): Mitcad's own test part (a cube and a
  # cylinder, inches, aluminium), saved and opened, and compared with a
  # STEP file of its bodies. Real part files under MITCAD_IPT_CORPUS are
  # imported and compared with their STEP references (ipt-corpus.cmake),
  # several at once; skipped without the corpus.
  add_test(NAME cli.ipt_file COMMAND test_exchange --write-ipt "${exchange}/part.ipt")
  set_tests_properties(cli.ipt_file PROPERTIES FIXTURES_SETUP cli_ipt)
  add_test(NAME cli.import_ipt COMMAND mitcad-cli import-ipt "${exchange}/part.ipt" --save "${exchange}/part.mitcad")
  set_tests_properties(cli.import_ipt PROPERTIES FIXTURES_REQUIRED cli_ipt FIXTURES_SETUP cli_ipt_saved
    PASS_REGULAR_EXPRESSION
    "Imported 2 bodies of part\\.ipt \\(2 solids, 2 valid; 0 not built\\).*part number: MITCAD-TEST-1; material: Aluminum \\(Mitcad's aluminum\\); units: in.*Body1 \\(F1\\.b0\\): valid solid, volume 1000\\.000 mm3.*Body2 \\(F2\\.b0\\): valid solid, volume 6283\\.185 mm3")
  add_test(NAME cli.info_ipt COMMAND mitcad-cli info "${exchange}/part.mitcad")
  set_tests_properties(cli.info_ipt PROPERTIES FIXTURES_REQUIRED cli_ipt_saved
    PASS_REGULAR_EXPRESSION "F1 Base1 \\(base\\): ok.*F2 Base2 \\(base\\): ok.*Body2 \\(F2\\.b0\\): volume 6283\\.185 mm3")
  add_test(NAME cli.export_ipt COMMAND mitcad-cli export "${exchange}/part.mitcad" "${exchange}/part.step")
  set_tests_properties(cli.export_ipt PROPERTIES FIXTURES_REQUIRED cli_ipt_saved FIXTURES_SETUP cli_ipt_step)
  add_test(NAME cli.import_ipt_reference
    COMMAND mitcad-cli import-ipt "${exchange}/part.ipt" --reference "${exchange}/part.step" --deviation)
  set_tests_properties(cli.import_ipt_reference PROPERTIES FIXTURES_REQUIRED cli_ipt_step PASS_REGULAR_EXPRESSION
    "reference part\\.step: 2 STEP solids, largest relative difference [0-9.e+-]+ \\(limit 1e-6\\), surface deviation [0-9.e+-]+ mm: pass")
  # A part with a design (mitcad#60, stages 2 and 3): the parameter, the
  # sketch and the extrusion replayed parametric, making the stored cube.
  add_test(NAME cli.ipt_design_file COMMAND test_exchange --write-ipt-design "${exchange}/designed.ipt")
  set_tests_properties(cli.ipt_design_file PROPERTIES FIXTURES_SETUP cli_ipt_design)
  # The replay's hang watchdog (--hang-limit): a try that hangs comparing
  # the final bodies is given up, and the next one compares by volume.
  add_test(NAME cli.import_ipt_hang_exit
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DFILE=${exchange}/designed.ipt"
            -DMODE=import-ipt -P "${CMAKE_CURRENT_SOURCE_DIR}/hang-exit-test.cmake")
  set_tests_properties(cli.import_ipt_hang_exit PROPERTIES FIXTURES_REQUIRED cli_ipt_design)
  add_test(NAME cli.import_ipt_design COMMAND mitcad-cli import-ipt "${exchange}/designed.ipt")
  set_tests_properties(cli.import_ipt_design PROPERTIES FIXTURES_REQUIRED cli_ipt_design PASS_REGULAR_EXPRESSION
    "Imported 1 bodies of designed\\.ipt.*parameters: 4 model, 1 user.*expressions: 5 translated, 5 agree.*2 timeline items: 2 parametric, 0 partial, 0 fallback, 0 skipped.*F1 Sketch1 \\(sketch\\): ok.*F2 Extrusion1 \\(extrude\\): ok")
  add_test(NAME ipt.corpus
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DRUNNER=$<TARGET_FILE:mitcad_run_parallel>"
            "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/ipt-corpus" -P "${CMAKE_CURRENT_SOURCE_DIR}/ipt-corpus.cmake")
  set_tests_properties(ipt.corpus PROPERTIES SKIP_REGULAR_EXPRESSION "ipt corpus: skipped")
  # The .iam import (mitcad#60, stage 4): a test project of assemblies saved
  # on another machine (mitcad_ipt::testassembly): parts found relative to
  # the assembly and by name, one missing, a sub-assembly, hidden, grounded
  # and suppressed occurrences; every placement checked against the one
  # the file displays and every placed part against its stored range box. Real assemblies under
  # MITCAD_IPT_CORPUS: iam-corpus.cmake.
  add_test(NAME cli.iam_files COMMAND test_exchange --write-iam "${exchange}/iam")
  set_tests_properties(cli.iam_files PROPERTIES FIXTURES_SETUP cli_iam)
  add_test(NAME cli.import_iam
    COMMAND mitcad-cli import-iam "${exchange}/iam/top.iam" --save "${exchange}/iam/top.mitcad")
  set_tests_properties(cli.import_iam PROPERTIES FIXTURES_REQUIRED cli_iam FIXTURES_SETUP cli_iam_saved
    PASS_REGULAR_EXPRESSION
    "Imported assembly top\\.iam: 7 occurrences placed \\(1 suppressed, 1 hidden, 1 of missing files, 0 not placed\\); 4 part files \\(3 imported, 1 missing, 0 failed; 3 identities confirmed, 0 differ\\), 1 sub-assemblies; units: mm.*files found: 1 by name, 1 missing, 3 relative to the assembly.*check: 7 of 7 placements agree with the displayed ones.*check: 5 of 5 parts. bodies lie within their stored range boxes")
  add_test(NAME cli.info_iam COMMAND mitcad-cli info "${exchange}/iam/top.mitcad")
  set_tests_properties(cli.info_iam PROPERTIES FIXTURES_REQUIRED cli_iam_saved
    PASS_REGULAR_EXPRESSION "F1 Base1 \\(base\\): ok.*Bodies: 5")
  # With --history, each part's design replayed: the cube's sketch and
  # extrusion.
  add_test(NAME cli.import_iam_history COMMAND mitcad-cli import-iam "${exchange}/iam/top.iam" --history)
  set_tests_properties(cli.import_iam_history PROPERTIES FIXTURES_REQUIRED cli_iam PASS_REGULAR_EXPRESSION
    "part [^\n]*cube\\.ipt [^\n]*imported: 1 bodies \\(1 valid, 0 not built\\); features 2 parametric")
  add_test(NAME cli.import_ipt_iam COMMAND mitcad-cli import-ipt "${exchange}/iam/sub/sub.iam")
  set_tests_properties(cli.import_ipt_iam PROPERTIES FIXTURES_REQUIRED cli_iam PASS_REGULAR_EXPRESSION
    "Imported assembly sub\\.iam: 2 occurrences placed.*1 part files \\(1 imported, 0 missing.*check: 2 of 2 placements agree.*check: 2 of 2 parts. bodies lie within their stored range boxes")
  add_test(NAME iam.corpus
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DRUNNER=$<TARGET_FILE:mitcad_run_parallel>"
            "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/iam-corpus" -P "${CMAKE_CURRENT_SOURCE_DIR}/iam-corpus.cmake")
  set_tests_properties(iam.corpus PROPERTIES SKIP_REGULAR_EXPRESSION "iam corpus: skipped")

  # FreeCAD .FCStd import: a small document made by the test (B-rep shapes
  # from mitcad-geomtool), imported against a reference, saved and opened.
  add_test(NAME cli.import_fcstd
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DGEOMTOOL=$<TARGET_FILE:mitcad-geomtool>"
            "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/fcstd" -P "${CMAKE_CURRENT_SOURCE_DIR}/fcstd-test.cmake")
  # FreeCAD documents under MITCAD_FCSTD_CORPUS (the reference models of
  # tools/freecad-export with their dumps, FreeCAD's examples): every file
  # imports, and those with a dump match it; skipped without the corpus.
  # The files are checked several at once (mitcad_run_parallel, mitcad#70).
  add_test(NAME freecad.corpus
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DRUNNER=$<TARGET_FILE:mitcad_run_parallel>"
            "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/fcstd-corpus" -P "${CMAKE_CURRENT_SOURCE_DIR}/fcstd-corpus.cmake")
  set_tests_properties(freecad.corpus PROPERTIES SKIP_REGULAR_EXPRESSION "FreeCAD corpus: skipped")

  # Recompute speed (P10): a generated model with the joins, patterns and
  # fillets that slowed down imported designs, saved, then opened with the
  # time of each feature reported (ctest -V, or Testing/Temporary/
  # LastTest.log). The time limit is about ten times the usual time (0.5 s
  # in a release build, 3.3 s in a Linux debug build), so only a large
  # slowdown fails: before T1d's patterns the model took 6.6 s in release.
  set(perf "${CMAKE_CURRENT_BINARY_DIR}/perf")
  file(MAKE_DIRECTORY "${perf}")
  add_test(NAME cli.perf_model COMMAND mitcad-cli run "${scripts}/perf_model.json" --save "${perf}/perf_model.mitcad")
  set_tests_properties(cli.perf_model PROPERTIES FIXTURES_SETUP cli_perf)
  if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(perf_limit 30)
    set(perf_join_limit 15)
    set(perf_holes_limit 10)
  else()
    set(perf_limit 5)
    set(perf_join_limit 5)
    set(perf_holes_limit 8)
  endif()
  add_test(NAME cli.perf_recompute COMMAND mitcad-cli info "${perf}/perf_model.mitcad" --timings)
  # The timed tests run alone (RUN_SERIAL), also under ctest -j.
  set_tests_properties(cli.perf_recompute PROPERTIES FIXTURES_REQUIRED cli_perf TIMEOUT ${perf_limit}
    RUN_SERIAL TRUE
    PASS_REGULAR_EXPRESSION "F12 Fillet2 \\(fillet\\): ok.*Recompute: 12 features in [0-9.]+ ms")
  # Joins near bodies they do not touch (T0d): a 90-copy pattern of pins on
  # a ring inside eight tubes, every pin within the tubes' boxes. The limit
  # is two to three times the usual time (5.9 s in a Linux debug build, 2 s
  # with the Windows build's RelWithDebInfo, 0.7 s in a Linux release
  # build); before T0d the model took 29.6 s in debug, 3.1 s in release.
  add_test(NAME cli.perf_join_model COMMAND mitcad-cli run "${scripts}/perf_join_pattern.json"
                                            --save "${perf}/perf_join.mitcad")
  set_tests_properties(cli.perf_join_model PROPERTIES FIXTURES_SETUP cli_perf_join)
  add_test(NAME cli.perf_join_recompute COMMAND mitcad-cli info "${perf}/perf_join.mitcad" --timings)
  set_tests_properties(cli.perf_join_recompute PROPERTIES FIXTURES_REQUIRED cli_perf_join
    TIMEOUT ${perf_join_limit} RUN_SERIAL TRUE
    PASS_REGULAR_EXPRESSION "F17 CircularPattern2 \\(circular_pattern\\): ok.*Recompute: 17 features in [0-9.]+ ms")

  # Perforated plates (mitcad#72): a 35 x 35 hole pattern, then cuts and a
  # join next to the holes. 2 s in a release build (6 s before mitcad#72),
  # 3 s in a Linux debug build; the limit is about three times the usual
  # time.
  add_test(NAME cli.perf_holes_model COMMAND mitcad-cli run "${scripts}/perf_holes.json"
                                             --save "${perf}/perf_holes.mitcad")
  set_tests_properties(cli.perf_holes_model PROPERTIES FIXTURES_SETUP cli_perf_holes)
  add_test(NAME cli.perf_holes_recompute COMMAND mitcad-cli info "${perf}/perf_holes.mitcad" --timings)
  set_tests_properties(cli.perf_holes_recompute PROPERTIES FIXTURES_REQUIRED cli_perf_holes
    TIMEOUT ${perf_holes_limit} RUN_SERIAL TRUE
    PASS_REGULAR_EXPRESSION "F11 Extrude5 \\(extrude\\): ok.*Recompute: 11 features in [0-9.]+ ms")

  # The result store (P7d): the perf model opened with an empty store fills
  # it (every result the store takes, however quick), opened again takes
  # them from it with the same bodies (the four sketches are evaluated), and
  # a copy with its last fillet edited evaluates only that after them.
  set(store "${perf}/store")
  set(volume "Body1 \\(F2\\.b0\\): volume 80967\\.896 mm3")
  add_test(NAME cli.store_clean COMMAND "${CMAKE_COMMAND}" -E rm -rf "${store}")
  set_tests_properties(cli.store_clean PROPERTIES FIXTURES_SETUP cli_store_clean)
  add_test(NAME cli.store_fill COMMAND mitcad-cli info "${perf}/perf_model.mitcad"
                                       --result-store "${store}" --persist-min-ms 0)
  set_tests_properties(cli.store_fill PROPERTIES FIXTURES_REQUIRED "cli_perf;cli_store_clean"
    FIXTURES_SETUP cli_store
    PASS_REGULAR_EXPRESSION "restored from store: 0, evaluated: 12; stored: 8 .*${volume}")
  add_test(NAME cli.store_hit COMMAND mitcad-cli info "${perf}/perf_model.mitcad"
                                      --result-store "${store}" --persist-min-ms 0)
  set_tests_properties(cli.store_hit PROPERTIES FIXTURES_REQUIRED cli_store
    PASS_REGULAR_EXPRESSION "restored from store: 8, evaluated: 4; stored: 0 .*F12 Fillet2 \\(fillet\\): ok.*${volume}")
  add_test(NAME cli.store_edit_file COMMAND mitcad-cli run "${scripts}/store_edit.json"
                                            --open "${perf}/perf_model.mitcad" --save "${perf}/perf_edit.mitcad")
  set_tests_properties(cli.store_edit_file PROPERTIES FIXTURES_REQUIRED cli_perf FIXTURES_SETUP cli_store_edited)
  add_test(NAME cli.store_edit COMMAND mitcad-cli info "${perf}/perf_edit.mitcad"
                                       --result-store "${store}" --persist-min-ms 0)
  set_tests_properties(cli.store_edit PROPERTIES FIXTURES_REQUIRED "cli_store;cli_store_edited"
    PASS_REGULAR_EXPRESSION "restored from store: 7, evaluated: 5; stored: 1 .*F12 Fillet2 \\(fillet\\): ok")
  # The caches' diagnostics (the cache query): where the results came from,
  # the process's memory and the store's files.
  add_test(NAME cli.store_diagnostics COMMAND mitcad-cli info "${perf}/perf_model.mitcad"
                                              --result-store "${store}" --diagnostics)
  set_tests_properties(cli.store_diagnostics PROPERTIES FIXTURES_REQUIRED cli_store
    PASS_REGULAR_EXPRESSION
    "Cache: {\"last_recompute\":{\"cached\":0,.*\"restored\":8},\"memory\":{.*\"process\":{\"heap\":[0-9]+,\"peak_resident\":[0-9]+,\"resident\":[1-9][0-9]*},\"store\":{.*\"disk\":{.*\"by_type\":\\[{.*\"type\":\"(rectangular|circular)_pattern\"")

  # Small UI gaps (P9): a full round and a chamfer as wide as a rib succeed
  # with a warning.
  add_test(NAME cli.p9_warnings COMMAND mitcad-cli run "${scripts}/p9_warnings.json")

  # Lofts with end conditions and rails (P2): analytic volumes, rails, and
  # a conditioned loft following an edit of the face it starts from.
  add_test(NAME cli.p2_loft_conditions COMMAND mitcad-cli run "${scripts}/p2_loft_conditions.json")

  # Decimal commas in values and expressions (mitcad#2): the same volumes
  # as with points, and expressions kept with points.
  add_test(NAME cli.decimal_commas COMMAND mitcad-cli run "${scripts}/decimal_commas.json")

  # The final render from the command line (mitcad#48): a design rendered by
  # the render worker in every format.
  add_test(NAME cli.render
    COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DWORKER=$<TARGET_FILE:mitcad-render>"
            "-DSCRIPT=${scripts}/render_scene.json" "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/render"
            -P "${CMAKE_CURRENT_SOURCE_DIR}/render-test.cmake")
  # Seconds normally; a hung GPU fails it in two minutes (mitcad#146).
  set_tests_properties(cli.render PROPERTIES TIMEOUT 120)

  # Joints between occurrences (mitcad#55): joints placed on the geometry
  # kernel's faces and edges, saved and opened again.
  set(joints "${CMAKE_CURRENT_BINARY_DIR}/joints")
  file(MAKE_DIRECTORY "${joints}")
  add_test(NAME cli.joints COMMAND mitcad-cli run "${scripts}/joints.json" --save "${joints}/joints.mitcad")
  set_tests_properties(cli.joints PROPERTIES FIXTURES_SETUP cli_joints)
  add_test(NAME cli.joints_info COMMAND mitcad-cli info "${joints}/joints.mitcad")
  set_tests_properties(cli.joints_info PROPERTIES FIXTURES_REQUIRED cli_joints PASS_REGULAR_EXPRESSION
    "F6 Joint1 \\(joint\\): ok.*F7 Joint2 \\(joint\\): ok")

  # Assembly constraints (mitcad#148): a pin inserted into a hole, on its
  # axis with the depth free and dragged along it, conflicting rows named,
  # saved and opened again.
  set(constraints "${CMAKE_CURRENT_BINARY_DIR}/constraints")
  file(MAKE_DIRECTORY "${constraints}")
  add_test(NAME cli.constraints COMMAND mitcad-cli run "${scripts}/constraints.json"
           --save "${constraints}/constraints.mitcad")
  set_tests_properties(cli.constraints PROPERTIES FIXTURES_SETUP cli_constraints)
  add_test(NAME cli.constraints_info COMMAND mitcad-cli info "${constraints}/constraints.mitcad")
  set_tests_properties(cli.constraints_info PROPERTIES FIXTURES_REQUIRED cli_constraints
    PASS_REGULAR_EXPRESSION "F6 Constraint2 \\(constraint_set\\): ok.*F8 Constraint3 \\(constraint_set\\): ok")

  # Component libraries (mitcad#64, mitcad#63): a small fastener library
  # made by the generator, fetched, its parts inserted, compared and
  # updated; needs git and Python.
  find_package(Python3 COMPONENTS Interpreter QUIET)
  if(GIT_FOUND AND Python3_Interpreter_FOUND)
    add_test(NAME cli.library
      COMMAND "${CMAKE_COMMAND}" "-DCLI=$<TARGET_FILE:mitcad-cli>" "-DGIT=${GIT_EXECUTABLE}"
              "-DPYTHON=${Python3_EXECUTABLE}"
              "-DGENERATOR=${PROJECT_SOURCE_DIR}/tools/libraries/make-fastener-library.py"
              "-DWORK=${CMAKE_CURRENT_BINARY_DIR}/library" -P "${CMAKE_CURRENT_SOURCE_DIR}/library-test.cmake")
  endif()
endif()

# Live updates (mitcad#89): mitcad-cli live test. Its tests need no broker:
# a password over plain text is refused, a broker that cannot be reached is
# reported (the client against brokers is tested in core.vcs).
target_sources(mitcad-cli PRIVATE live.cpp live.hpp)
if(BUILD_TESTING)
  add_test(NAME cli.live_insecure COMMAND mitcad-cli live test mqtt://127.0.0.1:9 --user someone)
  set_tests_properties(cli.live_insecure PROPERTIES ENVIRONMENT "MITCAD_MQTT_PASSWORD=secret"
    PASS_REGULAR_EXPRESSION "sent only over TLS")
  add_test(NAME cli.live_unreachable COMMAND mitcad-cli live test mqtt://127.0.0.1:9 --timeout 3)
  set_tests_properties(cli.live_unreachable PROPERTIES PASS_REGULAR_EXPRESSION
    "connect: failed \\([0-9]+ ms\\).*Warning: mqtt://127.0.0.1:9 is plain text.*Failed: ")
endif()

# Pattern at Points (mitcad#197): actual shared-component geometry in
# rotated nested occurrences, and the same definition opened in a fresh process.
if(BUILD_TESTING)
  set(point_components "${CMAKE_CURRENT_BINARY_DIR}/pattern_at_points_components.mitcad")
  add_test(NAME cli.pattern_at_points_components
    COMMAND mitcad-cli run "${scripts}/pattern_at_points_components.json" --save "${point_components}")
  set_tests_properties(cli.pattern_at_points_components PROPERTIES FIXTURES_SETUP cli_point_components)
  add_test(NAME cli.pattern_at_points_components_reopen
    COMMAND mitcad-cli run "${scripts}/pattern_at_points_components_reopen.json" --open "${point_components}")
  set_tests_properties(cli.pattern_at_points_components_reopen PROPERTIES FIXTURES_REQUIRED cli_point_components)
endif()

# Sheet Face (mitcad#206): analytic curved-hole/island geometry and rule
# lifecycle, including shared nested occurrences and independent copies.
if(BUILD_TESTING)
  foreach(sheet_case sheet_face_geometry sheet_face_components)
    set(sheet_file "${CMAKE_CURRENT_BINARY_DIR}/${sheet_case}.mitcad")
    add_test(NAME cli.${sheet_case}
      COMMAND mitcad-cli run "${scripts}/${sheet_case}.json" --save "${sheet_file}")
    set_tests_properties(cli.${sheet_case} PROPERTIES FIXTURES_SETUP cli_${sheet_case})
    add_test(NAME cli.${sheet_case}_reopen
      COMMAND mitcad-cli run "${scripts}/${sheet_case}_reopen.json" --open "${sheet_file}")
    set_tests_properties(cli.${sheet_case}_reopen PROPERTIES FIXTURES_REQUIRED cli_${sheet_case})
  endforeach()
endif()

# Folded sheet features (mitcad#207): independently calculated dimensions and
# volumes, both native Bend inputs, contour corners and persistence.
if(BUILD_TESTING)
  foreach(sheet_case sheet_flange_geometry sheet_fold_geometry sheet_fold_reliefs sheet_bend_coplanar
                     sheet_bend_positioned sheet_corner_round sheet_corner_chamfer
                     sheet_flange_components)
    set(sheet_file "${CMAKE_CURRENT_BINARY_DIR}/${sheet_case}.mitcad")
    add_test(NAME cli.${sheet_case}
      COMMAND mitcad-cli run "${scripts}/${sheet_case}.json" --save "${sheet_file}")
    set_tests_properties(cli.${sheet_case} PROPERTIES FIXTURES_SETUP cli_${sheet_case})
    add_test(NAME cli.${sheet_case}_reopen
      COMMAND mitcad-cli run "${scripts}/${sheet_case}_reopen.json" --open "${sheet_file}")
    set_tests_properties(cli.${sheet_case}_reopen PROPERTIES FIXTURES_REQUIRED cli_${sheet_case})
  endforeach()
endif()

# Sheet Unfold, Refold and Flat Pattern (mitcad#208): actual L, U and Z
# developments, partial opening, flat outlines, overlap refusal and persistence.
if(BUILD_TESTING)
  foreach(sheet_case sheet_unfold_l sheet_unfold_u sheet_unfold_z sheet_flat_overlap sheet_flat_components
                     sheet_cut_proof sheet_cut_partial)
    set(sheet_file "${CMAKE_CURRENT_BINARY_DIR}/${sheet_case}.mitcad")
    add_test(NAME cli.${sheet_case}
      COMMAND mitcad-cli run "${scripts}/${sheet_case}.json" --save "${sheet_file}")
    set_tests_properties(cli.${sheet_case} PROPERTIES FIXTURES_SETUP cli_${sheet_case})
    add_test(NAME cli.${sheet_case}_reopen
      COMMAND mitcad-cli run "${scripts}/${sheet_case}_reopen.json" --open "${sheet_file}")
    set_tests_properties(cli.${sheet_case}_reopen PROPERTIES FIXTURES_REQUIRED cli_${sheet_case})
  endforeach()
endif()

# Production sheet Cut and corner reliefs (mitcad#209): several sketches and
# contours across one and two bends, adjacency and rule edits, split bodies,
# removed domains, relief shapes and their refusals, each with a reopen pass.
if(BUILD_TESTING)
  foreach(sheet_case sheet_cut_multi sheet_cut_split sheet_corner_relief)
    set(sheet_file "${CMAKE_CURRENT_BINARY_DIR}/${sheet_case}.mitcad")
    add_test(NAME cli.${sheet_case}
      COMMAND mitcad-cli run "${scripts}/${sheet_case}.json" --save "${sheet_file}")
    set_tests_properties(cli.${sheet_case} PROPERTIES FIXTURES_SETUP cli_${sheet_case})
    add_test(NAME cli.${sheet_case}_reopen
      COMMAND mitcad-cli run "${scripts}/${sheet_case}_reopen.json" --open "${sheet_file}")
    set_tests_properties(cli.${sheet_case}_reopen PROPERTIES FIXTURES_REQUIRED cli_${sheet_case})
  endforeach()
endif()

# Flat Pattern DXF export (mitcad#210): contours, bend lines, layers and units
# read back from the files, rule edits, reopening, and the export-flat command
# (refused for an invalid pattern).
if(BUILD_TESTING)
  set(sheet_dxf "${CMAKE_CURRENT_BINARY_DIR}/sheet_flat_dxf")
  file(MAKE_DIRECTORY "${sheet_dxf}")
  add_test(NAME cli.sheet_flat_dxf
    COMMAND mitcad-cli run "${scripts}/sheet_flat_dxf.json" --save "${sheet_dxf}/sheet_flat_dxf.mitcad")
  set_tests_properties(cli.sheet_flat_dxf PROPERTIES WORKING_DIRECTORY "${sheet_dxf}"
    FIXTURES_SETUP cli_sheet_flat_dxf)
  add_test(NAME cli.sheet_flat_dxf_reopen
    COMMAND mitcad-cli run "${scripts}/sheet_flat_dxf_reopen.json" --open "${sheet_dxf}/sheet_flat_dxf.mitcad")
  set_tests_properties(cli.sheet_flat_dxf_reopen PROPERTIES WORKING_DIRECTORY "${sheet_dxf}"
    FIXTURES_REQUIRED cli_sheet_flat_dxf)
  add_test(NAME cli.export_flat
    COMMAND mitcad-cli export-flat "${sheet_dxf}/sheet_flat_dxf.mitcad" F3.b0 "${sheet_dxf}/flat-in.dxf"
            --unit in --tangents --json)
  set_tests_properties(cli.export_flat PROPERTIES FIXTURES_REQUIRED cli_sheet_flat_dxf
    PASS_REGULAR_EXPRESSION "\"entities\":8.*\"unit\":\"in\"")
  add_test(NAME cli.export_flat_invalid
    COMMAND mitcad-cli export-flat "${CMAKE_CURRENT_BINARY_DIR}/sheet_flat_overlap.mitcad" F3.b0
            "${sheet_dxf}/overlap.dxf")
  set_tests_properties(cli.export_flat_invalid PROPERTIES FIXTURES_REQUIRED cli_sheet_flat_overlap
    WILL_FAIL TRUE)
endif()

# Sheet-metal leftovers of mitcad#125: corner reliefs of bends without a common
# panel and of non-perpendicular bends, and Paste New of split pieces with their
# Flat Patterns, each with a reopen pass.
if(BUILD_TESTING)
  foreach(sheet_case sheet_corner_relief_paths sheet_corner_relief_angled sheet_copy_pieces)
    set(sheet_file "${CMAKE_CURRENT_BINARY_DIR}/${sheet_case}.mitcad")
    add_test(NAME cli.${sheet_case}
      COMMAND mitcad-cli run "${scripts}/${sheet_case}.json" --save "${sheet_file}")
    set_tests_properties(cli.${sheet_case} PROPERTIES FIXTURES_SETUP cli_${sheet_case})
    add_test(NAME cli.${sheet_case}_reopen
      COMMAND mitcad-cli run "${scripts}/${sheet_case}_reopen.json" --open "${sheet_file}")
    set_tests_properties(cli.${sheet_case}_reopen PROPERTIES FIXTURES_REQUIRED cli_${sheet_case})
  endforeach()
endif()

# mitcad-cli loads Qt's DLLs (mitcad-cli render): on Windows its tests find
# them through PATH, as app.unit does.
if(BUILD_TESTING AND WIN32)
  get_property(mitcad_cli_tests DIRECTORY PROPERTY TESTS)
  set_property(TEST ${mitcad_cli_tests} APPEND PROPERTY ENVIRONMENT_MODIFICATION
    "PATH=path_list_prepend:$<TARGET_FILE_DIR:Qt6::Core>")
endif()

# Headless MCP stdio server (mitcad#124), and the real CLI transport test.
target_sources(mitcad-cli PRIVATE mcp.cpp mcp.hpp mcp-connect.cpp mcp-connect.hpp
  "${app}/mcp/LocalEndpoint.cpp" "${app}/mcp/LocalEndpoint.hpp")
find_package(Threads REQUIRED)
target_link_libraries(mitcad-cli PRIVATE Qt6::Network Threads::Threads)
if(WIN32)
  target_sources(mitcad-cli PRIVATE "${app}/mcp/WindowsEndpointSecurity.cpp" "${app}/mcp/WindowsEndpointSecurity.hpp")
  target_link_libraries(mitcad-cli PRIVATE advapi32)
endif()
if(BUILD_TESTING)
  find_package(Python3 COMPONENTS Interpreter QUIET)
  if(Python3_Interpreter_FOUND)
    add_test(NAME cli.mcp
      COMMAND "${Python3_EXECUTABLE}" "${CMAKE_CURRENT_SOURCE_DIR}/mcp-test.py"
              "$<TARGET_FILE:mitcad-cli>" "${CMAKE_CURRENT_BINARY_DIR}/mcp")
    set_tests_properties(cli.mcp PROPERTIES TIMEOUT 180
      ENVIRONMENT "PYTHONDONTWRITEBYTECODE=1")
    if(WIN32)
      set_property(TEST cli.mcp APPEND PROPERTY ENVIRONMENT_MODIFICATION
        "PATH=path_list_prepend:$<TARGET_FILE_DIR:Qt6::Core>")
    endif()
  endif()
endif()
