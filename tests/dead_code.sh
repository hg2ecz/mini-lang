#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mini=./target/release/mini
"$mini" examples/library_selection.mini -o /tmp/mini_selected_$$
trap 'rm -f /tmp/mini_selected_$$' EXIT
set +e
/tmp/mini_selected_$$
result=$?
set -e
[[ "$result" -eq 42 ]]
