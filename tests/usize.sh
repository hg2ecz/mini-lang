#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=./target/release/mini
if [[ ! -x "$BIN" ]]; then cargo build --release; fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
"$BIN" examples/usize_ok.mini -o "$TMP/ok"
set +e
"$TMP/ok"
status=$?
set -e
[[ $status -eq 42 ]] || { echo "usize result wrong: $status" >&2; exit 1; }
"$BIN" examples/usize_overflow.mini -o "$TMP/overflow"
set +e
"$TMP/overflow"
status=$?
set -e
[[ $status -eq 101 ]] || { echo "overflow check failed: $status" >&2; exit 1; }
if "$BIN" examples/usize_reject.mini -o "$TMP/reject" >/dev/null 2>&1; then
 echo 'implicit signed-to-usize conversion accepted' >&2; exit 1
fi
printf 'usize ABI and safety tests passed\n'
