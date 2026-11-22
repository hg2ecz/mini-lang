#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test
cargo build --release
mini=./target/release/mini
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
$mini examples/struct.mini -o "$tmp/struct"
set +e
"$tmp/struct"; result=$?
set -e
[[ $result -eq 42 ]] || { echo "bad struct result: $result"; exit 1; }
$mini examples/hello.mini -o "$tmp/hello"
[[ "$("$tmp/hello")" == 'A' ]]
$mini examples/getpid.mini -o "$tmp/getpid"
"$tmp/getpid"
if $mini examples/unsafe_rejected.mini -o "$tmp/bad"; then echo 'unsafe call accepted'; exit 1; fi
if $mini examples/obsolete_rejected.mini -o "$tmp/bad2"; then echo 'obsolete builtin accepted'; exit 1; fi
file "$tmp/hello"
echo 'Mini 2.7 smoke: OK'
