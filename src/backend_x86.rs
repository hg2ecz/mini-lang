//! x86-64 IR lowering. Machine instruction bytes live only in encode_x86_64.rs.
use crate::{Ast, Frame, Ty, Res, elf};
use crate::ir::{Expr, ExprKind, Stmt, Function, Module};
use crate::encode_x86_64::{X86Encoder,Condition};
use std::collections::HashMap;

pub(crate) struct Gen<'a> {
    ast:&'a Ast,
    ir:&'a Module,
    frames:&'a HashMap<String,Frame>,
    fs:&'a HashMap<String,usize>,
    enc:X86Encoder,
    depth:usize,
}
impl<'a> Gen<'a> {
    pub(crate) fn new(ast:&'a Ast, frames:&'a HashMap<String,Frame>,fs:&'a HashMap<String,usize>,ir:&'a Module)->Self {
        Self{ast,ir,frames,fs,enc:X86Encoder::new(),depth:0}
    }
    fn pos(&self)->usize { self.enc.pos() }
    fn patch(&mut self,at:usize,dest:usize)->Res<()> { self.enc.patch_rel32(at,dest) }
    fn trap(&mut self) { self.enc.ud2(); }
    fn trap_unless(&mut self, condition:Condition)->Res<()> {
        let skip=self.enc.branch(condition);self.trap();self.patch(skip,self.pos())
    }
    // All fixed-width scalars occupy an internal eight-byte Mini ABI slot.
    // The value is *not* narrowed silently: reject values outside the type.
    fn check_integer_range(&mut self, ty:&Ty, source_signed:bool)->Res<()> {
        let Some((lo,hi))=crate::integer_bounds(ty) else{return Ok(());};
        let target_signed=matches!(ty,Ty::I64|Ty::Int{signed:true,..});
        if target_signed {
            if source_signed {
                if lo>i64::MIN as i128 {
                    self.enc.mov_r9_imm64(lo as i64);
                    self.enc.compare_rax_r9();
                    self.trap_unless(Condition::GreaterEqual)?;
                }
                if hi<i64::MAX as i128 {
                    self.enc.mov_r9_imm64(hi as i64);
                    self.enc.compare_rax_r9();
                    self.trap_unless(Condition::LessEqual)?;
                }
            } else {
                self.enc.mov_r9_imm64(hi as i64);
                self.enc.compare_rax_r9();
                self.trap_unless(Condition::BelowEqual)?;
            }
        } else {
            if source_signed {
                self.enc.mov_r9_imm64(0);
                self.enc.compare_rax_r9();
                self.trap_unless(Condition::GreaterEqual)?;
            }
            if hi<u64::MAX as i128 {
                self.enc.mov_r9_imm64(hi as i64);
                self.enc.compare_rax_r9();
                self.trap_unless(Condition::BelowEqual)?;
            }
        }
        Ok(())
    }
    fn is_signed(ty:&Ty)->bool {matches!(ty,Ty::I64|Ty::Int{signed:true,..})}
    fn push_rax(&mut self) { self.enc.push_rax();self.depth+=1; }
    fn pop_rcx(&mut self) { self.enc.pop_rcx();self.depth-=1; }
    fn emit_stmts(&mut self,body:&[Stmt],fr:&Frame,ret:&Ty)->Res<()> {
        for st in body {
            match st {
                Stmt::Trap=>self.trap(),
                Stmt::Unsafe(body)=>self.emit_stmts(body,fr,ret)?,
                Stmt::Asm(a)=>{
                    // Delay register assignments until all input expressions are evaluated.
                    for (_,e) in &a.inputs {self.expr_scalar(e,fr)?;self.push_rax();}
                    for (reg,_) in a.inputs.iter().rev() {
                        self.enc.pop_named(reg)?;self.depth-=1;
                    }
                    for op in &a.instructions {self.enc.asm_instruction(op)?;}
                    for (name,reg) in &a.outputs {
                        if reg!="rax" {return Err("asm output register must be rax".into());}
                        self.enc.local_store(fr.vars[name].0.off);
                    }
                },
                Stmt::Bind{name,ty,value}=>self.emit_to(value,fr.vars[name].0.off,ty,fr)?,
                Stmt::Assign{name,value}=>self.emit_to(value,fr.vars[name].0.off,&fr.vars[name].1,fr)?,
                Stmt::Return{value:e,cleanup}=>{
                    if let Ty::Struct(name)=ret {
                        match &e.kind {
                            ExprKind::Call{name:n,arguments:args} if n==name=>{
                                for (idx,arg) in args.iter().enumerate() {
                                    self.expr_scalar(arg,fr)?;
                                    self.push_rax();
                                    self.enc.local_load(fr.sret.unwrap().off);
                                    self.pop_rcx();
                                    self.enc.store_rcx_through_rax((idx*8) as i32);
                                }
                            }
                            ExprKind::Call{name:n,arguments:args}=>
                                self.call_user_forward(n,args,fr.sret.unwrap().off,fr)?,
                            _=>return Err("unsupported aggregate return".into()),
                        }
                        self.enc.mov_rax_imm64(0);
                    } else {self.expr_scalar(e,fr)?;
                        if crate::scalar_integer(ret) {self.check_integer_range(ret,Self::is_signed(&e.ty))?;}
                    }
                    // Evaluate the return value BEFORE calling destructors.
                    // Preserve rax through ordinary Mini function calls.
                    let scalar=matches!(ret,Ty::I64|Ty::Usize|Ty::Bool|Ty::Int{..});
                    if scalar && !cleanup.is_empty() {self.push_rax();}
                    for destructor in cleanup {self.expr_scalar(destructor,fr)?;}
                    if scalar && !cleanup.is_empty() {self.enc.pop_rax();self.depth-=1;}
                    if matches!(ret,Ty::Struct(_)){self.enc.mov_rax_imm64(0);}
                    self.enc.leave_frame();
                },
                Stmt::Evaluate(e)=>{
                    if matches!(&e.ty,Ty::Struct(_)) {return Err("unused aggregate".into());}
                    self.expr_scalar(e,fr)?;
                },
                Stmt::If {condition,yes,no}=>{
                    self.expr_scalar(condition,fr)?;
                    let false_jump=self.enc.branch_if_rax_zero();
                    self.emit_stmts(yes,fr,ret)?;
                    let end_jump=self.enc.jump();
                    self.patch(false_jump,self.pos())?;
                    self.emit_stmts(no,fr,ret)?;
                    self.patch(end_jump,self.pos())?;
                },
                Stmt::While {condition,body}=>{
                    let top=self.pos();
                    self.expr_scalar(condition,fr)?;
                    let done=self.enc.branch_if_rax_zero();
                    self.emit_stmts(body,fr,ret)?;
                    let back=self.enc.jump();self.patch(back,top)?;
                    self.patch(done,self.pos())?;
                },
            }
            if self.depth!=0 {return Err("internal stack imbalance".into());}
        }
        Ok(())
    }
    fn expr_scalar(&mut self,e:&Expr,fr:&Frame)->Res<()> {
        match &e.kind {
            ExprKind::ConstI64(n)=>self.enc.mov_rax_imm64(*n),
            ExprKind::ConstBool(v)=>self.enc.mov_rax_imm64(i64::from(*v)),
            ExprKind::Local(n)=>{
                let (slot,ty)=fr.vars.get(n).ok_or_else(||format!("unknown var {n}"))?;
                if !matches!(ty,Ty::I64|Ty::Usize|Ty::Bool|Ty::Int{..}){return Err("expected scalar variable".into());}
                self.enc.local_load(slot.off);
            },
            ExprKind::Field{owner,field}=>{
                let (slot,ty)=&fr.vars[owner];
                let Ty::Struct(s)=ty else{return Err("not struct".into());};
                let index=self.ast.structs[s].iter().position(|(name,_)|name==field)
                    .ok_or_else(||"missing field".to_string())?;
                if slot.size==0 { // borrowed struct: slot holds address
                    self.enc.local_load(slot.off);
                    self.enc.load_field_through_rax((index*8) as i32);
                } else {
                    self.enc.local_load(slot.off-(index as i32)*8);
                }
            },
            ExprKind::Neg(a)=>{
                self.expr_scalar(a,fr)?;
                self.enc.neg_rax();
                self.trap_unless(Condition::NoOverflow)?;
                self.check_integer_range(&e.ty,true)?;
            },
            ExprKind::Cast {value,to}=>{
                self.expr_scalar(value,fr)?;
                self.check_integer_range(to,Self::is_signed(&value.ty))?;
            },
            ExprKind::Binary{left,op,right}=>{
                self.expr_scalar(left,fr)?;self.push_rax();
                self.expr_scalar(right,fr)?;self.pop_rcx();
                let arithmetic_ty=if e.ty==Ty::Bool {
                    if left.ty==Ty::I64 && right.ty!=Ty::I64 {&right.ty}else{&left.ty}
                } else {&e.ty};
                let unsigned=!Self::is_signed(arithmetic_ty);
                match op.as_str() {
                    "+"=>{
                        self.enc.add_rax_rcx();
                        self.trap_unless(if unsigned{Condition::NoCarry}else{Condition::NoOverflow})?;
                    },
                    "-"=>{
                        self.enc.sub_rcx_rax_to_rax();
                        self.trap_unless(if unsigned{Condition::NoCarry}else{Condition::NoOverflow})?;
                    },
                    "*"=>{
                        if unsigned {
                            self.enc.multiply_unsigned_rax_rcx();
                            self.enc.test_rdx();
                            self.trap_unless(Condition::Zero)?;
                        }else{
                            self.enc.multiply_signed_rax_rcx();
                            self.trap_unless(Condition::NoOverflow)?;
                        }
                    },
                    "/"=>{
                        self.enc.test_rax();
                        let not_zero=self.enc.branch(Condition::NotZero);
                        self.trap();self.patch(not_zero,self.pos())?;
                        if unsigned {
                            self.enc.divide_unsigned_rcx_by_rax();
                        } else {
                            self.enc.compare_rax_minus_one();
                            let not_minus_one=self.enc.branch(Condition::NotZero);
                            self.enc.mov_r9_imm64(i64::MIN);
                            self.enc.compare_rcx_r9();
                            let overflow=self.enc.branch(Condition::Zero);
                            let divide_here=self.pos();self.patch(not_minus_one,divide_here)?;
                            self.enc.divide_signed_rcx_by_rax();
                            let skip=self.enc.jump();
                            self.patch(overflow,self.pos())?;
                            self.trap();self.patch(skip,self.pos())?;
                        }
                    },
                    "=="|"!="|"<"|">"|"<="|">="=>{
                        self.enc.compare_rcx_rax();
                        self.enc.set_comparison(op,unsigned)?;
                    },
                    _=>return Err("unknown operator".into()),
                }
                if !["==","!=","<",">","<=",">="].contains(&op.as_str()) {
                    self.check_integer_range(&e.ty,!unsigned)?;
                }
            },
            ExprKind::Call{name,arguments} if name=="raw_addr"=>{
                let ExprKind::Local(v)=&arguments[0].kind else{return Err("raw_addr requires variable".into());};
                self.enc.local_addr(fr.vars[v].0.off);
            },
            ExprKind::Call{name,arguments}=>{
                if !matches!(e.ty,Ty::I64|Ty::Usize|Ty::Bool|Ty::Int{..}|Ty::Void){return Err("aggregate in scalar position".into());}
                self.call_user(name,arguments,None,fr)?;
            },
        }
        Ok(())
    }
    fn emit_to(&mut self,e:&Expr,dest:i32,ty:&Ty,fr:&Frame)->Res<()> {
        match ty {
            Ty::I64|Ty::Usize|Ty::Bool|Ty::Int{..}=>{
                self.expr_scalar(e,fr)?;
                if crate::scalar_integer(ty){self.check_integer_range(ty,Self::is_signed(&e.ty))?;}
                self.enc.local_store(dest);
            },
            Ty::Struct(name)=>match &e.kind {
                ExprKind::Call{name:n,arguments:args} if n==name=>{
                    let field_count=self.ast.structs[name].len();
                    if field_count!=args.len(){return Err("constructor arity".into());}
                    for (i,arg) in args.iter().enumerate() {
                        self.expr_scalar(arg,fr)?;
                        self.enc.local_store(dest-(i as i32)*8);
                    }
                },
                ExprKind::Call{name:n,arguments:args}=>self.call_user(n,args,Some(dest),fr)?,
                ExprKind::Local(src)=>return Err(format!("implicit owned aggregate copy forbidden: {src}")),
                _=>return Err("invalid aggregate expression".into()),
            },
            Ty::PendingNumeric(n)=>return Err(format!("numeric backend for {n} is not implemented")),
            Ty::Void=>return Err("void assignment".into()),
        }
        Ok(())
    }
    fn push_call_args(&mut self,n:&str,args:&[Expr],fr:&Frame)->Res<()> {
        for (idx,arg) in args.iter().enumerate() {
            let param_ty=&self.ast.funcs[self.fs[n]].args[idx].1;
            if matches!(param_ty,Ty::Struct(_)){
                match &arg.kind {
                    ExprKind::Local(v)=>{
                        let (slot,_)=&fr.vars[v];
                        if slot.size==0 {self.enc.local_load(slot.off)}
                        else{self.enc.local_addr(slot.off)}
                    },
                    _=>return Err("aggregate argument must be a named variable (borrowed)".into()),
                }
            }else{self.expr_scalar(arg,fr)?;}
            self.push_rax();
        }
        Ok(())
    }
    fn begin_call(&mut self,words:usize)->usize {
        // Local stack is 16-aligned at function entry after frame setup.
        let pad=(self.depth+words)%2;
        if pad!=0 {self.enc.reserve_pad_word();self.depth+=1;}
        pad
    }
    fn end_call(&mut self,words:usize,pad:usize)->Res<()> {
        if words+pad>0 {self.enc.release_call_words(words+pad)?;self.depth-=words+pad;}
        Ok(())
    }
    fn call_user(&mut self,n:&str,args:&[Expr],dest:Option<i32>,fr:&Frame)->Res<()> {
        let ret_ty=self.ast.funcs[*self.fs.get(n).ok_or_else(||format!("unknown function {n}"))?].ret.clone();
        let words=args.len()+usize::from(dest.is_some());
        let pad=self.begin_call(words);
        self.push_call_args(n,args,fr)?;
        if let Some(slot)=dest {self.enc.local_addr(slot);self.push_rax();}
        self.enc.call_named(n);
        self.end_call(words,pad)?;
        if dest.is_none()&&matches!(ret_ty,Ty::Struct(_)) {return Err("struct call requires destination".into());}
        Ok(())
    }
    fn call_user_forward(&mut self,n:&str,args:&[Expr],sret_off:i32,fr:&Frame)->Res<()> {
        let words=args.len()+1;
        let pad=self.begin_call(words);
        self.push_call_args(n,args,fr)?;
        self.enc.local_load(sret_off);self.push_rax();
        self.enc.call_named(n);
        self.end_call(words,pad)
    }
    fn emit_function(&mut self,f:&Function)->Res<()> {
        self.enc.label(&f.name)?;self.depth=0;
        let fr=self.frames[&f.name].clone();
        self.enc.enter_frame(fr.size);
        if let Some(slot)=fr.sret {
            self.enc.load_argument_at_rbp_offset(16);
            self.enc.local_store(slot.off);
        }
        let n=f.params.len();
        for (i,(name,_)) in f.params.iter().enumerate() {
            let offset=16+8*(n-1-i+usize::from(fr.sret.is_some())) as i32;
            self.enc.load_argument_at_rbp_offset(offset);
            self.enc.local_store(fr.vars[name].0.off);
        }
        self.emit_stmts(&f.instructions,&fr,&f.result)?;
        if f.result==Ty::Void {
            for destructor in &f.end_cleanup {self.expr_scalar(destructor,&fr)?;}
            self.enc.mov_rax_imm64(0);self.enc.leave_frame();
        }
        Ok(())
    }
    pub(crate) fn finish(mut self)->Res<Vec<u8>> {
        self.enc.call_named("__main");
        self.enc.mov_rdi_rax();self.enc.mov_rax_imm64(60);self.enc.syscall();
        for f in &self.ir.functions.clone() {self.emit_function(f)?;}
        Ok(elf(&self.enc.finish()?))
    }
}
