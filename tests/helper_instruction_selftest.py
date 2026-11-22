#!/usr/bin/env python3
"""Independent machine-instruction fixture checks for 16-bit arithmetic helpers.

These assemble *reference sequences* independent of Rust, then execute them in
emulate8.py's deliberately restricted instruction interpreters. They test the
algorithm/ISA selection, NOT the Rust compiler's end-to-end emitted binaries.
"""
import random
import tempfile
from pathlib import Path
from emulate8 import C64, AVR

HL, HR, HM, HC = 0xcff0, 0xcff2, 0xcff4, 0xcff7
class Mos:
    def __init__(self): self.code=bytearray(); self.labels={}; self.refs=[]
    def label(self,s): assert s not in self.labels; self.labels[s]=len(self.code)
    def byte(self,x): self.code.append(x)
    def imm(self,op,x): self.code.extend((op,x))
    def ab(self,op,x): self.code.extend((op,x&255,x>>8))
    def jump(self,op,s): self.code.extend((op,0,0)); self.refs.append((len(self.code)-2,s))
    def bj(self,b,s): self.code.extend((b,3)); self.jump(0x4c,s)
    def lit(self,x): self.imm(0xa9,x&255); self.imm(0xa2,x>>8)
    def load(self,x): self.ab(0xad,x); self.ab(0xae,x+1)
    def save(self,x): self.ab(0x8d,x); self.ab(0x8e,x+1)
    def finish(self):
        for pos,n in self.refs:
            a=0x080d+self.labels[n]; self.code[pos:pos+2]=a.to_bytes(2,'little')
        stub=bytes((0x0b,0x08,10,0,0x9e,50,48,54,49,0,0,0))
        return b'\x01\x08'+stub+self.code

def mos_mul():
    m=Mos(); m.label('start'); m.lit(0); m.save(HM)
    m.label('top'); m.ab(0xad,HR); m.imm(0x29,1);m.bj(0xd0,'noadd')
    m.ab(0xad,HM);m.byte(0x18);m.ab(0x6d,HL);m.ab(0x8d,HM)
    m.ab(0xad,HM+1);m.ab(0x6d,HL+1);m.ab(0x8d,HM+1);m.bj(0x90,'trap')
    m.label('noadd');m.ab(0x4e,HR+1);m.ab(0x6e,HR)
    m.ab(0xad,HR);m.ab(0x0d,HR+1);m.bj(0xd0,'done')
    m.ab(0xad,HL+1);m.imm(0x29,128);m.bj(0xf0,'trap')
    m.ab(0x0e,HL);m.ab(0x2e,HL+1);m.jump(0x4c,'top')
    m.label('done');m.load(HM);m.byte(0x60)
    m.label('trap');m.jump(0x4c,'trap')
    return m.finish()

def mos_div():
    m=Mos();m.label('start');m.ab(0xad,HR);m.ab(0x0d,HR+1);m.bj(0xd0,'trap')
    m.imm(0xa9,0)
    for a in (HM,HM+1,HM+2):m.ab(0x8d,a)
    m.imm(0xa9,16);m.ab(0x8d,HC)
    m.label('top');m.ab(0x0e,HL);m.ab(0x2e,HL+1)
    for a in (HM,HM+1,HM+2):m.ab(0x2e,a)
    m.ab(0xad,HM+2);m.bj(0xf0,'sub')
    m.ab(0xad,HM+1);m.ab(0xcd,HR+1);m.bj(0xb0,'skip');m.bj(0xf0,'sub')
    m.ab(0xad,HM);m.ab(0xcd,HR);m.bj(0x90,'sub');m.jump(0x4c,'skip')
    m.label('sub');m.byte(0x38)
    m.ab(0xad,HM);m.ab(0xed,HR);m.ab(0x8d,HM)
    m.ab(0xad,HM+1);m.ab(0xed,HR+1);m.ab(0x8d,HM+1)
    m.ab(0xad,HM+2);m.imm(0xe9,0);m.ab(0x8d,HM+2)
    m.ab(0xad,HL);m.imm(0x09,1);m.ab(0x8d,HL)
    m.label('skip');m.ab(0xce,HC);m.bj(0xf0,'top')
    m.load(HL);m.byte(0x60);m.label('trap');m.jump(0x4c,'trap')
    return m.finish()

class A:
    def __init__(self):self.words=[];self.labels={};self.refs=[]
    def word(self,w): self.words.append(w)
    def label(self,k):self.labels[k]=len(self.words)
    def jump(self,k):self.word(0x940c);self.refs.append((len(self.words),k));self.word(0)
    def branch(self,op,k):self.word(op|(2<<3));self.jump(k)
    def rr(self,op,d,r):self.word(op|((r&16)<<5)|(d<<4)|(r&15))
    def ldi(self,r,n):self.word(0xe000|((n&0xf0)<<4)|((r-16)<<4)|(n&15))
    def andi(self,r,n):self.word(0x7000|((n&0xf0)<<4)|((r-16)<<4)|(n&15))
    def ori(self,r,n):self.word(0x6000|((n&0xf0)<<4)|((r-16)<<4)|(n&15))
    def single(self,op,r):self.word(op|(r<<4))
    def finish(self):
        for pos,k in self.refs:self.words[pos]=self.labels[k]
        assert len(self.words)<255
        self.words.extend([0]*(257-len(self.words)));self.words[256]=0xcfff
        data=b''.join(w.to_bytes(2,'little') for w in self.words)
        lines=[]
        for addr in range(0,len(data),16):
            chunk=data[addr:addr+16];record=bytes([len(chunk),addr>>8,addr&255,0])+chunk
            record+=bytes([-sum(record)&255]);lines.append(':'+record.hex().upper())
        return ('\n'.join(lines)+'\n:00000001FF\n').encode()

def avr_mul():
    a=A();a.label('start');a.rr(0x2c00,18,24);a.rr(0x2c00,19,25);a.ldi(24,0);a.ldi(25,0)
    a.label('top');a.rr(0x2c00,16,20);a.andi(16,1);a.branch(0xf401,'after')
    a.rr(0x0c00,24,18);a.rr(0x1c00,25,19);a.branch(0xf400,'trap')
    a.label('after');a.single(0x9406,21);a.single(0x9407,20)
    a.rr(0x2c00,16,20);a.rr(0x2800,16,21);a.branch(0xf401,'done')
    a.rr(0x2c00,16,19);a.andi(16,128);a.branch(0xf001,'trap')
    a.rr(0x0c00,18,18);a.rr(0x1c00,19,19);a.jump('top')
    a.label('done');a.word(0x9508);a.label('trap');a.word(0xcfff)
    return a.finish()

def avr_div():
    a=A();a.label('start');a.rr(0x2c00,16,20);a.rr(0x2800,16,21)
    a.branch(0xf401,'trap')
    for r in (22,23,18,19):a.ldi(r,0)
    a.ldi(17,16);a.label('top')
    a.rr(0x0c00,24,24);a.rr(0x1c00,25,25)
    a.rr(0x1c00,22,22);a.rr(0x1c00,23,23);a.rr(0x1c00,18,18)
    a.rr(0x2c00,16,18);a.rr(0x2800,16,16);a.branch(0xf001,'sub')
    a.rr(0x1400,22,20);a.rr(0x0400,23,21);a.branch(0xf000,'sub')
    a.jump('skip');a.label('sub');a.rr(0x1800,22,20);a.rr(0x0800,23,21)
    a.rr(0x0800,18,19);a.ori(24,1)
    a.label('skip');a.single(0x940a,17);a.branch(0xf001,'top')
    a.word(0x9508);a.label('trap');a.word(0xcfff)
    return a.finish()

def verify_one(image, target, x, y, expect=None, fail=False):
    with tempfile.TemporaryDirectory() as td:
        p=Path(td)/('p.prg' if target=='c64' else 'p.hex');p.write_bytes(image)
        if target=='c64':
            emu=C64(p);emu.mem[HL:HL+2]=x.to_bytes(2,'little');emu.mem[HR:HR+2]=y.to_bytes(2,'little')
        else:
            emu=AVR(p);emu.pc=0;emu.stack=[256]
            for r,value in [(24,x&255),(25,x>>8),(20,y&255),(21,y>>8)]:emu.reg[r]=value
        try:emu.run()
        except AssertionError as exc:
            assert fail and 'trap' in str(exc),(target,x,y,str(exc))
            return
        assert not fail,('missing trap',target,x,y)
        result=emu.a|(emu.x<<8) if target=='c64' else emu.reg[24]|(emu.reg[25]<<8)
        assert result==expect, (target,x,y,result,expect)

def main():
    cases=[0,1,2,3,5,7,15,16,17,31,64,127,128,255,256,257,511,1000,1024,4096,16384,32767,32768,65534,65535]
    random.seed(37)
    pairs=[(a,b) for a in cases for b in cases]
    pairs += [(random.randrange(65536),random.randrange(65536)) for _ in range(250)]
    for target, mul, div in [('c64',mos_mul(),mos_div()),('avr',avr_mul(),avr_div())]:
        for x,y in pairs:
            product=x*y
            verify_one(mul,target,x,y,product if product<=65535 else None,product>65535)
            verify_one(div,target,x,y,x//y if y else None,y==0)
        print(f'{target}: {len(pairs)} 16-bit mul and div instruction fixtures passed')

if __name__=='__main__':main()
