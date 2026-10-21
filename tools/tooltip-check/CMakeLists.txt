# SPDX-License-Identifier: MIT
# Keeps the app's tooltips to one short line (docs/development.md, code
# conventions): a command's at most 80 characters, any other control's 100.
add_executable(mitcad-check-tooltips check-tooltips.cpp)
mitcad_utf8_code_page(mitcad-check-tooltips)
target_link_libraries(mitcad-check-tooltips PRIVATE mitcad_warnings)

add_test(NAME repo.tooltips COMMAND mitcad-check-tooltips "${PROJECT_SOURCE_DIR}/app")
# The scan finds the long tooltips planted in it, and only those.
add_test(NAME repo.tooltips-self-test COMMAND mitcad-check-tooltips --self-test)
