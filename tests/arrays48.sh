#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test array48_checked_index_tests -- --nocapture
cargo build --release
BIN=./target/release/mini
for target in x86_64-linux aarch64-linux armv7-linux-soft; do
  "$BIN" --target "$target" examples/arrays/dynamic_index_x86.mini -o "/tmp/mini48_${target}"
done
"$BIN" --target avr8-atmega328p examples/arrays/dynamic_index_embedded.mini -o /tmp/mini48_avr.hex
"$BIN" --target c64-6510 examples/arrays/dynamic_index_embedded.mini -o /tmp/mini48_c64.prg
"$BIN" --device targets/devices/stm32f103c8.target examples/arrays/dynamic_index_embedded.mini -o /tmp/mini48_thumb.elf
if [[ "$(uname -m)" == x86_64 && "$(uname -s)" == Linux ]]; then
  chmod +x /tmp/mini48_x86_64-linux
  set +e
  /tmp/mini48_x86_64-linux
  status=$?
  set -e
  [[ "$status" == 42 ]] || { echo "unexpected exit code: $status" >&2; exit 1; }
fi
"$BIN" --target x86_64-linux examples/arrays/proven_index.mini -o /tmp/mini48_proven
if "$BIN" --target x86_64-linux examples/arrays/reject_dynamic_index.mini -o /tmp/mini48_invalid; then
  echo 'invalid signed index was accepted' >&2; exit 1
fi
echo 'Mini 4.8 array checks passed'
