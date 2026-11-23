//! Small AArch64 instruction encoder. All words are emitted little-endian.
//! The x86_64 and AArch64 backends share the same typed Mini HIR.
use crate::Res;
use crate::code_buffer::CodeBuffer;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Cond {
    Eq = 0,
    Ne = 1,
    Cs = 2,
    Cc = 3,
    Vc = 7,
    Hi = 8,
    Ls = 9,
    Ge = 10,
    Lt = 11,
    Gt = 12,
    Le = 13,
}

pub(crate) struct Encoder {
    code: CodeBuffer,
}
impl Encoder {
    pub(crate) fn new() -> Self {
        Self {
            code: CodeBuffer::new(),
        }
    }
    pub(crate) fn pos(&self) -> usize {
        self.code.position()
    }
    pub(crate) fn word(&mut self, w: u32) {
        self.code.emit_u32(w);
    }
    /// UDF #0 (permanently undefined); Linux delivers SIGILL. Mini runtime trap.
    pub(crate) fn udf(&mut self) {
        self.word(0x0000_0000);
    }
    pub(crate) fn label(&mut self, name: &str) -> Res<()> {
        self.code.define(name)
    }
    pub(crate) fn patch(&mut self, at: usize, to: usize, bits: u32) -> Res<()> {
        self.code.patch_a64_branch(at, to, bits)
    }
    pub(crate) fn b(&mut self) -> usize {
        let p = self.pos();
        self.word(0x1400_0000);
        p
    }
    pub(crate) fn b_cond(&mut self, c: Cond) -> usize {
        let p = self.pos();
        self.word(0x5400_0000 | c as u32);
        p
    }
    pub(crate) fn cbz(&mut self, r: u8) -> usize {
        let p = self.pos();
        self.word(0xB400_0000 | r as u32);
        p
    }
    pub(crate) fn cbnz(&mut self, r: u8) -> usize {
        let p = self.pos();
        self.word(0xB500_0000 | r as u32);
        p
    }
    pub(crate) fn call(&mut self, name: &str) {
        self.code.a64_call_to(name);
    }
    pub(crate) fn finish(self) -> Res<Vec<u8>> {
        self.code.finish()
    }
    pub(crate) fn mov(&mut self, d: u8, s: u8) {
        self.word(0xAA00_03E0 | ((s as u32) << 16) | d as u32);
    }
    pub(crate) fn imm(&mut self, d: u8, n: i64) {
        let n = n as u64;
        for half in 0..4 {
            let value = ((n >> (half * 16)) & 65535) as u32;
            self.word(
                (if half == 0 { 0xD280_0000 } else { 0xF280_0000 })
                    | ((half as u32) << 21)
                    | (value << 5)
                    | d as u32,
            );
        }
    }
    // Immediate-size restriction is centralized; the caller splits large offsets.
    pub(crate) fn add_imm(&mut self, d: u8, n: u8, v: u16) {
        assert!(v <= 4095);
        self.word(0x9100_0000 | ((v as u32) << 10) | ((n as u32) << 5) | d as u32);
    }
    pub(crate) fn sub_imm(&mut self, d: u8, n: u8, v: u16) {
        assert!(v <= 4095);
        self.word(0xD100_0000 | ((v as u32) << 10) | ((n as u32) << 5) | d as u32);
    }
    pub(crate) fn frame_start(&mut self, size: i32) {
        self.word(0xA9BF_7BFD); // stp x29,x30,[sp,#-16]!
        self.add_imm(29, 31, 0); // mov x29,sp
        self.adjust_sp(size, false);
    }
    pub(crate) fn frame_end(&mut self) {
        self.add_imm(31, 29, 0); // mov sp,x29
        self.word(0xA8C1_7BFD); // ldp x29,x30,[sp],#16
        self.word(0xD65F_03C0); // ret
    }
    fn adjust_sp(&mut self, mut size: i32, increase: bool) {
        assert!(size >= 0 && size % 16 == 0);
        while size > 0 {
            let n = size.min(4080) as u16;
            if increase {
                self.add_imm(31, 31, n);
            } else {
                self.sub_imm(31, 31, n);
            }
            size -= n as i32;
        }
    }
    pub(crate) fn push(&mut self, r: u8) {
        self.sub_imm(31, 31, 16);
        self.str(r, 31);
    }
    pub(crate) fn pop(&mut self, r: u8) {
        self.ldr(r, 31);
        self.add_imm(31, 31, 16);
    }
    pub(crate) fn pop_args(&mut self, n: usize) -> Res<()> {
        let bytes = n
            .checked_mul(16)
            .and_then(|b| i32::try_from(b).ok())
            .ok_or("argument-stack overflow")?;
        self.adjust_sp(bytes, true);
        Ok(())
    }
    pub(crate) fn ldr(&mut self, d: u8, addr: u8) {
        self.word(0xF940_0000 | ((addr as u32) << 5) | d as u32);
    }
    pub(crate) fn str(&mut self, r: u8, addr: u8) {
        self.word(0xF900_0000 | ((addr as u32) << 5) | r as u32);
    }
    fn address(&mut self, base: u8, displacement: i32) {
        self.mov(9, base);
        let mut left = displacement.unsigned_abs();
        while left > 0 {
            let n = left.min(4095) as u16;
            if displacement < 0 {
                self.sub_imm(9, 9, n);
            } else {
                self.add_imm(9, 9, n);
            }
            left -= n as u32;
        }
    }
    pub(crate) fn local_addr(&mut self, off: i32) {
        self.address(29, -off);
        self.mov(0, 9);
    }
    pub(crate) fn local_load(&mut self, off: i32) {
        self.address(29, -off);
        self.ldr(0, 9);
    }
    pub(crate) fn local_store(&mut self, off: i32) {
        self.address(29, -off);
        self.str(0, 9);
    }
    pub(crate) fn arg_load(&mut self, off: i32) {
        self.address(29, off);
        self.ldr(0, 9);
    }
    pub(crate) fn load_field(&mut self, off: i32) {
        self.address(0, off);
        self.ldr(0, 9);
    }
    pub(crate) fn store_field_from_x1(&mut self, off: i32) {
        self.address(0, off);
        self.str(1, 9);
    }
    pub(crate) fn adds(&mut self) {
        self.word(0xAB01_0000);
    } // adds x0,x0,x1
    pub(crate) fn subs_x1_x0(&mut self) {
        self.word(0xEB00_0020);
    } // subs x0,x1,x0
    pub(crate) fn negs(&mut self) {
        self.word(0xEB00_03E0);
    } // negs x0,x0
    pub(crate) fn cmp(&mut self, a: u8, b: u8) {
        self.word(0xEB00_001F | ((b as u32) << 16) | ((a as u32) << 5));
    }
    pub(crate) fn mul(&mut self) {
        self.word(0x9B00_7C20);
    } // mul x0,x1,x0
    pub(crate) fn umulh(&mut self) {
        self.word(0x9BC0_7C22);
    } // umulh x2,x1,x0
    pub(crate) fn smulh(&mut self) {
        self.word(0x9B40_7C22);
    } // smulh x2,x1,x0
    pub(crate) fn asr63(&mut self) {
        self.word(0x937F_FC03);
    } // asr x3,x0,#63
    pub(crate) fn udiv(&mut self) {
        self.word(0x9AC0_0820);
    } // udiv x0,x1,x0
    pub(crate) fn sdiv(&mut self) {
        self.word(0x9AC0_0C20);
    } // sdiv x0,x1,x0
    pub(crate) fn cset(&mut self, cond: Cond) {
        self.word(0x9A9F_07E0 | (((cond as u32) ^ 1) << 12)); // csinc x0,xzr,xzr,!cond
    }
    pub(crate) fn svc(&mut self) {
        self.word(0xD400_0001);
    }
    pub(crate) fn asm_instruction(&mut self, name: &str) -> Res<()> {
        match name {
            "nop" => self.word(0xD503_201F),
            "yield" => self.word(0xD503_203F),
            "dmb" => self.word(0xD503_3BBF), // dmb ish
            "isb" => self.word(0xD503_3FDF),
            "svc" => self.svc(),
            _ => return Err(format!("unsupported AArch64 asm instruction: {name}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encodes_known_a64_words() {
        let mut e = Encoder::new();
        e.imm(0, 42);
        e.asm_instruction("nop").unwrap();
        e.svc();
        let c = e.finish().unwrap();
        assert_eq!(u32::from_le_bytes(c[0..4].try_into().unwrap()), 0xD280_0540);
        assert_eq!(
            u32::from_le_bytes(c[16..20].try_into().unwrap()),
            0xD503_201F
        );
        assert_eq!(
            u32::from_le_bytes(c[20..24].try_into().unwrap()),
            0xD400_0001
        );
    }
    #[test]
    fn rejects_unaligned_or_too_far_branches() {
        let mut e = Encoder::new();
        let at = e.b();
        assert!(e.patch(at, 2, 26).is_err());
        assert!(e.patch(at, 1 << 29, 26).is_err());
        e.patch(at, 0, 26).unwrap();
        assert_eq!(e.finish().unwrap(), vec![0, 0, 0, 0x14]);
    }
    #[test]
    fn all_scalar_opcodes_match_clang_aarch64_assembler() {
        // Reference encodings cross-checked with clang 17 -target aarch64-linux-gnu.
        let mut e = Encoder::new();
        e.mov(0, 1);
        e.adds();
        e.subs_x1_x0();
        e.negs();
        e.cmp(1, 0);
        e.mul();
        e.umulh();
        e.smulh();
        e.asr63();
        e.udiv();
        e.sdiv();
        e.cset(Cond::Eq);
        for op in ["nop", "yield", "dmb", "isb", "svc"] {
            e.asm_instruction(op).unwrap();
        }
        let code = e.finish().unwrap();
        let got: Vec<u32> = code
            .chunks_exact(4)
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        assert_eq!(
            got,
            vec![
                0xAA01_03E0,
                0xAB01_0000,
                0xEB00_0020,
                0xEB00_03E0,
                0xEB00_003F,
                0x9B00_7C20,
                0x9BC0_7C22,
                0x9B40_7C22,
                0x937F_FC03,
                0x9AC0_0820,
                0x9AC0_0C20,
                0x9A9F_17E0,
                0xD503_201F,
                0xD503_203F,
                0xD503_3BBF,
                0xD503_3FDF,
                0xD400_0001,
            ]
        );
    }
    #[test]
    fn named_forward_and_backward_calls() {
        let mut e = Encoder::new();
        e.call("main");
        e.label("main").unwrap();
        e.call("main");
        let bytes = e.finish().unwrap();
        assert_eq!(
            u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            0x9400_0001
        );
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            0x9400_0000
        );
    }
}
