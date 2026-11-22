#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mini=${MINI:-./target/release/mini}
if [[ ! -x "$mini" ]]; then
    echo "Missing $mini (run cargo build --release first)" >&2
    exit 1
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for target in x86_64-linux aarch64-linux armv7-linux-soft armv7-linux-hard; do
    for name in file_auto_drop file_explicit_drop; do
        "$mini" --target "$target" "examples/$name.mini" -o "$work/$name.$target"
        echo "Compiled $name for $target"
    done
done
for name in use_after_drop double_drop adopt_without_unsafe invalid_path_without_unsafe; do
    if "$mini" --target x86_64-linux "tests/file_io/$name.mini" -o "$work/bad" 2>"$work/error"; then
        echo "ERROR: unsafe/ownership test unexpectedly compiled: $name" >&2
        exit 1
    fi
    echo "Rejected $name"
done
if [[ $(uname -sm) == 'Linux x86_64' ]]; then
    printf Z > "$work/a"
    for name in file_auto_drop file_explicit_drop; do
        chmod +x "$work/$name.x86_64-linux"
        ( cd "$work"; "./$name.x86_64-linux" > "$name.output" )
        [[ $(cat "$work/$name.output") == Z ]] || { echo "ERROR: unexpected output from $name" >&2; exit 1; }
        echo "Ran $name: output Z, exit 0"
    done
fi
