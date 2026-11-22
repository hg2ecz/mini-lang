#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test
cargo build --release
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
./target/release/mini --target x86_64-linux examples/scalars/checked_cast.mini -o "$TMP/checked_cast"
set +e
"$TMP/checked_cast"; code=$?
set -e
[[ "$code" -eq 42 ]] || { echo "expected exit 42, got $code"; exit 1; }
./target/release/mini --target x86_64-linux examples/scalars/checked_overflow.mini -o "$TMP/overflow"
set +e
"$TMP/overflow"; code=$?
set -e
[[ "$code" -eq 101 ]] || { echo "expected overflow exit 101, got $code"; exit 1; }
./target/release/mini --target x86_64-linux examples/scalars/signed_comparison.mini -o "$TMP/signed"
set +e
"$TMP/signed"; code=$?
set -e
[[ "$code" -eq 42 ]] || { echo "expected exit 42, got $code"; exit 1; }
echo 'Mini 4.3 integer/cast smoke tests passed'
