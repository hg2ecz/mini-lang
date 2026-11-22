#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
test -s docs/MINI_USER_GUIDE_HU.md
test -s docs/MINI_USER_GUIDE_EN.md
grep -q '^# Mini – felhasználói kézikönyv' docs/MINI_USER_GUIDE_HU.md
grep -q '^# Mini – User Guide' docs/MINI_USER_GUIDE_EN.md
! grep -q 'pub clobbers:' src/ir.rs
! grep -q 'functions:HashMap<String,Function>' src/backend_thumb.rs
grep -q '#\[cfg(test)\]' src/main.rs
grep -q 'rejects_unknown_and_duplicate_clobbers_before_ir_lowering' src/main.rs
bash tests/source_layout41.sh
printf 'Guide and source-layout static checks passed.\n'
