#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
MINI="${MINI:-target/release/mini}"
[[ -x "$MINI" ]] || { echo 'cargo build --release is required' >&2; exit 2; }
T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
for device in stm32f103c8 stm32f401cc; do
  "$MINI" --device "targets/devices/$device.target" examples/thumb_firmware.mini -o "$T/$device.elf"
  readelf -h "$T/$device.elf" | grep -q 'Machine:.*ARM'
  python3 - "$T/$device.elf" "$device" <<'PY'
import sys,struct
path,name=sys.argv[1:]
data=open(path,'rb').read()
assert data[:5]==b'\x7fELF\x01'
assert struct.unpack_from('<H',data,18)[0]==40 # EM_ARM
assert struct.unpack_from('<I',data,36)[0]==0x05000000
p_offset=struct.unpack_from('<I',data,56)[0]
flash=0x08000000
vectors={'stm32f103c8':59,'stm32f401cc':101}[name]
ram_end={'stm32f103c8':0x20005000,'stm32f401cc':0x20010000}[name]
assert struct.unpack_from('<I',data,p_offset)[0]==ram_end
assert struct.unpack_from('<I',data,p_offset+4)[0]==flash+4*vectors+1
assert struct.unpack_from('<I',data,24)[0]==flash+4*vectors+1
assert len(data)<0x100+{'stm32f103c8':65536,'stm32f401cc':262144}[name]
print(name,'ELF + Cortex-M vector table OK')
PY
done
cat > "$T/unsupported.mini" <<'EOF'
fn bad(a: usize) -> usize { return a * a }
program { let x: usize = bad(7); return 0 }
EOF
if "$MINI" --device targets/devices/stm32f103c8.target "$T/unsupported.mini" -o "$T/bad.elf" 2>"$T/error"; then
  echo 'unsafe silent fallback: unsupported Thumb multiply was accepted' >&2; exit 1
fi
grep -q 'unsupported' "$T/error"
cat > "$T/recursion.mini" <<'EOF'
fn recur(a: usize) -> usize { return recur(a) }
program { let a: usize = recur(1); return 0 }
EOF
if "$MINI" --device targets/devices/stm32f103c8.target "$T/recursion.mini" -o "$T/recursive.elf" 2>"$T/error"; then
  echo 'recursive Thumb code unexpectedly accepted' >&2; exit 1
fi
grep -q 'recursion' "$T/error"
cat > "$T/unsafe.mini" <<'EOF'
#include "missing.mini"
program { return 0 }
EOF
if "$MINI" --device targets/devices/stm32f103c8.target "$T/unsafe.mini" -o "$T/bad.elf" 2>/dev/null; then
 echo 'invalid include accepted' >&2; exit 1
fi
echo 'Thumb 4.1 source-level acceptance tests passed'
