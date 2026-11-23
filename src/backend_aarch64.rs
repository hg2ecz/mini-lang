//! AArch64 Linux native backend for the typed Mini IR.
//! Mini's *private* ABI passes one 64-bit stack slot (16 bytes aligned) per
//! scalar/borrowed argument, with an optional hidden aggregate result pointer.
//! No external C/AAPCS64 function calls are implemented in this backend.
use crate::encode_aarch64::{Cond, Encoder};
use crate::ir::{Expr, ExprKind, Function, Module, Stmt};
use crate::{Ast, Frame, Res, Ty, elf_with_machine};
use std::collections::HashMap;

pub(crate) struct Gen<'a> {
    ast: &'a Ast,
    ir: &'a Module,
    frames: &'a HashMap<String, Frame>,
    fs: &'a HashMap<String, usize>,
    enc: Encoder,
    depth: usize,
}
impl<'a> Gen<'a> {
    pub(crate) fn new(
        ast: &'a Ast,
        frames: &'a HashMap<String, Frame>,
        fs: &'a HashMap<String, usize>,
        ir: &'a Module,
    ) -> Self {
        Self {
            ast,
            frames,
            fs,
            ir,
            enc: Encoder::new(),
            depth: 0,
        }
    }
    fn pos(&self) -> usize {
        self.enc.pos()
    }
    fn patch_b(&mut self, at: usize, to: usize) -> Res<()> {
        self.enc.patch(at, to, 26)
    }
    fn patch_cond(&mut self, at: usize, to: usize) -> Res<()> {
        self.enc.patch(at, to, 19)
    }
    fn trap(&mut self) {
        self.enc.udf();
    }
    fn trap_unless(&mut self, c: Cond) -> Res<()> {
        let next = self.enc.b_cond(c);
        self.trap();
        self.patch_cond(next, self.pos())
    }
    fn signed(t: &Ty) -> bool {
        matches!(t, Ty::I64 | Ty::Int { signed: true, .. })
    }
    fn check_integer_range(&mut self, target: &Ty, source_signed: bool) -> Res<()> {
        let Some((lo, hi)) = crate::integer_bounds(target) else {
            return Ok(());
        };
        let target_signed = Self::signed(target);
        if target_signed {
            if source_signed {
                if lo > i64::MIN as i128 {
                    self.enc.imm(9, lo as i64);
                    self.enc.cmp(0, 9);
                    self.trap_unless(Cond::Ge)?;
                }
                if hi < i64::MAX as i128 {
                    self.enc.imm(9, hi as i64);
                    self.enc.cmp(0, 9);
                    self.trap_unless(Cond::Le)?;
                }
            } else {
                self.enc.imm(9, hi as i64);
                self.enc.cmp(0, 9);
                self.trap_unless(Cond::Ls)?;
            }
        } else {
            if source_signed {
                self.enc.imm(9, 0);
                self.enc.cmp(0, 9);
                self.trap_unless(Cond::Ge)?;
            }
            if hi < u64::MAX as i128 {
                self.enc.imm(9, hi as i64);
                self.enc.cmp(0, 9);
                self.trap_unless(Cond::Ls)?;
            }
        }
        Ok(())
    }
    fn push_x0(&mut self) {
        self.enc.push(0);
        self.depth += 1;
    }
    fn pop_x1(&mut self) {
        self.enc.pop(1);
        self.depth -= 1;
    }
    fn asm_reg(s: &str) -> Res<u8> {
        match s {
            "x0" => Ok(0),
            "x1" => Ok(1),
            "x2" => Ok(2),
            "x3" => Ok(3),
            "x4" => Ok(4),
            "x5" => Ok(5),
            "x8" => Ok(8),
            _ => Err(format!("invalid aarch64 asm input register: {s}")),
        }
    }
    fn stmts(&mut self, body: &[Stmt], fr: &Frame, ret: &Ty) -> Res<()> {
        for st in body {
            match st {
                Stmt::Trap => self.trap(),
                Stmt::Unsafe(body) => self.stmts(body, fr, ret)?,
                Stmt::Asm(a) => {
                    for (_, expr) in &a.inputs {
                        self.scalar(expr, fr)?;
                        self.push_x0();
                    }
                    for (reg, _) in a.inputs.iter().rev() {
                        let n = Self::asm_reg(reg)?;
                        self.enc.pop(n);
                        self.depth -= 1;
                    }
                    for instr in &a.instructions {
                        self.enc.asm_instruction(instr)?;
                    }
                    for (var, reg) in &a.outputs {
                        if reg != "x0" {
                            return Err("aarch64 asm output must use x0".into());
                        }
                        if matches!(&fr.vars[var].1, Ty::Usize | Ty::Int { .. }) {
                            self.check_integer_range(
                                &fr.vars[var].1,
                                Self::signed(&fr.vars[var].1),
                            )?;
                        }
                        self.enc.local_store(fr.vars[var].0.off);
                    }
                }
                Stmt::Bind { name, ty, value } => {
                    self.emit_to(value, fr.vars[name].0.off, ty, fr)?
                }
                Stmt::Assign { name, value } => {
                    self.emit_to(value, fr.vars[name].0.off, &fr.vars[name].1, fr)?
                }
                Stmt::Return {
                    value: expr,
                    cleanup,
                } => {
                    if let Ty::Struct(name) = ret {
                        match &expr.kind {
                            ExprKind::Call {
                                name: n,
                                arguments: args,
                            } if n == name => {
                                for (i, arg) in args.iter().enumerate() {
                                    self.scalar(arg, fr)?;
                                    self.push_x0();
                                    self.enc.local_load(fr.sret.unwrap().off);
                                    self.pop_x1();
                                    self.enc.store_field_from_x1((i as i32) * 8);
                                }
                            }
                            ExprKind::Call {
                                name: n,
                                arguments: args,
                            } => self.call_user(n, args, Some(fr.sret.unwrap().off), true, fr)?,
                            _ => return Err("unsupported struct return".into()),
                        }
                        self.enc.imm(0, 0);
                    } else {
                        self.scalar(expr, fr)?;
                        if crate::scalar_integer(ret) {
                            self.check_integer_range(ret, Self::signed(&expr.ty))?;
                        }
                    }
                    let scalar = matches!(ret, Ty::I64 | Ty::Usize | Ty::Bool | Ty::Int { .. });
                    if scalar && !cleanup.is_empty() {
                        self.push_x0();
                    }
                    for destructor in cleanup {
                        self.scalar(destructor, fr)?;
                    }
                    if scalar && !cleanup.is_empty() {
                        self.enc.pop(0);
                        self.depth -= 1;
                    }
                    if matches!(ret, Ty::Struct(_)) {
                        self.enc.imm(0, 0);
                    }
                    self.enc.frame_end();
                }
                Stmt::Evaluate(expr) => {
                    if matches!(&expr.ty, Ty::Struct(_)) {
                        return Err("unused aggregate".into());
                    }
                    self.scalar(expr, fr)?;
                }
                Stmt::If { condition, yes, no } => {
                    self.scalar(condition, fr)?;
                    let false_at = self.enc.cbz(0);
                    self.stmts(yes, fr, ret)?;
                    let end = self.enc.b();
                    self.patch_cond(false_at, self.pos())?;
                    self.stmts(no, fr, ret)?;
                    self.patch_b(end, self.pos())?;
                }
                Stmt::While { condition, body } => {
                    let top = self.pos();
                    self.scalar(condition, fr)?;
                    let done = self.enc.cbz(0);
                    self.stmts(body, fr, ret)?;
                    let back = self.enc.b();
                    self.patch_b(back, top)?;
                    self.patch_cond(done, self.pos())?;
                }
            }
            if self.depth != 0 {
                return Err("AArch64 internal stack imbalance".into());
            }
        }
        Ok(())
    }
    fn scalar(&mut self, expr: &Expr, fr: &Frame) -> Res<()> {
        match &expr.kind {
            ExprKind::ConstI64(n) => self.enc.imm(0, *n),
            ExprKind::ConstBool(b) => self.enc.imm(0, i64::from(*b)),
            ExprKind::Local(name) => {
                let (slot, ty) = &fr.vars[name];
                if !matches!(ty, Ty::I64 | Ty::Usize | Ty::Bool | Ty::Int { .. }) {
                    return Err("expected scalar local".into());
                }
                self.enc.local_load(slot.off);
            }
            ExprKind::Field { owner, field } => {
                let (slot, ty) = &fr.vars[owner];
                let Ty::Struct(s) = ty else {
                    return Err("expected struct".into());
                };
                let idx = self.ast.structs[s]
                    .iter()
                    .position(|(n, _)| n == field)
                    .ok_or("missing field")?;
                if slot.size == 0 {
                    self.enc.local_load(slot.off);
                    self.enc.load_field((idx as i32) * 8);
                } else {
                    self.enc.local_load(slot.off - (idx as i32) * 8);
                }
            }
            ExprKind::Neg(value) => {
                self.scalar(value, fr)?;
                self.enc.negs();
                self.trap_unless(Cond::Vc)?;
                self.check_integer_range(&expr.ty, true)?;
            }
            ExprKind::Binary { op, left, right } => {
                self.scalar(left, fr)?;
                self.push_x0();
                self.scalar(right, fr)?;
                self.pop_x1();
                let unsigned = crate::scalar_integer(&left.ty) && !Self::signed(&left.ty);
                match op.as_str() {
                    "+" => {
                        self.enc.adds();
                        self.trap_unless(if unsigned { Cond::Cc } else { Cond::Vc })?;
                    }
                    "-" => {
                        self.enc.subs_x1_x0();
                        self.trap_unless(if unsigned { Cond::Cs } else { Cond::Vc })?;
                    }
                    "*" => {
                        if unsigned {
                            self.enc.umulh();
                            self.enc.mul();
                            self.enc.cmp(2, 31);
                            self.trap_unless(Cond::Eq)?;
                        } else {
                            self.enc.smulh();
                            self.enc.mul();
                            self.enc.asr63();
                            self.enc.cmp(2, 3);
                            self.trap_unless(Cond::Eq)?;
                        }
                    }
                    "/" => {
                        let notzero = self.enc.cbnz(0);
                        self.trap();
                        self.patch_cond(notzero, self.pos())?;
                        if unsigned {
                            self.enc.udiv();
                        } else {
                            self.enc.imm(2, -1);
                            self.enc.cmp(0, 2);
                            let notnegone = self.enc.b_cond(Cond::Ne);
                            self.enc.imm(2, i64::MIN);
                            self.enc.cmp(1, 2);
                            let notmin = self.enc.b_cond(Cond::Ne);
                            self.trap();
                            let after = self.pos();
                            self.patch_cond(notnegone, after)?;
                            self.patch_cond(notmin, after)?;
                            self.enc.sdiv();
                        }
                    }
                    "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                        self.enc.cmp(1, 0);
                        let cond = match op.as_str() {
                            "==" => Cond::Eq,
                            "!=" => Cond::Ne,
                            "<" if unsigned => Cond::Cc,
                            ">" if unsigned => Cond::Hi,
                            "<=" if unsigned => Cond::Ls,
                            ">=" if unsigned => Cond::Cs,
                            "<" => Cond::Lt,
                            ">" => Cond::Gt,
                            "<=" => Cond::Le,
                            ">=" => Cond::Ge,
                            _ => unreachable!(),
                        };
                        self.enc.cset(cond);
                    }
                    _ => return Err(format!("unsupported operator: {op}")),
                }
                if crate::scalar_integer(&expr.ty) {
                    self.check_integer_range(&expr.ty, Self::signed(&left.ty))?;
                }
            }
            ExprKind::Cast { value, to } => {
                self.scalar(value, fr)?;
                self.check_integer_range(to, Self::signed(&value.ty))?;
            }
            ExprKind::Call { name, arguments } if name == "raw_addr" => {
                let ExprKind::Local(name) = &arguments[0].kind else {
                    return Err("raw_addr needs local".into());
                };
                self.enc.local_addr(fr.vars[name].0.off);
            }
            ExprKind::Call { name, arguments } => {
                if !matches!(
                    expr.ty,
                    Ty::I64 | Ty::Usize | Ty::Bool | Ty::Void | Ty::Int { .. }
                ) {
                    return Err("struct in scalar expression".into());
                }
                self.call_user(name, arguments, None, false, fr)?;
            }
        }
        Ok(())
    }
    fn emit_to(&mut self, expr: &Expr, dest: i32, ty: &Ty, fr: &Frame) -> Res<()> {
        match ty {
            Ty::I64 | Ty::Usize | Ty::Bool | Ty::Int { .. } => {
                self.scalar(expr, fr)?;
                if crate::scalar_integer(ty) {
                    self.check_integer_range(ty, Self::signed(&expr.ty))?;
                }
                self.enc.local_store(dest);
            }
            Ty::Struct(s) => match &expr.kind {
                ExprKind::Call { name, arguments } if name == s => {
                    if self.ast.structs[s].len() != arguments.len() {
                        return Err("constructor arity".into());
                    }
                    for (i, arg) in arguments.iter().enumerate() {
                        self.scalar(arg, fr)?;
                        self.enc.local_store(dest - (i as i32) * 8);
                    }
                }
                ExprKind::Call { name, arguments } => {
                    self.call_user(name, arguments, Some(dest), false, fr)?
                }
                ExprKind::Local(src) => {
                    return Err(format!("implicit owned struct copying forbidden: {src}"));
                }
                _ => return Err("invalid aggregate expression".into()),
            },
            Ty::PendingNumeric(n) => {
                return Err(format!("numeric backend for {n} is not implemented"));
            }
            Ty::Void => return Err("void destination".into()),
        }
        Ok(())
    }
    fn call_user(
        &mut self,
        name: &str,
        args: &[Expr],
        result_dest: Option<i32>,
        forward: bool,
        fr: &Frame,
    ) -> Res<()> {
        let callee = &self.ast.funcs[*self
            .fs
            .get(name)
            .ok_or_else(|| format!("unknown function {name}"))?];
        let returns_struct = matches!(&callee.ret, Ty::Struct(_));
        let param_types: Vec<Ty> = callee.args.iter().map(|(_, t)| t.clone()).collect();
        if returns_struct != result_dest.is_some() {
            return Err("aggregate destination / result mismatch".into());
        }
        for (i, arg) in args.iter().enumerate() {
            if matches!(&param_types[i], Ty::Struct(_)) {
                let ExprKind::Local(v) = &arg.kind else {
                    return Err("borrowed struct parameter must be a local".into());
                };
                let (slot, _) = &fr.vars[v];
                if slot.size == 0 {
                    self.enc.local_load(slot.off)
                } else {
                    self.enc.local_addr(slot.off)
                }
            } else {
                self.scalar(arg, fr)?;
            }
            self.push_x0();
        }
        if let Some(slot) = result_dest {
            if forward {
                self.enc.local_load(slot);
            } else {
                self.enc.local_addr(slot);
            }
            self.push_x0();
        }
        self.enc.call(name);
        self.enc
            .pop_args(args.len() + usize::from(result_dest.is_some()))?;
        self.depth -= args.len() + usize::from(result_dest.is_some());
        Ok(())
    }
    fn emit_function(&mut self, f: &Function) -> Res<()> {
        self.enc.label(&f.name)?;
        self.depth = 0;
        let fr = self.frames[&f.name].clone();
        self.enc.frame_start(fr.size);
        if let Some(sret) = fr.sret {
            self.enc.arg_load(16);
            self.enc.local_store(sret.off);
        }
        let n = f.params.len();
        for (i, (name, _)) in f.params.iter().enumerate() {
            let off = 16 + 16 * (n - 1 - i + usize::from(fr.sret.is_some())) as i32;
            self.enc.arg_load(off);
            self.enc.local_store(fr.vars[name].0.off);
        }
        self.stmts(&f.instructions, &fr, &f.result)?;
        if f.result == Ty::Void {
            for destructor in &f.end_cleanup {
                self.scalar(destructor, &fr)?;
            }
            self.enc.imm(0, 0);
            self.enc.frame_end();
        }
        Ok(())
    }
    pub(crate) fn finish(mut self) -> Res<Vec<u8>> {
        // _start: no runtime and no external linker. Linux AArch64 exit(2) is #93.
        self.enc.call("__main");
        self.enc.imm(8, 93);
        self.enc.svc();
        for f in self.ir.functions.clone().iter() {
            self.emit_function(f)?;
        }
        Ok(elf_with_machine(&self.enc.finish()?, 183)) // EM_AARCH64
    }
}
