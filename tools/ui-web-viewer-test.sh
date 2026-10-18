#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/web app/files app/projects app/MainWindow.cpp
# check-all sources: core/vcs core/model/src/file/settings.rs core/ffi/src/vcs.rs tools/cli
# A project's web viewer (mitcad#423) through the real UI, checked with the
# system's git:
#   1. A project of two designs (made with mitcad-cli, one in a folder):
#      Update Web Viewer says it is not turned on; File > Project > Web
#      Viewer writes both pages, the open design's thumbnail, designs.json
#      and index.html (no scripts, both designs linked), records them with
#      the setting in one version, and git status is clean.
#   2. A change saved: the version holds the design, its page (rewritten)
#      and the index; the other design's page is untouched.
#   3. Project Settings > Web viewer: Update on each Save turned off (a
#      version of the marker); the next Save records the design but leaves
#      its page as it was.
#   4. The design in the folder renamed with git: Update Web Viewer writes
#      its page under the new name and removes the old one, in a version.
#
# git's configuration and HOME are the test's own. Runs headless on Xvfb
# (see ui-test-lib.sh).
# Usage: tools/ui-web-viewer-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

command -v git > /dev/null || { echo "git is required (apt-get install git)" >&2; exit 2; }
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}

WORK=$(mktemp -d /tmp/mitcad-ui-web-viewer.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
export HOME=$WORK/home
mkdir -p "$HOME"
export GIT_CONFIG_GLOBAL=$WORK/gitconfig
export GIT_CONFIG_NOSYSTEM=1
unset EMAIL GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL GIT_DIR GIT_WORK_TREE MITCAD_GIT
printf '[user]\n\tname = Ada Tester\n\temail = ada@example.invalid\n[init]\n\tdefaultBranch = main\n' \
  > "$GIT_CONFIG_GLOBAL"
AUTHOR="Ada Tester <ada@example.invalid>"
PROJECT=$HOME/Documents/Mitcad/showroom
WEB=$PROJECT/web
PLATE=$PROJECT/plate.mitcad
BRACKET=$PROJECT/parts/bracket.mitcad

cli() {
  "$CLI" "$@" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli $*"; }
}

in_git() { git -C "$PROJECT" "$@"; }

expect_clean() {
  local status
  status=$(in_git status --porcelain 2>&1)
  [ -z "$status" ] || ui_fail "$1: git status: $status"
  echo "ok   $1"
}

# expect_in_head path... description: the newest version changed these
# files (and maybe others).
expect_in_head() {
  local changed description=${*: -1}
  changed=$(in_git diff-tree --no-commit-id --name-only -r HEAD)
  for path in "${@:1:$#-1}"; do
    grep -qxF -- "$path" <<< "$changed" || ui_fail "$description: $path is not in the version ($changed)"
  done
  echo "ok   $description"
}

expect_file() {
  [ -s "$1" ] || ui_fail "$2: $1 is missing"
  echo "ok   $2"
}

digest() { sha256sum "$1" | cut -c1-64; }

quit() {
  ui_focus_main
  ui_key ctrl+q
  for _ in $(seq 1 50); do
    kill -0 "$UI_RUNNER" 2> /dev/null || break
    sleep 0.2
  done
  kill -0 "$UI_RUNNER" 2> /dev/null && ui_fail "Mitcad did not exit"
  UI_RUNNER=""
  echo "ok   exited"
}

echo "--- A project of two designs"
cat > "$WORK/plate.json" << 'EOF'
[
  {"cmd": "add_parameter", "name": "w", "value": 10},
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [-20, -15],
    "length": 40, "width": 30, "height": 20, "operation": "new_body"}}
]
EOF
cat > "$WORK/bracket.json" << 'EOF'
[
  {"cmd": "add_feature", "def": {"type": "box", "plane": "xy", "corner": [0, 0],
    "length": 10, "width": 60, "height": 5, "operation": "new_body"}}
]
EOF
cli project init "$PROJECT" --author "$AUTHOR"
mkdir -p "$PROJECT/parts"
cli run "$WORK/plate.json" --save "$PLATE"
cli run "$WORK/bracket.json" --save "$BRACKET"
cli version save "$PLATE" "$BRACKET" -m "Two designs" --author "$AUTHOR"
expect_clean "the designs are a version"

ui_start_display

echo "--- 1. Web Viewer: both pages, the index, one version"
ui_start_app --open "$PLATE"
UI_MARK=0
ui_expect_log "Opened $PLATE" "the plate opened"
ui_mark
ui_step "update web viewer (search)"     ui_command "Update Web Viewer"
ui_expect_new "Web viewer: not turned on in $PROJECT" "Update does nothing before the viewer is on"
[ ! -e "$WEB" ] || ui_fail "Update made $WEB"
ui_mark
ui_step "web viewer (search)"            ui_command "Web Viewer"
ui_expect_new "Web viewer: page of parts/bracket.mitcad: parts--bracket.html (" "the bracket's page" 60
ui_expect_new "Web viewer: page of plate.mitcad: plate.html (" "the plate's page" 60
ui_expect_new "Web viewer: wrote 2 page(s) and the index in $WEB" "two pages and the index" 60
ui_expect_new "Web viewer: recorded as version " "recorded as a version" 20
expect_file "$WEB/index.html" "web/index.html"
expect_file "$WEB/designs.json" "web/designs.json"
expect_file "$WEB/plate.html" "the plate's page"
expect_file "$WEB/parts--bracket.html" "the bracket's page"
expect_file "$WEB/plate.png" "the open design's thumbnail"
grep -q 'href="plate.html"' "$WEB/index.html" || ui_fail "the index does not link plate.html"
grep -q 'href="parts--bracket.html"' "$WEB/index.html" || ui_fail "the index does not link parts--bracket.html"
grep -q '<img src="plate.png"' "$WEB/index.html" || ui_fail "the index lacks the plate's thumbnail"
grep -q '<h1>showroom</h1>' "$WEB/index.html" || ui_fail "the index lacks the project's name"
grep -q 'Saved by Ada Tester, ' "$WEB/index.html" || ui_fail "the index lacks who saved"
grep -qi '<script' "$WEB/index.html" && ui_fail "the index has a script"
grep -q '"page": "parts--bracket.html"' "$WEB/designs.json" || ui_fail "designs.json lacks the bracket"
grep -q '"file": "parts/bracket.mitcad"' "$WEB/designs.json" || ui_fail "designs.json lacks the bracket's file"
echo "ok   the index links both pages, without scripts"
expect_in_head .mitcad/project.json web/index.html web/designs.json web/plate.html web/parts--bracket.html \
  web/plate.png "the setting and the folder in one version"
[ "$(in_git log -1 --format=%s)" = "Change project settings: web viewer on" ] ||
  ui_fail "the version is '$(in_git log -1 --format=%s)'"
grep -q '"update_on_save": true' "$PROJECT/.mitcad/project.json" || ui_fail "the marker lacks the setting"
expect_clean "git status is clean"
bracket_page=$(digest "$WEB/parts--bracket.html")
plate_page=$(digest "$WEB/plate.html")
quit

echo "--- 2. A Save rewrites the design's page and the index, in its version"
ui_start_app --open "$PLATE" --set w=12
UI_MARK=0
ui_expect_log "Opened $PLATE" "the plate opened, changed"
ui_mark
ui_step "save (Ctrl+S)"                  ui_key ctrl+s
ui_expect_new "Web viewer: wrote the page of plate.mitcad and the index in $WEB" "the page written on Save" 60
ui_expect_new "Version recorded: plate.mitcad " "the version recorded" 20
# The index shows the time to the minute: within the same minute it is
# unchanged and so not in the version.
expect_in_head plate.mitcad web/plate.html web/designs.json "the page in the design's version"
[ "$(digest "$WEB/plate.html")" != "$plate_page" ] || ui_fail "the plate's page was not rewritten"
[ "$(digest "$WEB/parts--bracket.html")" = "$bracket_page" ] || ui_fail "the bracket's page changed"
grep -q 'href="parts--bracket.html"' "$WEB/index.html" || ui_fail "the index lost the bracket"
echo "ok   only the saved design's page changed"
expect_clean "git status is clean after the Save"
plate_page=$(digest "$WEB/plate.html")

echo "--- 3. Project Settings: not updated on each Save"
ui_focus_main
ui_mark
ui_step "project settings (search)"      ui_command "Project Settings"
ui_focus_dialog '^Project Settings - showroom$'
ui_expect_new "Project Settings web viewer: on, updated on each Save" "the setting shown"
ui_mark
ui_step "update on each Save off (Alt+U)" ui_key alt+u
ui_expect_new "Project Settings: web viewer updated on each Save off, recorded: " "turned off, recorded"
ui_step "close it (Escape)"              ui_key Escape
ui_focus_main
[ "$(in_git log -1 --format=%s)" = "Change project settings: web viewer not updated on each save" ] ||
  ui_fail "the version is '$(in_git log -1 --format=%s)'"
echo "ok   the setting is a version"
ui_mark
ui_step "undo the change of w (Ctrl+Z)"  ui_key ctrl+z
ui_step "save (Ctrl+S)"                  ui_key ctrl+s
ui_expect_new "Web viewer: not updated on Save of plate.mitcad (off in Project Settings)" "the page left" 20
ui_expect_new "Version recorded: plate.mitcad " "the design still recorded" 20
[ "$(digest "$WEB/plate.html")" = "$plate_page" ] || ui_fail "the plate's page changed"
expect_in_head plate.mitcad "the design's version"
in_git diff-tree --no-commit-id --name-only -r HEAD | grep -q '^web/' && ui_fail "the version changed web/"
echo "ok   nothing of web/ in that version"

echo "--- 4. A design renamed: Update Web Viewer moves its page"
in_git mv parts/bracket.mitcad parts/holder.mitcad
in_git commit -qm "Rename the bracket"
ui_focus_main
ui_mark
ui_step "update web viewer (search)"     ui_command "Update Web Viewer"
ui_expect_new "Web viewer: wrote 2 page(s) and the index in $WEB, removed $WEB/parts--bracket.html" \
  "the new page written, the old one removed" 60
ui_expect_new "Web viewer: recorded as version " "recorded" 20
[ ! -e "$WEB/parts--bracket.html" ] || ui_fail "the old page is still there"
expect_file "$WEB/parts--holder.html" "the page under the new name"
grep -q 'href="parts--holder.html"' "$WEB/index.html" || ui_fail "the index does not link the new page"
grep -q 'parts--bracket' "$WEB/index.html" && ui_fail "the index still links the old page"
expect_in_head web/parts--bracket.html web/parts--holder.html web/index.html "the move in a version"
[ "$(in_git log -1 --format=%s)" = "Update the web viewer" ] || ui_fail "the version is '$(in_git log -1 --format=%s)'"
expect_clean "git status is clean at the end"
quit

ui_finish "UI web viewer test"
