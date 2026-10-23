# SPDX-License-Identifier: MIT
# Required hardware validation must reject CPU/absent devices and invalid
# comparisons, even on a machine without Metal. No actual GPU run is claimed.
if(NOT DEFINED WORKER)
  message(FATAL_ERROR "WORKER is required")
endif()

function(expect_status name required status diagnostic)
  execute_process(
    COMMAND "${CMAKE_COMMAND}" -E env "MITCAD_RENDER_REQUIRE_METAL=${required}"
            "${WORKER}" ${ARGN}
    RESULT_VARIABLE result OUTPUT_VARIABLE out ERROR_VARIABLE err TIMEOUT 20)
  if(NOT "${result}" STREQUAL "${status}" OR NOT "${out}${err}" MATCHES "${diagnostic}")
    message(FATAL_ERROR "${name}: expected status ${status} and ${diagnostic}, got ${result}: ${out}${err}")
  endif()
  message(STATUS "${name}: rejected as expected (${result})")
endfunction()

set(comparison --bench 64x48 --samples 2 --compare cpu --tolerance 2:24)
expect_status(cpu 0 3 "Required Metal device is unavailable"
              --require-metal ${comparison} --device cpu)
expect_status(absent 0 3 "Required Metal device is unavailable"
              --require-metal ${comparison} --device NO_SUCH_DEVICE)
expect_status(environment 1 3 "Required Metal device is unavailable"
              ${comparison} --device cpu)
expect_status(no_benchmark 0 2 "require-metal needs --bench" --require-metal)
expect_status(no_comparison 0 2 "needs --compare and a valid --tolerance"
              --require-metal --bench 64x48 --tolerance 2:24)
expect_status(no_tolerance 0 2 "needs --compare and a valid --tolerance"
              --require-metal --bench 64x48 --compare cpu)
foreach(tolerance nan:24 -1:24 2:256 2:24:1 invalid)
  expect_status("invalid tolerance ${tolerance}" 0 2 "needs --compare and a valid --tolerance"
                --require-metal --bench 64x48 --compare cpu --tolerance "${tolerance}")
endforeach()
# Preserve the optional comparison's documented skip code; required mode
# above turns the same absent-device request into a failure instead.
expect_status(optional_absent 0 77 "render-skip no NO_SUCH_DEVICE device"
              ${comparison} --device NO_SUCH_DEVICE)
set(swap --swap-test "${CMAKE_CURRENT_BINARY_DIR}/required-metal-contract-fixture")
expect_status(swap_cpu 0 3 "Required Metal device is unavailable"
              --require-metal ${swap} --device cpu)
expect_status(swap_required_absent 0 3 "render-failed There is no render device"
              --require-metal ${swap} --device NO_SUCH_DEVICE)
expect_status(swap_optional_absent 0 77 "render-skip There is no render device"
              ${swap} --device NO_SUCH_DEVICE)
message(STATUS "Required Metal contract passed")
