#!/usr/bin/env bash
# Build on any Rust host; execute on AArch64 Linux or when qemu-aarch64 is installed.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test
cargo build --release
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mini='./target/release/mini'

"$mini" --target aarch64-linux examples/hello_arm64.mini -o "$work/hello"
"$mini" --target aarch64-linux examples/target_linux.mini -o "$work/target"
if command -v readelf >/dev/null; then
    readelf -h "$work/hello" | grep -q 'Machine:.*AArch64'
    readelf -l "$work/hello" | grep -q 'LOAD'
fi
if command -v file >/dev/null; then file "$work/hello"; fi

# Dead code elimination is architecture independent.
printf 'program { return 42 }\n' > "$work/dead.mini"
printf 'fn unused() -> i64 { return 14 }\nprogram { return 42 }\n' > "$work/extra.mini"
"$mini" --target aarch64-linux "$work/dead.mini" -o "$work/dead"
"$mini" --target aarch64-linux "$work/extra.mini" -o "$work/extra"
cmp "$work/dead" "$work/extra"

# Reject x86-only assembly and missing Linux syscall memory effects.
printf 'program { unsafe { asm { pause } }; return 42 }\n' > "$work/bad.mini"
if "$mini" --target aarch64-linux "$work/bad.mini" -o "$work/bad"; then
    echo 'ERROR: accepted x86 pause on AArch64' >&2; exit 1
fi
printf 'program { unsafe { asm { in x8 = 172; svc } }; return 42 }\n' > "$work/bad.mini"
if "$mini" --target aarch64-linux "$work/bad.mini" -o "$work/bad"; then
    echo 'ERROR: accepted svc without clobber memory' >&2; exit 1
fi

runner=()
if [[ "$(uname -s)" == 'Linux' && ( "$(uname -m)" == 'aarch64' || "$(uname -m)" == 'arm64' ) ]]; then
    runner=()
elif command -v qemu-aarch64 >/dev/null 2>&1; then
    runner=(qemu-aarch64)
else
    echo 'Cross-compilation and ELF checks: OK. Execution SKIPPED (needs ARM64 Linux or qemu-aarch64).'
    exit 0
fi

run_case() {
    local name="$1" expected="$2" source="$3" status
    printf '%s\n' "$source" > "$work/$name.mini"
    "$mini" --target aarch64-linux "$work/$name.mini" -o "$work/$name"
    set +e
    "${runner[@]}" "$work/$name" > "$work/$name.stdout" 2> "$work/$name.stderr"
    status=$?
    set -e
    if [[ "$status" -ne "$expected" ]]; then
        echo "FAIL $name: expected $expected, actual $status" >&2
        cat "$work/$name.stderr" >&2;exit 1
    fi
    echo "PASS $name (exit $status)"
}

run_case add 42 'program { return 20 + 22 }'
run_case sub 42 'program { return 45 - 3 }'
run_case mul 42 'program { return 21 * 2 }'
run_case div 42 'program { return 84 / 2 }'
run_case neg 42 'program { let x: i64 = -42; return -x }'
run_case compare 42 'program { if 3 < 4 { return 42 } else { return 1 } }'
run_case while_false 42 'program { while false { return 99 }; return 42 }'
run_case function_args 42 'fn sum(a: i64,b: i64)->i64 { return a + b } program { return sum(20,22) }'
run_case scalar_struct 42 'struct R { value:i64; ok:bool } fn mk(x:i64, ok:bool)->R { return R(x,ok) } program { let r:R=mk(42,true); if r.ok { return r.value } else { return 1 } }'
run_case forwarded_struct 42 'struct R { value:i64; ok:bool } fn make(x:i64)->R { return R(x,true) } fn forward(x:i64)->R { return make(x) } program { let r:R=forward(42); return r.value }'
run_case unsigned 42 'program { let a:usize=7; let b:usize=7; if a>=b { return 42 } else { return 1 } }'
run_case many_args 42 'fn sum(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64)->i64 { return a+b+c+d+e+f } program { return sum(1,2,3,4,5,27) }'
run_case borrowed_struct 42 'struct R { value:i64; ok:bool } fn get(v:R)->i64 { return v.value } program { let r:R=R(42,true); return get(r) }'
run_case signed_compare 42 'program { let x:i64=-1; if x < 0 { return 42 } else { return 1 } }'
run_case unsigned_compare 42 'program { let x:usize=10; let y:usize=9; if x > y { return 42 } else { return 1 } }'

run_case signed_overflow_add 101 'program { return 9223372036854775807 + 1 }'
run_case signed_overflow_sub 101 'program { return -9223372036854775807 - 2 }'
run_case signed_overflow_mul 101 'program { return 3037000500 * 3037000500 }'
run_case unsigned_underflow 101 'program { let a:usize=0; let b:usize=1; return a - b }'
run_case divide_zero 101 'program { return 5 / 0 }'
run_case division_overflow 101 'program { let a:i64=-9223372036854775807 - 1; return a / -1 }'
# Source-relative include resolution is required: run the original file, not a copy.
set +e
"${runner[@]}" "$work/target";status=$?
set -e
[[ "$status" -eq 42 ]] || { echo 'FAIL #if target Linux wrapper' >&2; exit 1; }
output="$("${runner[@]}" "$work/hello")"
[[ "$output" == 'A' ]] || { echo 'FAIL ARM64 included sys_write' >&2; exit 1; }
echo 'PASS Linux AArch64 asm syscall write and getpid'
echo 'Mini AArch64 Linux acceptance: OK'
