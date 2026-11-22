#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for file in src/encode_x86_64.rs src/encode_aarch64.rs src/encode_arm32.rs src/encode_thumb.rs src/backend_aarch64.rs; do
  [[ -f "$file" ]] || { echo "missing $file" >&2; exit 1; }
done
for old in src/x86_encode.rs src/a64_encode.rs src/arm32_encode.rs src/backend_a64.rs; do
  [[ ! -e "$old" ]] || { echo "obsolete filename remains: $old" >&2; exit 1; }
done
python3 - <<'PYCODE'
from pathlib import Path
import re
source = Path('src/main.rs').read_text()
assert 'fn linux_64_bit_backends_accept_hook_without_new_ir_opcode()' in source
assert 'fn eight_bit_targets_reject_owned_aggregate_safely()' in source
for p in Path('src').glob('*.rs'):
    for match in re.finditer(r'\bfn\s+([0-9][A-Za-z0-9_]*)',p.read_text()):
        raise SystemExit(f'{p}: invalid function name {match.group(1)}')
print('Rust source naming rules OK')
PYCODE
