# SPDX-License-Identifier: MIT
# mitcad-cli render with texture map sets (mitcad#314): the box of
# tools/cli/tests/render_texture_maps.json rendered on the CPU (not
# denoised) in each of
# its appearances. The embedded normal and roughness maps change the image
# against the same appearance without them; a map whose file is missing
# is left out with the message on stderr, and one that is no image with
# the worker's warning, and both still render, as the appearance without
# the map does.
#   cmake -DCLI=<mitcad-cli> -DWORKER=<mitcad-render> -DSCRIPT=<render_texture_maps.json> -DWORK=<dir>
#         -P render-texture-maps-test.cmake

file(REMOVE_RECURSE "${WORK}")
file(MAKE_DIRECTORY "${WORK}")
# A roughness map that is no image (broken_rough.png, next to the scripts).
file(WRITE "${WORK}/broken_rough.png" "not an image\n")

file(READ "${SCRIPT}" design)
# render name appearance: the design with the box in `appearance`, rendered
# to name.png; its stderr in `${name}_err`.
function(render name appearance)
  string(REGEX REPLACE "\\][ \t\r\n]*$"
         ",\n  {\"cmd\": \"set_body_appearance\", \"uid\": \"F1.b0\", \"appearance\": \"${appearance}\"}\n]\n"
         script "${design}")
  file(WRITE "${WORK}/${name}.json" "${script}")
  execute_process(COMMAND "${CLI}" render "${WORK}/${name}.json" --worker "${WORKER}" --device cpu --no-denoise
                          -o "${WORK}/${name}.png"
                  RESULT_VARIABLE result OUTPUT_VARIABLE out ERROR_VARIABLE err)
  if(NOT result EQUAL 0 OR NOT EXISTS "${WORK}/${name}.png")
    message(FATAL_ERROR "${name} did not render (${result}):\n${out}\n${err}")
  endif()
  message(STATUS "${name}: ${out}${err}")
  set(${name}_err "${err}" PARENT_SCOPE)
endfunction()

render(mapped mapped)
render(unmapped unmapped)
render(lost lost)
render(broken broken)

foreach(name mapped unmapped)
  if(${name}_err MATCHES "not drawn|cannot read")
    message(FATAL_ERROR "${name}: a texture image is said to be missing:\n${${name}_err}")
  endif()
endforeach()
# The application finds the missing file and says the map is left out.
if(NOT lost_err MATCHES "a texture image is not drawn: Lost Normal: the normal map is left out, the image [^\n]*lost_nor_gl.png is missing")
  message(FATAL_ERROR "lost: no message that the normal map is left out:\n${lost_err}")
endif()
# The worker cannot read the broken map and leaves it out.
if(NOT broken_err MATCHES "Warning: cannot read the texture image [^\n]*broken_rough.png: [^\n]*; the map is left out")
  message(FATAL_ERROR "broken: no warning that the roughness map is left out:\n${broken_err}")
endif()

file(SHA256 "${WORK}/mapped.png" mapped)
file(SHA256 "${WORK}/unmapped.png" unmapped)
file(SHA256 "${WORK}/lost.png" lost)
file(SHA256 "${WORK}/broken.png" broken)
# The maps change the image.
if(mapped STREQUAL unmapped)
  message(FATAL_ERROR "the normal and roughness maps did not change the image")
endif()
# Without the map that is left out, the same image as without any (the
# CPU renders the same scene to the same pixels; not denoised).
if(NOT lost STREQUAL unmapped OR NOT broken STREQUAL unmapped)
  message(FATAL_ERROR "a texture with a map left out renders differently from one without it")
endif()
message(STATUS "Texture maps render test passed")
