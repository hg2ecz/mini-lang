#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test fixed_array_47_tests -- --nocapture
cargo build --release
BIN=./target/release/mini
if [[ "$(uname -m)" == "x86_64" && "$(uname -s)" == "Linux" ]]; then
  for name in fixed_usize fixed_bool fixed_u8_x86_64; do
    "$BIN" --target x86_64-linux "examples/arrays/$name.mini" -o "/tmp/mini_47_${name}"
    chmod +x "/tmp/mini_47_${name}"
    set +e
    "/tmp/mini_47_${name}"
    actual=$?
    set -e
    if [[ $actual -ne 42 ]]; then echo "FAIL: $name returned $actual (expected 42)" >&2; exit 1; fi
  done
fi
"$BIN" --target avr8-atmega328p examples/arrays/fixed_embedded.mini -o /tmp/mini_47_avr.hex
"$BIN" --target c64-6510 examples/arrays/fixed_embedded.mini -o /tmp/mini_47_c64.prg
for name in reject_oob reject_dynamic_index; do
  if "$BIN" --target x86_64-linux "examples/arrays/$name.mini" -o "/tmp/mini_47_${name}"; then
    echo "FAIL: $name unexpectedly compiled" >&2; exit 1
  fi
done
echo 'Mini 4.7 fixed-array tests passed'
