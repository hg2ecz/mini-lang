#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mini="${MINI:-./target/release/mini}"
if [[ ! -x "$mini" ]]; then
  echo "Mini executable not found: $mini. Run cargo build --release first." >&2
  exit 2
fi
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
arch="$(uname -m)"
runner=()
case "$arch" in
  x86_64) target=x86_64-linux ;;
  aarch64|arm64) target=aarch64-linux ;;
  armv7*|armv6*) target=armv7-linux-soft ;;
  *) echo "Unsupported host architecture for execution: $arch" >&2; exit 2 ;;
esac
count=0
for src in examples/tutorial/*.mini; do
  base="$(basename "$src" .mini)"
  "$mini" --target "$target" "$src" -o "$work/$base"
  set +e
  "$work/$base" >"$work/$base.stdout" 2>"$work/$base.stderr"
  status=$?
  set -e
  expected=42
  if [[ "$base" == 25_runtime_div_zero ]]; then expected=132; fi
  if [[ "$status" -ne "$expected" ]]; then
    echo "FAIL $base: exit $status, expected $expected" >&2
    cat "$work/$base.stderr" >&2
    exit 1
  fi
  if [[ "$base" == 21_linux_write ]]; then
    [[ "$(cat "$work/$base.stdout")" == A ]] || { echo "FAIL $base: expected stdout A" >&2; exit 1; }
  fi
  count=$((count+1))
done
for src in examples/negative/*.mini; do
  base="$(basename "$src" .mini)"
  if "$mini" --target "$target" "$src" -o "$work/bad_$base" >"$work/err" 2>&1; then
    echo "FAIL $base: invalid program compiled" >&2
    exit 1
  fi
done
echo "PASS: $count tutorial programs and compile-time negative tests on $target"
