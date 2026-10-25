#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/view/ViewController.cpp app/files/RemotePreferences.cpp app/files/RemotePreferences.hpp
# check-all sources: app/files/Print3d.cpp core/vcs/src/remote/cli.rs
# Preferences on a low screen (mitcad#690), with git and without it, on a
# test screen 768 pixels high:
#   1. git found on PATH: the Cloud page names it.
#   2. MITCAD_GIT naming a file that is not there: the Cloud page says so.
#   3. No git on PATH (git not installed): the Cloud page's long message,
#      which once made the window wider than the screen, wraps.
# Each time Preferences opens on its Cloud page: the size it asks for (its
# largest page's) and its minimum size fit a 1366 x 768 screen with a title
# bar and a taskbar, it is resizable, its frame lies within the screen's
# available area, and the page list is as wide as its entries; on its
# General page (Home in the page list) the minimum is that page's, and it
# fits too. tools/ui-windows-preferences-test.ps1 does the same in the
# Windows VM.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-preferences-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

UI_SCREEN_HEIGHT=768
WORK=$(mktemp -d /tmp/mitcad-ui-preferences.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
unset MITCAD_GIT

# since "sed expression": the last line logged since the mark that the
# expression prints.
since() { tail -n +$((UI_MARK + 1)) "$UI_LOG" | sed -n "$1" | tail -1; }

# fits what width height: within 1366 x 690, what a 1366 x 768 screen shows
# below a title bar and above a taskbar (mitcad#25).
fits() {
  [ "${2:-0}" -gt 0 ] && [ "$2" -le 1366 ] && [ "${3:-0}" -gt 0 ] && [ "$3" -le 690 ] ||
    ui_fail "Preferences: $1 ${2:-?} x ${3:-?}, more than a 1366 x 768 screen shows"
  echo "ok   Preferences: $1 $2 x $3 fits a 1366 x 768 screen"
}

# expect_cloud "regex" description: the Cloud page's line about the git
# program ("Preferences: cloud: <text>") since the mark matches.
expect_cloud() {
  local text
  for _ in $(seq 1 30); do
    text=$(since 's/^Preferences: cloud: //p')
    if [ -n "$text" ]; then
      grep -qE -- "^$1" <<< "$text" || ui_fail "$2: the Cloud page says '$text'"
      echo "ok   $2: $text"
      return
    fi
    sleep 0.2
  done
  ui_fail "$2: the Cloud page's git line is not in the log"
}

# preferences_fit "Cloud page regex" description: Preferences opened on its
# Cloud page, the Cloud page's text about git, the window's sizes and
# frame, then its General page, then closed.
preferences_fit() {
  local width height x y screen_width screen_height screen_x screen_y state list entries
  ui_mark
  ui_step "Preferences: Cloud (search)"      ui_command "Preferences: Cloud"
  ui_expect_new "Preferences opened: Cloud, " "Preferences on its Cloud page"
  expect_cloud "$1" "$2"
  ui_expect_new "Preferences page list " "Preferences shown"
  read -r width _ height <<< "$(since 's/^Preferences opened: Cloud, \([0-9]* x [0-9]*\)$/\1/p')"
  fits "the size asked for" "$width" "$height"
  read -r width height state <<< "$(since 's/^Preferences minimum \([0-9]*\) x \([0-9]*\), \([a-z]*\)$/\1 \2 \3/p')"
  fits "the minimum size" "$width" "$height"
  [ "$state" = resizable ] || ui_fail "Preferences cannot be resized (${state:-not logged})"
  echo "ok   Preferences can be resized"
  read -r width height x y screen_width screen_height screen_x screen_y <<< "$(since \
    's/^Preferences frame \([0-9]*\) x \([0-9]*\) at \(-*[0-9]*\),\(-*[0-9]*\) on a screen of \([0-9]*\) x \([0-9]*\) at \(-*[0-9]*\),\(-*[0-9]*\)$/\1 \2 \3 \4 \5 \6 \7 \8/p')"
  [ -n "${screen_y:-}" ] || ui_fail "Preferences' frame not logged"
  [ "$screen_height" -le 768 ] || ui_fail "the test screen is $screen_width x $screen_height, not 768 high"
  [ "$x" -ge "$screen_x" ] && [ "$y" -ge "$screen_y" ] &&
    [ $((x + width)) -le $((screen_x + screen_width)) ] && [ $((y + height)) -le $((screen_y + screen_height)) ] ||
    ui_fail "Preferences' frame $width x $height at $x,$y is not within the screen's $screen_width x $screen_height at $screen_x,$screen_y"
  [ "$width" -le 1366 ] || ui_fail "Preferences' frame is $width wide, more than a 1366 x 768 screen"
  echo "ok   Preferences' frame $width x $height at $x,$y lies within the screen ($screen_width x $screen_height)"
  read -r list entries <<< "$(since 's/^Preferences page list \([0-9]*\) wide, its entries \([0-9]*\)$/\1 \2/p')"
  [ "${entries:-0}" -gt 0 ] && [ "${list:-0}" -ge "$entries" ] ||
    ui_fail "the page list is ${list:-?} wide, its entries need ${entries:-?}"
  echo "ok   the page list ($list) shows its entries whole ($entries)"
  ui_focus_dialog '^Preferences$'
  ui_mark
  ui_step "the General page (Home)"          ui_key Home
  ui_expect_new "Preferences page: General, minimum " "Preferences on its General page"
  read -r width _ height <<< "$(since 's/^Preferences page: General, minimum \([0-9]* x [0-9]*\)$/\1/p')"
  fits "the General page's minimum size" "$width" "$height"
  ui_step "close it (Esc)"                   ui_key Escape
  ui_focus_main
}

ui_start_display

echo "--- 1. git found on PATH"
command -v git > /dev/null || ui_fail "git is not installed on this machine"
ui_start_app
preferences_fit "git [0-9][^ ]* at " "the Cloud page names the git program found"
ui_stop_app

echo "--- 2. MITCAD_GIT naming a file that is not there"
export MITCAD_GIT=$WORK/no-such-git
ui_start_app
unset MITCAD_GIT
preferences_fit "the git program set for Mitcad \(MITCAD_GIT\) is unavailable: " "the Cloud page says it is not there"
ui_stop_app

echo "--- 3. No git on PATH: git not installed"
# Mitcad started with a PATH of only the folders without a git program.
NO_GIT_PATH=""
IFS=: read -r -a PATH_FOLDERS <<< "$PATH"
for folder in "${PATH_FOLDERS[@]}"; do
  [ -n "$folder" ] && [ ! -e "$folder/git" ] && NO_GIT_PATH=${NO_GIT_PATH:+$NO_GIT_PATH:}$folder
done
cat > "$WORK/mitcad-without-git" << EOF
#!/bin/sh
PATH='$NO_GIT_PATH' exec '$UI_APP' "\$@"
EOF
chmod +x "$WORK/mitcad-without-git"
APP=$UI_APP
UI_APP=$WORK/mitcad-without-git
ui_start_app
UI_APP=$APP
preferences_fit "git is not installed: " "the Cloud page says git is not installed"
ui_stop_app

ui_finish "UI preferences test"
