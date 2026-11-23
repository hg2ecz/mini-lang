//! x86-64 instruction encoder. IR and inline assembly share this encoder.
//! No arbitrary machine-code bytes are accepted from Mini source code.
use crate::code_buffer::CodeBuffer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Condition {
    Zero, NotZero, NoOverflow, NoCarry, Above, Below, AboveEqual, BelowEqual,
    Less, Greater, LessEqual, GreaterEqual,
}
impl Condition {
    fn branch_code(self) -> u8 {
        match self {
            Self::Zero => 0x84, Self::NotZero => 0x85,
            Self::NoOverflow => 0x81, Self::NoCarry => 0x83,
            Self::Above => 0x87, Self::Below => 0x82,
            Self::AboveEqual => 0x83, Self::BelowEqual => 0x86,
            Self::Less => 0x8c, Self::Greater => 0x8f,
            Self::LessEqual => 0x8e, Self::GreaterEqual => 0x8d,
        }
    }
    fn set_code(self) -> u8 {
        // SETcc uses the same condition low nibble as Jcc, with high nibble 0x9.
        self.branch_code() + 0x10
    }
}

pub(crate) struct X86Encoder { code: CodeBuffer }
impl X86Encoder {
    pub(crate) fn new() -> Self { Self { code: CodeBuffer::new() } }
    pub(crate) fn pos(&self) -> usize { self.code.position() }
    fn bytes(&mut self, bytes: &[u8]) { self.code.emit(bytes); }
    fn imm32(&mut self, n: i32) { self.code.emit_i32(n); }
    fn imm64(&mut self, n: i64) { self.code.emit_i64(n); }
    pub(crate) fn patch_rel32(&mut self, at: usize, to: usize) -> Result<(), String> { self.code.patch_rel32(at, to) }
    pub(crate) fn label(&mut self, name: &str) -> Result<(), String> { self.code.define(name) }

    pub(crate) fn call_named(&mut self, name: &str) {
        self.bytes(&[0xe8]); self.code.rel32_to(name);
    }
    pub(crate) fn jump(&mut self) -> usize {
        self.bytes(&[0xe9]); let at=self.pos(); self.imm32(0); at
    }
    pub(crate) fn branch(&mut self, condition: Condition) -> usize {
        self.bytes(&[0x0f, condition.branch_code()]); let at=self.pos(); self.imm32(0); at
    }
    pub(crate) fn branch_if_rax_zero(&mut self) -> usize {
        self.test_rax(); self.branch(Condition::Zero)
    }

    pub(crate) fn mov_rax_imm64(&mut self, n: i64) { self.bytes(&[0x48,0xb8]); self.imm64(n); }
    pub(crate) fn mov_r9_imm64(&mut self, n: i64) { self.bytes(&[0x49,0xb9]); self.imm64(n); }
    pub(crate) fn mov_rdi_rax(&mut self) { self.bytes(&[0x48,0x89,0xc7]); }
    pub(crate) fn push_rax(&mut self) { self.bytes(&[0x50]); }
    pub(crate) fn pop_rcx(&mut self) { self.bytes(&[0x59]); }
    pub(crate) fn pop_rax(&mut self) { self.bytes(&[0x58]); }
    pub(crate) fn pop_named(&mut self, reg: &str) -> Result<(), String> {
        let bytes: &[u8] = match reg {
            "rax" => &[0x58], "rdi" => &[0x5f], "rsi" => &[0x5e],
            "rdx" => &[0x5a], "r10" => &[0x41,0x5a],
            "r8" => &[0x41,0x58], "r9" => &[0x41,0x59],
            _ => return Err(format!("unsupported assembly input register: {reg}")),
        };
        self.bytes(bytes); Ok(())
    }
    pub(crate) fn local_addr(&mut self, off:i32) { self.bytes(&[0x48,0x8d,0x85]);self.imm32(-off); }
    pub(crate) fn local_load(&mut self, off:i32) { self.bytes(&[0x48,0x8b,0x85]);self.imm32(-off); }
    pub(crate) fn local_store(&mut self, off:i32) { self.bytes(&[0x48,0x89,0x85]);self.imm32(-off); }
    pub(crate) fn load_argument_at_rbp_offset(&mut self, off:i32) {
        self.bytes(&[0x48,0x8b,0x85]);self.imm32(off);
    }
    pub(crate) fn load_field_through_rax(&mut self, off:i32) {
        self.bytes(&[0x48,0x8b,0x80]);self.imm32(off);
    }
    pub(crate) fn store_rcx_through_rax(&mut self, off:i32) {
        self.bytes(&[0x48,0x89,0x88]);self.imm32(off);
    }
    pub(crate) fn enter_frame(&mut self, bytes:i32) {
        self.bytes(&[0x55,0x48,0x89,0xe5,0x48,0x81,0xec]);self.imm32(bytes);
    }
    pub(crate) fn leave_frame(&mut self) { self.bytes(&[0xc9,0xc3]); }
    pub(crate) fn reserve_pad_word(&mut self) { self.bytes(&[0x48,0x83,0xec,0x08]); }
    pub(crate) fn release_call_words(&mut self, count:usize) -> Result<(), String> {
        let bytes = count.checked_mul(8).and_then(|n|i32::try_from(n).ok())
            .ok_or_else(||"call stack adjustment overflow".to_string())?;
        self.bytes(&[0x48,0x81,0xc4]);self.imm32(bytes);Ok(())
    }
    pub(crate) fn neg_rax(&mut self) { self.bytes(&[0x48,0xf7,0xd8]); }
    pub(crate) fn test_rax(&mut self) { self.bytes(&[0x48,0x85,0xc0]); }
    pub(crate) fn test_rdx(&mut self) { self.bytes(&[0x48,0x85,0xd2]); }
    pub(crate) fn compare_rcx_rax(&mut self) { self.bytes(&[0x48,0x39,0xc1]); }
    pub(crate) fn compare_rcx_r9(&mut self) { self.bytes(&[0x4c,0x39,0xc9]); }
    pub(crate) fn compare_rax_r9(&mut self) { self.bytes(&[0x4c,0x39,0xc8]); }
    pub(crate) fn compare_rax_minus_one(&mut self) { self.bytes(&[0x48,0x83,0xf8,0xff]); }
    pub(crate) fn add_rax_rcx(&mut self) { self.bytes(&[0x48,0x01,0xc8]); }
    pub(crate) fn sub_rcx_rax_to_rax(&mut self) { self.bytes(&[0x48,0x29,0xc1,0x48,0x89,0xc8]); }
    pub(crate) fn multiply_signed_rax_rcx(&mut self) { self.bytes(&[0x48,0x0f,0xaf,0xc1]); }
    pub(crate) fn multiply_unsigned_rax_rcx(&mut self) { self.bytes(&[0x48,0xf7,0xe1]); }
    pub(crate) fn divide_unsigned_rcx_by_rax(&mut self) {
        // Copy divisor to r8, dividend to rax, clear rdx and perform DIV r8.
        self.bytes(&[0x49,0x89,0xc0,0x48,0x89,0xc8,0x48,0x31,0xd2,0x49,0xf7,0xf0]);
    }
    pub(crate) fn divide_signed_rcx_by_rax(&mut self) {
        // Copy divisor to r8, dividend to rax, CQO then IDIV r8.
        self.bytes(&[0x49,0x89,0xc0,0x48,0x89,0xc8,0x48,0x99,0x49,0xf7,0xf8]);
    }
    pub(crate) fn set_comparison(&mut self, op:&str, unsigned:bool) -> Result<(), String> {
        let condition=match op {
            "=="=>Condition::Zero,"!="=>Condition::NotZero,
            "<" if unsigned=>Condition::Below,
            ">" if unsigned=>Condition::Above,
            "<=" if unsigned=>Condition::BelowEqual,
            ">=" if unsigned=>Condition::AboveEqual,
            "<"=>Condition::Less,">"=>Condition::Greater,
            "<="=>Condition::LessEqual,">="=>Condition::GreaterEqual,
            _=>return Err(format!("unsupported comparison: {op}")),
        };
        self.bytes(&[0x0f,condition.set_code(),0xc0,0x48,0x0f,0xb6,0xc0]);
        Ok(())
    }
    pub(crate) fn syscall(&mut self) { self.bytes(&[0x0f,0x05]); }
    /// UD2: guaranteed invalid opcode; Linux delivers SIGILL. Used as Mini's
    /// runtime trap so a failed check can never look like a normal exit code.
    pub(crate) fn ud2(&mut self) { self.bytes(&[0x0f,0x0b]); }
    pub(crate) fn asm_instruction(&mut self, name: &str) -> Result<(), String> {
        match name {
            "nop"=>self.bytes(&[0x90]),"pause"=>self.bytes(&[0xf3,0x90]),
            "lfence"=>self.bytes(&[0x0f,0xae,0xe8]),
            "sfence"=>self.bytes(&[0x0f,0xae,0xf8]),
            "mfence"=>self.bytes(&[0x0f,0xae,0xf0]),
            "syscall"=>self.syscall(),
            _=>return Err(format!("unsupported asm instruction: {name}")),
        }
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Vec<u8>, String> { self.code.finish() }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn named_asm_uses_same_encoder_as_ir() {
        let mut e=X86Encoder::new();e.mov_rax_imm64(42);e.asm_instruction("pause").unwrap();
        assert_eq!(e.finish().unwrap(),vec![0x48,0xb8,42,0,0,0,0,0,0,0,0xf3,0x90]);
    }
    #[test] fn comparisons_use_correct_signedness() {
        let mut e=X86Encoder::new();
        e.set_comparison("<", false).unwrap(); e.set_comparison("<", true).unwrap();
        assert_eq!(e.finish().unwrap(),vec![0x0f,0x9c,0xc0,0x48,0x0f,0xb6,0xc0,0x0f,0x92,0xc0,0x48,0x0f,0xb6,0xc0]);
    }
    #[test] fn known_calls_and_branches_use_rel32() {
        let mut e=X86Encoder::new();
        e.call_named("target");e.branch(Condition::Zero);e.label("target").unwrap();
        assert_eq!(e.finish().unwrap(),vec![0xe8,6,0,0,0,0x0f,0x84,0,0,0,0]);
    }
    #[test] fn arithmetic_instructions_match_x86_64_encoding() {
        let mut e=X86Encoder::new();
        e.add_rax_rcx();e.sub_rcx_rax_to_rax();e.multiply_signed_rax_rcx();
        e.multiply_unsigned_rax_rcx();e.test_rdx();
        assert_eq!(e.finish().unwrap(),vec![
            0x48,0x01,0xc8, 0x48,0x29,0xc1,0x48,0x89,0xc8,
            0x48,0x0f,0xaf,0xc1,0x48,0xf7,0xe1,0x48,0x85,0xd2,
        ]);
    }
    #[test] fn unsupported_asm_fails() { assert!(X86Encoder::new().asm_instruction("mov").is_err()); }
}
