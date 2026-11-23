#!/usr/bin/env bash
# Compile and inspect on any host; execute only with native ARM Linux or QEMU user mode.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test
cargo build --release
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mini=./target/release/mini
for target in aarch64-linux armv7-linux-soft armv7-linux-hard; do
    for example in cast_and_update signed_multiply overflow cast_out_of_range; do
        "$mini" --target "$target" "examples/scalars/arm/$example.mini" -o "$work/${target}_${example}"
    done
    if command -v readelf >/dev/null; then
        case "$target" in
            aarch64-linux) readelf -h "$work/${target}_cast_and_update" | grep -q 'Machine:.*AArch64' ;;
            *) readelf -h "$work/${target}_cast_and_update" | grep -q 'Machine:.*ARM' ;;
        esac
    fi
    runner=()
    case "$target" in
        aarch64-linux)
            if [[ "$(uname -m)" == aarch64 ]]; then runner=();
            elif command -v qemu-aarch64 >/dev/null; then runner=(qemu-aarch64);
            else continue; fi;;
        *)
            if [[ "$(uname -m)" == armv7* ]]; then runner=();
            elif command -v qemu-arm >/dev/null; then runner=(qemu-arm);
            else continue; fi;;
    esac
    for case_and_status in cast_and_update:42 signed_multiply:42 overflow:132 cast_out_of_range:132; do
        example="${case_and_status%:*}"; expected="${case_and_status#*:}"
        set +e
        "${runner[@]}" "$work/${target}_${example}"
        actual=$?
        set -e
        if [[ "$actual" -ne "$expected" ]]; then echo "FAIL $target $example: expected $expected, got $actual" >&2; exit 1; fi
    done
done
# Valid u64 on ARM64, but no unsigned 64-bit arithmetic on ARM32 yet.
"$mini" --target aarch64-linux examples/scalars/arm/unsigned_u64_unsupported.mini -o "$work/aarch64_u64"
if "$mini" --target armv7-linux-soft examples/scalars/arm/unsigned_u64_unsupported.mini -o "$work/arm32_u64"; then
    echo 'ARM32 unexpectedly accepted unsupported unsigned u64 arithmetic' >&2; exit 1
fi
# ARM32 unsigned usize requires 32-bit overflow detection.
"$mini" --target armv7-linux-soft examples/scalars/arm/usize32_overflow.mini -o "$work/usize_overflow"
echo 'ARM integer compilation checks passed; runtime checks executed only where a runner was available.'
