# SPDX-License-Identifier: MIT
# BUILD on Mitcad's generated parts library (mitcad#376, mitcad#377)
# through mitcad-cli. The library is a folder made by
# tools/libraries/make-profile-library.py (a git repository with
# mitcad-library.json) named by MITCAD_PROFILES_LIBRARY; it is not in the
# repository, and the test is skipped without it. Nothing goes over the
# network.
#
# 1. The library fetched from its folder.
# 2. One profile of each system and slot width (I-type slot 5, 6 and 8,
#    B-type slot 6, 8 and 10) placed 500 mm long: build_parts lists it, it
#    has every slot frame its build.frames lists, and its cut-list mass
#    agrees with build.mass_per_metre within 2 %.
# 3. On a frame of two 30x30 B8 profiles: a rail lying along a slot on its
#    base frame (its box checked) with its carriage, a bar
#    with a linear bearing, a lead screw with its nut, a pulley and a gear.
#    Each mate's joint is driven (drive_joint) and the part moves by the
#    driven value: the carriage, the bearing and the nut along their axes,
#    the pulley and the gear a quarter turn (the occurrences' placements of
#    the two reports compared).
#
# cmake -DCLI=<mitcad-cli> -DWORK=<folder> -P build-profiles-test.cmake

foreach(var CLI WORK)
  if(NOT DEFINED ${var})
    message(FATAL_ERROR "build-profiles-test.cmake needs -D${var}=...")
  endif()
endforeach()

set(library "$ENV{MITCAD_PROFILES_LIBRARY}")
if(library STREQUAL "")
  message("build profiles: skipped (set MITCAD_PROFILES_LIBRARY to the generated parts library's folder)")
  return()
endif()
if(NOT EXISTS "${library}/mitcad-library.json")
  message(FATAL_ERROR "MITCAD_PROFILES_LIBRARY is ${library}, which has no mitcad-library.json")
endif()
file(TO_CMAKE_PATH "${library}" url)
set(ENV{MITCAD_LIBRARIES_DIR} "${WORK}/cache")
file(REMOVE_RECURSE "${WORK}")
file(MAKE_DIRECTORY "${WORK}")

# mitcad-cli with its standard output in `out` and both in `all`.
function(cli)
  execute_process(COMMAND "${CLI}" ${ARGN} RESULT_VARIABLE status OUTPUT_VARIABLE out ERROR_VARIABLE err)
  if(NOT status EQUAL 0)
    message(FATAL_ERROR "mitcad-cli ${ARGN} failed (${status}):\n${out}${err}")
  endif()
  set(out "${out}" PARENT_SCOPE)
  set(all "${out}${err}" PARENT_SCOPE)
endfunction()

function(same actual wanted what)
  if(NOT "${actual}" STREQUAL "${wanted}")
    message(FATAL_ERROR "${what}: '${actual}', not '${wanted}'")
  endif()
endfunction()

# A JSON number's text (as string(JSON) gives it: 415, -15.000000000000002,
# 1.2246467991473532e-16) as an integer of millionths, cut towards zero.
function(millionths text out_var)
  if(NOT text MATCHES "^(-?)([0-9]+)(\\.([0-9]*))?([eE]([-+]?)0*([0-9]+))?$")
    message(FATAL_ERROR "not a number: '${text}'")
  endif()
  set(sign "${CMAKE_MATCH_1}")
  set(digits "${CMAKE_MATCH_2}${CMAKE_MATCH_4}")
  string(LENGTH "${CMAKE_MATCH_4}" places)
  set(exponent 0)
  if("${CMAKE_MATCH_6}" STREQUAL "-")
    set(exponent "-${CMAKE_MATCH_7}")
  elseif(NOT "${CMAKE_MATCH_7}" STREQUAL "")
    set(exponent "${CMAKE_MATCH_7}")
  endif()
  math(EXPR shift "(${exponent}) - ${places} + 6")
  if(shift GREATER 12)
    message(FATAL_ERROR "too large a number: '${text}'")
  elseif(shift GREATER_EQUAL 0)
    string(REPEAT "0" ${shift} zeros)
    string(APPEND digits "${zeros}")
  else()
    math(EXPR drop "0 - (${shift})")
    string(LENGTH "${digits}" length)
    if(drop GREATER_EQUAL length)
      set(digits 0)
    else()
      math(EXPR keep "${length} - ${drop}")
      string(SUBSTRING "${digits}" 0 ${keep} digits)
    endif()
  endif()
  string(REGEX REPLACE "^0+([0-9])" "\\1" digits "${digits}")
  if(digits STREQUAL "0")
    set(sign "")
  endif()
  set(${out_var} "${sign}${digits}" PARENT_SCOPE)
endfunction()

# 1. The library.
cli(library fetch "${library}")
if(NOT all MATCHES "Fetched library")
  message(FATAL_ERROR "the fetch:\n${all}")
endif()
file(READ "${library}/mitcad-library.json" manifest)
string(JSON library_id GET "${manifest}" id)
string(JSON components LENGTH "${manifest}" components)
math(EXPR last_component "${components} - 1")
set(component_ids "")
foreach(i RANGE ${last_component})
  string(JSON id GET "${manifest}" components ${i} id)
  list(APPEND component_ids "${id}")
endforeach()

# The index of a component in the manifest.
function(component_index wanted out_var)
  list(FIND component_ids "${wanted}" i)
  if(i LESS 0)
    message(FATAL_ERROR "the library has no ${wanted}")
  endif()
  set(${out_var} ${i} PARENT_SCOPE)
endfunction()

# A part of the library as build_place takes it.
function(part id out_var)
  component_index("${id}" index)
  set(${out_var} "{\"library\": \"${library_id}\", \"url\": \"${url}\", \"rev\": \"HEAD\", \"component\": \"${id}\"}"
      PARENT_SCOPE)
endfunction()

# 2. A profile of each system and slot width, 500 mm long.
foreach(entry IN ITEMS "p20x20-i5|I|5" "p30x30l-i6|I|6" "p40x40l-i8|I|8" "p20x20-b6|B|6" "p30x30-b8|B|8"
                       "p40x40-b10|B|10")
  string(REPLACE "|" ";" fields "${entry}")
  list(GET fields 0 id)
  list(GET fields 1 kind)
  list(GET fields 2 width)
  component_index("${id}" index)
  string(JSON system_kind GET "${manifest}" components ${index} build system type)
  string(JSON system_slot GET "${manifest}" components ${index} build system slot)
  same("${system_kind}" "${kind}" "the system of ${id}")
  if(NOT system_slot MATCHES "^${width}(\\.0*)?$")
    message(FATAL_ERROR "the slot width of ${id}: ${system_slot}, not ${width}")
  endif()
  part("${id}" spec)
  set(steps "{\"cmd\": \"build_place\", \"part\": ${spec}, \"at\": {\"point\": [0, 0, 0]}, \"length\": 500},
  {\"expect\": {\"feature\": \"F1\", \"status\": \"ok\"}},
  {\"expect\": {\"query\": {\"query\": \"build_parts\"},
               \"result\": {\"parts\": [{\"component\": \"C1\", \"role\": \"profile\", \"part\": \"F1\", \"length\": 500}]}}}")
  # Its slot frames, as its build block lists them.
  string(JSON frames LENGTH "${manifest}" components ${index} build frames)
  math(EXPR last_frame "${frames} - 1")
  set(slots 0)
  foreach(i RANGE ${last_frame})
    string(JSON frame_kind GET "${manifest}" components ${index} build frames ${i} kind)
    string(JSON frame_name GET "${manifest}" components ${index} build frames ${i} name)
    if(frame_kind STREQUAL "slot")
      math(EXPR slots "${slots} + 1")
      string(APPEND steps ",
  {\"expect\": {\"query\": {\"query\": \"joint_frame\", \"occurrence\": \"O1\",
                          \"geometry\": {\"mount\": \"F1:mount(${frame_name})\"}},
               \"result\": {\"component\": \"C1\"}}}")
    endif()
  endforeach()
  if(slots EQUAL 0)
    message(FATAL_ERROR "${id} lists no slot frames")
  endif()
  # The cut list's mass of the 500 mm piece: half the mass per metre,
  # within 2 % (the script's tolerance is relative to the value, or to 1
  # below 1).
  string(JSON per_metre GET "${manifest}" components ${index} build mass_per_metre)
  if(NOT per_metre MATCHES "^([0-9]+(\\.[0-9]*)?) *kg$")
    message(FATAL_ERROR "the mass per metre of ${id}: '${per_metre}'")
  endif()
  millionths("${CMAKE_MATCH_1}" milligrams)
  math(EXPR half "${milligrams} / 2")
  if(half LESS 1000000)
    set(tolerance "${milligrams}e-8")
  else()
    set(tolerance 0.02)
  endif()
  string(APPEND steps ",
  {\"expect\": {\"query\": {\"query\": \"build_cut_list\"},
               \"result\": {\"rows\": [{\"length\": 500, \"mass\": ${half}e-6}]}, \"tolerance\": ${tolerance}}}")
  file(WRITE "${WORK}/${id}.json" "[\n  ${steps}\n]\n")
  cli(run "${WORK}/${id}.json")
  message("${id}: ${system_kind}${system_slot}, ${slots} slot frames, ${per_metre} per metre")
endforeach()

# 3. The linear parts on a frame of two 30x30 B8 profiles: P1 (O1, F1)
#    grounded along +Z, 500 long; P2 (O2, F2, joint F3) on its x+ slot
#    400 up, 400 long. Each part after them is a derived part and its
#    joint: the rail (O3, F4/F5) 400 long lying on P2's y+ slot by its
#    base frame, along all of P2, its carriage
#    (O4, F6/F7) on the rail's frame; the bar (O5, F8/F9) on P1's end B,
#    its bearing (O6, F10/F11); the lead screw (O7, F12/F13) on P2's end B,
#    its nut (O8, F14/F15) and the pulley (O9, F16/F17); the gear (O10,
#    F18/F19) on the bar.
foreach(id p30x30-b8 rail-hgw15 carriage-hgw15ca bar-hex-sw19 lm10uu screw-tr8x1-5 nut-tr8x1-5-hexagonal-aluminium-beta
           pulley-gt2-20t-w6-b8 gear-m1-20t-8)
  string(REPLACE "-" "_" var "${id}")
  part("${id}" ${var})
endforeach()
function(on occurrence feature frame out_var)
  set(${out_var} "{\"occurrence\": \"${occurrence}\", \"geometry\": {\"mount\": \"${feature}:mount(${frame})\"}}"
      PARENT_SCOPE)
endfunction()
on(O1 F1 slot_x+_1 at_p2)
on(O2 F2 slot_y+_1 at_rail)
string(REPLACE "}}" "}, \"offset\": 400}" at_p2 "${at_p2}")
on(O3 F4 rail at_carriage)
on(O1 F1 end_b at_bar)
on(O5 F8 shaft at_bearing)
on(O2 F2 end_b at_screw)
on(O7 F12 shaft at_nut)
on(O7 F12 shaft at_pulley)
on(O5 F8 shaft at_gear)
set(parts_expected "")
foreach(row "C1|F1|null|profile" "C2|F2|\"F3\"|profile" "C3|F4|\"F5\"|linear" "C4|F6|\"F7\"|linear"
            "C5|F8|\"F9\"|linear" "C6|F10|\"F11\"|linear" "C7|F12|\"F13\"|linear" "C8|F14|\"F15\"|linear"
            "C9|F16|\"F17\"|linear" "C10|F18|\"F19\"|linear")
  string(REPLACE "|" ";" fields "${row}")
  list(GET fields 0 c)
  list(GET fields 1 f)
  list(GET fields 2 j)
  list(GET fields 3 role)
  if(NOT parts_expected STREQUAL "")
    string(APPEND parts_expected ", ")
  endif()
  string(APPEND parts_expected "{\"component\": \"${c}\", \"part\": \"${f}\", \"joint\": ${j}, \"role\": \"${role}\"}")
endforeach()
file(WRITE "${WORK}/frame.json" "[
  {\"cmd\": \"build_place\", \"part\": ${p30x30_b8}, \"at\": {\"point\": [0, 0, 0]}, \"length\": 500},
  {\"cmd\": \"build_place\", \"part\": {\"source\": \"S1\"}, \"at\": ${at_p2}, \"length\": 400},
  {\"cmd\": \"build_place\", \"part\": ${rail_hgw15}, \"at\": ${at_rail}, \"length\": 400},
  {\"cmd\": \"build_place\", \"part\": ${carriage_hgw15ca}, \"at\": ${at_carriage}},
  {\"cmd\": \"build_place\", \"part\": ${bar_hex_sw19}, \"at\": ${at_bar}, \"length\": 300},
  {\"cmd\": \"build_place\", \"part\": ${lm10uu}, \"at\": ${at_bearing}},
  {\"cmd\": \"build_place\", \"part\": ${screw_tr8x1_5}, \"at\": ${at_screw}, \"length\": 300},
  {\"cmd\": \"build_place\", \"part\": ${nut_tr8x1_5_hexagonal_aluminium_beta}, \"at\": ${at_nut}},
  {\"cmd\": \"build_place\", \"part\": ${pulley_gt2_20t_w6_b8}, \"at\": ${at_pulley}},
  {\"cmd\": \"build_place\", \"part\": ${gear_m1_20t_8}, \"at\": ${at_gear}},
  {\"comment\": \"A revolute mate is placed with its turn free, where the solver leaves it (the pulley a quarter turn off its frame's alignment): the pulley and the gear are driven to 0 first, so that the quarter turn below is measured from there.\"},
  {\"cmd\": \"drive_joint\", \"joint\": \"F17\", \"values\": {\"rz\": 0}},
  {\"cmd\": \"drive_joint\", \"joint\": \"F19\", \"values\": {\"rz\": 0}},
  {\"expect\": {\"query\": {\"query\": \"build_parts\"}, \"result\": {\"parts\": [${parts_expected}]}}},
  {\"comment\": \"P2 runs along +X from x = 15 to 415, centred on y = 0, z = 400; its y+ slot faces -Y on y = -15 (the profile turned onto P1's x+ slot). The 15 x 15 rail lies flat on that face along all of P2.\"},
  {\"expect\": {\"occurrence\": \"O3\", \"body\": \"F4.b0\",
               \"bbox\": {\"min\": [15, -30, 392.5], \"max\": [415, -15, 407.5]}, \"tolerance\": 0.5}},
  {\"expect\": {\"feature\": \"F7\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F11\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F15\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F17\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F19\", \"status\": \"ok\"}}
]
")
cli(run "${WORK}/frame.json" --save "${WORK}/frame.mitcad" --json)
set(before "${out}")

# Each mate's joint driven: the carriage 120 mm along the rail, the
# bearing 80 mm along the bar, the nut 30 mm along the screw, the pulley
# and the gear a quarter turn from 0 (where the build left them).
file(WRITE "${WORK}/drive.json" "[
  {\"cmd\": \"drive_joint\", \"joint\": \"F7\", \"values\": {\"tx\": 120}},
  {\"cmd\": \"drive_joint\", \"joint\": \"F11\", \"values\": {\"tz\": 80}},
  {\"cmd\": \"drive_joint\", \"joint\": \"F15\", \"values\": {\"tz\": 30}},
  {\"cmd\": \"drive_joint\", \"joint\": \"F17\", \"values\": {\"rz\": 1.5707963267948966}},
  {\"cmd\": \"drive_joint\", \"joint\": \"F19\", \"values\": {\"rz\": 1.5707963267948966}},
  {\"expect\": {\"feature\": \"F7\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F11\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F15\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F17\", \"status\": \"ok\"}},
  {\"expect\": {\"feature\": \"F19\", \"status\": \"ok\"}}
]
")
cli(run "${WORK}/drive.json" --open "${WORK}/frame.mitcad" --json)
set(after "${out}")

# An occurrence's placement in the design from a report: its rotation's
# nine entries and its translation, in millionths.
function(placement report occurrence rotation_var translation_var)
  string(JSON count LENGTH "${report}" components occurrences)
  math(EXPR last "${count} - 1")
  foreach(i RANGE ${last})
    string(JSON uid GET "${report}" components occurrences ${i} uid)
    if(uid STREQUAL occurrence)
      set(rotation "")
      set(translation "")
      foreach(r 0 1 2)
        foreach(c 0 1 2 3)
          string(JSON v GET "${report}" components occurrences ${i} world ${r} ${c})
          millionths("${v}" m)
          if(c EQUAL 3)
            list(APPEND translation "${m}")
          else()
            list(APPEND rotation "${m}")
          endif()
        endforeach()
      endforeach()
      set(${rotation_var} "${rotation}" PARENT_SCOPE)
      set(${translation_var} "${translation}" PARENT_SCOPE)
      return()
    endif()
  endforeach()
  message(FATAL_ERROR "the report has no occurrence ${occurrence}")
endfunction()

# Checks how an occurrence moved between the reports: `distance` (mm) its
# origin travelled, or `any`; `turn` the rotation between its placements,
# by the sum of the products of their rotations' entries (3 unturned, 1 a
# quarter turn about any axis), or `any`.
function(moved occurrence what distance turn)
  placement("${before}" ${occurrence} r0 t0)
  placement("${after}" ${occurrence} r1 t1)
  if(NOT distance STREQUAL "any")
    set(square 0)
    foreach(i 0 1 2)
      list(GET t0 ${i} a)
      list(GET t1 ${i} b)
      math(EXPR square "${square} + ((${b}) - (${a})) * ((${b}) - (${a}))")
    endforeach()
    # Within 0.01 mm: |s - d^2| <= 2 d 0.01 mm, in millionths squared.
    math(EXPR wanted "${distance} * 1000000 * ${distance} * 1000000")
    math(EXPR tolerance "2 * ${distance} * 1000000 * 10000 + 100000000")
    math(EXPR off "${square} - ${wanted}")
    if(off LESS 0)
      math(EXPR off "0 - ${off}")
    endif()
    if(off GREATER tolerance)
      message(FATAL_ERROR "${what} (${occurrence}) moved by sqrt(${square}) millionths of a mm, not ${distance} mm\n"
                          "before: ${t0}\nafter: ${t1}")
    endif()
  endif()
  if(NOT turn STREQUAL "any")
    set(sum 0)
    foreach(i RANGE 8)
      list(GET r0 ${i} a)
      list(GET r1 ${i} b)
      math(EXPR sum "${sum} + (${a}) * (${b})")
    endforeach()
    if(turn STREQUAL "none")
      set(wanted 3000000000000)
    else()
      set(wanted 1000000000000)
    endif()
    math(EXPR off "${sum} - ${wanted}")
    if(off LESS 0)
      math(EXPR off "0 - ${off}")
    endif()
    if(off GREATER 10000000000)
      message(FATAL_ERROR "${what} (${occurrence}) turned wrongly: ${sum} (in 1e-12), not ${wanted}\n"
                          "before: ${r0}\nafter: ${r1}")
    endif()
  endif()
  message("${what} (${occurrence}): moved as driven")
endfunction()

moved(O3 "the rail" 0 none)
moved(O4 "the carriage" 120 none)
moved(O6 "the bearing" 80 none)
# The nut's turn about the screw is free (a cylindrical joint): only its
# travel is driven; its bore's axis runs through its origin.
moved(O8 "the nut" 30 any)
# The pulley's and the gear's bore axes run through their origins.
moved(O9 "the pulley" 0 quarter)
moved(O10 "the gear" 0 quarter)
