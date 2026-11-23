//! Linux ARMv7-A A32 backend. Private Mini ABI: 8 bytes per scalar/borrowed
//! argument on an 8-byte aligned stack, r0:r1 for scalar i64 results, optional
//! hidden first (topmost) 8-byte stack slot for aggregate output pointer.
//! This is intentionally NOT the AAPCS32 C calling convention.
use crate::{Ast,Frame,Ty,Res,elf32_arm};
use crate::ir::{Expr,ExprKind,Stmt,Function,Module};
use crate::encode_arm32::{Encoder,Cond};
use crate::target::FloatAbi;
use std::collections::HashMap;

pub(crate) struct Gen<'a> {
    ast:&'a Ast, ir:&'a Module, frames:&'a HashMap<String,Frame>,
    fs:&'a HashMap<String,usize>, enc:Encoder, depth:usize, float_abi:FloatAbi,
}
impl<'a> Gen<'a> {
    pub(crate) fn new(ast:&'a Ast,frames:&'a HashMap<String,Frame>,fs:&'a HashMap<String,usize>,ir:&'a Module,float_abi:FloatAbi)->Self{
        Self{ast,frames,fs,ir,enc:Encoder::new(),depth:0,float_abi}
    }
    fn pos(&self)->usize{self.enc.pos()}
    fn patch(&mut self,at:usize,dest:usize)->Res<()>{self.enc.patch(at,dest)}
    fn trap(&mut self){self.enc.trap()}
    fn trap_unless(&mut self,condition:Cond)->Res<()> {
        let skip=self.enc.bc(condition);self.trap();self.patch(skip,self.pos())
    }
    fn signed(t:&Ty)->bool{matches!(t,Ty::I64|Ty::Int{signed:true,..})}
    // Compare r1:r0 with a bound in r5:r4, preserving the value in r1:r0.
    // For signed bounds the high words compare signed; low words always unsigned.
    fn lower_bound(&mut self,bound:i128,signed:bool)->Res<()> {
        let bits=bound as u64;
        self.enc.imm32(4,bits as u32);self.enc.imm32(5,(bits>>32) as u32);
        self.enc.cmp(1,5);
        let greater=self.enc.bc(if signed{Cond::Gt}else{Cond::Hi});
        self.trap_unless(if signed{Cond::Ge}else{Cond::Cs})?;
        self.enc.cmp(0,4);self.trap_unless(Cond::Cs)?;
        self.patch(greater,self.pos())
    }
    fn upper_bound(&mut self,bound:i128,signed:bool)->Res<()> {
        let bits=bound as u64;
        self.enc.imm32(4,bits as u32);self.enc.imm32(5,(bits>>32) as u32);
        self.enc.cmp(1,5);
        let less=self.enc.bc(if signed{Cond::Lt}else{Cond::Cc});
        self.trap_unless(if signed{Cond::Le}else{Cond::Ls})?;
        self.enc.cmp(0,4);self.trap_unless(Cond::Ls)?;
        self.patch(less,self.pos())
    }
    fn check_integer_range(&mut self,dest:&Ty,source_signed:bool)->Res<()> {
        let Some((lo,mut hi))=crate::integer_bounds(dest) else{return Ok(());};
        if matches!(dest,Ty::Usize){hi=u32::MAX as i128;}
        let target_signed=Self::signed(dest);
        if target_signed {
            if source_signed {
                if lo>i64::MIN as i128 {self.lower_bound(lo,true)?;}
                if hi<i64::MAX as i128 {self.upper_bound(hi,true)?;}
            }else{self.upper_bound(hi,false)?;}
        }else{
            if source_signed {self.lower_bound(0,true)?;}
            if hi<u64::MAX as i128 {self.upper_bound(hi,false)?;}
        }
        Ok(())
    }
    fn push(&mut self){self.enc.push_pair();self.depth+=1;}
    fn pop_r2r3(&mut self){self.enc.pop_pair(2,3);self.depth-=1;}
    fn asm_reg(s:&str)->Res<u8>{match s{
        "r0"=>Ok(0),"r1"=>Ok(1),"r2"=>Ok(2),"r3"=>Ok(3),"r4"=>Ok(4),
        "r5"=>Ok(5),"r6"=>Ok(6),"r7"=>Ok(7),_=>Err(format!("unsupported ARM32 asm input register: {s}")),
    }}
    fn stmts(&mut self,body:&[Stmt],fr:&Frame,ret:&Ty)->Res<()> {
        for st in body {
            match st {
                Stmt::Trap=>self.trap(),
                Stmt::Unsafe(body)=>self.stmts(body,fr,ret)?,
                Stmt::Asm(a)=>{
                    // Stage all expression evaluations first, then restore to
                    // syscall argument registers in reverse order.
                    for (_,e) in &a.inputs {self.scalar(e,fr)?;self.push();}
                    for (reg,_) in a.inputs.iter().rev(){
                        self.enc.pop_low(Self::asm_reg(reg)?);self.depth-=1;
                    }
                    for ins in &a.instructions{self.enc.asm_instruction(ins)?;}
                    for (var,reg) in &a.outputs{
                        if reg!="r0"{return Err("ARM32 asm output must use r0".into());}
                        if Self::signed(&fr.vars[var].1){self.enc.asr31(1,0)}else{self.enc.zero(1)}
                        if matches!(&fr.vars[var].1,Ty::Usize|Ty::Int{..}){self.check_integer_range(&fr.vars[var].1,Self::signed(&fr.vars[var].1))?;}
                        self.enc.local_store(fr.vars[var].0.off);
                    }
                },
                Stmt::Bind{name,ty,value}=>self.emit_to(value,fr.vars[name].0.off,ty,fr)?,
                Stmt::Assign{name,value}=>self.emit_to(value,fr.vars[name].0.off,&fr.vars[name].1,fr)?,
                Stmt::Return{value:expr,cleanup}=>{
                    if let Ty::Struct(name)=ret {
                        match &expr.kind{
                            ExprKind::Call{name:n,arguments:args} if n==name=>{
                                for (i,arg) in args.iter().enumerate(){
                                    self.scalar(arg,fr)?;self.push();
                                    self.enc.local_load(fr.sret.unwrap().off);
                                    self.pop_r2r3();self.enc.store_field_r2r3((i as i32)*8);
                                }
                            },
                            ExprKind::Call{name:n,arguments:args}=>self.call_user(n,args,Some(fr.sret.unwrap().off),true,fr)?,
                            _=>return Err("ARM32 aggregate return requires constructor or call".into()),
                        }
                        self.enc.zero(0);self.enc.zero(1);
                    }else {self.scalar(expr,fr)?;
                        if crate::scalar_integer(ret){self.check_integer_range(ret,Self::signed(&expr.ty))?;}
                    }
                    let scalar=matches!(ret,Ty::I64|Ty::Usize|Ty::Bool|Ty::Int{..});
                    if scalar && !cleanup.is_empty(){self.push();}
                    for destructor in cleanup {self.scalar(destructor,fr)?;}
                    if scalar && !cleanup.is_empty(){self.enc.pop_pair(0,1);self.depth-=1;}
                    if matches!(ret,Ty::Struct(_)){self.enc.zero(0);self.enc.zero(1);}
                    self.enc.frame_end();
                },
                Stmt::Evaluate(expr)=>{
                    if matches!(&expr.ty,Ty::Struct(_)){return Err("unused aggregate".into());}
                    self.scalar(expr,fr)?;
                },
                Stmt::If{condition,yes,no}=>{
                    self.scalar(condition,fr)?;self.enc.zero(2);self.enc.cmp(0,2);
                    let false_at=self.enc.bc(Cond::Eq);
                    self.stmts(yes,fr,ret)?;
                    let end=self.enc.b();self.patch(false_at,self.pos())?;
                    self.stmts(no,fr,ret)?;self.patch(end,self.pos())?;
                },
                Stmt::While{condition,body}=>{
                    let start=self.pos();self.scalar(condition,fr)?;
                    self.enc.zero(2);self.enc.cmp(0,2);
                    let done=self.enc.bc(Cond::Eq);
                    self.stmts(body,fr,ret)?;
                    let back=self.enc.b();self.patch(back,start)?;
                    self.patch(done,self.pos())?;
                },
            }
            if self.depth!=0{return Err("ARM32 internal stack imbalance".into());}
        }
        Ok(())
    }
    fn scalar(&mut self,e:&Expr,fr:&Frame)->Res<()> {
        match &e.kind {
            ExprKind::ConstI64(n)=>self.enc.imm64(*n),
            ExprKind::ConstBool(b)=>{self.enc.imm32(0,u32::from(*b));self.enc.zero(1);},
            ExprKind::Local(n)=>{
                if !matches!(&fr.vars[n].1,Ty::I64|Ty::Usize|Ty::Bool|Ty::Int{..}){return Err("expected scalar local".into());}
                self.enc.local_load(fr.vars[n].0.off);
            },
            ExprKind::Field{owner,field}=>{
                let (slot,ty)=&fr.vars[owner];let Ty::Struct(s)=ty else {return Err("expected struct".into());};
                let idx=self.ast.structs[s].iter().position(|(name,_)|name==field).ok_or("missing field")?;
                if slot.size==0{self.enc.local_load(slot.off);self.enc.load_field((idx as i32)*8);}
                else{self.enc.local_load(slot.off-(idx as i32)*8);}
            },
            ExprKind::Neg(v)=>{
                self.scalar(v,fr)?;self.enc.rsbs_zero(0,0);self.enc.rsc_zero(1,1,true);
                self.trap_unless(Cond::Vc)?;self.check_integer_range(&e.ty,true)?;
            },
            ExprKind::Binary{op,left,right}=>{
                self.scalar(left,fr)?;self.push();
                self.scalar(right,fr)?;self.pop_r2r3();
                if matches!(left.ty,Ty::Int{bits:64,signed:false}) ||
                   matches!(right.ty,Ty::Int{bits:64,signed:false}) {
                    return Err("ARM32 unsigned u64 arithmetic/comparison is not implemented".into());
                }
                let unsigned=crate::scalar_integer(&left.ty)&&!Self::signed(&left.ty);
                match op.as_str() {
                    "+"=>{
                        self.enc.adds(0,2,0);
                        if unsigned {self.trap_unless(Cond::Cc)?;self.enc.zero(1);}
                        else {self.enc.adcs(1,3,1);self.trap_unless(Cond::Vc)?;}
                    },
                    "-"=>{
                        self.enc.subs(0,2,0);
                        if unsigned {self.trap_unless(Cond::Cs)?;self.enc.zero(1);}
                        else {self.enc.sbcs(1,3,1);self.trap_unless(Cond::Vc)?;}
                    },
                    "*"=>{
                        if unsigned{self.mul_u32()?;}else{self.mul_i64()?;}
                    },
                    "/"=>{
                        if unsigned{self.div_u32()?;}else{self.div_i64()?;}
                    },
                    "=="|"!="|"<"|">"|"<="|">="=>self.compare(op,unsigned)?,
                    _=>return Err(format!("unsupported ARM32 IR operator: {op}")),
                }
                if crate::scalar_integer(&e.ty){self.check_integer_range(&e.ty,Self::signed(&left.ty))?;}
            },
            ExprKind::Cast{value,to}=>{self.scalar(value,fr)?;self.check_integer_range(to,Self::signed(&value.ty))?;},
            ExprKind::Call{name,arguments} if name=="raw_addr"=>{
                let ExprKind::Local(n)=&arguments[0].kind else{return Err("raw_addr requires local".into());};
                self.enc.local_addr(fr.vars[n].0.off);
            },
            ExprKind::Call{name,arguments}=>{
                if !matches!(&e.ty,Ty::I64|Ty::Usize|Ty::Bool|Ty::Void|Ty::Int{..}){return Err("struct in scalar expression".into());}
                self.call_user(name,arguments,None,false,fr)?;
            },
        }
        Ok(())
    }
    fn compare(&mut self,op:&str,unsigned:bool)->Res<()> {
        let mut true_jumps=Vec::new();let mut false_jumps=Vec::new();
        if unsigned {
            self.enc.cmp(2,0);
            true_jumps.push(self.enc.bc(match op{
                "=="=>Cond::Eq,"!="=>Cond::Ne,"<"=>Cond::Cc,">"=>Cond::Hi,
                "<="=>Cond::Ls,">="=>Cond::Cs,_=>unreachable!(),
            }));
        } else if op=="=="||op=="!="{
            self.enc.cmp(3,1);
            if op=="=="{false_jumps.push(self.enc.bc(Cond::Ne))}
            else{true_jumps.push(self.enc.bc(Cond::Ne))}
            self.enc.cmp(2,0);
            true_jumps.push(self.enc.bc(if op=="=="{Cond::Eq}else{Cond::Ne}));
        }else{
            self.enc.cmp(3,1);
            let (hi_true,hi_false,lo)=match op{
                "<"=>(Cond::Lt,Cond::Gt,Cond::Cc),
                ">"=>(Cond::Gt,Cond::Lt,Cond::Hi),
                "<="=>(Cond::Lt,Cond::Gt,Cond::Ls),
                ">="=>(Cond::Gt,Cond::Lt,Cond::Cs),
                _=>unreachable!(),
            };
            true_jumps.push(self.enc.bc(hi_true));false_jumps.push(self.enc.bc(hi_false));
            self.enc.cmp(2,0);true_jumps.push(self.enc.bc(lo));
        }
        let false_at=self.pos();self.enc.zero(0);
        let end_jump=self.enc.b();let true_at=self.pos();self.enc.imm32(0,1);
        let end=self.pos();self.enc.zero(1);
        for at in true_jumps{self.patch(at,true_at)?;}
        for at in false_jumps{self.patch(at,false_at)?;}
        self.patch(end_jump,end)?;
        Ok(())
    }
    fn mul_u32(&mut self)->Res<()> {
        self.enc.umull(4,5,2,0);self.enc.zero(6);self.enc.cmp(5,6);
        self.trap_unless(Cond::Eq)?;self.enc.mov(0,4);self.enc.zero(1);Ok(())
    }
    /// Exact signed 64x64->128 using four UMULL products, then check that the
    /// high 64 bits equal the sign extension of the returned low 64 bits.
    fn mul_i64(&mut self)->Res<()> {
        self.enc.umull(4,5,2,0); // p00
        self.enc.umull(6,7,2,1); // p01
        self.enc.umull(8,9,3,0); // p10
        self.enc.umull(10,12,3,1); // p11
        self.enc.adds(5,5,6);self.enc.zero(6);
        self.enc.adcs(7,7,10);self.enc.adc(12,12,6);
        self.enc.adds(5,5,8);self.enc.adcs(7,7,9);self.enc.adc(12,12,6);
        // Convert unsigned 128 product to signed 128 product.
        self.enc.cmp(3,6);let positive_a=self.enc.bc(Cond::Ge);
        self.enc.subs(7,7,0);self.enc.sbc(12,12,1);
        self.patch(positive_a,self.pos())?;
        self.enc.cmp(1,6);let positive_b=self.enc.bc(Cond::Ge);
        self.enc.subs(7,7,2);self.enc.sbc(12,12,3);
        self.patch(positive_b,self.pos())?;
        self.enc.asr31(6,5);self.enc.cmp(7,6);self.trap_unless(Cond::Eq)?;
        self.enc.cmp(12,6);self.trap_unless(Cond::Eq)?;
        self.enc.mov(0,4);self.enc.mov(1,5);Ok(())
    }
    /// Unsigned restoring division, 32 iterations; doesn't require optional
    /// ARMv7 UDIV/SDIV extensions.
    fn div_u32(&mut self)->Res<()> {
        self.enc.zero(5);self.enc.cmp(0,5);
        let valid=self.enc.bc(Cond::Ne);self.trap();self.patch(valid,self.pos())?;
        self.enc.mov(4,2);self.enc.zero(5);self.enc.imm32(6,32);self.enc.imm32(3,1);
        let loop_at=self.pos();
        self.enc.adds(4,4,4);self.enc.adcs(5,5,5);
        let force_sub=self.enc.bc(Cond::Cs);
        self.enc.cmp(5,0);let less=self.enc.bc(Cond::Cc);
        self.patch(force_sub,self.pos())?;
        self.enc.subs(5,5,0);self.enc.orr_imm1(4);
        self.patch(less,self.pos())?;
        self.enc.subs(6,6,3);let again=self.enc.bc(Cond::Ne);self.patch(again,loop_at)?;
        self.enc.mov(0,4);self.enc.zero(1);Ok(())
    }
    /// Signed 64-bit restoring division via unsigned magnitudes. Checks
    /// zero divisor and MIN / -1, which must trap instead of wrapping.
    fn div_i64(&mut self)->Res<()> {
        self.enc.zero(8);self.enc.cmp(0,8);let nonzero_lo=self.enc.bc(Cond::Ne);
        self.enc.cmp(1,8);let nonzero_hi=self.enc.bc(Cond::Ne);
        self.trap();self.patch(nonzero_lo,self.pos())?;self.patch(nonzero_hi,self.pos())?;
        self.enc.mov(4,0);self.enc.mov(5,1);self.enc.zero(12);
        self.enc.cmp(3,8);let a_positive=self.enc.bc(Cond::Ge);
        self.enc.rsbs_zero(2,2);self.enc.rsc_zero(3,3,false);self.enc.eor_imm1(12);
        self.patch(a_positive,self.pos())?;
        self.enc.cmp(5,8);let b_positive=self.enc.bc(Cond::Ge);
        self.enc.rsbs_zero(4,4);self.enc.rsc_zero(5,5,false);self.enc.eor_imm1(12);
        self.patch(b_positive,self.pos())?;
        self.enc.mov(6,2);self.enc.mov(7,3);self.enc.zero(8);self.enc.zero(9);
        self.enc.imm32(10,64);self.enc.imm32(0,1);
        let loop_at=self.pos();
        self.enc.adds(6,6,6);self.enc.adcs(7,7,7);
        self.enc.adcs(8,8,8);self.enc.adcs(9,9,9);
        let force_sub=self.enc.bc(Cond::Cs);
        self.enc.cmp(9,5);let hi_more=self.enc.bc(Cond::Hi);
        let hi_less=self.enc.bc(Cond::Cc);
        self.enc.cmp(8,4);let low_less=self.enc.bc(Cond::Cc);
        self.patch(force_sub,self.pos())?;self.patch(hi_more,self.pos())?;
        self.enc.subs(8,8,4);self.enc.sbc(9,9,5);self.enc.orr_imm1(6);
        let next=self.pos();self.patch(hi_less,next)?;self.patch(low_less,next)?;
        self.enc.subs(10,10,0);let again=self.enc.bc(Cond::Ne);self.patch(again,loop_at)?;
        // Negative magnitude: 2^63 is representable as MIN. Positive: not.
        self.enc.zero(0);self.enc.cmp(12,0);let positive=self.enc.bc(Cond::Eq);
        self.enc.rsbs_zero(6,6);self.enc.rsc_zero(7,7,false);
        let end=self.enc.b();self.patch(positive,self.pos())?;
        self.enc.cmp(7,0);self.trap_unless(Cond::Ge)?;
        self.patch(end,self.pos())?;
        self.enc.mov(0,6);self.enc.mov(1,7);Ok(())
    }
    fn emit_to(&mut self,e:&Expr,dest:i32,ty:&Ty,fr:&Frame)->Res<()> {
        match ty{
            Ty::I64|Ty::Usize|Ty::Bool|Ty::Int{..}=>{self.scalar(e,fr)?;if crate::scalar_integer(ty){self.check_integer_range(ty,Self::signed(&e.ty))?;}self.enc.local_store(dest);},
            Ty::Struct(s)=>match &e.kind{
                ExprKind::Call{name,arguments} if name==s=>{
                    for (i,arg) in arguments.iter().enumerate(){self.scalar(arg,fr)?;self.enc.local_store(dest-(i as i32)*8);}
                },
                ExprKind::Call{name,arguments}=>self.call_user(name,arguments,Some(dest),false,fr)?,
                ExprKind::Local(src)=>return Err(format!("implicit owned struct copying forbidden: {src}")),
                _=>return Err("invalid aggregate expression".into()),
            },
            Ty::PendingNumeric(n)=>return Err(format!("numeric backend for {n} is not implemented")),
            Ty::Void=>return Err("void destination".into()),
        }
        Ok(())
    }
    fn call_user(&mut self,name:&str,args:&[Expr],dest:Option<i32>,forward:bool,fr:&Frame)->Res<()> {
        let callee=&self.ast.funcs[*self.fs.get(name).ok_or_else(||format!("unknown function {name}"))?];
        let returns_struct=matches!(&callee.ret,Ty::Struct(_));
        if returns_struct != dest.is_some(){return Err("aggregate destination/result mismatch".into());}
        for (arg,(_,pt)) in args.iter().zip(&callee.args){
            if matches!(pt,Ty::Struct(_)){
                let ExprKind::Local(n)=&arg.kind else{return Err("borrowed struct argument must be local".into());};
                let slot=fr.vars[n].0;
                if slot.size==0{self.enc.local_load(slot.off)}else{self.enc.local_addr(slot.off)}
            }else{self.scalar(arg,fr)?;}
            self.push();
        }
        if let Some(slot)=dest{
            if forward{self.enc.local_load(slot)}else{self.enc.local_addr(slot)}
            self.push();
        }
        self.enc.call(name);
        let n=args.len()+usize::from(dest.is_some());self.enc.discard_pairs(n)?;self.depth-=n;
        Ok(())
    }
    fn emit_function(&mut self,f:&Function)->Res<()> {
        self.enc.label(&f.name)?;self.depth=0;
        let fr=self.frames[&f.name].clone();self.enc.frame_start(fr.size);
        if let Some(sret)=fr.sret{self.enc.arg_load(8);self.enc.local_store(sret.off);}
        let n=f.params.len();
        for (i,(name,_)) in f.params.iter().enumerate(){
            let off=8+8*(n-1-i+usize::from(fr.sret.is_some())) as i32;
            self.enc.arg_load(off);self.enc.local_store(fr.vars[name].0.off);
        }
        self.stmts(&f.instructions,&fr,&f.result)?;
        if f.result==Ty::Void {
            for destructor in &f.end_cleanup {self.scalar(destructor,&fr)?;}
            self.enc.zero(0);self.enc.zero(1);self.enc.frame_end();
        }
        Ok(())
    }
    pub(crate) fn finish(mut self)->Res<Vec<u8>> {
        // Linux ARM EABI exit(2): r7 = 1, r0 = low 32 bits of result.
        self.enc.call("__main");self.enc.imm32(7,1);self.enc.svc();
        for f in &self.ir.functions.clone(){self.emit_function(f)?;}
        elf32_arm(&self.enc.finish()?,self.float_abi)
    }
}
