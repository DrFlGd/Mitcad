#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# check-all sources: app/files
# The Cloud section of New Project takes a git address first (mitcad#381):
# an address typed into its Repository address field gives the service,
# server, owner, repository and way to connect (the log's "Cloud address:"
# line, and the fields under Details), whichever form it has: https://,
# git@host:owner/repo.git, ssh://, the address of the repository's page in a
# browser. The Details fields stay editable and make the address again. A
# malformed address is refused with a message before git is asked. Hosts are
# the reserved .invalid ones: nothing is reached.
#
# Runs headless on Xvfb (see ui-test-lib.sh); Qt's own file dialogs
# (--no-native-dialogs). The paste into the Server, Owner and Repository
# fields (a change by more than one character) is checked by the unit test
# of looksLikeRepositoryAddress (app.unit): xdotool types one character at a
# time and no clipboard tool is needed here.
# Usage: tools/ui-cloud-address-test.sh

source "$(dirname "$0")/ui-test-lib.sh"

WORK=$(mktemp -d /tmp/mitcad-ui-cloud-address.XXXXXX)
trap 'ui_cleanup; rm -rf "$WORK"' EXIT
export HOME=$WORK/home
mkdir -p "$HOME"
FOLDER=$HOME/Documents/Mitcad/parts

expect_exit() {
  for _ in $(seq 1 50); do
    kill -0 "$UI_RUNNER" 2> /dev/null || break
    sleep 0.2
  done
  kill -0 "$UI_RUNNER" 2> /dev/null && ui_fail "$1: Mitcad did not exit"
  UI_RUNNER=""
  echo "ok   $1"
}

# address text expected description: the Repository address field gets the
# text; the log says what the fields became.
address() {
  ui_mark
  ui_step "the address (Alt+E)"          ui_key alt+e
  ui_step "type $1"                      ui_type_text "$1"
  ui_expect_new "$2" "$3" 20
}

ui_start_display
ui_start_app --no-native-dialogs
ui_mark
ui_step "new project (search)"           ui_command "New Project"
ui_focus_dialog '^New Project$'
ui_step "the folder (Alt+F)"             ui_key alt+f
ui_step "type the folder"                ui_type_text "$FOLDER"
ui_step "cloud (Alt+O)"                  ui_key alt+o

address "https://git.example.invalid/team/parts.git" \
  "Cloud address: service Forgejo / Gitea, server git.example.invalid, owner team, repository parts, HTTPS" \
  "an https address on a self-hosted server"
address "git@git.example.invalid:team/parts.git" \
  "Cloud address: service Forgejo / Gitea, server git.example.invalid, owner team, repository parts, SSH" \
  "git's short SSH form"
address "ssh://git@git.example.invalid/team/parts" \
  "Cloud address: service Forgejo / Gitea, server git.example.invalid, owner team, repository parts, SSH" \
  "an ssh:// address without .git"
address "https://git.example.invalid/team/parts/src/branch/main/README.md" \
  "Cloud address: service Forgejo / Gitea, server git.example.invalid, owner team, repository parts, HTTPS" \
  "the address of a page in a browser"
address "ssh://git@git.example.invalid:2222/team/parts.git" \
  "Cloud address: service Other address" \
  "an ssh:// address with a port is used as it is"

echo "--- The details: derived from the address, editable"
address "https://git.example.invalid/team/parts.git" \
  "Cloud address: service Forgejo / Gitea, server git.example.invalid, owner team, repository parts, HTTPS" \
  "the address again"
ui_mark
ui_step "to Details (Tab)"               ui_key Tab
ui_step "show them (Space)"              ui_key space
ui_expect_new "Cloud section: details shown (service Forgejo / Gitea, server git.example.invalid, owner team, repository parts)" \
  "the fields show what the address gave" 10
ui_mark
ui_step "the owner field (Tab x3)"       ui_key Tab Tab Tab
ui_step "type another owner"             ui_type_text "club"
ui_expect_new "Cloud address: service Forgejo / Gitea, server git.example.invalid, owner club, repository parts, HTTPS" \
  "the owner changed: the address follows" 20

echo "--- A malformed address is refused before git is asked"
BAD="git@git.example.invalid/team/parts.git:team/parts.git"
address "$BAD" \
  "Cloud check $BAD: invalid_url: This is not a valid repository address: $BAD" \
  "the malformed address refused with its reason"
address "parts" \
  "Cloud check parts: invalid_url: This is not a valid repository address: parts" \
  "a bare word refused"

ui_step "close (Esc)"                    ui_key Escape
ui_focus_main
ui_key ctrl+q
expect_exit "exited"

ui_finish "UI cloud address test"
