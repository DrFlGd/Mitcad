#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/projects app/files app/framework/AppSettings.cpp app/framework/FloatingLayout.cpp
# check-all sources: core/vcs core/model/src/file/settings.rs core/ffi/src/vcs.rs core/ffi/src/projects.rs tools/cli
# The Projects panel's defaults and its projects' settings through the real
# UI (mitcad#151, #152, #153), without a network:
#   1. A fresh profile without projects starts with the panel hidden; with
#      projects configured (no choice made yet) it starts shown, in the
#      Floating card and in the Docked dock.
#   2. A Cloud project's row shows owner/repository from its remote, a
#      Local one the folder's name. A double-click on the Cloud project
#      opens its Project Settings (not opening the project); a display
#      name typed there shows in the panel, stays after a restart, and
#      cleared (a double-click on its host and owner) the default is back.
#      The folder and the repository keep their names.
#   3. The settings button opens Project Settings of the selected project
#      for a project row, a folder row, a design row (the current
#      project's) and a location holding one project.
#   4. Hiding the panel is remembered at the next start; so is showing it.
#
# git's configuration and HOME are the test's own. Runs headless on Xvfb
# (see ui-test-lib.sh).
# Usage: tools/ui-projects-settings-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

command -v git > /dev/null || { echo "git is required (apt-get install git)" >&2; exit 2; }
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}

WORK=$(mktemp -d /tmp/mitcad-ui-projects-settings.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
export HOME=$WORK/home
mkdir -p "$HOME"
DOCUMENTS=$HOME/Documents/Mitcad
export GIT_CONFIG_GLOBAL=$WORK/gitconfig
export GIT_CONFIG_NOSYSTEM=1
unset EMAIL GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL GIT_DIR GIT_WORK_TREE MITCAD_GIT
printf '[user]\n\tname = Ada Tester\n\temail = ada@example.invalid\n[init]\n\tdefaultBranch = main\n' \
  > "$GIT_CONFIG_GLOBAL"
AUTHOR="Ada Tester <ada@example.invalid>"

cli() {
  "$CLI" "$@" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli $*"; }
}

expect_exit() {
  for _ in $(seq 1 50); do
    kill -0 "$UI_RUNNER" 2> /dev/null || break
    sleep 0.2
  done
  kill -0 "$UI_RUNNER" 2> /dev/null && ui_fail "$1: Mitcad did not exit"
  UI_RUNNER=""
  echo "ok   $1"
}

quit() {
  ui_focus_main
  ui_key ctrl+q
  expect_exit "exited"
}

row() {
  ui_expect_log "Projects row $1 at " "the row $1 is shown" 20
}

# settings_button "row key" "logged line" description: selects the row,
# then the toolbar's settings button, which must log the line.
settings_button() {
  row "$1"
  ui_mark
  ui_step "select $1"                    ui_click_logged "Projects row $1"
  # Logged when the selection changes: it may be selected already.
  ui_expect_log "Projects panel selected $1" "$1 selected"
  ui_step "the settings button"          ui_click_logged "Projects button Project Settings"
  ui_expect_new "$2" "$3" 20
}

# close_settings "dialog title": Escape closes Project Settings.
close_settings() {
  ui_focus_dialog "$(ui_title_regex "$1")"
  ui_mark
  ui_step "close it (Escape)"            ui_key Escape
  ui_expect_new "Project Settings closed" "closed"
  ui_focus_main
}

ui_start_display

echo "--- A fresh profile without projects: the panel is hidden"
ui_start_app
UI_MARK=0
ui_expect_log "Projects panel at the start: hidden (no choice yet, no projects)" "hidden by default"
quit

echo "--- Projects configured: a Local one, a Cloud one (a remote on a service; no network)"
SHOP=$DOCUMENTS/Shop
GRIPPER=$DOCUMENTS/gripper-work
cli project create "$SHOP" --author "$AUTHOR"
cli project new-design "$SHOP" parts/plate.mitcad --author "$AUTHOR"
cli project create "$GRIPPER" --author "$AUTHOR"
git -C "$GRIPPER" remote add origin git@github.com:acme/gripper.git
echo "ok   projects made"

echo "--- Floating: shown by default in its card; owner/repository; a double-click opens the settings"
ui_start_app --chrome=floating
UI_MARK=0
ui_expect_log "Projects panel at the start: shown (no choice yet, projects configured)" "shown by default"
ui_expect_log "Projects names: Local/Shop = Shop | github.com/acme/gripper-work = acme/gripper" \
  "a Cloud project as owner/repository, a Local one as its folder" 20
row "github.com/acme/gripper-work"
ui_double_click_expect "Projects row github.com/acme/gripper-work" \
  "Projects panel: settings of $GRIPPER (double-click on github.com/acme/gripper-work)" \
  "a double-click on the Cloud project asks for its settings"
ui_expect_log "Projects panel: Project Settings of $GRIPPER (not the current project)" \
  "opened without opening the project" 20
ui_focus_dialog '^Project Settings - gripper-work$'
ui_expect_log "Project Settings dialog: gripper-work, cloud git@github.com:acme/gripper.git" "its settings"
ui_mark
ui_step "the display name (Alt+D)"       ui_key alt+d
ui_type_text "Gripper (team)"
ui_step "take it (Tab)"                  ui_key Tab
ui_expect_new "Project Settings: display name Gripper (team)" "the display name set"
close_settings "Project Settings - gripper-work"
ui_expect_log "Projects names: Local/Shop = Shop | github.com/acme/gripper-work = Gripper (team) (set)" \
  "the panel shows the display name" 20
[ -d "$GRIPPER" ] || ui_fail "the folder was renamed"
[ "$(git -C "$GRIPPER" remote get-url origin)" = "git@github.com:acme/gripper.git" ] ||
  ui_fail "the remote changed"
grep -q '"Gripper (team)"' "$GRIPPER/.mitcad/local/name.json" || ui_fail "the display name is not in name.json"
[ -z "$(git -C "$GRIPPER" status --porcelain)" ] || ui_fail "the display name changed the project's files"
echo "ok   the folder, the repository and the versions are untouched"
quit

echo "--- Docked: shown by default; the display name stays; the settings button"
ui_start_app
UI_MARK=0
ui_expect_log "Projects panel at the start: shown (no choice yet, projects configured)" "still shown by default"
ui_expect_log "Projects names: Local/Shop = Shop | github.com/acme/gripper-work = Gripper (team) (set)" \
  "the display name after a restart" 20
settings_button "Local/Shop" "Projects panel: settings of $SHOP (Local/Shop selected)" \
  "the settings button with a project row selected"
ui_expect_log "Project Settings dialog: Shop, local" "Project Settings of the project"
close_settings "Project Settings - Shop"
ui_double_click_expect "Projects row Local/Shop" "Projects row Local/Shop/parts/ at " \
  "a double-click on a Local project shows its folders"
settings_button "Local/Shop/parts/" "Projects panel: settings of $SHOP (Local/Shop/parts/ selected)" \
  "the settings button with a folder row selected"
close_settings "Project Settings - Shop"
row "Local/Shop/parts/plate.mitcad"
ui_double_click_expect "Projects row Local/Shop/parts/plate.mitcad" "Opened $SHOP/parts/plate.mitcad" \
  "a double-click opens the design"
ui_focus_main
settings_button "Local/Shop/parts/plate.mitcad" \
  "Projects panel: settings of $SHOP (Local/Shop/parts/plate.mitcad selected)" \
  "the settings button with a design row selected"
ui_expect_log "Projects panel: Project Settings of the current project $SHOP" "the current project's"
close_settings "Project Settings - Shop"
settings_button "github.com/acme" "Projects panel: settings of $GRIPPER (github.com/acme selected)" \
  "the settings button with the Cloud project's location selected"
close_settings "Project Settings - gripper-work"

echo "--- The display name cleared: the default again"
ui_double_click_expect "Projects row github.com/acme" \
  "Projects panel: settings of $GRIPPER (double-click on github.com/acme)" \
  "a double-click on the host and owner asks for its project's settings"
ui_focus_dialog '^Project Settings - gripper-work$'
ui_mark
ui_step "the display name (Alt+D)"       ui_key alt+d
ui_step "select it"                      ui_key ctrl+a
ui_step "clear it"                       ui_key BackSpace
ui_step "take it (Tab)"                  ui_key Tab
ui_expect_new "Project Settings: display name cleared: acme/gripper" "cleared"
close_settings "Project Settings - gripper-work"
ui_expect_log "Projects names: Local/Shop = Shop | github.com/acme/gripper-work = acme/gripper" \
  "owner/repository again" 20
[ ! -e "$GRIPPER/.mitcad/local/name.json" ] || ui_fail "name.json is still there"
echo "ok   no display name kept"

echo "--- Hidden by the user: remembered"
# The view takes the keyboard back from the panel's tree (its letters).
ui_step "click the view"                 ui_click 75 50
ui_mark
ui_step "projects panel (search)"        ui_command "Projects"
ui_expect_new "Projects panel: hidden at the next start" "the choice remembered"
quit
ui_start_app
UI_MARK=0
ui_expect_log "Projects panel at the start: hidden (as chosen)" "hidden at the next start"
ui_mark
ui_step "projects panel (search)"        ui_command "Projects"
ui_expect_new "Projects panel: shown at the next start" "shown again, remembered"
quit
ui_start_app --chrome=floating
UI_MARK=0
ui_expect_log "Projects panel at the start: shown (as chosen)" "shown at the next start (Floating too)"
row "Local/Shop"
quit

ui_finish "UI projects settings test"
