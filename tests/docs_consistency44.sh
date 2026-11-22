#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
for f in docs/MINI_LANGUAGE_REFERENCE_HU.md docs/MINI_LANGUAGE_REFERENCE_EN.md docs/MINI_USER_GUIDE_HU.md docs/MINI_USER_GUIDE_EN.md docs/README.md docs/SCALARS_4_4_HU.md docs/SCALARS_4_4_EN.md; do
  test -s "$f" || { echo "missing documentation: $f" >&2; exit 1; }
done
for f in HU EN; do
  grep -q '4.4' "docs/MINI_LANGUAGE_REFERENCE_${f}.md"
  grep -q 'u8' "docs/MINI_LANGUAGE_REFERENCE_${f}.md"
  grep -q 'f32' "docs/MINI_LANGUAGE_REFERENCE_${f}.md"
  grep -q 'as' "docs/MINI_LANGUAGE_REFERENCE_${f}.md"
done
python3 - <<'PYLINKS'
from pathlib import Path
import re
for path in [Path('README.md'), *Path('docs').glob('*.md')]:
    text = path.read_text()
    for link in re.findall(r'(?<!!)\[[^]]+\]\(([^)]+)\)',text):
        if link.startswith(('http:','https:','#','mailto:')): continue
        target = link.split('#',1)[0]
        if target and not (path.parent/target).exists():
            raise SystemExit(f'broken doc link in {path}: {link}')
print('documentation file links OK')
PYLINKS
echo 'Mini 4.4 documentation consistency OK'
