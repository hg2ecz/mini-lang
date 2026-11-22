#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 - <<'PY'
from pathlib import Path
source=Path('lib/memory/linux_buffer.mini').read_text()
assert '#if os == "linux"' in source
assert '#error Linux Buffer requires a Linux target' in source
for sym in ['struct Buffer', 'fn buffer_new(', 'fn __drop_Buffer(', 'fn buffer_valid(', 'fn buffer_fits(', 'unsafe fn buffer_read_fd(', 'unsafe fn buffer_write_fd(', 'unsafe fn buffer_raw_address(']:
    assert sym in source, sym
assert 'sys_mmap(' in source and 'sys_munmap(' in source
assert 'return count <= buffer.capacity - offset' in source
print('source/API invariants: OK')
PY
if command -v cargo >/dev/null 2>&1; then
  cargo test
  cargo build --release
  if [[ "$(uname -m)" == x86_64 && "$(uname -s)" == Linux ]]; then
    ./target/release/mini --target x86_64-linux examples/memory/linux_owned_buffer.mini -o /tmp/mini46_buffer
    ./target/release/mini --target x86_64-linux examples/memory/linux_explicit_drop.mini -o /tmp/mini46_drop
    set +e
    /tmp/mini46_drop
    rc=$?
    set -e
    [[ $rc -eq 42 ]] || { echo "explicit drop exit code: $rc != 42" >&2; exit 1; }
    printf 'hello' | /tmp/mini46_buffer >/tmp/mini46_out
    cmp -s /tmp/mini46_out <(printf 'hello')
    echo 'native examples: OK'
  fi
else
  echo 'Cargo unavailable: compiler and native smoke tests NOT EXECUTED' >&2
fi
