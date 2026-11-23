#!/usr/bin/env bash
# One entry point for CI and developers: build, unit tests, every tests/*.sh.
# Optional runners: qemu-aarch64, qemu-arm (Linux targets), python3 + unicorn
# (Cortex-M). Missing runners make the affected scripts report SKIPPED.
set -uo pipefail
cd "$(dirname "$0")/.."
cargo build --release || exit 1
cargo test || exit 1
failed=()
for script in tests/*.sh; do
    [[ $script == tests/run_all.sh ]] && continue
    if timeout 300 bash "$script" >"/tmp/mini_test_$$.log" 2>&1; then
        if grep -qi 'skip' "/tmp/mini_test_$$.log"; then echo "ok (partly skipped)  $script"; else echo "ok                   $script"; fi
    else
        echo "FAILED               $script"; tail -n 5 "/tmp/mini_test_$$.log" | sed 's/^/    /'
        failed+=("$script")
    fi
done
rm -f "/tmp/mini_test_$$.log"
if (( ${#failed[@]} )); then echo "${#failed[@]} script(s) failed"; exit 1; fi
echo "All test scripts passed"
