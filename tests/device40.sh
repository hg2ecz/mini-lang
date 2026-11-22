#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
MINI="${MINI:-target/release/mini}"
if [[ ! -x "$MINI" ]]; then echo 'build first: cargo build --release' >&2; exit 2; fi
T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
"$MINI" --check-device targets/devices/atmega328p.target | grep -q 'features: jmp,call,mul,lpm'
"$MINI" --check-device targets/devices/stm32f103c8.target | grep -q 'features: thumb2,udiv'
"$MINI" --check-device targets/devices/stm32f401cc.target | grep -q 'features: thumb2,fpv4_sp,dsp,udiv'
"$MINI" --check-device targets/devices/stm32f401cc.target | grep -q 'backend: available'
"$MINI" --device targets/devices/atmega328p.target examples/device_features.mini -o "$T/feat.hex"
"$MINI" --device targets/devices/atmega328p.target examples/eightbit_platform.mini -o "$T/base.hex"
cmp "$T/feat.hex" "$T/base.hex"
# Cortex-M3/M4 now have a bounded Thumb firmware backend.
"$MINI" --device targets/devices/stm32f401cc.target examples/thumb_firmware.mini -o "$T/thumb41.elf"
readelf -h "$T/thumb41.elf" | grep -q 'ARM'
cp targets/devices/atmega328p.target "$T/invalid.target"
for line in 'features = jmp,call,mul,mul' 'features = jmp,call,fpv4_sp' 'vectors = 26' 'format = ihex'; do
    cp targets/devices/atmega328p.target "$T/invalid.target"
    if [[ "$line" == features\ =* ]]; then sed -i "s/^features =.*/$line/" "$T/invalid.target"; else printf '%s\n' "$line" >> "$T/invalid.target"; fi
    if "$MINI" --check-device "$T/invalid.target" >/dev/null 2>&1; then
        echo "invalid target accepted: $line" >&2; exit 1
    fi
done
# Cortex-M3 with a falsely declared FPU must be rejected.
sed 's/^features =.*/features = thumb2,fpv4_sp/' targets/devices/stm32f103c8.target > "$T/invalid.target"
if "$MINI" --check-device "$T/invalid.target" >/dev/null 2>&1; then
    echo 'Cortex-M3 FPU declaration accepted' >&2; exit 1
fi
bash tests/device39.sh
echo 'device40 OK'
