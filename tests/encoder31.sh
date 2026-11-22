#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test
cargo build --release
out="$(mktemp)"
trap 'rm -f "$out"' EXIT
./target/release/mini examples/library_selection.mini -o "$out"
if command -v file >/dev/null 2>&1; then
  file "$out" | grep -q 'ELF 64-bit'
fi
set +e
"$out"
status=$?
set -e
if [[ "$status" -ne 42 ]]; then
  echo "Expected exit code 42, got $status" >&2
  exit 1
fi
echo "IR -> shared x86 encoder -> ELF: OK"
