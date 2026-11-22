#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
BIN=./target/release/mini
if [[ ! -x "$BIN" ]]; then echo 'Build with cargo build --release first' >&2; exit 1; fi
out=$(mktemp); err=$(mktemp); trap 'rm -f "$out" "$err"' EXIT
"$BIN" examples/target_select.mini -o "$out"
set +e
"$out"
status=$?
set -e
[[ $status -eq 42 ]] || { echo "exit code $status, expected 42" >&2; exit 1; }
# An unsupported backend must fail without creating an executable.
if "$BIN" --target avr8-none examples/target_select.mini -o "$out" 2>"$err"; then
  echo 'AVR backend must not be accepted' >&2; exit 1
fi
grep -Eq 'not implemented|unavailable' "$err"
# Unmatched directive must fail.
tmp=$(mktemp --suffix=.mini); trap 'rm -f "$out" "$err" "$tmp"' EXIT
printf '#if os == "linux"\nprogram { return 0 }\n' > "$tmp"
if "$BIN" "$tmp" -o "$out" 2>"$err"; then echo 'Unclosed #if accepted' >&2; exit 1; fi
grep -q 'unterminated #if' "$err"
echo 'preprocessor tests passed'
