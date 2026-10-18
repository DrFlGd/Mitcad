#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/framework/ChromeStyle.cpp app/framework/ChromeStyle.hpp app/framework/Restart.cpp
# check-all sources: app/framework/Restart.hpp app/framework/FloatingLayout.cpp app/framework/FloatingLayout.hpp
# check-all sources: app/framework/GlassCard.cpp app/framework/TitleBar.cpp app/framework/Ribbon.cpp
# check-all sources: app/main.cpp app/view/ViewController.cpp app/MainWindow.cpp app/OcctViewer.cpp tools/cli
# The window's layout chosen in Preferences (mitcad#419) through the real UI:
#   1. Without a choice the platform's layout (Docked here); Preferences >
#      Display > Layout Floating asks Restart Mitcad, Later keeps it in the
#      settings for the next start.
#   2. Docked again is the window's layout: saved, no question.
#   3. Floating, Restart Now with unsaved changes: the window asks about
#      them as quitting does; Cancel keeps Mitcad open and nothing restarts.
#   4. Saved, Floating, Restart Now: Mitcad quits and would start again
#      (MITCAD_TEST_NO_RESTART=1 logs the command) with the run's settings
#      options and the design, without --chrome.
#   5. The next start is Floating by the preference (a plate open): only
#      the menu bar is above the view (mitcad#425); the title card and the
#      group cards float over its top, every other card starts below them,
#      at the default size, in a narrow and a wide window; with the plate
#      zoomed in to fill the view, a click between two group cards selects
#      its face and a click on the title card selects nothing.
#   6. --chrome=docked overrides the preference.
#
# Runs headless on Xvfb (see ui-test-lib.sh).
# Usage: tools/ui-chrome-layout-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

WORK=$(mktemp -d /tmp/mitcad-ui-chrome-layout.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
FILE=$WORK/part.mitcad
# A 200 x 200 x 10 mm plate centred on the origin: Body1 (F2.b0), for the
# click between the group cards.
CLI=${UI_CLI:-$(cd "$(dirname "$UI_APP")/.." && pwd)/tools/cli/mitcad-cli}
PLATE=$WORK/plate.mitcad
cat > "$WORK/plate.json" << 'JSON'
[
  {"cmd": "sketch.create"},
  {"cmd": "sketch.add_rectangle", "sketch": "F1", "corner": [-100, -100], "width": 200, "height": 200},
  {"cmd": "add_feature", "def": {"type": "extrude",
    "profiles": [{"sketch": "F1", "region": "r{c1[c4,c2],c2[c1,c3],c3[c2,c4],c4[c3,c1]}"}],
    "extent": {"type": "distance", "distance": 10}, "operation": "new_body"}}
]
JSON
"$CLI" run "$WORK/plate.json" --save "$PLATE" > "$WORK/cli.log" 2>&1 || { cat "$WORK/cli.log"; ui_fail "mitcad-cli"; }
CONF=$XDG_CONFIG_HOME/Mitcad/Mitcad.conf
# The restart is logged, not started: the test starts the next run itself.
export MITCAD_TEST_NO_RESTART=1

# choose_layout key: Preferences on its Display page, the Layout (Alt+L)
# set with the key (Home: Docked, End: Floating), OK.
choose_layout() {
  ui_mark
  ui_step "Preferences: Display (search)"  ui_command "Preferences: Display"
  ui_expect_new "Preferences opened: Display, " "Preferences on its Display page"
  ui_focus_dialog '^Preferences$'
  ui_step "Layout (Alt+L, $1)"             ui_key alt+l "$1"
  ui_step "OK (Enter)"                     ui_key Return
  ui_focus_main
}

# expect_exit description: the app has ended on its own, normally.
expect_exit() {
  for _ in $(seq 1 50); do
    kill -0 "$UI_RUNNER" 2> /dev/null || break
    sleep 0.2
  done
  kill -0 "$UI_RUNNER" 2> /dev/null && ui_fail "$1: Mitcad did not exit"
  if command -v gdb > /dev/null && [ "${UI_GDB:-0}" = 1 ]; then
    grep -q "exited normally" "$UI_LOG" || ui_fail "$1: abnormal exit"
  else
    wait "$UI_RUNNER" || ui_fail "$1: abnormal exit"
  fi
  UI_RUNNER=""
  echo "ok   $1"
}

# cards_below_bar description min max: the last "Floating cards:" line, of
# a window min to max wide (waited for): only the menu bar is above the view
# (the view's top is the bar's bottom, mitcad#425); the title card and at
# least two group cards float over the view below the bar; the Browser's
# card and every other card start below the floating rows' bottom.
cards_below_bar() {
  local line="" width="" bar view rows rest name xy size y count=0 groups=0
  for _ in $(seq 1 25); do
    line=$(grep '^Floating cards: ' "$UI_LOG" | tail -1)
    width=$(sed -n 's/^Floating cards: window \([0-9]*\)x.*/\1/p' <<< "$line")
    [ -n "$width" ] && [ "$width" -ge "$2" ] && [ "$width" -le "$3" ] && break
    sleep 0.2
  done
  [ -n "$width" ] && [ "$width" -ge "$2" ] && [ "$width" -le "$3" ] ||
    ui_fail "$1: no Floating cards line of a window $2 to $3 wide (last: '$line')"
  read -r bar view rows <<< "$(sed -n 's/^Floating cards: window [0-9]*x[0-9]*, top bar bottom \([0-9]*\), view top \([0-9]*\), floating rows bottom \([0-9]*\);.*/\1 \2 \3/p' <<< "$line")"
  [ -n "$bar" ] && [ "$bar" -gt 0 ] || ui_fail "$1: no top bar in '$line'"
  [ "$view" = "$bar" ] || ui_fail "$1: the view's top $view is not the menu bar's bottom $bar ($line)"
  [ "$rows" -gt "$bar" ] || ui_fail "$1: no floating rows below the bar ($line)"
  rest=${line#*; }
  while read -r name xy size; do
    [ -n "$name" ] || continue
    y=${xy#*,}
    [ "$y" -ge "$bar" ] || ui_fail "$1: $name's top $y is above the top bar's bottom $bar ($line)"
    case $name in
      titleCard | noticeCard) ;;
      ribbonGroup_*) groups=$((groups + 1)) ;;
      *) [ "$y" -ge "$rows" ] || ui_fail "$1: $name's top $y is above the floating rows' bottom $rows ($line)" ;;
    esac
    count=$((count + 1))
  done < <(tr ';' '\n' <<< "$rest" | sed 's/^ *//')
  case $rest in
    *"titleCard "*) ;;
    *) ui_fail "$1: no title card in '$line'" ;;
  esac
  [ "$groups" -ge 2 ] || ui_fail "$1: $groups group cards in '$line'"
  case $rest in
    *browserDock*) ;;
    *) ui_fail "$1: no Browser card in '$line'" ;;
  esac
  echo "ok   $1: the view below the menu bar ($bar), $count cards, $groups group cards, the others below the rows' bottom $rows (window ${width} wide)"
}

# count_new "text": how many lines logged since the mark contain the text.
count_new() { tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -cF -- "$1"; }

# click_between_cards: the plate seen from the top, zoomed in until it fills
# the view (also behind the floating rows); a click between two group cards
# selects its face, a click on the title card's middle selects nothing.
click_between_cards() {
  local line card xy size cx cy
  ui_step "top view (search)"              ui_command "Top View"
  ui_step "fit (F6)"                       ui_key F6
  ui_sync
  ui_step "zoom in at the plate's middle"  xdotool mousemove $(ui_logged_at "Body F2.b0") click --repeat 14 --delay 60 4
  ui_wait_idle
  ui_expect_log "Floating view between the cards at" "a place between two group cards is logged"
  ui_mark
  ui_step "click between two group cards"  ui_click_logged "Floating view between the cards"
  ui_expect_new "Selected: 1 face [face F2:" "the click between the cards reached the view: the plate's face"
  line=$(grep '^Floating cards: ' "$UI_LOG" | tail -1)
  card=$(tr ';' '\n' <<< "${line#*; }" | sed 's/^ *//' | grep '^titleCard ')
  read -r _ xy size <<< "$card"
  [ -n "$size" ] || ui_fail "no title card in '$line'"
  cx=$((${xy%,*} + ${size%x*} / 2))
  cy=$((${xy#*,} + ${size#*x} / 2))
  ui_mark
  ui_step "click the title card's middle"  xdotool mousemove $((X + cx * UI_SCALE)) $((Y + cy * UI_SCALE)) click 1
  ui_sync
  sleep 0.5
  [ "$(count_new "Selected: ")" = 0 ] || ui_fail "a click on the title card reached the view"
  echo "ok   the title card takes its click"
}

ui_start_display

echo "--- 1. The platform's layout; Floating chosen, Later"
ui_start_app --no-native-dialogs
ui_expect_log "Chrome: docked (the platform's default)" "Docked without a choice"
grep -q '^Floating cards: ' "$UI_LOG" && ui_fail "Floating cards in the Docked layout"
choose_layout End
ui_expect_new "Layout preference: floating (restart asked)" "Floating chosen, a restart asked"
ui_focus_dialog '^Restart Mitcad$'
ui_step "later (Escape)"                  ui_key Escape
ui_focus_main
ui_expect_new "Layout restart: later (floating at the next start)" "kept for the next start"
grep -q '^layout=floating$' "$CONF" || ui_fail "the layout is not saved"
echo "ok   saved as appearance/layout"
ui_crashed && ui_fail "Mitcad ended after Later"

echo "--- 2. Docked again: the window's layout, no question"
choose_layout Home
ui_expect_new "Layout preference: docked (in use)" "Docked is the window's layout"
grep -q '^layout=docked$' "$CONF" || ui_fail "Docked is not saved"
echo "ok   saved"

echo "--- 3. Floating, Restart Now with unsaved changes: Cancel keeps Mitcad open"
ui_step "create a sketch"                 ui_create_sketch xy
ui_step "rectangle tool (R)"              ui_key r
ui_step "first corner at the origin"      ui_sketch_click 0 0
ui_step "opposite corner"                 ui_sketch_click 40 20
ui_expect_log "Added rectangle" "a rectangle: a change to save"
ui_step "finish it (Ctrl+Enter)"          ui_key ctrl+Return
ui_expect_log "Sketch finished" "a change to save"
choose_layout End
ui_expect_new "Layout preference: floating (restart asked)" "a restart asked"
ui_focus_dialog '^Restart Mitcad$'
ui_step "restart now (Enter)"             ui_key Return
ui_expect_new "Layout restart: now" "Restart Now"
ui_expect_new "Restart: closing the window" "the window closes first"
ui_focus_dialog '^Mitcad$'
ui_step "cancel (Escape)"                 ui_key Escape
ui_expect_new "Restart cancelled: Mitcad stayed open" "cancelled as quitting would be"
ui_crashed && ui_fail "Mitcad ended although the close was cancelled"
grep -q '^Restart (test, not started)' "$UI_LOG" && ui_fail "a restart after the cancel"
echo "ok   Mitcad still runs, nothing restarted"
ui_focus_main

echo "--- 4. Saved; Floating, Restart Now: Mitcad quits and starts again"
ui_step "save as (Ctrl+Shift+S)"          ui_key ctrl+shift+s
ui_step "type the path, Enter"            ui_type_path "Save As" "$FILE"
ui_expect_log "Saved $FILE" "saved"
ui_focus_main
choose_layout Home
ui_expect_new "Layout preference: docked (in use)" "Docked again"
choose_layout End
ui_expect_new "Layout preference: floating (restart asked)" "a restart asked"
ui_focus_dialog '^Restart Mitcad$'
ui_key Return
ui_expect_new "Restart (test, not started): " "the restart's command" 20
COMMAND=$(sed -n 's/^Restart (test, not started): //p' "$UI_LOG" | tail -1)
case " $COMMAND " in
  *" --chrome"*) ui_fail "the restart keeps --chrome: $COMMAND" ;;
esac
for part in "--no-native-dialogs" "--no-recovery" "--open $FILE"; do
  case " $COMMAND " in
    *" $part "*) ;;
    *) ui_fail "the restart has no '$part': $COMMAND" ;;
  esac
done
echo "ok   the run's settings options and the design, no --chrome: $COMMAND"
expect_exit "Mitcad quit to restart"

echo "--- 5. The next start: Floating by the preference; only the menu bar above the view, the cards"
ui_start_app --no-native-dialogs --open "$PLATE"
ui_expect_log "Chrome: floating (the preference)" "Floating by the preference"
ui_expect_log "Floating cards: window " "the cards placed"
cards_below_bar "at the default size" 1280 1280
click_between_cards
# As narrow as the window gets: the ribbon's groups collapse.
ui_step "a narrow window"                 xdotool windowsize "$UI_WINDOW" 800 640
cards_below_bar "in a narrow window" 1 1279
ui_step "a wide window again"             xdotool windowsize "$UI_WINDOW" 1400 900
cards_below_bar "in a wide window" 1400 1400
ui_stop_app

echo "--- 6. --chrome=docked overrides the preference"
ui_start_app --no-native-dialogs --chrome=docked
ui_expect_log "Chrome: docked (--chrome; the preference floating)" "--chrome decides, the preference named"
grep -q '^Floating cards: ' "$UI_LOG" && ui_fail "Floating cards in the Docked layout"
echo "ok   no cards"

ui_finish "UI chrome layout test"
