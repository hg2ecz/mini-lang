//! Minimal Thumb-2 encoder for Cortex-M3/M4. All instruction effects are known.
//! This is deliberately not a general-purpose assembler.
use crate::Res;

pub(crate) struct Encoder {
    data: Vec<u8>,
    calls: Vec<(usize, String)>,
    labels: std::collections::HashMap<String, usize>,
}
impl Encoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            calls: Vec::new(),
            labels: Default::default(),
        }
    }
    pub fn pos(&self) -> usize {
        self.data.len()
    }
    #[cfg(test)]
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }
    pub fn h(&mut self, hw: u16) {
        self.data.extend_from_slice(&hw.to_le_bytes());
    }
    pub fn w(&mut self, first: u16, second: u16) {
        self.h(first);
        self.h(second);
    }
    pub fn label(&mut self, name: &str) -> Res<()> {
        if self.labels.insert(name.to_owned(), self.pos()).is_some() {
            return Err(format!("duplicate Thumb label: {name}"));
        }
        Ok(())
    }
    pub fn call(&mut self, name: &str) {
        let at = self.pos();
        self.w(0xF000, 0xF800);
        self.calls.push((at, name.to_owned()));
    }
    pub fn patch_bl(&mut self, at: usize, target: usize) -> Res<()> {
        if at % 2 != 0 || target % 2 != 0 || at + 4 > self.data.len() {
            return Err("Thumb BL address misaligned/outside code".into());
        }
        let delta = target as i64 - at as i64 - 4;
        if delta % 2 != 0 || !(-16_777_216..=16_777_214).contains(&delta) {
            return Err("Thumb BL outside +/-16MiB range".into());
        }
        let imm = (delta as i32 as u32) & 0x01ff_ffff;
        let s = (imm >> 24) & 1;
        let i1 = (imm >> 23) & 1;
        let i2 = (imm >> 22) & 1;
        let j1 = (!(i1 ^ s)) & 1;
        let j2 = (!(i2 ^ s)) & 1;
        let first = 0xF000 | ((s as u16) << 10) | (((imm >> 12) & 0x3ff) as u16);
        // imm11 holds halfword offset bits [11:1]; bit 0 of the byte offset is implicit.
        let second =
            0xF800 | ((j1 as u16) << 13) | ((j2 as u16) << 11) | (((imm >> 1) & 0x7ff) as u16);
        self.data[at..at + 2].copy_from_slice(&first.to_le_bytes());
        self.data[at + 2..at + 4].copy_from_slice(&second.to_le_bytes());
        Ok(())
    }
    pub fn branch(&mut self) -> usize {
        let at = self.pos();
        self.h(0xE000);
        at
    }
    pub fn cond(&mut self, cc: u16) -> usize {
        let at = self.pos();
        self.h(0xD000 | ((cc & 15) << 8));
        at
    }
    pub fn patch_b(&mut self, at: usize, to: usize) -> Res<()> {
        if at % 2 != 0 || to % 2 != 0 || at + 2 > self.pos() {
            return Err("invalid Thumb B offset".into());
        }
        let delta = to as i64 - at as i64 - 4;
        if delta % 2 != 0 || !(-2048..=2046).contains(&delta) {
            return Err("Thumb short B exceeds +/-2KiB; split function".into());
        }
        let old = u16::from_le_bytes(self.data[at..at + 2].try_into().unwrap());
        self.data[at..at + 2]
            .copy_from_slice(&(0xE000 | ((delta as i16 as u16 >> 1) & 0x07ff)).to_le_bytes());
        if old & 0xf800 != 0xe000 {
            return Err("expected Thumb B placeholder".into());
        }
        Ok(())
    }
    pub fn patch_cond(&mut self, at: usize, to: usize) -> Res<()> {
        if at % 2 != 0 || to % 2 != 0 || at + 2 > self.pos() {
            return Err("invalid Thumb B.cond offset".into());
        }
        let delta = to as i64 - at as i64 - 4;
        if delta % 2 != 0 || !(-256..=254).contains(&delta) {
            return Err("Thumb conditional branch exceeds +/-256 bytes".into());
        }
        let old = u16::from_le_bytes(self.data[at..at + 2].try_into().unwrap());
        if old & 0xf000 != 0xd000 || (old >> 8) & 0xf >= 14 {
            return Err("expected Thumb conditional placeholder".into());
        }
        self.data[at..at + 2]
            .copy_from_slice(&((old & 0xff00) | ((delta as i16 as u16 >> 1) & 0xff)).to_le_bytes());
        Ok(())
    }
    pub fn movs(&mut self, reg: u8, imm: u8) {
        self.h(0x2000 | ((reg as u16) << 8) | imm as u16);
    }
    pub fn imm32(&mut self, value: u32) {
        let bs = value.to_be_bytes();
        self.movs(0, bs[0]);
        for &byte in &bs[1..] {
            self.h(0x0200); // lsls r0,r0,#8
            if byte != 0 {
                self.movs(1, byte);
                self.h(0x1840);
            } // adds r0,r0,r1
        }
    }
    pub fn push_lr(&mut self) {
        self.h(0xB510);
    } // push {r4,lr}, 8-byte aligned
    pub fn pop_pc(&mut self) {
        self.h(0xBD10);
    } // pop {r4,pc}
    pub fn save0(&mut self) {
        self.h(0xB405);
    } // push {r0,r2} (8 bytes)
    pub fn restore0(&mut self) {
        self.h(0xBC05);
    } // pop {r0,r2}
    pub fn move_r1_r0(&mut self) {
        self.h(0x4601);
    }
    pub fn sub_sp(&mut self, bytes: u16) -> Res<()> {
        if bytes > 508 || bytes % 4 != 0 {
            return Err("Thumb frame must be <=508 bytes, word aligned".into());
        }
        self.h(0xB080 | bytes / 4);
        Ok(())
    }
    pub fn add_sp(&mut self, bytes: u16) -> Res<()> {
        if bytes > 508 || bytes % 4 != 0 {
            return Err("Thumb frame must be <=508 bytes, word aligned".into());
        }
        self.h(0xB000 | bytes / 4);
        Ok(())
    }
    pub fn store(&mut self, reg: u8, offset: u16) -> Res<()> {
        if reg > 7 || offset % 4 != 0 || offset > 1020 {
            return Err("Thumb SP-relative STR out of range".into());
        }
        self.h(0x9000 | ((reg as u16) << 8) | (offset / 4));
        Ok(())
    }
    pub fn load(&mut self, reg: u8, offset: u16) -> Res<()> {
        if reg > 7 || offset % 4 != 0 || offset > 1020 {
            return Err("Thumb SP-relative LDR out of range".into());
        }
        self.h(0x9800 | ((reg as u16) << 8) | (offset / 4));
        Ok(())
    }
    pub fn trap(&mut self) {
        self.h(0xBE00);
        self.h(0xE7FE);
    } // BKPT #0 + fail-closed infinite loop: Mini runtime trap
    /// Normal end of firmware (`program` returned): BKPT #1 + loop, distinct from a trap.
    pub fn halt(&mut self) {
        self.h(0xBE01);
        self.h(0xE7FE);
    }
    pub fn finish(mut self) -> Res<Vec<u8>> {
        let calls = std::mem::take(&mut self.calls);
        for (at, name) in calls {
            let dest = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("unresolved Thumb function: {name}"))?;
            self.patch_bl(at, dest)?;
        }
        Ok(self.data)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thumb_immediates_and_returns() {
        let mut e = Encoder::new();
        e.movs(0, 42);
        e.push_lr();
        e.pop_pc();
        assert_eq!(e.finish().unwrap(), [0x2a, 0x20, 0x10, 0xb5, 0x10, 0xbd]);
    }
    #[test]
    fn thumb_forward_and_backward_branches() {
        let mut e = Encoder::new();
        let a = e.branch();
        e.h(0xbf00);
        e.patch_b(a, e.pos()).unwrap();
        assert_eq!(&e.bytes()[..2], &[0, 0xe0]);
        let b = e.branch();
        e.patch_b(b, b).unwrap();
        assert_eq!(&e.bytes()[b..b + 2], &[0xfe, 0xe7]);
    }
    #[test]
    fn thumb_bl_rel32() {
        let mut e = Encoder::new();
        e.call("f");
        e.h(0xbf00);
        e.label("f").unwrap();
        e.h(0x4770);
        let b = e.finish().unwrap();
        assert_eq!(&b[..4], &[0, 0xf0, 1, 0xf8]);
    }
    /// Reference encodings produced by `llvm-mc -triple=thumbv7m-none-eabi` (LLVM 23).
    /// The delta is the byte distance from (BL address + 4) to the target.
    #[test]
    fn thumb_bl_matches_llvm_reference() {
        let cases: [(i64, u16, u16); 9] = [
            (2, 0xf000, 0xf801),
            (6, 0xf000, 0xf803),
            (2048, 0xf000, 0xfc00),
            (4094, 0xf000, 0xffff),
            (4096, 0xf001, 0xf800),
            (1_048_576, 0xf100, 0xf800),
            (-8, 0xf7ff, 0xfffc),
            (-2054, 0xf7ff, 0xfbfd),
            (-1_048_580, 0xf6ff, 0xfffe),
        ];
        for (delta, first, second) in cases {
            let size = 2_200_000usize;
            let at: usize = if delta < 0 { 1_100_000 } else { 0 };
            let target = (at as i64 + 4 + delta) as usize;
            let mut e = Encoder::new();
            e.data = vec![0; size];
            e.patch_bl(at, target).unwrap();
            let got = [
                u16::from_le_bytes([e.data[at], e.data[at + 1]]),
                u16::from_le_bytes([e.data[at + 2], e.data[at + 3]]),
            ];
            assert_eq!(got, [first, second], "delta {delta}");
        }
    }
}
