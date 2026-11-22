#!/usr/bin/env python3
"""Tiny *test-only* interpreters for the emitted AVR / MOS6510 instruction subsets.
Not part of the Mini compiler. Strictly rejects unsupported opcodes. Does not
replace C64/AVR hardware or VICE testing; intended for algorithm regression.
"""
import pathlib
import sys

LIMIT = 100_000


def read_hex(path):
    mem = {}
    for line in path.read_text().splitlines():
        if not line:
            continue
        assert line[0] == ':', line
        r = bytes.fromhex(line[1:])
        assert sum(r) % 256 == 0, line
        n, hi, lo, kind = r[:4]
        assert n == len(r) - 5
        if kind == 1:
            break
        assert kind == 0
        for i, b in enumerate(r[4:-1]):
            addr = (hi << 8) | lo
            assert addr + i not in mem
            mem[addr + i] = b
    return bytes(mem[i] for i in range(len(mem)))


class C64:
    def __init__(self, path):
        prg = path.read_bytes()
        assert prg[:2] == b'\x01\x08'
        self.mem = bytearray(65536)
        self.mem[0x0801:0x0801+len(prg)-2] = prg[2:]
        self.pc = 0x080d
        self.a = self.x = self.y = 0
        self.c = self.z = False
        self.stack = []
        self.writes = []

    def byte(self):
        b = self.mem[self.pc]
        self.pc = (self.pc + 1) & 0xffff
        return b

    def word(self):
        lo = self.byte()
        return lo | self.byte() << 8

    def setz(self, n):
        self.z = (n & 255) == 0

    def cmp(self, left, right):
        self.c = left >= right
        self.z = left == right

    def run(self):
        for _ in range(LIMIT):
            old = self.pc
            op = self.byte()
            if op == 0xa9:  # LDA #imm
                self.a = self.byte(); self.setz(self.a)
            elif op == 0xa2:  # LDX #imm
                self.x = self.byte(); self.setz(self.x)
            elif op in (0xad, 0xae, 0x8d, 0x8e, 0x6d, 0xed, 0xcd, 0x0d, 0x0e, 0x2e, 0x4e, 0x6e, 0xce):
                addr = self.word()
                v = self.mem[addr]
                if op == 0xad:
                    self.a = v; self.setz(self.a)
                elif op == 0xae:
                    self.x = v; self.setz(self.x)
                elif op == 0x8d:
                    self.mem[addr] = self.a; self.writes.append((addr, self.a))
                elif op == 0x8e:
                    self.mem[addr] = self.x
                elif op == 0xcd:
                    self.cmp(self.a, v)
                elif op == 0x0d: # ORA abs
                    self.a |= v; self.setz(self.a)
                elif op in (0x0e, 0x2e, 0x4e, 0x6e):
                    carry = int(self.c)
                    if op == 0x0e: result = v << 1; self.c = bool(v & 128)
                    elif op == 0x2e: result = (v << 1) | carry; self.c = bool(v & 128)
                    elif op == 0x4e: result = v >> 1; self.c = bool(v & 1)
                    else: result = (v >> 1) | (carry << 7); self.c = bool(v & 1)
                    self.mem[addr] = result & 255; self.setz(result)
                elif op == 0xce: # DEC abs
                    self.mem[addr] = (v - 1) & 255; self.setz(self.mem[addr])
                else:
                    if op == 0x6d:  # ADC
                        r = self.a + v + int(self.c)
                        self.c = r > 255
                    else:  # SBC
                        r = self.a - v - (1 - int(self.c))
                        self.c = r >= 0
                    self.a = r & 255
                    self.setz(self.a)
            elif op == 0x29: # AND immediate
                self.a &= self.byte(); self.setz(self.a)
            elif op == 0x09: # ORA immediate
                self.a |= self.byte(); self.setz(self.a)
            elif op == 0xe9: # SBC immediate
                rhs = self.byte()
                n = self.a - rhs - (1 - int(self.c))
                self.c = n >= 0; self.a = n & 255; self.setz(self.a)
            elif op == 0xc9:  # CMP #imm
                self.cmp(self.a, self.byte())
            elif op == 0xe0:  # CPX #imm
                self.cmp(self.x, self.byte())
            elif op == 0x18: self.c = False  # CLC
            elif op == 0x38: self.c = True   # SEC
            elif op == 0xa8: self.y = self.a; self.setz(self.y)
            elif op == 0x8a: self.a = self.x; self.setz(self.a)
            elif op == 0xaa: self.x = self.a; self.setz(self.x)
            elif op == 0x98: self.a = self.y; self.setz(self.a)
            elif op in (0xd0, 0xf0, 0x90, 0xb0):  # BNE, BEQ, BCC, BCS
                k = self.byte()
                if k >= 128: k -= 256
                yes = {0xd0: not self.z, 0xf0: self.z, 0x90: not self.c, 0xb0: self.c}[op]
                if yes: self.pc = (self.pc + k) & 0xffff
            elif op == 0x4c:  # JMP abs
                target = self.word()
                if target == old: raise AssertionError(f'C64 trap at ${old:04x}')
                self.pc = target
            elif op == 0x20:  # JSR abs
                target = self.word()
                self.stack.append(self.pc)
                self.pc = target
            elif op == 0x60:  # RTS
                if not self.stack: return self.writes
                self.pc = self.stack.pop()
            elif op in (0xea, 0x78, 0x58, 0xd8):  # NOP, SEI, CLI, CLD
                pass
            else:
                raise AssertionError(f'unknown 6510 opcode ${op:02x} at ${old:04x}')
        raise AssertionError('C64 instruction limit exceeded (possibly stuck in loop)')


class AVR:
    def __init__(self, path):
        self.flash = read_hex(path)
        self.mem = bytearray(0x900)
        self.pc = 0  # AVR program counter counts 16-bit words
        self.reg = [0] * 32
        self.c = self.z = False
        self.stack = []
        self.io = {}
        self.finished_main = False

    def word(self):
        assert 2*self.pc+1 < len(self.flash), f'AVR PC out of flash: {self.pc}'
        v = int.from_bytes(self.flash[2*self.pc:2*self.pc+2], 'little')
        self.pc += 1
        return v

    def binary(self, w):
        d = (w >> 4) & 31
        r = (w & 15) | ((w >> 5) & 16)
        return d, r

    def compute(self, op, d, r):
        old_z = self.z
        lhs, rhs = self.reg[d], self.reg[r]
        if op == 0x0c00:
            v = lhs + rhs; self.c = v > 255; self.z = (v & 255) == 0
        elif op == 0x1c00:
            v = lhs + rhs + int(self.c); self.c = v > 255; self.z = old_z and (v & 255) == 0
        elif op in (0x1800, 0x1400):
            v = lhs - rhs; self.c = v < 0; self.z = (v & 255) == 0
        else:  # SBC, CPC
            v = lhs - rhs - int(self.c); self.c = v < 0; self.z = old_z and (v & 255) == 0
        if op not in (0x1400, 0x0400): self.reg[d] = v & 255

    def run(self):
        for _ in range(LIMIT):
            old = self.pc
            w = self.word()
            if w in (0x940c, 0x940e):  # JMP/CALL absolute 2 words for 32KiB MCU
                target = self.word()
                if w == 0x940e: self.stack.append(self.pc)
                self.pc = target
            elif w == 0x9508:  # RET
                assert self.stack, 'AVR RET without caller'
                self.pc = self.stack.pop()
                if not self.stack: self.finished_main = True
            elif (w & 0xf000) == 0xc000:  # RJMP signed 12-bit
                k = w & 0x0fff
                if k >= 0x800: k -= 0x1000
                self.pc += k
                if self.pc == old:
                    if self.finished_main: return self.io
                    raise AssertionError(f'AVR trap at word {old:04x}')
            elif (w & 0xf000) == 0xe000:  # LDI
                d = 16 + ((w >> 4) & 15)
                self.reg[d] = ((w >> 4) & 0xf0) | (w & 15)
            elif (w & 0xf800) == 0xb800:  # OUT
                port = ((w >> 5) & 0x30) | (w & 15)
                r = (w >> 4) & 31
                self.io[port] = self.reg[r]
            elif (w & 0xfe0f) in (0x9200, 0x9000):  # STS/LDS abs
                addr = self.word()
                assert 0 <= addr < len(self.mem)
                reg = (w >> 4) & 31
                if (w & 0xfe0f) == 0x9200: self.mem[addr] = self.reg[reg]
                else: self.reg[reg] = self.mem[addr]
            elif (w & 0xf000) in (0x7000, 0x6000): # ANDI, ORI
                d = 16 + ((w >> 4) & 15)
                imm = ((w >> 4) & 0xf0) | (w & 15)
                if (w & 0xf000) == 0x7000: self.reg[d] &= imm
                else: self.reg[d] |= imm
                self.z = self.reg[d] == 0
            elif (w & 0xfe0f) in (0x9406, 0x9407, 0x940a): # LSR, ROR, DEC
                d = (w >> 4) & 31
                old = self.reg[d]
                if (w & 0xfe0f) == 0x9406:
                    self.c = bool(old & 1); self.reg[d] = old >> 1
                elif (w & 0xfe0f) == 0x9407:
                    c = int(self.c); self.c = bool(old & 1)
                    self.reg[d] = (old >> 1) | (c << 7)
                else:
                    self.reg[d] = (old - 1) & 255
                self.z = self.reg[d] == 0
            elif (w & 0xfc00) in (0x2c00, 0x2800):  # MOV / OR
                d, r = self.binary(w)
                if (w & 0xfc00) == 0x2800:
                    self.reg[d] |= self.reg[r]
                    self.z = self.reg[d] == 0
                else: self.reg[d] = self.reg[r]
            elif (w & 0xfc00) in (0x0c00, 0x1c00, 0x1800, 0x0800, 0x1400, 0x0400):
                self.compute(w & 0xfc00, *self.binary(w))
            elif (w & 0xfc00) in (0xf000, 0xf400):  # BRBS/BRBC (C/Z only)
                k = (w >> 3) & 127
                if k >= 64: k -= 128
                s = w & 7
                assert s in (0,1), f'unsupported SREG flag bit {s}'
                value = self.c if s == 0 else self.z
                if value == ((w & 0x0400) == 0): self.pc += k
            elif w in (0, 0x9478, 0x94f8):  # NOP / SEI / CLI
                pass
            else:
                raise AssertionError(f'unknown AVR instruction ${w:04x} at word {old:04x}')
        raise AssertionError('AVR instruction limit exceeded')


def main(directory):
    checks = [('base', 6, 32), ('arith', 6, 32), ('nested', 0, 0), ('ops', 6, 32), ('muldiv', 6, 32), ('muldiv-edge', 6, 32)]
    for stem, expected_c64, expected_avr in checks:
        c64 = C64(directory / f'{stem}-c64-6510.prg').run()
        border = [v for addr,v in c64 if addr == 53280]
        assert border == [expected_c64], (stem, border, expected_c64)
        avr = AVR(directory / f'{stem}-avr8-atmega328p.hex').run()
        assert avr.get(4) == expected_avr and avr.get(5) == expected_avr, (stem, avr)
        print(f'{stem}: 6510 border={border[-1]}, AVR PORTB={avr[5]}')
    for stem in ('bad-range', 'overflow', 'mul-overflow', 'div-zero'):
        for t, ext, runner in [('c64-6510','prg',C64),('avr8-atmega328p','hex',AVR)]:
            try:
                runner(directory / f'{stem}-{t}.{ext}').run()
            except AssertionError as err:
                assert 'trap' in str(err), (stem, t, err)
            else:
                raise AssertionError(f'{stem} did not trap on {t}')
    print('8-bit interpreters: 6 successful programs and 4 traps on both targets')


if __name__ == '__main__':
    assert len(sys.argv) == 2
    main(pathlib.Path(sys.argv[1]))
