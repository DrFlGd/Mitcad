#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/files core/ipt core/import core/f3d core/ffi/src/iam_import.rs core/ffi/src/ipt_import.rs
# check-all sources: core/ffi/src/ipt_history.rs core/model/src/document/components.rs core/tests
# An .iam assembly's parts with their design history (mitcad#507), through
# File > Open:
#   - the test project of test_exchange --write-iam: Import Assembly asks
#     how the parts come in, Components with their design history on by
#     default. On, the progress follows the parts (part n of 3), the cube
#     part's sketch and extrusion are replayed into its component, the
#     report says per part whether it was replayed or came in as its stored
#     bodies and why. Off, every part comes in as its stored bodies, one
#     base feature per part. The components' bodies have the same volume
#     both ways; the choice is remembered (the next dialog shows it off).
#   - with the test files' assembly 1dc60c2a (seven parts; never committed):
#     MITCAD_IAM_UI_ASSEMBLY, else found by its content id under the
#     folders of MITCAD_IPT_CORPUS (an INDEX.tsv or index.tsv of a folder
#     naming it by its SHA-256, else every .iam hashed): the same, each
#     part's replayed feature count listed. Skipped without it.
#
# Runs headless on Xvfb (see ui-test-lib.sh); Qt's own file dialogs
# (--no-native-dialogs), so that paths can be typed.
# Usage: tools/ui-iam-history-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

TEST_EXCHANGE=${UI_TEST_EXCHANGE:-$(cd "$(dirname "$UI_APP")/.." && pwd)/core/test_exchange}
WORK=$(mktemp -d "${TMPDIR:-/tmp}/mitcad-ui-iam.XXXXXX")
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
# How long an import may take (a real assembly's parts replayed).
IMPORT_WAIT=${UI_IAM_IMPORT_WAIT:-1200}

# open_assembly path: a new app, File > Open (Ctrl+O) of the assembly.
open_assembly() {
  ui_stop_app
  ui_start_app --demo --no-native-dialogs
  ui_mark
  ui_step "open $(basename "$1") (Ctrl+O)" ui_key ctrl+o
  ui_type_path 'Open' "$1"
}

# import_assembly path on|off label: the assembly opened, Import Assembly
# set to the history wanted, the import to its end and its report closed;
# the lines it logged kept in $WORK/<label>.log.
import_assembly() {
  local path=$1 want=$2 label=$3 name shown
  name=$(basename "$path")
  open_assembly "$path"
  ui_expect_new "Import Assembly dialog opened: $name, parts with their design history " \
    "Import Assembly asks how the parts come in"
  shown=$(tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -F "Import Assembly dialog opened: $name" | tail -1 |
    sed 's/.*design history //')
  ui_focus_dialog '^Import Assembly$'
  if [ "$shown" != "$want" ]; then
    ui_step "parts with their design history $want (Alt+H)" ui_key alt+h
    ui_expect_new "Import Assembly: parts with their design history $want" "the box toggled"
  fi
  ui_step "import (Enter)"                 ui_key Return
  ui_expect_new "Import Assembly: $name, parts with their design history $want" "imported with the history $want"
  ui_expect_new "Import of $name: parts with their design history $want" "the import runs with that choice"
  ui_expect_new "Import report: $name: " "the import finished with a report" "$IMPORT_WAIT"
  ui_focus_dialog '^Import Report$'
  ui_step "close the report (Enter)"       ui_key Return
  ui_focus_main
  ui_expect_new "Imported $name: " "a new document of the assembly's components"
  tail -n +$((UI_MARK + 1)) "$UI_LOG" > "$WORK/$label.log"
}

# components label: "<name>\t<features>\t<bodies>\t<volume>" for each
# component the import logged.
components() {
  sed -n 's/^.*Imported [^:]*: component \(.*\): \([0-9]*\) features, \([0-9]*\) bodies, volume \([0-9.e+-]*\) mm3$/\1\t\2\t\3\t\4/p' \
    "$WORK/$1.log" | LC_ALL=C sort -t $'\t' -k1,1
}

# compare_volumes on off: every component of the import with the history
# has the volume of the one without within the import's tolerance (1e-3
# relative, as the import report matches bodies; at least 1e-6 mm3).
compare_volumes() {
  local result
  result=$(LC_ALL=C join -t $'\t' <(components "$1" | cut -f1,4) <(components "$2" | cut -f1,4) | awk -F'\t' '
    { d = $2 - $3; if (d < 0) d = -d; a = $3 < 0 ? -$3 : $3; limit = 1e-3 * a; if (limit < 1e-6) limit = 1e-6
      if (d > limit) { bad = bad sprintf("%s: %s against %s mm3; ", $1, $2, $3) }
      r = a > 0 ? d / a : d; if (r > worst) worst = r; n++ }
    END { printf "%d %g %s", n, worst, bad }')
  read -r compared worst problems <<< "$result"
  [ "${compared:-0}" -gt 0 ] || ui_fail "no components to compare between $1 and $2"
  [ -z "$problems" ] || ui_fail "the bodies differ with the history: $problems"
  [ "$(components "$1" | wc -l)" = "$(components "$2" | wc -l)" ] ||
    ui_fail "$(components "$1" | wc -l) components with the history, $(components "$2" | wc -l) without"
  echo "ok   $compared components' volumes agree both ways (largest relative difference $worst)"
}

"$TEST_EXCHANGE" --write-iam "$WORK/iam" || ui_fail "cannot write the test assemblies"
ui_start_display

echo "--- the test project, parts with their design history (the default)"
import_assembly "$WORK/iam/top.iam" on test-on
grep -q "Import Assembly dialog opened: top.iam, parts with their design history on" "$WORK/test-on.log" ||
  ui_fail "the history is not on by default"
echo "ok   on by default"
grep -q "Import of top.iam: part [123] of 3: cube.ipt: replayed in " "$WORK/test-on.log" ||
  ui_fail "the progress did not follow the parts: $(grep 'Import of top.iam: part' "$WORK/test-on.log")"
echo "ok   the progress followed the parts (of 3)"
grep -q "Import report part: cube.ipt: replayed, 2 items (2 parametric, 0 partial, 0 fallback, 0 skipped)" \
  "$WORK/test-on.log" || ui_fail "the cube was not replayed: $(grep 'Import report part' "$WORK/test-on.log")"
grep -q "Import report part: pair.ipt: stored bodies: the part has no design history" "$WORK/test-on.log" ||
  ui_fail "the part without a design is not reported so: $(grep 'Import report part' "$WORK/test-on.log")"
grep -q "Import report summary: top.iam (assembly) .* Components with their design history: 1 replayed, 2 as their stored bodies\." \
  "$WORK/test-on.log" || ui_fail "the report does not sum the parts up"
echo "ok   the report: the cube replayed, the parts without a design as their stored bodies"
cube=$(components test-on | awk -F'\t' '$1 == "cube" { print $2 }')
[ "${cube:-0}" -ge 2 ] || ui_fail "the cube's component has ${cube:-no} features: $(components test-on)"
echo "ok   the cube's component has its $cube features"

echo "--- the test project, the parts' stored bodies"
import_assembly "$WORK/iam/top.iam" off test-off
[ "$(grep -c "Import report part: " "$WORK/test-off.log")" = 3 ] || ui_fail "the report does not list the 3 parts"
if grep "Import report part: " "$WORK/test-off.log" | grep -v ": stored bodies: the stored bodies chosen$"; then
  ui_fail "a part came in otherwise than as its stored bodies"
fi
cube=$(components test-off | awk -F'\t' '$1 == "cube" { print $2 }')
[ "${cube:-0}" = 1 ] || ui_fail "the cube's component has ${cube:-no} features, not its bodies' base feature"
echo "ok   every part as its stored bodies, one base feature each"
compare_volumes test-on test-off

echo "--- the choice remembered"
open_assembly "$WORK/iam/top.iam"
ui_expect_new "Import Assembly dialog opened: top.iam, parts with their design history off" \
  "the dialog shows the choice made last"
ui_focus_dialog '^Import Assembly$'
ui_step "cancel (Esc)"                     ui_key Escape
ui_expect_new "Import Assembly cancelled" "cancelled: nothing imported"
ui_focus_main
sleep 1
tail -n +$((UI_MARK + 1)) "$UI_LOG" | grep -qE "Import of top.iam started|Import report" &&
  ui_fail "the cancelled import started"
echo "ok   the import did not start"

# The test files' seven-part assembly (content id 1dc60c2a), never committed.
ASSEMBLY=${MITCAD_IAM_UI_ASSEMBLY:-}
if [ -z "$ASSEMBLY" ] && [ -n "${MITCAD_IPT_CORPUS:-}" ]; then
  IFS=: read -ra dirs <<< "$MITCAD_IPT_CORPUS"
  for dir in "${dirs[@]}"; do
    [ -d "$dir" ] || continue
    for index in "$dir/INDEX.tsv" "$dir/index.tsv"; do
      [ -f "$index" ] || continue
      found=$(awk -F'\t' '{ for (i = 2; i <= NF; i++) if ($i ~ /^1dc60c2a/) { print $1; exit } }' "$index")
      [ -n "$found" ] && [ -f "$dir/$found" ] && { ASSEMBLY=$dir/$found; break 2; }
    done
  done
  if [ -z "$ASSEMBLY" ]; then
    for dir in "${dirs[@]}"; do
      [ -d "$dir" ] || continue
      while IFS= read -r -d '' file; do
        if [ "$(sha256sum "$file" | cut -c1-8)" = 1dc60c2a ]; then
          ASSEMBLY=$file
          break 2
        fi
      done < <(find "$dir" -iname '*.iam' -print0 2> /dev/null)
    done
  fi
fi
if [ -n "$ASSEMBLY" ] && [ -f "$ASSEMBLY" ]; then
  NAME=$(basename "$ASSEMBLY")
  echo "--- the assembly 1dc60c2a, parts with their design history"
  import_assembly "$ASSEMBLY" on real-on
  grep -q "Import of $NAME: part 1 of " "$WORK/real-on.log" || ui_fail "the progress did not follow the parts"
  parts=$(grep -c "Import report part: " "$WORK/real-on.log")
  replayed=$(grep -c "Import report part: .*: replayed, " "$WORK/real-on.log")
  echo "ok   $parts parts, $replayed replayed:"
  grep "Import report part: " "$WORK/real-on.log" | sed 's/.*Import report part: /       /'
  [ "$replayed" -gt 0 ] || ui_fail "no part of 1dc60c2a was replayed"
  featured=$(components real-on | awk -F'\t' '$2 > 1' | wc -l)
  [ "$featured" -gt 0 ] || ui_fail "no component of 1dc60c2a has its features: $(components real-on)"
  echo "ok   $featured components with their parts' features"
  echo "--- the assembly 1dc60c2a, the parts' stored bodies"
  import_assembly "$ASSEMBLY" off real-off
  if grep "Import report part: " "$WORK/real-off.log" | grep -v ": stored bodies: the stored bodies chosen$"; then
    ui_fail "a part of 1dc60c2a came in otherwise than as its stored bodies"
  fi
  compare_volumes real-on real-off
else
  echo "skip the assembly 1dc60c2a: neither MITCAD_IAM_UI_ASSEMBLY nor MITCAD_IPT_CORPUS has it"
fi

grep -q "Recompute failed" "$UI_LOG" && ui_fail "a recompute failed"
ui_finish "UI .iam history test"
