#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
for file in examples/reassignment/count.mini examples/reassignment/usize.mini; do
  bin="$(mktemp)"
  trap 'rm -f "$bin"' EXIT
  ./target/release/mini --target x86_64-linux "$file" -o "$bin"
  chmod +x "$bin"
  set +e
  "$bin"
  result=$?
  set -e
  if [ "$result" -ne 42 ]; then echo "FAIL $file exit=$result" >&2; exit 1; fi
  rm -f "$bin"
done
echo 'reassignment examples passed'
