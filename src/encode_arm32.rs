//! Little-endian ARMv7-A A32 (ARM-state) encoder. No assembler or linker needed.
//! All emitted instructions use ARM state. Thumb and pre-ARMv7 CPUs are rejected.
use crate::{Res, code_buffer::CodeBuffer};

#[repr(u32)]
#[derive(Clone,Copy,Debug)]
pub(crate) enum Cond { Eq=0, Ne=1, Cs=2, Cc=3, Vc=7, Hi=8, Ls=9, Ge=10, Lt=11, Gt=12, Le=13 }

pub(crate) struct Encoder { code:CodeBuffer }
impl Encoder {
    pub(crate) fn new()->Self {Self{code:CodeBuffer::new()}}
    pub(crate) fn pos(&self)->usize {self.code.position()}
    pub(crate) fn word(&mut self,w:u32){self.code.emit_u32(w)}
    pub(crate) fn label(&mut self,name:&str)->Res<()> {self.code.define(name)}
    pub(crate) fn finish(self)->Res<Vec<u8>> {self.code.finish()}
    pub(crate) fn b(&mut self)->usize {let p=self.pos();self.word(0xEA00_0000);p}
    pub(crate) fn bc(&mut self,c:Cond)->usize {let p=self.pos();self.word(((c as u32)<<28)|0x0A00_0000);p}
    pub(crate) fn patch(&mut self,at:usize,to:usize)->Res<()> {self.code.patch_arm32_branch(at,to)}
    pub(crate) fn call(&mut self,name:&str){self.code.arm32_call_to(name)}
    pub(crate) fn movw(&mut self,rd:u8,n:u16){
        let i=n as u32; self.word(0xE300_0000|((i&0xF000)<<4)|((rd as u32)<<12)|(i&0xFFF));
    }
    pub(crate) fn movt(&mut self,rd:u8,n:u16){
        let i=n as u32; self.word(0xE340_0000|((i&0xF000)<<4)|((rd as u32)<<12)|(i&0xFFF));
    }
    pub(crate) fn imm32(&mut self,rd:u8,n:u32){self.movw(rd,n as u16);self.movt(rd,(n>>16) as u16);}
    pub(crate) fn imm64(&mut self,n:i64){let bits=n as u64;self.imm32(0,bits as u32);self.imm32(1,(bits>>32) as u32);}
    pub(crate) fn zero(&mut self,rd:u8){self.movw(rd,0)}
    // ARM data processing with an unshifted register (operand2 = rm).
    fn dp(&mut self,op:u32,s:bool,rd:u8,rn:u8,rm:u8){
        self.word(0xE000_0000|(op<<21)|((s as u32)<<20)|((rn as u32)<<16)|((rd as u32)<<12)|(rm as u32));
    }
    pub(crate) fn mov(&mut self,d:u8,s:u8){self.dp(13,false,d,0,s)}
    pub(crate) fn cmp(&mut self,a:u8,b:u8){self.dp(10,true,0,a,b)}
    pub(crate) fn adds(&mut self,d:u8,a:u8,b:u8){self.dp(4,true,d,a,b)}
    pub(crate) fn add(&mut self,d:u8,a:u8,b:u8){self.dp(4,false,d,a,b)}
    pub(crate) fn adcs(&mut self,d:u8,a:u8,b:u8){self.dp(5,true,d,a,b)}
    pub(crate) fn adc(&mut self,d:u8,a:u8,b:u8){self.dp(5,false,d,a,b)}
    pub(crate) fn subs(&mut self,d:u8,a:u8,b:u8){self.dp(2,true,d,a,b)}
    pub(crate) fn sub(&mut self,d:u8,a:u8,b:u8){self.dp(2,false,d,a,b)}
    pub(crate) fn sbcs(&mut self,d:u8,a:u8,b:u8){self.dp(6,true,d,a,b)}
    pub(crate) fn sbc(&mut self,d:u8,a:u8,b:u8){self.dp(6,false,d,a,b)}
    pub(crate) fn rsbs_zero(&mut self,rd:u8,rm:u8){
        // RSBS rd, rm, #0 (immediate 0)
        self.word(0xE270_0000|((rm as u32)<<16)|((rd as u32)<<12));
    }
    pub(crate) fn rsc_zero(&mut self,rd:u8,rm:u8,set_flags:bool){
        self.word(0xE2E0_0000|((set_flags as u32)<<20)|((rm as u32)<<16)|((rd as u32)<<12));
    }
    pub(crate) fn orr_imm1(&mut self,d:u8){self.word(0xE380_0001|((d as u32)<<16)|((d as u32)<<12));}
    pub(crate) fn eor_imm1(&mut self,d:u8){self.word(0xE220_0001|((d as u32)<<16)|((d as u32)<<12));}
    pub(crate) fn asr31(&mut self,d:u8,s:u8){self.word(0xE1A0_0000|((d as u32)<<12)|(31<<7)|(2<<5)|(s as u32));}
    pub(crate) fn umull(&mut self,lo:u8,hi:u8,a:u8,b:u8){
        // UMULL rdlo,rdhi,rm,rs: registers must not alias sources.
        self.word(0xE080_0090|((hi as u32)<<16)|((lo as u32)<<12)|((b as u32)<<8)|(a as u32));
    }
    pub(crate) fn svc(&mut self){self.word(0xEF00_0000)}
    pub(crate) fn nop(&mut self){self.word(0xE320_F000)}
    pub(crate) fn dmb_ish(&mut self){self.word(0xF57F_F05B)}
    // LDR/STR 32-bit words at [base,#imm], imm<=4095.
    fn mem(&mut self,load:bool,reg:u8,base:u8,imm:u16) {
        assert!(imm<=4095);
        self.word(0xE580_0000|((load as u32)<<20)|((base as u32)<<16)|((reg as u32)<<12)|(imm as u32));
    }
    pub(crate) fn ldr(&mut self,rd:u8,base:u8,off:u16){self.mem(true,rd,base,off)}
    pub(crate) fn str(&mut self,rs:u8,base:u8,off:u16){self.mem(false,rs,base,off)}
    pub(crate) fn frame_start(&mut self,size:i32){
        self.word(0xE92D_4800); // push {r11,lr}
        self.mov(11,13);        // mov fp,sp
        if size>0 {self.imm32(12,size as u32);self.sub(13,13,12);}
    }
    pub(crate) fn frame_end(&mut self){
        self.mov(13,11);self.word(0xE8BD_8800); // mov sp,fp; pop {r11,pc}
    }
    pub(crate) fn push_pair(&mut self){
        self.word(0xE24D_D008); // sub sp,sp,#8
        self.str(0,13,0);self.str(1,13,4);
    }
    pub(crate) fn pop_pair(&mut self,lo:u8,hi:u8){
        self.ldr(lo,13,0);self.ldr(hi,13,4);self.word(0xE28D_D008);
    }
    pub(crate) fn pop_low(&mut self,reg:u8){self.ldr(reg,13,0);self.word(0xE28D_D008);}
    pub(crate) fn discard_pairs(&mut self,n:usize)->Res<()> {
        let bytes=n.checked_mul(8).and_then(|x|u32::try_from(x).ok()).ok_or("ARM32 stack argument overflow")?;
        if bytes>0 {self.imm32(12,bytes);self.add(13,13,12);}Ok(())
    }
    // r10 and r12 are the dedicated non-preserved address calculation scratch registers.
    pub(crate) fn address(&mut self,base:u8,displacement:i32){
        self.mov(10,base);
        if displacement!=0 {self.imm32(12,displacement.unsigned_abs());
            if displacement<0{self.sub(10,10,12)}else{self.add(10,10,12)}
        }
    }
    pub(crate) fn local_addr(&mut self,off:i32){self.address(11,-off);self.mov(0,10);self.zero(1);}
    pub(crate) fn local_load(&mut self,off:i32){self.address(11,-off);self.ldr(0,10,0);self.ldr(1,10,4);}
    pub(crate) fn local_store(&mut self,off:i32){self.address(11,-off);self.str(0,10,0);self.str(1,10,4);}
    pub(crate) fn arg_load(&mut self,off:i32){self.address(11,off);self.ldr(0,10,0);self.ldr(1,10,4);}
    pub(crate) fn load_field(&mut self,off:i32){self.address(0,off);self.ldr(0,10,0);self.ldr(1,10,4);}
    pub(crate) fn store_field_r2r3(&mut self,off:i32){self.address(0,off);self.str(2,10,0);self.str(3,10,4);}
    /// A32 UDF #0 (permanently undefined); Linux delivers SIGILL. Mini runtime trap.
    pub(crate) fn trap(&mut self){self.word(0xE7F0_00F0);}
    pub(crate) fn asm_instruction(&mut self,name:&str)->Res<()> {
        match name {"svc"=>self.svc(),"nop"=>self.nop(),"dmb"=>self.dmb_ish(), _=>return Err(format!("unsupported ARMv7 asm instruction: {name}"))}Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(e:Encoder)->Vec<u32>{e.finish().unwrap().chunks_exact(4).map(|w|u32::from_le_bytes(w.try_into().unwrap())).collect()}
    #[test] fn canonical_a32_encodings(){
        let mut e=Encoder::new();e.movw(0,42);e.movt(0,0x1234);e.mov(11,13);e.adds(0,2,0);e.adcs(1,3,1);
        e.umull(4,5,2,0);e.rsbs_zero(0,0);e.rsc_zero(1,1,true);e.svc();e.nop();
        assert_eq!(words(e),vec![0xE300_002A,0xE341_0234,0xE1A0_B00D,0xE092_0000,
            0xE0B3_1001,0xE085_4092,0xE270_0000,0xE2F1_1000,0xEF00_0000,0xE320_F000]);
    }
    #[test] fn a32_forward_backward_calls(){
        let mut e=Encoder::new();e.call("f");e.label("f").unwrap();e.call("f");
        assert_eq!(words(e),vec![0xEBFF_FFFF,0xEBFF_FFFE]);
    }
}
