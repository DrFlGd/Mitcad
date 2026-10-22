#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/projects app/files/MainWindowProjects.cpp app/files/MainWindowFiles.cpp
# check-all sources: app/files/MainWindowLocks.cpp app/framework/AppSettings.cpp core/vcs core/ffi/src/vcs.rs
# check-all sources: core/ffi/src/projects.rs tools/cli
# File > Open from Project (mitcad#530) through the real UI. Two projects of
# the projects folder: Shop (Local) with a design in its top, a folder with
# a design and a sub-folder with another; Team (Cloud: a bare repository in
# a folder stands in for the cloud, no network) with a design in a folder.
#   1. Open from Project (the command search) lists both projects and
#      starts in the current project's top; Enter and a double-click go
#      into a folder and its sub-folder, Backspace goes up, the path bar's
#      project goes back to the top.
#   2. The search finds a design of the other project by part of its name
#      as it is typed (in every project; none in the place's project); Enter
#      opens it.
#   3. The next time it starts in that design's folder (remembered); with
#      Read-only checked the search's design opens read-only.
#   4. After a restart it starts in the place shown last; Cancel closes it.
# The window's log lines: app/COMMANDS.md, Logs that UI tests read
# ("Project browser ...").
#
# git's configuration and HOME are the test's own. Runs headless on Xvfb
# (see ui-test-lib.sh).
# Usage: tools/ui-open-from-project-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

command -v git > /dev/null || { echo "git is required (apt-get install git)" >&2; exit 2; }
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}

WORK=$(mktemp -d /tmp/mitcad-ui-open-from-project.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
export HOME=$WORK/home
mkdir -p "$HOME"
export GIT_CONFIG_GLOBAL=$WORK/gitconfig
export GIT_CONFIG_NOSYSTEM=1
unset EMAIL GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL GIT_DIR GIT_WORK_TREE MITCAD_GIT
printf '[user]\n\tname = Ada Tester\n\temail = ada@example.invalid\n[init]\n\tdefaultBranch = main\n' \
  > "$GIT_CONFIG_GLOBAL"
AUTHOR="Ada Tester <ada@example.invalid>"
export MITCAD_PROJECTS_DIR=$WORK/projects
SHOP=$MITCAD_PROJECTS_DIR/Shop
TEAM=$MITCAD_PROJECTS_DIR/Team
# A Cloud project's name is its repository's address: a shared folder's path.
TEAM_NAME=$WORK/remotes/team.git

cli() {
  "$CLI" "$@" >> "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli $*"; }
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

# open_browser: File > Open from Project through the command search.
open_browser() {
  ui_mark
  ui_step "open from project (search)"   ui_command "Open from Project"
  ui_expect_new "Project browser Open from Project: 2 projects: $TEAM_NAME (cloud) | Shop (local)" \
    "both projects listed, by name" 20
  ui_focus_dialog '^Open from Project$'
}

echo "--- Two projects with folders and designs"
mkdir -p "$WORK/remotes" "$MITCAD_PROJECTS_DIR"
printf '%s\n' '[{"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],' \
  '  "length": 10, "width": 60, "height": 5, "operation": "new_body"}}]' > "$WORK/block.json"
cli project init "$SHOP" --author "$AUTHOR"
mkdir -p "$SHOP/parts/sub"
for design in plate parts/gear parts/sub/pin; do
  cli run "$WORK/block.json" --save "$SHOP/$design.mitcad"
  cli version save "$SHOP/$design.mitcad" -m "Add $design" --author "$AUTHOR"
done
git init -q --bare "$TEAM_NAME"
cli project create "$TEAM" --author "$AUTHOR" --url "$TEAM_NAME"
mkdir -p "$TEAM/bolts"
cli run "$WORK/block.json" --save "$TEAM/bolts/hex-bolt.mitcad"
cli version save "$TEAM/bolts/hex-bolt.mitcad" -m "Add the bolt" --author "$AUTHOR"
echo "ok   the projects are made"

ui_start_display

echo "--- 1. The projects, a folder and its sub-folder"
ui_start_app --no-native-dialogs --open "$SHOP/plate.mitcad"
ui_expect_log "Opened $SHOP/plate.mitcad" "a design of Shop is open" 20
ui_focus_main
open_browser
ui_expect_new "Project browser starts at Shop (current project)" "it starts in the current project"
ui_expect_new "Project browser shows Shop: parts/ | plate.mitcad v1" "the top: the folder first, then the design"
ui_expect_log "Project browser folder Shop at " "the projects are in the folders' tree"
ui_expect_log "Project browser folder Team at " "the other project too"
ui_expect_log "Project browser folder Shop/parts/sub/ at " "with their folders"
ui_mark
ui_step "into the folder (Enter)"       ui_key Return
ui_expect_new "Project browser shows Shop/parts/: sub/ | gear.mitcad v1" "the folder: its sub-folder, its design"
ui_mark
ui_double_click_expect "Project browser item Shop/parts/sub/" "Project browser shows Shop/parts/sub/: pin.mitcad v1" \
  "a double-click goes into the sub-folder"
ui_mark
ui_step "up (Backspace)"                 ui_key BackSpace
ui_expect_new "Project browser shows Shop/parts/: sub/ | gear.mitcad v1" "Backspace goes up"
ui_mark
ui_step "the path bar's project"        ui_click_logged "Project browser path Shop"
ui_expect_new "Project browser shows Shop: parts/ | plate.mitcad v1" "the path bar goes back to the top"

echo "--- 2. The search finds a design of the other project; Enter opens it"
ui_mark
ui_step "the search (Alt+S)"             ui_key alt+s
ui_type_text "hex"
ui_expect_new "Project browser search \"hex\" in all projects: 1 found: Team/bolts/hex-bolt.mitcad" \
  "found in the other project as it is typed"
ui_mark
ui_step "in this project (Alt+I, Down)"  ui_key alt+i Down
ui_expect_new "Project browser search scope: this project" "the scope"
ui_expect_new "Project browser search \"hex\" in Shop: 0 found" "not in the place's project"
ui_mark
ui_step "in all projects again (Up)"     ui_key Up
ui_expect_new "Project browser search \"hex\" in all projects: 1 found: Team/bolts/hex-bolt.mitcad" "found again"
ui_step "back to the search (Alt+S)"     ui_key alt+s
ui_mark
ui_step "open it (Enter)"                ui_key Return
ui_expect_new "Project browser Open from Project: chose $TEAM/bolts/hex-bolt.mitcad" "chosen"
ui_focus_main
ui_expect_new "Opened $TEAM/bolts/hex-bolt.mitcad" "the other project's design opened" 20

echo "--- 3. The place remembered; Read-only"
open_browser
ui_expect_new "Project browser starts at Team/bolts/ (remembered)" "it starts in the design's folder"
ui_expect_new "Project browser shows Team/bolts/: hex-bolt.mitcad v1" "the folder's design"
ui_mark
ui_step "the search (Alt+S)"             ui_key alt+s
ui_type_text "gear"
ui_expect_new "Project browser search \"gear\" in all projects: 1 found: Shop/parts/gear.mitcad" "found"
ui_mark
ui_step "read-only (Alt+R)"              ui_key alt+r
ui_expect_new "Project browser read-only: on" "Read-only checked"
ui_step "back to the search (Alt+S)"     ui_key alt+s
ui_mark
ui_step "open it (Enter)"                ui_key Return
ui_expect_new "Project browser Open from Project: chose $SHOP/parts/gear.mitcad (read-only)" "chosen read-only"
ui_focus_main
ui_expect_new "Open Read-Only: $SHOP/parts/gear.mitcad" "opened read-only" 20
ui_key ctrl+q
expect_exit "exited"

echo "--- 4. After a restart: the place shown last; Cancel"
ui_start_app --no-native-dialogs
UI_MARK=0 # a new log
open_browser
ui_expect_new "Project browser starts at Shop/parts/ (remembered)" "the place shown last"
ui_expect_new "Project browser shows Shop/parts/: sub/ | gear.mitcad v1" "its contents"
ui_mark
ui_step "cancel (Escape)"                ui_key Escape
ui_expect_new "Project browser Open from Project: cancelled" "cancelled"
ui_focus_main
ui_key ctrl+q
expect_exit "exited"

ui_finish "UI Open from Project test"
