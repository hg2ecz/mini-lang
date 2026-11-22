#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
MINI="${MINI:-target/release/mini}"
if [[ ! -x "$MINI" ]]; then echo "build first: cargo build --release" >&2; exit 2; fi
T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
"$MINI" --check-device targets/devices/atmega328p.target | grep -q 'backend: available'
"$MINI" --check-device targets/devices/atmega168p.target | grep -q '0x00000500'
"$MINI" --check-device targets/devices/stm32f103c8.target | grep -q 'backend: available'
"$MINI" --device targets/devices/atmega328p.target examples/eightbit_platform.mini -o "$T/328.hex"
"$MINI" --device targets/devices/atmega168p.target examples/eightbit_platform.mini -o "$T/168.hex"
"$MINI" --target avr8-atmega328p examples/eightbit_platform.mini -o "$T/legacy.hex"
cmp "$T/328.hex" "$T/legacy.hex"
# Verify two distinct initial stack values in actual firmware bytes.
python3 - "$T/328.hex" "$T/168.hex" <<'PY'
import sys

def decode(path):
    data=bytearray()
    for line in open(path):
        if not line.startswith(':'): raise ValueError('bad ihex')
        record=bytes.fromhex(line[1:].strip())
        assert sum(record)%256 == 0
        if record[3]==1: break
        assert record[3]==0
        addr=int.from_bytes(record[1:3], 'big')
        if len(data)<addr+record[0]: data.extend(b'\x00'*(addr+record[0]-len(data)))
        data[addr:addr+record[0]]=record[4:-1]
    return bytes(data)

for file,sp in zip(sys.argv[1:], (0x08ff, 0x04ff)):
    code=decode(file)
    # AVR LDI r16, H; OUT 0x3E, r16; LDI r16, L; OUT 0x3D, r16
    pattern=bytes((0x00 | ((sp>>8)&15), 0xE0 | ((sp>>8)&0xf0), 0x0e, 0xbf,
                   sp & 15, 0xe0 | ((sp & 0xf0) >> 4), 0x0d, 0xbf))
    assert pattern in code, (file,pattern.hex())
    assert len(code)<(32768 if sp==0x08ff else 16384)
print('device map stack patterns and HEX checksums verified')
PY
# Cortex-M profile must select Thumb (not A32) firmware output.
"$MINI" --device targets/devices/stm32f103c8.target examples/thumb_firmware.mini -o "$T/thumb41.elf"
readelf -h "$T/thumb41.elf" | grep -q 'ARM'
cp targets/devices/atmega328p.target "$T/invalid.target"
printf '\nstack = 9999\n' >>"$T/invalid.target"
if "$MINI" --check-device "$T/invalid.target" >/dev/null 2>&1; then
  echo 'duplicate key unexpectedly accepted' >&2; exit 1
fi
echo 'device39 OK'
