#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 tests/test_minifmt.py
if command -v vim >/dev/null 2>&1; then
    tmpfile="$(mktemp)"
    trap 'rm -f "$tmpfile"' EXIT
    vim -Nu NONE -n -es \
        "+set rtp^=$(pwd)/editor/vim" \
        '+filetype on' '+syntax on' \
        '+edit examples/hello.mini' \
        "+call writefile([&filetype, b:current_syntax], '$tmpfile')" \
        '+qa!'
    test "$(sed -n '1p' "$tmpfile")" = 'mini'
    test "$(sed -n '2p' "$tmpfile")" = 'mini'
    echo 'Vim Mini filetype and syntax: OK'
else
    echo 'Vim not installed; skipped integration check'
fi
# Static validation only (mcedit not available on every build host).
grep -q '^context default' editor/mcedit/mini.syntax
grep -q '^file ' editor/mcedit/Syntax.entry
echo 'mcedit syntax files present (runtime not verified)'
