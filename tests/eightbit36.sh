#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mini="${MINI_BIN:-./target/release/mini}"
[[ -x "$mini" ]] || { echo 'Build first: cargo build --release' >&2; exit 1; }
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
for target in c64-6510 avr8-atmega328p; do
    ext=prg
    [[ "$target" == avr8-* ]] && ext=hex
    "$mini" --target "$target" examples/eightbit_platform.mini -o "$out/base-$target.$ext"
    "$mini" --target "$target" examples/eightbit_arith.mini -o "$out/arith-$target.$ext"
    "$mini" --target "$target" examples/eightbit_nested.mini -o "$out/nested-$target.$ext"
    "$mini" --target "$target" examples/eightbit_ops.mini -o "$out/ops-$target.$ext"
    "$mini" --target "$target" examples/eightbit_bad_range.mini -o "$out/bad-range-$target.$ext"
    "$mini" --target "$target" examples/eightbit_overflow.mini -o "$out/overflow-$target.$ext"
    "$mini" --target "$target" examples/eightbit_muldiv.mini -o "$out/muldiv-$target.$ext"
    "$mini" --target "$target" examples/eightbit_muldiv_edge.mini -o "$out/muldiv-edge-$target.$ext"
    "$mini" --target "$target" examples/eightbit_mul_overflow.mini -o "$out/mul-overflow-$target.$ext"
    "$mini" --target "$target" examples/eightbit_div_zero.mini -o "$out/div-zero-$target.$ext"
    cat > "$out/recursion.mini" <<'MINI'
fn spin(x: usize) -> usize { return spin(x) }
program { let x: usize = 1; let v: usize = spin(x); return 0 }
MINI
    if "$mini" --target "$target" "$out/recursion.mini" -o "$out/fail.$ext" >/dev/null 2>&1; then
        echo "Recursion incorrectly accepted: $target" >&2; exit 1
    fi
    cat > "$out/overflow.mini" <<'MINI'
program { let x: usize = 65536; return 0 }
MINI
    if "$mini" --target "$target" "$out/overflow.mini" -o "$out/fail.$ext" >/dev/null 2>&1; then
        echo "Overflowing usize incorrectly accepted: $target" >&2; exit 1
    fi
    cat > "$out/unsafe.mini" <<'MINI'
unsafe fn device() { unsafe { asm { nop } } }
program { device(); return 0 }
MINI
    if "$mini" --target "$target" "$out/unsafe.mini" -o "$out/fail.$ext" >/dev/null 2>&1; then
        echo "Unsafe call incorrectly accepted: $target" >&2; exit 1
    fi
done
python3 - "$out" <<'PY'
import pathlib,sys
base=pathlib.Path(sys.argv[1]);
for file in base.glob('*.prg'):
    data=file.read_bytes(); assert data[:14]==bytes([1,8,11,8,10,0,158,50,48,54,49,0,0,0]),file
    assert data[14:16]==bytes.fromhex('d84c'),file # CLD; JMP main
    code=data[15:]; entry=int.from_bytes(code[1:3],'little')
    assert 0x080d<=entry<0x080d+len(code),file
    assert 0x080d+len(code)<=0xa000,file
    if 'arith' in file.name:
        assert bytes.fromhex('a914a200') in code,file # let x: usize = 20
        assert 0x6d in code and 0xed in code,file # ADC / SBC
        assert bytes.fromhex('8d20d0') in code,file # VIC-II border write
    if 'base' in file.name:
        assert bytes.fromhex('8d20d0') in code,file
        assert bytes.fromhex('8d21d0') not in code,file # unused library function omitted
for file in base.glob('*.hex'):
    records=file.read_text().strip().splitlines(); assert records[-1]==':00000001FF',file
    mem={}
    for line in records[:-1]:
        assert line.startswith(':'),file
        r=bytes.fromhex(line[1:]); assert sum(r)%256==0,file
        n,hi,lo,kind=r[:4];assert kind==0 and n==len(r)-5,file
        for i,b in enumerate(r[4:-1]):
            k=hi*256+lo+i; assert k not in mem,file;mem[k]=b
    data=bytes(mem[i] for i in range(len(mem)))
    assert len(data)<32768 and data[:2]==bytes.fromhex('0c94'),file
    assert bytes.fromhex('08e00ebf0fef0dbf') in data,file # SRAM stack reset
    if 'arith' in file.name:
        assert bytes.fromhex('840f951f') in data,file # ADD r24,r20; ADC r25,r21
        assert bytes.fromhex('841b950b') in data,file # SUB r24,r20; SBC r25,r21
    if 'base' in file.name:
        assert bytes.fromhex('04b905b9') in data,file # DDRB and PORTB
print('8-bit 3.7 static regression tests: OK')
PY
python3 tests/emulate8.py "$out"
python3 tests/helper_instruction_selftest.py
