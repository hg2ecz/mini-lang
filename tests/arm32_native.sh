#!/usr/bin/env bash
# Run on ARMv7-A Linux or use qemu-arm. Compiles both EABI variants.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test
cargo build --release
mini=./target/release/mini
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for abi in soft hard; do
    "$mini" --target "armv7-linux-$abi" examples/hello_arm32.mini -o "$work/hello-$abi"
    "$mini" --target "armv7-linux-$abi" examples/target_linux.mini -o "$work/target-$abi"
    if command -v readelf >/dev/null 2>&1; then
        readelf -h "$work/hello-$abi" | grep -q 'Machine:.*ARM'
        readelf -h "$work/hello-$abi" | grep -q 'ELF32'
        readelf -l "$work/hello-$abi" | grep -q 'GNU_STACK'
    fi
    # Test byte-level e_flags: EABI version 5, soft=0x200, hard=0x400.
    python3 - "$work/hello-$abi" "$abi" <<'PY'
import pathlib,sys
b=pathlib.Path(sys.argv[1]).read_bytes()
abi=sys.argv[2]
flags=int.from_bytes(b[36:40], 'little')
assert flags==0x05000000+(0x400 if abi=='hard' else 0x200),hex(flags)
assert int.from_bytes(b[24:28], 'little')==0x11000
assert int.from_bytes(b[18:20], 'little')==40
PY
done

# The IR pruner removes dead functions before either backend runs.
printf 'program { return 42 }\n' > "$work/dead.mini"
printf 'fn unused()->i64 { return 100 }\nprogram { return 42 }\n' > "$work/extra.mini"
"$mini" --target armv7-linux-soft "$work/dead.mini" -o "$work/dead"
"$mini" --target armv7-linux-soft "$work/extra.mini" -o "$work/extra"
cmp "$work/dead" "$work/extra"

reject() {
    local name="$1" src="$2"
    printf '%s\n' "$src" > "$work/reject.mini"
    if "$mini" --target armv7-linux-soft "$work/reject.mini" -o "$work/reject"; then
        echo "FAIL: accepted invalid program: $name" >&2; exit 1
    fi
    echo "PASS reject: $name"
}
reject usize_out_of_range 'program { let x:usize=4294967296; return 0 }'
reject wrong_asm_reg 'program { unsafe { asm { in r11=20; svc; clobber memory } }; return 0 }'
reject missing_syscall_clobber 'program { unsafe { asm { in r7=20; svc } }; return 0 }'
reject x86_instruction 'program { unsafe { asm { pause } }; return 0 }'
reject unsafe_wrapper 'unsafe fn danger()->i64 { return 42 } program { return danger() }'
reject wrong_arch_asm 'program { unsafe { asm { in x8=172; svc; clobber memory } }; return 0 }'

runner=()
arch="$(uname -m)"
if [[ "$(uname -s)" == Linux && ( "$arch" == armv7l || "$arch" == armv7 || "$arch" == armhf ) ]]; then
    runner=()
elif command -v qemu-arm >/dev/null 2>&1; then
    runner=(qemu-arm)
else
    echo 'ARM32 ELF32 headers, both ABI variants, dead-code and invalid-program tests: OK.'
    echo 'ARM32 execution SKIPPED (requires ARMv7-A Linux or qemu-arm).'
    exit 0
fi

run_program() {
    local name="$1" expected="$2" src="$3" status
    printf '%s\n' "$src" > "$work/$name.mini"
    "$mini" --target armv7-linux-soft "$work/$name.mini" -o "$work/$name"
    set +e
    "${runner[@]}" "$work/$name" >"$work/$name.out" 2>"$work/$name.err"
    status=$?
    set -e
    if [[ "$status" -ne "$expected" ]]; then
        echo "FAIL $name: wanted $expected, got $status" >&2
        cat "$work/$name.err" >&2
        exit 1
    fi
    echo "PASS $name: exit $status"
}
run_program add_i64 42 'program { return 20 + 22 }'
run_program sub_i64 42 'program { return 45 - 3 }'
run_program mul_i64 42 'program { return 21 * 2 }'
run_program div_i64 42 'program { return 84 / 2 }'
run_program mul_i64_high 42 'program { let a:i64=4294967296; let b:i64=4; if a*b == 17179869184 { return 42 } else { return 1 } }'
run_program div_i64_high 42 'program { let a:i64=17179869184; let b:i64=4; if a/b == 4294967296 { return 42 } else { return 1 } }'
run_program signed_negative 42 'program { let a:i64=-9223372036854775807; let b:i64=7; if a/b < 0 { return 42 } else { return 1 } }'
run_program negative_mul 42 'program { let a:i64=-12; let b:i64=-7; if a*b == 84 { return 42 } else { return 1 } }'
run_program branch 42 'program { if 0 < 1 { return 42 } else { return 1 } }'
run_program loop_false 42 'program { while false { return 9 }; return 42 }'
run_program multiple_args 42 'fn sum(a:i64,b:i64,c:i64,d:i64,e:i64)->i64 { return a+b+c+d+e } program { return sum(1,2,3,4,32) }'
run_program struct_result 42 'struct R { value:i64; ok:bool } fn f(x:i64,ok:bool)->R { return R(x,ok) } program { let r:R=f(42,true); if r.ok { return r.value } else { return 1 } }'
run_program forwarded_struct 42 'struct R { value:i64; ok:bool } fn mk(x:i64)->R { return R(x,true) } fn f(x:i64)->R { return mk(x) } program { let r:R=f(42); return r.value }'
run_program borrowed_struct 42 'struct R { value:i64; ok:bool } fn get(r:R)->i64 { return r.value } program { let v:R=R(42,true); return get(v) }'
run_program usize_32 42 'program { let a:usize=4294967295; let b:usize=4294967295; if a==b { return 42 } else { return 1 } }'
run_program usize_cmp 42 'program { let a:usize=4294967295; let b:usize=10; if a>b { return 42 } else { return 1 } }'
run_program usize_div 42 'program { let a:usize=84; let b:usize=2; let c:usize=a/b; let d:usize=42; if c==d { return 42 } else { return 1 } }'
run_program i64_min 42 'program { let a:i64=-9223372036854775807 - 1; if a<0 { return 42 } else { return 1 } }'
run_program signed_add_overflow 132 'program { return 9223372036854775807 + 1 }'
run_program signed_sub_overflow 132 'program { return -9223372036854775807 - 2 }'
run_program signed_mul_overflow 132 'program { let a:i64=4294967296; return a*a }'
run_program signed_div_overflow 132 'program { let a:i64=-9223372036854775807 - 1; return a / -1 }'
run_program signed_neg_overflow 132 'program { let a:i64=-9223372036854775807 - 1; return -a }'
run_program unsigned_add_overflow 132 'program { let a:usize=4294967295; let b:usize=1; return (a+b) as i64 }'
run_program unsigned_mul_overflow 132 'program { let a:usize=65536; let b:usize=65536; return (a*b) as i64 }'
run_program unsigned_sub_underflow 132 'program { let a:usize=0; let b:usize=1; return (a-b) as i64 }'
run_program divide_zero 132 'program { return 5 / 0 }'

for abi in soft hard; do
    set +e
    "${runner[@]}" "$work/target-$abi" >/dev/null 2>"$work/target-$abi.err"
    status=$?
    set -e
    [[ "$status" -eq 42 ]] || { echo "FAIL ARM32 $abi Linux getpid: exit $status" >&2; exit 1; }
    output="$("${runner[@]}" "$work/hello-$abi")"
    [[ "$output" == 'A' ]] || { echo "FAIL ARM32 $abi Linux write (actual '$output')" >&2; exit 1; }
    echo "PASS ARM32 $abi included Linux syscall library"
done
printf 'ARMv7 Linux acceptance: OK\n'
