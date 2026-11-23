#!/usr/bin/env bash
# ARM64/ARM32: compile smoke; x86-64 Linux: compile and execute.
set -euo pipefail
cd "$(dirname "$0")/.."
mini=${MINI:-./target/release/mini}
if [[ ! -x "$mini" ]]; then
    echo "Missing $mini; run cargo build --release first" >&2
    exit 1
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
compile() { "$mini" --target "$1" "$2" -o "$work/out"; }

for target in x86_64-linux aarch64-linux armv7-linux-soft armv7-linux-hard; do
    compile "$target" examples/ownership_linux.mini
    echo "OK: $target with library-owned file descriptor"
done

for invalid in double_drop unsafe_fabrication direct_hook_call invalid_dtor_signature branch_hidden_let; do
    if compile x86_64-linux "tests/ownership/$invalid.mini" 2>"$work/err"; then
        echo "BAD: invalid ownership program was accepted: $invalid" >&2
        exit 1
    fi
    echo "OK: rejected $invalid"
done

if compile x86_64-linux tests/ownership/branch_state.mini 2>"$work/err"; then
    echo "BAD: path-dependent owner state was accepted" >&2
    exit 1
fi

test_exit() {
    local src=$1 expected=$2 actual
    compile x86_64-linux "tests/ownership/$src.mini"
    chmod +x "$work/out"
    set +e
    "$work/out"
    actual=$?
    set -e
    if [[ "$actual" != "$expected" ]]; then
        echo "BAD: $src exited $actual instead of $expected" >&2
        exit 1
    fi
    echo "OK: $src exit $expected"
}
if [[ $(uname -sm) == 'Linux x86_64' ]]; then
    test_exit auto_exit 73
    test_exit reverse_exit 62
    test_exit explicit_exit 71
    test_exit return_preserved 42
    test_exit void_exit 74
    test_exit early_return 75
else
    echo "Skipping x86-64 native execution on this host"
fi

echo 'Ownership-drop smoke tests completed'
