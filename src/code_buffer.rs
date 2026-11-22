//! Target-neutral byte buffer, labels, x86-64 rel32 and AArch64 branch relocations.
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct CodeBuffer {
    bytes: Vec<u8>,
    labels: HashMap<String, usize>,
    relocations: Vec<(usize, String)>,
    a64_calls: Vec<(usize, String)>,
    arm32_calls: Vec<(usize, String)>,
}
impl CodeBuffer {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn position(&self) -> usize {
        self.bytes.len()
    }
    pub(crate) fn emit(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub(crate) fn emit_u32(&mut self, value: u32) {
        self.emit(&value.to_le_bytes());
    }
    pub(crate) fn emit_i32(&mut self, value: i32) {
        self.emit(&value.to_le_bytes());
    }
    pub(crate) fn emit_i64(&mut self, value: i64) {
        self.emit(&value.to_le_bytes());
    }
    pub(crate) fn define(&mut self, name: &str) -> Result<(), String> {
        let at = self.position();
        if self.labels.insert(name.to_owned(), at).is_some() {
            return Err(format!("duplicate machine-code label: {name}"));
        }
        Ok(())
    }
    pub(crate) fn rel32_to(&mut self, name: &str) {
        let at = self.position();
        self.emit_i32(0);
        self.relocations.push((at, name.to_owned()));
    }
    pub(crate) fn patch_rel32(&mut self, at: usize, dest: usize) -> Result<(), String> {
        if at.checked_add(4).is_none_or(|end| end > self.bytes.len()) {
            return Err("invalid relocation offset".into());
        }
        let displacement = dest as i128 - at as i128 - 4;
        let displacement =
            i32::try_from(displacement).map_err(|_| "rel32 displacement overflow".to_string())?;
        self.bytes[at..at + 4].copy_from_slice(&displacement.to_le_bytes());
        Ok(())
    }
    /// AArch64 instruction PC-relative displacement. `bits` is 19 (B.cond/CBZ)
    /// or 26 (B/BL); the encoded delta is measured in 4-byte instructions.
    pub(crate) fn patch_a64_branch(
        &mut self,
        at: usize,
        dest: usize,
        bits: u32,
    ) -> Result<(), String> {
        if at % 4 != 0
            || dest % 4 != 0
            || at.checked_add(4).is_none_or(|end| end > self.bytes.len())
        {
            return Err("AArch64 branch address out of bounds or unaligned".into());
        }
        let (shift, mask) = match bits {
            19 => (5, (1u32 << 19) - 1),
            26 => (0, (1u32 << 26) - 1),
            _ => return Err("invalid AArch64 branch width".into()),
        };
        let disp = (dest as i64 - at as i64) / 4;
        let limit = 1i64 << (bits - 1);
        if disp < -limit || disp >= limit {
            return Err(format!("AArch64 branch out of range: {disp}"));
        }
        let prev = u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap());
        let next = (prev & !(mask << shift)) | (((disp as u32) & mask) << shift);
        self.bytes[at..at + 4].copy_from_slice(&next.to_le_bytes());
        Ok(())
    }
    pub(crate) fn a64_call_to(&mut self, name: &str) {
        let at = self.position();
        self.emit_u32(0x9400_0000);
        self.a64_calls.push((at, name.to_string()));
    }
    /// A32 B/BL: signed imm24 word offset from PC (instruction address + 8).
    pub(crate) fn patch_arm32_branch(&mut self, at: usize, dest: usize) -> Result<(), String> {
        if at % 4 != 0
            || dest % 4 != 0
            || at.checked_add(4).is_none_or(|end| end > self.bytes.len())
        {
            return Err("ARM32 branch address out of bounds or unaligned".into());
        }
        let delta = dest as i128 - at as i128 - 8;
        let words = delta / 4;
        if words < -(1 << 23) || words >= (1 << 23) {
            return Err("ARM32 branch out of range".into());
        }
        let mut prior = u32::from_le_bytes(self.bytes[at..at + 4].try_into().unwrap());
        prior = (prior & 0xFF00_0000) | ((words as u32) & 0x00FF_FFFF);
        self.bytes[at..at + 4].copy_from_slice(&prior.to_le_bytes());
        Ok(())
    }
    pub(crate) fn arm32_call_to(&mut self, name: &str) {
        let at = self.position();
        self.emit_u32(0xEB00_0000);
        self.arm32_calls.push((at, name.to_owned()));
    }
    pub(crate) fn finish(mut self) -> Result<Vec<u8>, String> {
        for (at, name) in std::mem::take(&mut self.relocations) {
            let dest = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("undefined machine-code label: {name}"))?;
            self.patch_rel32(at, dest)?;
        }
        for (at, name) in std::mem::take(&mut self.a64_calls) {
            let dest = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("undefined AArch64 machine-code label: {name}"))?;
            self.patch_a64_branch(at, dest, 26)?;
        }
        for (at, name) in std::mem::take(&mut self.arm32_calls) {
            let dest = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("undefined ARM32 machine-code label: {name}"))?;
            self.patch_arm32_branch(at, dest)?;
        }
        Ok(self.bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a64_branch_and_named_call_relocations() {
        let mut b = CodeBuffer::new();
        b.a64_call_to("f");
        b.define("f").unwrap();
        b.emit_u32(0x1400_0000);
        b.patch_a64_branch(4, 0, 26).unwrap();
        let out = b.finish().unwrap();
        assert_eq!(&out[0..4], &0x9400_0001u32.to_le_bytes());
        assert_eq!(&out[4..8], &0x17ff_ffffu32.to_le_bytes());
    }
    #[test]
    fn a64_rejects_unaligned_and_out_of_range() {
        let mut b = CodeBuffer::new();
        b.emit_u32(0x5400_0000);
        assert!(b.patch_a64_branch(0, 2, 19).is_err());
        assert!(b.patch_a64_branch(0, 1 << 22, 19).is_err());
    }
    #[test]
    fn arm32_branches_and_calls() {
        let mut b = CodeBuffer::new();
        b.arm32_call_to("entry");
        b.define("entry").unwrap();
        b.emit_u32(0xEA00_0000);
        b.patch_arm32_branch(4, 0).unwrap();
        let code = b.finish().unwrap();
        assert_eq!(
            u32::from_le_bytes(code[0..4].try_into().unwrap()),
            0xEBFF_FFFF
        );
        assert_eq!(
            u32::from_le_bytes(code[4..8].try_into().unwrap()),
            0xEAFF_FFFD
        );
    }
    #[test]
    fn arm32_rejects_invalid_branches() {
        let mut b = CodeBuffer::new();
        b.emit_u32(0xEA00_0000);
        assert!(b.patch_arm32_branch(0, 2).is_err());
        assert!(b.patch_arm32_branch(0, 1 << 28).is_err());
    }
    #[test]
    fn forward_and_backward_relocations() {
        let mut b = CodeBuffer::new();
        b.emit(&[0xe9]);
        b.rel32_to("end");
        b.define("start").unwrap();
        b.emit(&[0x90]);
        b.define("end").unwrap();
        b.emit(&[0xe9]);
        b.rel32_to("start");
        let out = b.finish().unwrap();
        assert_eq!(&out[1..5], &1i32.to_le_bytes());
        assert_eq!(&out[7..11], &(-6i32).to_le_bytes());
    }
    #[test]
    fn reject_undefined_label() {
        let mut b = CodeBuffer::new();
        b.rel32_to("missing");
        assert!(b.finish().unwrap_err().contains("undefined"));
    }
    #[test]
    fn reject_duplicate_label() {
        let mut b = CodeBuffer::new();
        b.define("x").unwrap();
        assert!(b.define("x").is_err());
    }
}
