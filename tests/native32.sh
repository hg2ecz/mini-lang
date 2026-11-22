#!/usr/bin/env bash
# Mini 3.2 native acceptance test. Run on Linux x86-64 with Rust 2024 toolchain.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo "Native ELF run tests require x86-64 Linux" >&2
  exit 2
fi
cargo test
cargo build --release
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

run_case() {
  local label="$1" expected="$2" source="$3" actual
  printf '%s\n' "$source" > "$work/$label.mini"
  ./target/release/mini "$work/$label.mini" -o "$work/$label"
  set +e
  "$work/$label" > "$work/$label.stdout" 2> "$work/$label.stderr"
  actual=$?
  set -e
  if [[ "$actual" -ne "$expected" ]]; then
    printf 'FAIL %s: expected exit %s, got %s\n' "$label" "$expected" "$actual" >&2
    exit 1
  fi
  echo "PASS $label ($actual)"
}
run_case add 42 'program { return 20 + 22 }'
run_case sub 42 'program { return 45 - 3 }'
run_case mul 42 'program { return 21 * 2 }'
run_case div 42 'program { return 84 / 2 }'
run_case neg 42 'program { let x: i64 = -42; return -x }'
run_case compare 42 'program { if 3 < 4 { return 42 } else { return 1 } }'
run_case while_false 42 'program { while false { return 99 }; return 42 }'
run_case function_args 42 'fn sum(a: i64, b: i64) -> i64 { return a + b } program { return sum(20,22) }'
run_case scalar_struct 42 'struct R { value: i64; ok: bool } fn mk(x: i64, ok: bool) -> R { return R(x,ok) } program { let v: R = mk(42,true); if v.ok { return v.value } else { return 1 } }'
run_case forwarded_struct 42 'struct R { value: i64; ok: bool } fn make(x: i64)->R { return R(x,true) } fn forward(x: i64)->R { return make(x) } program { let v: R = forward(42); return v.value }'
run_case unsigned 42 'program { let a: usize = 7; let b: usize = 7; if a >= b { return 42 } else { return 0 } }'

# A failed checked operation must terminate with exit 101, not SIGFPE or wraparound.
run_case overflow_add 101 'program { return 9223372036854775807 + 1 }'
run_case overflow_mul 101 'program { return 3037000500 * 3037000500 }'
run_case overflow_sub 101 'program { return -9223372036854775807 - 2 }'
run_case overflow_neg 101 'program { let x: i64 = -9223372036854775807 - 1; return -x }'
run_case overflow_div 101 'program { let x: i64 = -9223372036854775807 - 1; return x / -1 }'
run_case div_zero 101 'program { return 5 / 0 }'
run_case unsigned_underflow 101 'program { let a: usize = 0; let b: usize = 1; return a - b }'

# Architecture- and ABI-specific Linux asm stays in the included Mini library.
./target/release/mini examples/hello.mini -o "$work/hello"
[[ "$("$work/hello")" == 'A' ]] || { echo "FAIL hello from included Linux asm" >&2; exit 1; }

printf 'program { return 42 }\n' > "$work/dead.mini"
./target/release/mini "$work/dead.mini" -o "$work/dead"
printf 'fn not_called() -> i64 { return 999 }\nprogram { return 42 }\n' > "$work/dead_extra.mini"
./target/release/mini "$work/dead_extra.mini" -o "$work/dead_extra"
cmp "$work/dead" "$work/dead_extra"

if command -v readelf >/dev/null 2>&1; then
    readelf -h "$work/dead" | grep -q 'Machine:.*Advanced Micro Devices X86-64'
fi
if command -v objdump >/dev/null 2>&1; then
    objdump -D -b binary -m i386:x86-64 --adjust-vma=0x401000 \
        --start-address=0x401000 "$work/dead" >/dev/null || true
fi
printf 'Mini 3.2 native acceptance: OK\n'
