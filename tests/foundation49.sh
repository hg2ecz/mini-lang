#!/usr/bin/env bash
# Mini 4.9 foundation regressions: parser fix, unified trap, pipe-safe output.
set -euo pipefail
cd "$(dirname "$0")/.."
MINI=${MINI:-./target/release/mini}
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
run() { # run TARGET BINARY -> prints shell status
    local status
    set +e
    case $1 in
        x86_64-linux) timeout 10 "$2" 2>/dev/null ;;
        aarch64-linux) command -v qemu-aarch64 >/dev/null || { echo skip; set -e; return; }; timeout 10 qemu-aarch64 "$2" 2>/dev/null ;;
        armv7-linux-soft) command -v qemu-arm >/dev/null || { echo skip; set -e; return; }; timeout 10 qemu-arm "$2" 2>/dev/null ;;
    esac
    status=$?; set -e; echo "$status"
}
check() { # check NAME EXPECTED SOURCE
    printf '%s\n' "$3" > "$T/$1.mini"
    for target in x86_64-linux aarch64-linux armv7-linux-soft; do
        "$MINI" --target "$target" "$T/$1.mini" -o "$T/$1.$target"
        local got; got=$(run "$target" "$T/$1.$target")
        [[ $got == skip ]] && { echo "SKIP $1 on $target (no emulator)"; continue; }
        [[ $got == "$2" ]] || { echo "FAIL $1 on $target: status $got, expected $2" >&2; exit 1; }
        echo "PASS $1 on $target ($2)"
    done
}
# 4.8 regression: `if` without `else` followed by another statement.
check if_without_else 7 'program {
    let a: i64 = 1
    if a > 0 {
        a = 7
    }
    return a
}'
# A normal exit code 101 must not look like a trap, and a trap must be SIGILL (132).
check normal_101 101 'program { return 101 }'
check overflow_trap 132 'program {
    let a: i64 = 9223372036854775807
    return a + 1
}'
# A failed runtime bounds check uses the same trap instead of a halt loop.
check bounds_trap 132 'program {
    let a: [usize; 4] = [7; 4]
    let i: usize = 9
    return a[i] as i64
}'
# Driver output into a closed pipe must not panic.
"$MINI" --emit-ir examples/hello.mini | head -n 1 >/dev/null
"$MINI" --check-device targets/devices/stm32f103c8.target | grep -q 'backend: available'
echo "PASS pipe-safe driver output"
# Diagnostics: position inside an included file, stable code, source line.
mkdir -p "$T/inc"
printf '// helper library\nfn helper() -> i64 {\n    let x: i64 = 1\n    drop x\n    return 0\n}\n' > "$T/inc/lib.mini"
printf '#include "inc/lib.mini"\nprogram {\n    return helper()\n}\n' > "$T/diag.mini"
if "$MINI" "$T/diag.mini" -o "$T/diag" 2>"$T/diag.err"; then echo "FAIL diagnostics: program accepted" >&2; exit 1; fi
grep -q 'error\[E0304\]: drop requires an aggregate' "$T/diag.err"
grep -q -- "--> $T/inc/lib.mini:4" "$T/diag.err"
grep -q ' 4 |     drop x' "$T/diag.err"
printf 'program {\n    let a: i64 = 1 $\n}\n' > "$T/lex.mini"
"$MINI" "$T/lex.mini" -o "$T/lex" 2>"$T/lex.err" && exit 1
grep -q -- "--> $T/lex.mini:2:20" "$T/lex.err"
"$MINI" --explain E0301 | grep -q 'Use after drop'
echo "PASS diagnostics (file:line:col, error code, source line, --explain)"
# The error catalogue in the docs is generated from the compiler and must match.
"$MINI" --print-errors | diff -q - docs/HIBAKODOK.md >/dev/null || { echo "FAIL docs/HIBAKODOK.md is stale: run mini --print-errors > docs/HIBAKODOK.md" >&2; exit 1; }
echo "PASS docs/HIBAKODOK.md matches the compiler"
echo "Mini 4.9 foundation tests passed"
