# SPDX-License-Identifier: MIT
# The BUILD parts of Mitcad's parts library (mitcad#375) through
# mitcad-cli: the generator
# (tools/libraries/make-profile-library.py) on supplier files made in this
# test (two slot profiles and an angle bracket written as STEP, nothing of
# a real supplier's) and on a panel, a shaft and a lead screw modelled from
# their dimensions (the screw with its thread, mitcad#449), with one part
# whose file is missing, a row with its own supplier file and a file left
# out with its reason; the library checked and recorded as v1.0.0,
# its manifest's BUILD fields and frames, a profile's design cut to another
# length with end B following it, the library fetched, searched by role,
# system and part number, and the connectors that fit a junction. Nothing
# goes over the network.
#
# cmake -DCLI=<mitcad-cli> -DGIT=<git> -DPYTHON=<python3> -DGENERATOR=<make-profile-library.py>
#       -DWORK=<folder> -P profile-library-test.cmake

foreach(var CLI GIT PYTHON GENERATOR WORK)
  if(NOT DEFINED ${var})
    message(FATAL_ERROR "profile-library-test.cmake needs -D${var}=...")
  endif()
endforeach()

set(library "${WORK}/mitcad-parts")
set(source "${WORK}/supplier")
set(groups "${WORK}/groups")
set(ENV{MITCAD_LIBRARIES_DIR} "${WORK}/cache")
set(ENV{GIT_CONFIG_NOSYSTEM} 1)
set(ENV{GIT_CONFIG_GLOBAL} "${WORK}/gitconfig")

function(cli expect)
  execute_process(COMMAND "${CLI}" ${ARGN} RESULT_VARIABLE status OUTPUT_VARIABLE out ERROR_VARIABLE err)
  if(expect STREQUAL "ok" AND NOT status EQUAL 0)
    message(FATAL_ERROR "mitcad-cli ${ARGN} failed (${status}):\n${out}${err}")
  elseif(expect STREQUAL "fail" AND NOT status EQUAL 1)
    message(FATAL_ERROR "mitcad-cli ${ARGN} should fail with 1 (${status}):\n${out}${err}")
  endif()
  set(out "${out}${err}" PARENT_SCOPE)
endfunction()

function(expect pattern what)
  if(NOT out MATCHES "${pattern}")
    message(FATAL_ERROR "${what}:\n${out}")
  endif()
endfunction()

function(same actual wanted what)
  if(NOT "${actual}" STREQUAL "${wanted}")
    message(FATAL_ERROR "${what}: '${actual}', not '${wanted}'")
  endif()
endfunction()

file(REMOVE_RECURSE "${WORK}")
file(MAKE_DIRECTORY "${WORK}" "${source}" "${groups}")
file(WRITE "${WORK}/gitconfig" "")

# 1. The supplier's files: made with mitcad-cli and written as STEP.
#    p20x20: a 20 x 20 square, 100 long, with a 5.2 mm wide, 6 mm deep slot
#    in each side and a 4.2 mm core bore. p20x40: 20 x 40, 60 long, two
#    slots on each long side, one on the top, the bottom closed, two bores,
#    turned to run along +Y (the group file's place turns it back).
#    bracket: an angle of two 3 mm legs, 30 x 20. rail: 15 x 12, 100
#    long, with two counterbored holes from the top (4.5 mm, counterbore
#    7.5 mm 5.3 mm deep, through) 20 mm from end A and 60 mm apart.
function(make_step name script)
  file(WRITE "${WORK}/${name}.json" "${script}")
  cli(ok run "${WORK}/${name}.json" --save "${WORK}/${name}.mitcad")
  cli(ok export "${WORK}/${name}.mitcad" "${source}/${name}.step")
endfunction()

function(box corner length width height operation out_var)
  set(${out_var} "{\"cmd\": \"add_feature\", \"def\": {\"type\": \"box\", \"plane\": \"xy\", \"corner\": [${corner}],
    \"length\": ${length}, \"width\": ${width}, \"height\": ${height}, \"operation\": \"${operation}\"}}" PARENT_SCOPE)
endfunction()

function(bore center height out_var)
  set(${out_var} "{\"cmd\": \"add_feature\", \"def\": {\"type\": \"cylinder\", \"plane\": \"xy\", \"center\": [${center}],
    \"diameter\": 4.2, \"height\": ${height}, \"operation\": \"cut\"}}" PARENT_SCOPE)
endfunction()

box("-10, -10" 20 20 100 new_body body)
box("4, -2.6" 6 5.2 100 cut right)
box("-10, -2.6" 6 5.2 100 cut left)
box("-2.6, 4" 5.2 6 100 cut top)
box("-2.6, -10" 5.2 6 100 cut bottom)
bore("0, 0" 100 core)
make_step(p20x20 "[${body}, ${right}, ${left}, ${top}, ${bottom}, ${core}, {\"expect\": {\"bodies\": 1}}]")

box("-10, -20" 20 40 60 new_body body)
box("4, -12.6" 6 5.2 60 cut right1)
box("4, 7.4" 6 5.2 60 cut right2)
box("-10, -12.6" 6 5.2 60 cut left1)
box("-10, 7.4" 6 5.2 60 cut left2)
box("-2.6, 14" 5.2 6 60 cut top)
bore("0, -10" 60 core1)
bore("0, 10" 60 core2)
set(along_y "{\"cmd\": \"add_feature\", \"def\": {\"type\": \"move\", \"bodies\": [\"F1.b0\"],
  \"transform\": {\"type\": \"rotate\", \"axis\": \"x\", \"angle\": \"-90 deg\"}}}")
make_step(p20x40 "[${body}, ${right1}, ${right2}, ${left1}, ${left2}, ${top}, ${core1}, ${core2}, ${along_y},
  {\"expect\": {\"bodies\": 1}}]")

box("0, 0" 30 20 3 new_body leg_a)
box("0, 0" 3 20 30 join leg_b)
make_step(bracket "[${leg_a}, ${leg_b}, {\"expect\": {\"bodies\": 1}}]")
# The bracket's black row has a file of its own; a copy of p20x20 is left
# out with its reason.
file(COPY_FILE "${source}/bracket.step" "${source}/bracket-black.step")
file(COPY_FILE "${source}/p20x20.step" "${source}/p20x20-copy.step")

box("-7.5, -6" 15 12 100 new_body rail)
make_step(rail "[${rail}, {\"cmd\": \"add_feature\", \"def\": {\"type\": \"hole\",
    \"placement\": {\"type\": \"face\", \"body\": \"F1.b0\", \"face\": \"F1:side2\", \"points\": [[0, 6, 20], [0, 6, 80]]},
    \"diameter\": 4.5, \"kind\": {\"type\": \"counterbore\", \"diameter\": 7.5, \"depth\": 5.3},
    \"extent\": {\"type\": \"through_all\"}}},
  {\"expect\": {\"total_volume\": 17318.5885534, \"tolerance\": 1e-5}}]")

# 2. The group files (read in the order of their names): profiles as an
#    object with categories, the other parts as a list.
set(i5 "{\"type\": \"I\", \"slot\": 5, \"grid\": 20}")
set(length "{\"parameter\": \"L\", \"min\": \"10 mm\", \"max\": \"3000 mm\", \"stock\": [\"3000 mm\"]}")
file(WRITE "${groups}/a-profiles.json" "{
  \"categories\": [{\"id\": \"profiles\", \"name\": \"Slot profiles\"}],
  \"parts\": [
    {\"id\": \"p20x20-i5\", \"name\": \"Profile 20x20 I-type slot 5\", \"category\": \"profiles\",
     \"source\": \"p20x20.step\", \"make\": \"section\", \"supplier\": \"Test Supplier\",
     \"part_number\": \"TEST-P20\", \"supplier_url\": \"https://example.org/p20\",
     \"designation\": \"Profile 20x20 I5 L={L}\",
     \"build\": {\"role\": \"profile\", \"system\": ${i5}, \"length\": ${length},
               \"slot\": {\"depth\": \"6 mm\", \"inner_width\": \"5.2 mm\"},
               \"frames\": [{\"name\": \"slot_x+_1\", \"kind\": \"slot\", \"accepts\": [\"connector\"]}]},
     \"frames\": {\"auto\": \"profile\"}},
    {\"id\": \"p20x40-i5\", \"name\": \"Profile 20x40 I-type slot 5\", \"category\": \"profiles\",
     \"source\": \"p20x40.step\", \"make\": \"section\", \"supplier\": \"Test Supplier\",
     \"part_number\": \"TEST-P2040\", \"place\": {\"z\": [0, 1, 0], \"x\": [1, 0, 0]},
     \"build\": {\"role\": \"profile\", \"system\": ${i5}, \"length\": ${length},
               \"slot\": {\"depth\": \"6 mm\"}},
     \"frames\": {\"auto\": \"profile\", \"add\": [{\"name\": \"mid\", \"kind\": \"face\",
                \"origin\": [10, 0, \"L / 2\"], \"z\": [0, 0, 1], \"x\": [1, 0, 0]}]}},
    {\"id\": \"p-missing\", \"name\": \"A profile without its file\", \"category\": \"profiles\",
     \"source\": \"missing.step\", \"make\": \"section\",
     \"build\": {\"role\": \"profile\", \"system\": ${i5}, \"length\": ${length}}}
  ],
  \"left_out\": [{\"source\": \"p20x20-copy.step\", \"reason\": \"a copy of p20x20.step\"}]}")
# A rod's frames: its ends, end B at its length, and its axis.
set(rod_frames "[{\"name\": \"end_a\", \"kind\": \"end\", \"origin\": [0, 0, 0], \"z\": [0, 0, -1], \"x\": [1, 0, 0]},
    {\"name\": \"end_b\", \"kind\": \"end\", \"origin\": [0, 0, \"L\"], \"z\": [0, 0, 1], \"x\": [1, 0, 0]},
    {\"name\": \"shaft\", \"kind\": \"shaft\", \"origin\": [0, 0, 0], \"z\": [0, 0, 1], \"x\": [1, 0, 0]}]")
file(WRITE "${groups}/b-parts.json" "[
  {\"id\": \"bracket-20-i5\", \"name\": \"Bracket 20 I-type slot 5\", \"category\": \"connectors\",
   \"source\": \"bracket.step\", \"make\": \"solid\", \"supplier\": \"Test Supplier\", \"part_number\": \"TEST-B20\",
   \"rows\": [{\"name\": \"Grey\", \"select\": {\"Colour\": \"Grey\"}, \"values\": {}},
            {\"name\": \"Black\", \"select\": {\"Colour\": \"Black\"}, \"values\": {}, \"part_number\": \"TEST-B20-B\",
             \"source\": \"bracket-black.step\"}],
   \"build\": {\"role\": \"connector\", \"systems\": [${i5}],
             \"connector\": {\"junctions\": [\"L\", \"T\"], \"space\": \"inner_corner\",
                           \"mates\": [{\"frame\": \"mount_a\", \"to\": \"slot\"}, {\"frame\": \"mount_b\", \"to\": \"slot\"}]}},
   \"frames\": [{\"name\": \"mount_a\", \"kind\": \"face\", \"origin\": [15, 10, 0], \"z\": [0, 0, -1], \"x\": [1, 0, 0]},
              {\"name\": \"mount_b\", \"kind\": \"face\", \"origin\": [0, 10, 15], \"z\": [-1, 0, 0], \"x\": [0, 0, 1]}]},
  {\"id\": \"panel-generic\", \"name\": \"Generic panel\", \"category\": \"panels\", \"make\": \"model\",
   \"dimensions\": {\"shape\": \"plate\", \"width\": \"W\", \"thickness\": \"T\", \"length\": \"H\",
                  \"parameters\": {\"W\": \"200 mm\", \"H\": \"150 mm\", \"T\": \"3 mm\"}},
   \"build\": {\"role\": \"panel\", \"panel\": {\"mount\": \"slot\", \"thickness\": \"T\", \"width\": \"W\",
                                               \"height\": \"H\", \"clearance\": \"0.5 mm\"}},
   \"frames\": [{\"name\": \"face\", \"kind\": \"face\", \"origin\": [0, 0, 0], \"z\": [0, -1, 0], \"x\": [0, 0, 1]}],
   \"rows\": [{\"name\": \"T3\", \"select\": {\"Thickness\": \"3\"}, \"values\": {\"T\": \"3 mm\"},
              \"part_number\": \"TEST-PANEL-3\"},
             {\"name\": \"T5\", \"select\": {\"Thickness\": \"5\"}, \"values\": {\"T\": \"5 mm\"}}]},
  {\"id\": \"clamp-25\", \"name\": \"Clamping angle 25\", \"category\": \"panels\", \"make\": \"model\",
   \"dimensions\": {\"shape\": \"angle\", \"legs\": [\"25 mm\", \"25 mm\"], \"width\": \"20 mm\",
                  \"thickness\": \"3 mm\"},
   \"build\": {\"role\": \"connector\", \"systems\": [${i5}],
             \"connector\": {\"junctions\": [\"surface\"], \"space\": \"outer\",
                           \"mates\": [{\"frame\": \"mount_a\", \"to\": \"slot\"}, {\"frame\": \"mount_b\", \"to\": \"face\"}]}},
   \"frames\": [{\"name\": \"mount_a\", \"kind\": \"slot\", \"origin\": [0, 0, 12.5], \"z\": [0, -1, 0], \"x\": [0, 0, 1]},
              {\"name\": \"mount_b\", \"kind\": \"face\", \"origin\": [0, 12.5, 3], \"z\": [0, 0, 1], \"x\": [0, 1, 0]}]},
  {\"id\": \"rail-15\", \"name\": \"Rail 15\", \"category\": \"linear\", \"source\": \"rail.step\", \"make\": \"section\",
   \"build\": {\"role\": \"linear\", \"linear\": {\"kind\": \"rail\", \"size\": \"R15\"},
             \"length\": {\"parameter\": \"L\", \"min\": \"40 mm\", \"max\": \"1000 mm\",
                        \"pitch\": {\"distance\": \"60 mm\", \"first\": \"20 mm\"}}},
   \"frames\": [{\"name\": \"end_a\", \"kind\": \"end\"}, {\"name\": \"end_b\", \"kind\": \"end\"},
              {\"name\": \"rail\", \"kind\": \"rail\", \"origin\": [0, 6, 0], \"z\": [0, 1, 0], \"x\": [0, 0, 1]}]},
  {\"id\": \"shaft-8\", \"name\": \"Shaft 8\", \"category\": \"linear\", \"make\": \"model\", \"supplier\": \"Test Supplier\",
   \"dimensions\": {\"shape\": \"rod\", \"diameter\": \"8 mm\", \"length\": \"500 mm\"},
   \"build\": {\"role\": \"linear\", \"linear\": {\"kind\": \"shaft\", \"size\": \"8 mm\"}, \"length\": ${length}},
   \"frames\": ${rod_frames}},
  {\"id\": \"screw-tr8\", \"name\": \"Lead screw Tr8x1.5\", \"category\": \"linear\", \"make\": \"model\",
   \"dimensions\": {\"shape\": \"rod\", \"diameter\": \"D\", \"length\": \"L\",
                  \"parameters\": {\"D\": \"8 mm\", \"L\": \"200 mm\"},
                  \"thread\": {\"standard\": \"trapezoidal\", \"designation\": \"Tr8x1.5\", \"class\": \"7e\"}},
   \"build\": {\"role\": \"linear\", \"linear\": {\"kind\": \"screw\", \"size\": \"Tr8x1.5\", \"lead\": \"1.5 mm\"},
             \"length\": ${length}},
   \"frames\": ${rod_frames}}
]")

# 3. The library, v1.0.0.
execute_process(COMMAND "${PYTHON}" "${GENERATOR}" --cli "${CLI}" --source "${source}" --out "${library}"
                        --groups "${groups}" --work "${WORK}" --version 1.0.0 --no-render --commit
                RESULT_VARIABLE status OUTPUT_VARIABLE out ERROR_VARIABLE err)
set(out "${out}${err}")
if(NOT status EQUAL 0)
  message(FATAL_ERROR "the generator failed (${status}):\n${out}")
endif()
expect("mitcad-parts 1.0.0: 8 components" "the generator's summary")
expect("left out p-missing: the source missing.step is not in --source" "the part without its file")
expect("left out: 1 \\(p-missing\\)" "the parts left out")
# The rail's design has its source's holes: no warning of a difference.
if(out MATCHES "warning rail-15")
  message(FATAL_ERROR "the rail's design and its source differ:\n${out}")
endif()
cli(ok library check "${library}")
expect("mitcad-parts: 8 components, no errors" "the library's check")
foreach(file mitcad-library.json LICENSE SOURCES.md README.md .mitcad/project.json profiles/p20x20-i5.mitcad
             connectors/bracket-20-i5.mitcad panels/panel-generic.mitcad previews/p20x20-i5.png
             previews/bracket-20-i5.png)
  if(NOT EXISTS "${library}/${file}")
    message(FATAL_ERROR "the generator wrote no ${file}")
  endif()
endforeach()
file(READ "${library}/previews/p20x20-i5.png" signature HEX LIMIT 4)
same("${signature}" "89504e47" "the preview's signature")
file(READ "${library}/SOURCES.md" sources)
if(NOT sources MATCHES "p20x20-i5 \\(Profile 20x20 I-type slot 5\\): p20x20.step; Test Supplier TEST-P20")
  message(FATAL_ERROR "SOURCES.md:\n${sources}")
endif()
# A row's own file, the file left out with its reason, and a modelled part.
foreach(line "  - Black: bracket-black.step; TEST-B20-B" "- p20x20-copy.step: a copy of p20x20.step"
             "shaft-8 \\(Shaft 8\\): the supplier's dimensions; Test Supplier")
  if(NOT sources MATCHES "${line}")
    message(FATAL_ERROR "SOURCES.md has no '${line}':\n${sources}")
  endif()
endforeach()
if(out MATCHES "warning [^\n]*(bracket-black|p20x20-copy)")
  message(FATAL_ERROR "the row's file or the left-out file is not found:\n${out}")
endif()

# The manifest: licence, supplier fields, the BUILD block with its frames
# found from the sections, and what the generator filled in.
file(READ "${library}/mitcad-library.json" manifest)
string(JSON license GET "${manifest}" license)
same("${license}" "MIT" "the library's licence")
string(JSON category GET "${manifest}" categories 0 name)
same("${category}" "Slot profiles" "the profiles' category")
function(frames index wanted)
  string(JSON id GET "${manifest}" components ${index} id)
  string(JSON count LENGTH "${manifest}" components ${index} build frames)
  set(names "")
  math(EXPR last "${count} - 1")
  foreach(i RANGE ${last})
    string(JSON name GET "${manifest}" components ${index} build frames ${i} name)
    list(APPEND names "${name}")
  endforeach()
  same("${names}" "${wanted}" "the frames of ${id}")
endfunction()
string(JSON id GET "${manifest}" components 0 id)
same("${id}" "p20x20-i5" "the first component")
string(JSON supplier GET "${manifest}" components 0 supplier)
same("${supplier}" "Test Supplier" "the profile's supplier")
string(JSON number GET "${manifest}" components 0 part_number)
same("${number}" "TEST-P20" "the profile's part number")
frames(0 "end_a;end_b;core;slot_x+_1;slot_x-_1;slot_y+_1;slot_y-_1")
string(JSON accepts GET "${manifest}" components 0 build frames 3 accepts 0)
same("${accepts}" "connector" "what the slot frame accepts (from the group file)")
string(JSON bore GET "${manifest}" components 0 build frames 2 diameter)
same("${bore}" "4.2 mm" "the core bore")
string(JSON opening GET "${manifest}" components 0 build slot opening)
same("${opening}" "5.2 mm" "the slot opening found from the section")
frames(1 "end_a;end_b;core_1;core_2;slot_x+_1;slot_x+_2;slot_x-_1;slot_x-_2;slot_y+_1;mid")
string(JSON width GET "${manifest}" components 1 build section width)
string(JSON height GET "${manifest}" components 1 build section height)
same("${width} x ${height}" "20 mm x 40 mm" "the section's size")
string(JSON mass GET "${manifest}" components 1 build mass_per_metre)
same("${mass}" "1.664 kg" "the mass per metre of aluminium")
frames(2 "mount_a;mount_b")

# The designs hold their features in the root: no components, occurrences,
# active component or root policy (a section, a solid and a model part).
foreach(design profiles/p20x20-i5.mitcad connectors/bracket-20-i5.mitcad panels/panel-generic.mitcad
               linear/rail-15.mitcad)
  file(READ "${library}/${design}" text)
  foreach(field components occurrences active_component root_policy)
    string(JSON value ERROR_VARIABLE missing GET "${text}" ${field})
    if(NOT missing AND NOT value STREQUAL "C0" AND NOT value STREQUAL "[]")
      message(FATAL_ERROR "${design} has ${field}: ${value}")
    endif()
  endforeach()
  string(JSON count LENGTH "${text}" features)
  if(count EQUAL 0)
    message(FATAL_ERROR "${design} has no features in its root")
  endif()
  math(EXPR last "${count} - 1")
  foreach(i RANGE ${last})
    # A feature of the root names no component.
    string(JSON owner ERROR_VARIABLE root GET "${text}" features ${i} component)
    if(NOT root AND NOT owner STREQUAL "C0")
      message(FATAL_ERROR "${design}: feature ${i} is in ${owner}, not in the root")
    endif()
  endforeach()
endforeach()

# 4. The profile's design: the section extruded by L; cut to 250 mm its
#    body and end B follow, end A stays at the origin pointing along -Z.
file(WRITE "${WORK}/cut.json" "[
  {\"expect\": {\"bodies\": 1}},
  {\"expect\": {\"total_volume\": 26134.5576397669, \"tolerance\": 1e-5}},
  {\"cmd\": \"set_parameter\", \"name\": \"L\", \"expression\": \"250 mm\"},
  {\"expect\": {\"total_volume\": 65336.3940994172, \"tolerance\": 1e-5}},
  {\"expect\": {\"query\": {\"query\": \"datum\", \"uid\": \"F3\"}, \"result\": {\"origin\": [0, 0, 0], \"normal\": [0, 0, -1]}}},
  {\"expect\": {\"query\": {\"query\": \"datum\", \"uid\": \"F5\"}, \"result\": {\"origin\": [0, 0, 250], \"normal\": [0, 0, 1]}}}
]")
cli(ok run "${WORK}/cut.json" --open "${library}/profiles/p20x20-i5.mitcad")
cli(ok info "${library}/profiles/p20x20-i5.mitcad")
expect("mount:slot_y-_1" "the slot frame's joint origin")

#    The 20x40 turned back by place: its section's area, and the frame
#    whose origin is at L / 2 (after its plane, the timeline's 14th
#    feature) halfway along it at any length.
file(WRITE "${WORK}/mid.json" "[
  {\"expect\": {\"total_volume\": 36977.4691681, \"tolerance\": 1e-5}},
  {\"expect\": {\"query\": {\"query\": \"datum\", \"uid\": \"F14\"}, \"result\": {\"origin\": [10, 0, 30], \"normal\": [0, 0, 1]}}},
  {\"cmd\": \"set_parameter\", \"name\": \"L\", \"expression\": \"100 mm\"},
  {\"expect\": {\"query\": {\"query\": \"datum\", \"uid\": \"F14\"}, \"result\": {\"origin\": [10, 0, 50]}}}
]")
cli(ok run "${WORK}/mid.json" --open "${library}/profiles/p20x40-i5.mitcad")

#    The angle from its dimensions: two legs, centred across its width.
file(WRITE "${WORK}/angle.json" "[
  {\"expect\": {\"total_volume\": 2820, \"tolerance\": 1e-6}},
  {\"expect\": {\"query\": {\"query\": \"bodies\", \"properties\": true},
               \"result\": [{\"bbox\": {\"min\": [-10, 0, 0], \"max\": [10, 25, 25]}}]}}
]")
cli(ok run "${WORK}/angle.json" --open "${library}/panels/clamp-25.mitcad")

#    The rail keeps its holes at the pitch: two in 100 mm (as the source),
#    four in 250 mm, one in 70 mm (a hole is 340.7 mm3 of the 15 x 12 bar).
file(WRITE "${WORK}/rail.json" "[
  {\"expect\": {\"total_volume\": 17318.5885534, \"tolerance\": 1e-5}},
  {\"cmd\": \"set_parameter\", \"name\": \"L\", \"expression\": \"250 mm\"},
  {\"expect\": {\"total_volume\": 43637.1771069, \"tolerance\": 1e-5}},
  {\"cmd\": \"set_parameter\", \"name\": \"L\", \"expression\": \"70 mm\"},
  {\"expect\": {\"total_volume\": 12259.2942767, \"tolerance\": 1e-5}}
]")
cli(ok run "${WORK}/rail.json" --open "${library}/linear/rail-15.mitcad")

#    The shaft: a rod of 8 mm on the Z axis, 500 mm long by default; cut
#    to 250 mm its body and end B (after the rod and end A's frame, its
#    plane F3 and frame F4) follow, and it has no thread. The lead screw's
#    cosmetic thread (Tr8x1.5) runs along all of it at any length, as the
#    threads query lists it.
file(WRITE "${WORK}/shaft.json" "[
  {\"expect\": {\"bodies\": 1}},
  {\"expect\": {\"total_volume\": 25132.7412287, \"tolerance\": 1e-6}},
  {\"cmd\": \"set_parameter\", \"name\": \"L\", \"expression\": \"250 mm\"},
  {\"expect\": {\"total_volume\": 12566.3706144, \"tolerance\": 1e-6}},
  {\"expect\": {\"query\": {\"query\": \"bodies\", \"properties\": true},
               \"result\": [{\"bbox\": {\"min\": [-4, -4, 0], \"max\": [4, 4, 250]}}]}},
  {\"expect\": {\"query\": {\"query\": \"datum\", \"uid\": \"F4\"}, \"result\": {\"origin\": [0, 0, 250], \"normal\": [0, 0, 1]}}},
  {\"expect\": {\"query\": {\"query\": \"threads\"}, \"result\": []}}
]")
cli(ok run "${WORK}/shaft.json" --open "${library}/linear/shaft-8.mitcad")
file(WRITE "${WORK}/screw.json" "[
  {\"expect\": {\"total_volume\": 10053.0964915, \"tolerance\": 1e-6}},
  {\"expect\": {\"query\": {\"query\": \"threads\"},
               \"result\": [{\"standard\": \"trapezoidal\", \"designation\": \"Tr8x1.5\", \"class\": \"7e\",
                           \"modeled\": false, \"major_diameter\": 8, \"pitch\": 1.5, \"internal\": false,
                           \"start\": [0, 0, 0], \"end\": [0, 0, 200]}]}},
  {\"cmd\": \"set_parameter\", \"name\": \"L\", \"expression\": \"300 mm\"},
  {\"expect\": {\"query\": {\"query\": \"threads\"}, \"result\": [{\"designation\": \"Tr8x1.5\", \"end\": [0, 0, 300]}]}}
]")
cli(ok run "${WORK}/screw.json" --open "${library}/linear/screw-tr8.mitcad")
string(JSON size GET "${manifest}" components 7 build linear size)
same("${size}" "Tr8x1.5" "the lead screw's size")
string(JSON parameter GET "${manifest}" components 7 build length parameter)
same("${parameter}" "L" "the lead screw's length parameter")
frames(7 "end_a;end_b;shaft")

# 5. Fetched, searched by role, system and part number (a size's own).
cli(ok library fetch "${library}")
expect("Fetched library Mitcad parts \\(mitcad-parts\\)" "the fetch")
cli(ok library search --role profile --system I/5/20)
expect("mitcad-parts/p20x20-i5  Profile 20x20 I-type slot 5  Test Supplier TEST-P20  MIT" "the profiles of I5")
expect("mitcad-parts/p20x40-i5" "the other profile of I5")
cli(ok library search --role profile --system B/8)
if(out MATCHES "p20x20-i5")
  message(FATAL_ERROR "an I-type profile found for B-type slot 8:\n${out}")
endif()
cli(ok library search --part-number test-panel-3)
expect("mitcad-parts/panel-generic" "the panel by its size's part number")
cli(ok library show mitcad-parts)
expect("panel-generic Generic panel: panels/panel-generic.mitcad, 2 sizes \\(default T3\\)" "the panel's sizes")

# 6. The connectors that fit a corner of two I5 profiles, and one of B8
#    profiles, where the bracket is greyed with the reason.
file(WRITE "${WORK}/corner.json"
     "{\"id\": \"J1\", \"kind\": \"L\", \"systems\": [${i5}, ${i5}], \"space\": {\"x\": 30, \"y\": 30, \"z\": 20}}")
cli(ok library candidates --junction "${WORK}/corner.json")
expect("mitcad-parts/bracket-20-i5  Bracket 20 I-type slot 5  Test Supplier TEST-B20" "the bracket for the corner")
file(WRITE "${WORK}/corner-b8.json"
     "{\"id\": \"J2\", \"kind\": \"L\", \"systems\": [{\"type\": \"B\", \"slot\": 8, \"grid\": 30}]}")
cli(ok library candidates --junction "${WORK}/corner-b8.json")
expect("not mitcad-parts/bracket-20-i5  Bracket 20 I-type slot 5: it is made for I-type slot 5 \\(20\\), not B-type slot 8 \\(30\\)"
       "the bracket greyed for B8")
cli(ok library candidates --role panel)
expect("mitcad-parts/panel-generic" "the panel picker")
