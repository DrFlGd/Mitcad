# SPDX-License-Identifier: MIT
# The timber parts of Mitcad's generated parts library (mitcad#478)
# through mitcad-cli. The library is a folder made by
# tools/libraries/make-profile-library.py (or make-parts-library.py; a
# git repository with mitcad-library.json) named by
# MITCAD_PROFILES_LIBRARY; it is not in the repository, and the test is
# skipped without it. Nothing goes over the network.
#
# 1. The three timber families (sawn, planed, dimensioned), each in a
#    category of its own: every size of the tables a row, the less
#    common ones marked, every row's section its size (re-sawn 2 mm under nominal), its species' density, 20 %
#    moisture, the stock lengths 2.7 to 5.4 m; no supplier.
# 2. Their designs at two sizes each: the body W x T x L and its mass
#    (the density times the volume).
# 3. The wood fasteners' sizes and axis frames, the connectors' fits,
#    mount frames and holes, the sheets.
# 4. The pickers: the Profile picker lists no timber, the Timber picker
#    the three families, sheets only on a face.
#
# cmake -DCLI=<mitcad-cli> -DWORK=<folder> -P build-timber-test.cmake

foreach(var CLI WORK)
  if(NOT DEFINED ${var})
    message(FATAL_ERROR "build-timber-test.cmake needs -D${var}=...")
  endif()
endforeach()

set(library "$ENV{MITCAD_PROFILES_LIBRARY}")
if(library STREQUAL "")
  message("build timber: skipped (set MITCAD_PROFILES_LIBRARY to the generated parts library's folder)")
  return()
endif()
if(NOT EXISTS "${library}/mitcad-library.json")
  message(FATAL_ERROR "MITCAD_PROFILES_LIBRARY is ${library}, which has no mitcad-library.json")
endif()
set(ENV{MITCAD_LIBRARIES_DIR} "${WORK}/cache")
file(REMOVE_RECURSE "${WORK}")
file(MAKE_DIRECTORY "${WORK}")

function(cli)
  execute_process(COMMAND "${CLI}" ${ARGN} RESULT_VARIABLE status OUTPUT_VARIABLE out ERROR_VARIABLE err)
  if(NOT status EQUAL 0)
    message(FATAL_ERROR "mitcad-cli ${ARGN} failed (${status}):\n${out}${err}")
  endif()
  set(out "${out}${err}" PARENT_SCOPE)
endfunction()

function(same actual wanted what)
  if(NOT "${actual}" STREQUAL "${wanted}")
    message(FATAL_ERROR "${what}: '${actual}', not '${wanted}'")
  endif()
endfunction()

file(READ "${library}/mitcad-library.json" manifest)
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

# A field of a component's build block, or "(none)".
function(build_field id out_var)
  component_index("${id}" index)
  string(JSON value ERROR_VARIABLE missing GET "${manifest}" components ${index} build ${ARGN})
  if(missing)
    set(value "(none)")
  endif()
  set(${out_var} "${value}" PARENT_SCOPE)
endfunction()

function(expect_field id wanted)
  build_field("${id}" value ${ARGN})
  same("${value}" "${wanted}" "${id}: build.${ARGN}")
endfunction()

# 1. The families: rows, standard and less common, per family.
set(densities "spruce=450;pine=520")
foreach(entry IN ITEMS "timber-sawn|sawn|66|44" "timber-planed|planed|57|12" "timber-dimensioned|dimensioned|20|8")
  string(REPLACE "|" ";" fields "${entry}")
  list(GET fields 0 id)
  list(GET fields 1 finish)
  list(GET fields 2 count)
  list(GET fields 3 less)
  component_index("${id}" index)
  # Each family a group of the pickers of its own (mitcad#484).
  string(JSON category GET "${manifest}" components ${index} category)
  same("${category}" "${finish}-timber" "${id}: its category")
  foreach(key supplier part_number supplier_url)
    string(JSON value ERROR_VARIABLE missing GET "${manifest}" components ${index} ${key})
    if(NOT missing)
      message(FATAL_ERROR "${id} names a ${key}: ${value}")
    endif()
  endforeach()
  expect_field("${id}" "timber" kind)
  expect_field("${id}" "profile" role)
  expect_field("${id}" "${finish}" material finish)
  expect_field("${id}" "" material grade)
  expect_field("${id}" "20" material moisture)
  expect_field("${id}" "(none)" system)
  expect_field("${id}" "(none)" mass_per_metre)
  string(JSON stock GET "${manifest}" components ${index} build length stock)
  string(REGEX REPLACE "[][ \n]" "" stock "${stock}")
  same("${stock}" "2700,3000,3300,3600,3900,4200,4500,4800,5100,5400" "${id}: the stock lengths")
  string(JSON rows LENGTH "${manifest}" components ${index} build rows)
  same("${rows}" "${count}" "${id}: its sizes")
  math(EXPR last "${rows} - 1")
  set(marked 0)
  foreach(i RANGE ${last})
    string(JSON name MEMBER "${manifest}" components ${index} build rows ${i})
    string(JSON row GET "${manifest}" components ${index} build rows ${name})
    string(JSON availability GET "${row}" availability)
    if(availability STREQUAL "less common")
      math(EXPR marked "${marked} + 1")
    elseif(NOT availability STREQUAL "standard")
      message(FATAL_ERROR "${id} ${name}: availability '${availability}'")
    endif()
    # The section: the thickness of the name, the width the nominal's or
    # 2 mm under it (re-sawn).
    string(REGEX MATCH "^([0-9]+)x([0-9]+)$" match "${name}")
    set(thickness "${CMAKE_MATCH_1}")
    set(width "${CMAKE_MATCH_2}")
    string(JSON actual_t GET "${row}" section height)
    string(JSON actual_w GET "${row}" section width)
    string(JSON nominal_w GET "${row}" nominal width)
    same("${actual_t}" "${thickness} mm" "${id} ${name}: its thickness")
    same("${nominal_w}" "${width} mm" "${id} ${name}: its nominal width")
    math(EXPR resawn "${width} - 2")
    if(NOT actual_w STREQUAL "${width} mm" AND NOT actual_w STREQUAL "${resawn} mm")
      message(FATAL_ERROR "${id} ${name}: its width ${actual_w}")
    endif()
    string(JSON species GET "${row}" material species)
    string(JSON density GET "${row}" material density)
    if(NOT "${species}=${density}" IN_LIST densities)
      message(FATAL_ERROR "${id} ${name}: ${species} of ${density} kg/m3")
    endif()
    string(JSON tolerance GET "${row}" tolerance width)
  endforeach()
  same("${marked}" "${less}" "${id}: its less common sizes")
  message("${id}: ${rows} sizes, ${marked} less common")
endforeach()
# The footnotes: re-sawn sizes, the species by thickness, the marking.
expect_field(timber-sawn "48 mm" rows 22x50 section width)
expect_field(timber-sawn "73 mm" rows 50x75 section width)
expect_field(timber-sawn "73 mm" rows 75x75 section width)
expect_field(timber-sawn "less common" rows 75x75 availability)
expect_field(timber-sawn "standard" rows 75x200 availability)
expect_field(timber-sawn "pine" rows 19x100 material species)
expect_field(timber-sawn "pine" rows 25x225 material species)
expect_field(timber-sawn "spruce" rows 44x250 material species)
expect_field(timber-sawn "-1" rows 22x100 tolerance width 0)
expect_field(timber-sawn "4" rows 50x150 tolerance width 1)
expect_field(timber-planed "pine" rows 15x15 material species)
expect_field(timber-planed "pine" rows 21x195 material species)
expect_field(timber-planed "spruce" rows 18x45 material species)
expect_field(timber-planed "less common" rows 33x21 availability)
expect_field(timber-planed "0.5" rows 18x45 tolerance thickness 1)
expect_field(timber-dimensioned "pine" rows 20x95 material species)
expect_field(timber-dimensioned "less common" rows 48x248 availability)
expect_field(timber-dimensioned "standard" rows 42x66 availability)

# 2. The designs at two sizes: the body and its mass (density x volume,
#    in grams).
function(member id row width thickness density)
  math(EXPR volume "${width} * ${thickness} * 3000")
  math(EXPR grams "${volume} * ${density} / 1000000")
  file(WRITE "${WORK}/${id}-${row}.json" "[
  {\"cmd\": \"apply_configuration\", \"name\": \"${row}\"},
  {\"cmd\": \"set_parameter\", \"name\": \"L\", \"expression\": \"3000 mm\"},
  {\"expect\": {\"bodies\": 1}},
  {\"expect\": {\"total_volume\": ${volume}, \"tolerance\": 1e-6}}
]")
  component_index("${id}" index)
  string(JSON path GET "${manifest}" components ${index} path)
  cli(run "${WORK}/${id}-${row}.json" --open "${library}/${path}")
  build_field("${id}" listed rows ${row} material density)
  same("${listed}" "${density}" "${id} ${row}: its density")
  message("${id} ${row}: ${width} x ${thickness} x 3000 mm, ${grams} g")
endfunction()
member(timber-sawn 22x100 100 22 450)
member(timber-sawn 22x50 48 22 450)
member(timber-planed 21x95 95 21 520)
member(timber-planed 70x195 195 70 450)
member(timber-dimensioned 48x148 148 48 450)
member(timber-dimensioned 20x145 145 20 520)

# 3. The wood fasteners: sizes and the axis frame; the connectors: fits,
#    frames and fasteners; the sheets.
foreach(entry IN ITEMS "wood-screw|26|screw" "connector-screw|5|screw" "ring-nail|7|nail" "structural-screw|25|screw")
  string(REPLACE "|" ";" fields "${entry}")
  list(GET fields 0 id)
  list(GET fields 1 count)
  list(GET fields 2 kind)
  component_index("${id}" index)
  string(JSON rows LENGTH "${manifest}" components ${index} build rows)
  same("${rows}" "${count}" "${id}: its sizes")
  expect_field("${id}" "fastener" role)
  expect_field("${id}" "${kind}" wood kind)
  expect_field("${id}" "axis" frames 0 name)
  expect_field("${id}" "shaft" frames 0 kind)
endforeach()
expect_field(wood-screw "40 mm" rows 4.5x40 wood length)
expect_field(wood-screw "4.5 mm" rows 4.5x40 wood diameter)
foreach(height 98 123 148 173 198 223)
  expect_field("joist-hanger-48x${height}" "hanger" connector fit kind)
  expect_field("joist-hanger-48x${height}" "${height}" connector fit section 1)
  expect_field("joist-hanger-48x${height}" "outer" connector placement)
  expect_field("joist-hanger-48x${height}" "mount" connector mates 0 frame)
  expect_field("joist-hanger-48x${height}" "mount" frames 0 name)
  expect_field("joist-hanger-48x${height}" "connector-screw" connector fasteners 0 screw component)
  expect_field("joist-hanger-48x${height}" "4" connector fasteners 0 count)
endforeach()
foreach(size 40 60 90)
  component_index("angle-bracket-${size}" index)
  expect_field("angle-bracket-${size}" "angle" connector fit kind)
endforeach()
foreach(size 98 123)
  expect_field("post-base-${size}" "post_base" connector fit kind)
  expect_field("post-base-${size}" "${size}" connector fit section 0)
  expect_field("post-base-${size}" "base" frames 1 name)
endforeach()
foreach(id plywood-spruce plywood-birch osb-3 chipboard-p5)
  expect_field("${id}" "face" panel mount)
  expect_field("${id}" "T" panel thickness)
endforeach()

#    The hanger 48x148 in its mount frame's coordinates: round the
#    joist's end at the origin, x up; one solid with its holes.
file(WRITE "${WORK}/hanger.json" "[
  {\"expect\": {\"bodies\": 1}},
  {\"expect\": {\"query\": {\"query\": \"bodies\", \"properties\": true},
               \"result\": [{\"bbox\": {\"min\": [-76, -56, -70], \"max\": [56, 56, 0]}}]}}
]")
cli(run "${WORK}/hanger.json" --open "${library}/timber-connectors/joist-hanger-48x148.mitcad")

# 4. The pickers.
cli(library fetch "${library}")
cli(library candidates --role profile)
if(out MATCHES "timber-")
  message(FATAL_ERROR "the Profile picker lists timber:\n${out}")
endif()
cli(library candidates --role profile --kind timber)
foreach(id timber-sawn timber-planed timber-dimensioned)
  if(NOT out MATCHES "mitcad-parts/${id} ")
    message(FATAL_ERROR "the Timber picker has no ${id}:\n${out}")
  endif()
endforeach()
if(out MATCHES "p30x30")
  message(FATAL_ERROR "the Timber picker lists slot profiles:\n${out}")
endif()
cli(library candidates --role panel --mount face)
foreach(id plywood-spruce plywood-birch osb-3 chipboard-p5)
  if(NOT out MATCHES "mitcad-parts/${id} ")
    message(FATAL_ERROR "the sheets have no ${id}:\n${out}")
  endif()
endforeach()
cli(library candidates --role panel)
if(out MATCHES "plywood|osb-3|chipboard")
  message(FATAL_ERROR "the slot panels' picker lists sheets:\n${out}")
endif()
message("the timber parts: their sizes, designs, fasteners, connectors, sheets and pickers")
