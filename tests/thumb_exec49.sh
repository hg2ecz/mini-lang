#!/usr/bin/env bash
# Mini 4.9: execute Cortex-M firmware on the Unicorn emulator and check results.
# Computed values are written to a fake peripheral address and read back from
# the emulator's MMIO log. Requires: python3 with `pip install unicorn`.
set -euo pipefail
cd "$(dirname "$0")/.."
MINI=${MINI:-./target/release/mini}
DEVICE=targets/devices/stm32f103c8.target
if ! python3 -c 'import unicorn' 2>/dev/null; then
    echo "Thumb execution SKIPPED (python3 module 'unicorn' not installed)"
    exit 0
fi
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
# 1073741824 = 0x40000000, an address outside Flash/RAM (logged as MMIO).
PRELUDE='#include "'"$PWD"'/lib/arm/mmio.mini"
fn report(value: usize) {
    unsafe { write32(1073741824, value) }
}'

expect_mmio() {
    local name=$1 expected=$2 body=$3 out
    printf '%s\n%s\n' "$PRELUDE" "$body" > "$T/$name.mini"
    "$MINI" --device "$DEVICE" "$T/$name.mini" -o "$T/$name.elf"
    out=$(python3 tools/run_cortexm.py "$DEVICE" "$T/$name.elf")
    if ! grep -q "mmio-write 0x40000000 size=4 value=$expected\$" <<<"$out" || ! grep -q '^exit' <<<"$out"; then
        echo "FAIL $name: expected MMIO value $expected and normal exit, got:" >&2
        echo "$out" >&2
        exit 1
    fi
    echo "PASS $name ($expected)"
}
expect_trap() {
    local name=$1 body=$2 out
    printf '%s\n%s\n' "$PRELUDE" "$body" > "$T/$name.mini"
    "$MINI" --device "$DEVICE" "$T/$name.mini" -o "$T/$name.elf"
    set +e
    out=$(python3 tools/run_cortexm.py "$DEVICE" "$T/$name.elf")
    local status=$?
    set -e
    if [[ $status -ne 3 ]]; then
        echo "FAIL $name: expected trap, got status $status:" >&2
        echo "$out" >&2
        exit 1
    fi
    echo "PASS $name (trap)"
}

expect_mmio argument_order 0x2a 'fn sub(a: usize, b: usize) -> usize {
    return a - b
}
program {
    report(sub(50, 8))
    return 0
}'
expect_mmio nested_temporaries 0x24 'program {
    let a: usize = 1
    let b: usize = 2
    let c: usize = 3
    let d: usize = 30
    report((a + b) + (c + d))
    return 0
}'
expect_mmio call_in_operand 0x2a 'fn twice(x: usize) -> usize {
    return x + x
}
program {
    let base: usize = 2
    report(base + twice(20))
    return 0
}'
expect_mmio while_loop 0x37 'program {
    let i: usize = 0
    let sum: usize = 0
    while i < 10 {
        i = i + 1
        sum = sum + i
    }
    report(sum)
    return 0
}'
expect_trap add_overflow 'program {
    let big: usize = 4294967295
    report(big + 1)
    return 0
}'
expect_trap sub_underflow 'program {
    let zero: usize = 0
    report(zero - 1)
    return 0
}'

"$MINI" --device "$DEVICE" examples/thumb_firmware.mini -o "$T/example.elf"
python3 tools/run_cortexm.py "$DEVICE" "$T/example.elf" | grep -q '^exit'
echo "PASS examples/thumb_firmware.mini (normal exit)"
echo "Thumb execution tests passed"
