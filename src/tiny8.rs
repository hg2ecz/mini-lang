//! Minimal 16-bit Mini ABI for two independent 8-bit machines.
//! No language/IR changes. Static per-function slots are safe ONLY because the
//! complete reachable direct-call graph is checked to be acyclic; no interrupts,
//! callbacks, reentrancy or indirect calls may invoke Mini functions.
//! Unsupported types/ops fail closed before machine code is emitted.
use crate::device_config::DeviceLayout;
use crate::ir::{AsmIr, Expr, ExprKind, Function, Module, Stmt};
use crate::target::{Arch, Target};
use crate::{AsmBlock, Ast, Ex, Res, Ty, referenced_vars, type_expr, validate_unsafe_expr};
use std::collections::{HashMap, HashSet};

pub(crate) fn check_asm(
    a: &AsmBlock,
    vars: &HashMap<String, Ty>,
    dead: &HashSet<String>,
    ast: &Ast,
    fs: &HashMap<String, usize>,
    target: Target,
) -> Res<()> {
    if a.instructions.is_empty() || a.instructions.len() > 64 {
        return Err("8-bit asm requires 1..64 instructions".into());
    }
    if !a.outputs.is_empty() {
        return Err(
            "8-bit asm outputs not supported: use a Mini function return only for normal code"
                .into(),
        );
    }
    if a.inputs.len() > 1 {
        return Err("8-bit asm currently accepts at most one input register".into());
    }
    if a.clobbers.len() > 1 || a.clobbers.iter().any(|c| c != "memory") {
        return Err("8-bit asm accepts only clobber memory".into());
    }
    let avr = target.arch == Arch::Avr8;
    let mut bound = None::<String>;
    for (reg, value) in &a.inputs {
        if avr {
            let num = reg.strip_prefix('r').and_then(|s| s.parse::<u8>().ok());
            if !matches!(num, Some(16..=31)) {
                return Err(format!("AVR asm input register must be r16..r31: {reg}"));
            }
        } else if reg != "a" {
            return Err("C64 asm input must be accumulator a".into());
        }
        let ty = type_expr(value, vars, ast, fs)?;
        if !matches!(ty, Ty::I64 | Ty::Usize) {
            return Err("8-bit asm input must be integer".into());
        }
        validate_unsafe_expr(value, true, ast, fs)?;
        for v in referenced_vars(value) {
            if dead.contains(&v) {
                return Err(format!("use after drop: {v}"));
            }
        }
        match value {
            Ex::Num(n) if (0..=255).contains(n) => {}
            _ if ty == Ty::Usize && (!avr || reg == "r16") => {}
            _ => return Err(
                "dynamic 8-bit asm requires usize and AVR register r16; i64 narrowing forbidden"
                    .into(),
            ),
        }
        bound = Some(reg.clone());
    }
    let mut memory = false;
    for instruction in &a.instructions {
        match (avr, instruction.as_str()) {
            (_, "nop") => {}
            (_, "sei" | "cli") => memory = true,
            (false, s) if s.starts_with("sta:") => {
                if bound.as_deref() != Some("a") {
                    return Err("C64 sta requires in a = value".into());
                }
                s[4..]
                    .parse::<u16>()
                    .map_err(|_| "invalid C64 absolute address")?;
                memory = true;
            }
            (true, s) if s.starts_with("outio:") || s.starts_with("sts:") => {
                let parts: Vec<_> = s.split(':').collect();
                if parts.len() != 3 {
                    return Err("malformed AVR store".into());
                }
                let addr = parts[1].parse::<u16>().map_err(|_| "invalid AVR address")?;
                if parts[0] == "outio" && addr > 63 {
                    return Err("AVR OUT requires I/O address 0..63".into());
                }
                if parts[0] == "sts"
                    && u32::from(addr) >= target.layout.ok_or("AVR device map missing")?.ram.end
                {
                    return Err("AVR STS outside configured data space".into());
                }
                if bound.as_deref() != Some(parts[2]) {
                    return Err("AVR store must use the bound input register".into());
                }
                memory = true;
            }
            _ => {
                return Err(format!(
                    "unsupported 8-bit assembly instruction {instruction}"
                ));
            }
        }
    }
    if memory && !a.clobbers.iter().any(|x| x == "memory") {
        return Err("I/O, memory stores and interrupt state need clobber memory".into());
    }
    Ok(())
}

fn check_expr(e: &Expr, fs: &HashMap<&str, &Function>) -> Res<()> {
    match &e.kind {
        ExprKind::ConstI64(n) if (0..=65535).contains(n)=>Ok(()),
        ExprKind::ConstI64(_)=>Err("8-bit backend: i64 value outside 16-bit scalar subset".into()),
        ExprKind::ConstBool(_)|ExprKind::Local(_)=>Ok(()),
        ExprKind::Binary{op,left,right}=>{
            if !((left.ty==Ty::Usize && right.ty==Ty::Usize)
                || (left.ty==Ty::Usize && matches!(&right.kind,ExprKind::ConstI64(n) if (0..=65535).contains(n))))
            {return Err("8-bit arithmetic requires usize operands or a fitting constant".into());}
            if !matches!(op.as_str(),"+"|"-"|"*"|"/"|"=="|"!="|"<"|"<="|">"|">="){
                return Err(format!("8-bit operation {op} not implemented; never truncate to 16 bits"));
            }
            check_expr(left,fs)?;check_expr(right,fs)
        },
        ExprKind::Call{name,arguments}=>{
            let f=fs.get(name.as_str()).ok_or_else(||format!("unavailable 8-bit call: {name}"))?;
            if f.params.len()!=arguments.len(){return Err("internal: call arity changed".into());}
            for (arg,(_,ty)) in arguments.iter().zip(&f.params) {
                if !matches!(ty,Ty::Usize|Ty::Bool){return Err("8-bit arguments must be usize/bool".into());}
                check_expr(arg,fs)?;
            }
            Ok(())
        },
        _=>Err("8-bit backend: unsupported expression (i64 runtime, struct, raw address, unary negative)".into()),
    }
}
fn check_stmts(stmts: &[Stmt], f: &Function, fs: &HashMap<&str, &Function>) -> Res<()> {
    for s in stmts {
        match s {
            Stmt::Trap => {}
            Stmt::Bind { ty, value, .. } => {
                if !matches!(ty, Ty::Usize | Ty::Bool) {
                    return Err("8-bit locals must be usize/bool".into());
                }
                check_expr(value, fs)?;
            }
            Stmt::Return { value: e, cleanup } => {
                if !cleanup.is_empty() {
                    return Err("8-bit destructors are not supported yet".into());
                }
                if f.name == "__main" {
                    if !matches!(&e.kind, ExprKind::ConstI64(0)) {
                        return Err(
                            "8-bit program must return literal 0 (no OS exit status)".into()
                        );
                    }
                } else {
                    if !matches!(f.result, Ty::Usize | Ty::Bool) {
                        return Err("8-bit void function must not contain return expression".into());
                    }
                    check_expr(e, fs)?;
                }
            }
            Stmt::Assign { value, .. } => {
                check_expr(value, fs)?;
            }
            Stmt::Evaluate(e) => {
                check_expr(e, fs)?;
            }
            Stmt::If { condition, yes, no } => {
                if condition.ty != Ty::Bool {
                    return Err("if requires bool".into());
                }
                check_expr(condition, fs)?;
                check_stmts(yes, f, fs)?;
                check_stmts(no, f, fs)?;
            }
            Stmt::While { condition, body } => {
                if condition.ty != Ty::Bool {
                    return Err("while requires bool".into());
                }
                check_expr(condition, fs)?;
                check_stmts(body, f, fs)?;
            }
            Stmt::Unsafe(children) => check_stmts(children, f, fs)?,
            Stmt::Asm(a) => {
                for (_, e) in &a.inputs {
                    check_expr(e, fs)?;
                }
            }
        }
    }
    Ok(())
}
fn check_subset(ir: &Module) -> Res<()> {
    let mut fs = HashMap::<&str, &Function>::new();
    for f in &ir.functions {
        if fs.insert(f.name.as_str(), f).is_some() {
            return Err(format!("duplicate IR function: {}", f.name));
        }
        if f.name == "__main" {
            if f.result != Ty::I64 || !f.params.is_empty() {
                return Err("internal: program signature invalid".into());
            }
        } else if !matches!(f.result, Ty::Void | Ty::Usize | Ty::Bool) {
            return Err(format!("8-bit result unsupported: {}", f.name));
        }
        if f.params.len() > 16 {
            return Err("8-bit ABI allows max 16 parameters".into());
        }
        for (_, ty) in &f.params {
            if !matches!(ty, Ty::Usize | Ty::Bool) {
                return Err("8-bit parameters must be usize or bool".into());
            }
        }
    }
    if !fs.contains_key("__main") {
        return Err("8-bit program missing".into());
    }
    for f in &ir.functions {
        check_stmts(&f.instructions, f, &fs)?;
    }
    // Static slots are legal only for non-reentrant functions. Every reachable
    // direct-call edge must be acyclic, including calls buried in expressions.
    fn expr_calls(e: &Expr, cb: &mut Vec<String>) {
        match &e.kind {
            ExprKind::Call { name, arguments } => {
                cb.push(name.clone());
                for e in arguments {
                    expr_calls(e, cb);
                }
            }
            ExprKind::Binary { left, right, .. } => {
                expr_calls(left, cb);
                expr_calls(right, cb);
            }
            ExprKind::Neg(v) => expr_calls(v, cb),
            _ => {}
        }
    }
    fn stmt_calls(ss: &[Stmt], out: &mut Vec<String>) {
        for s in ss {
            match s {
                Stmt::Trap => {}
                Stmt::Bind { value, .. } | Stmt::Assign { value, .. } | Stmt::Evaluate(value) => {
                    expr_calls(value, out)
                }
                Stmt::Return { value, cleanup } => {
                    expr_calls(value, out);
                    for d in cleanup {
                        expr_calls(d, out);
                    }
                }
                Stmt::If { condition, yes, no } => {
                    expr_calls(condition, out);
                    stmt_calls(yes, out);
                    stmt_calls(no, out);
                }
                Stmt::While { condition, body } => {
                    expr_calls(condition, out);
                    stmt_calls(body, out);
                }
                Stmt::Unsafe(ss) => stmt_calls(ss, out),
                Stmt::Asm(a) => {
                    for (_, v) in &a.inputs {
                        expr_calls(v, out);
                    }
                }
            }
        }
    }
    fn max_call_depth(
        n: &str,
        fs: &HashMap<&str, &Function>,
        visiting: &mut HashSet<String>,
        cache: &mut HashMap<String, usize>,
    ) -> Res<usize> {
        if let Some(&saved) = cache.get(n) {
            return Ok(saved);
        }
        if !visiting.insert(n.to_string()) {
            return Err(format!("8-bit static ABI cannot compile recursion: {n}"));
        }
        let f = fs
            .get(n)
            .ok_or_else(|| format!("unknown 8-bit function {n}"))?;
        let mut calls = Vec::new();
        stmt_calls(&f.instructions, &mut calls);
        let mut depth = 1usize;
        for call in calls {
            depth = depth.max(1 + max_call_depth(&call, fs, visiting, cache)?);
            if depth > 32 {
                return Err("8-bit call depth exceeds 32 (stack protection)".into());
            }
        }
        visiting.remove(n);
        cache.insert(n.to_string(), depth);
        Ok(depth)
    }
    max_call_depth("__main", &fs, &mut HashSet::new(), &mut HashMap::new())?;
    Ok(())
}

#[derive(Clone)]
struct Frame8 {
    vars: HashMap<String, u16>,
    scratch: Vec<u16>,
    call_stage: Vec<Vec<u16>>,
}
fn expr_depth(e: &Expr) -> usize {
    match &e.kind {
        ExprKind::Binary { left, right, .. } => 1 + expr_depth(left).max(expr_depth(right)),
        ExprKind::Call { arguments, .. } => 1 + arguments.iter().map(expr_depth).max().unwrap_or(0),
        ExprKind::Neg(v) => expr_depth(v),
        _ => 0,
    }
}
fn max_depth(ss: &[Stmt]) -> usize {
    ss.iter()
        .map(|s| match s {
            Stmt::Trap => 0,
            Stmt::Bind { value, .. } | Stmt::Assign { value, .. } | Stmt::Evaluate(value) => {
                expr_depth(value)
            }
            Stmt::Return { value, cleanup } => {
                expr_depth(value).max(cleanup.iter().map(expr_depth).max().unwrap_or(0))
            }
            Stmt::If { condition, yes, no } => {
                expr_depth(condition).max(max_depth(yes)).max(max_depth(no))
            }
            Stmt::While { condition, body } => expr_depth(condition).max(max_depth(body)),
            Stmt::Unsafe(xs) => max_depth(xs),
            Stmt::Asm(a) => a
                .inputs
                .iter()
                .map(|(_, v)| expr_depth(v))
                .max()
                .unwrap_or(0),
        })
        .max()
        .unwrap_or(0)
}
// Stage all arguments in caller-owned storage before updating callee slots.
// Without this, f(f(1), f(2)) would corrupt the first argument.
fn arg_staging_expr(e: &Expr, d: usize, arities: &mut Vec<usize>) {
    match &e.kind {
        ExprKind::Call { arguments, .. } => {
            if arities.len() <= d {
                arities.resize(d + 1, 0);
            }
            arities[d] = arities[d].max(arguments.len());
            for a in arguments {
                arg_staging_expr(a, d + 1, arities);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            arg_staging_expr(left, d + 1, arities);
            arg_staging_expr(right, d + 1, arities);
        }
        ExprKind::Neg(a) => arg_staging_expr(a, d, arities),
        _ => {}
    }
}
fn arg_staging_stmts(ss: &[Stmt], arities: &mut Vec<usize>) {
    for s in ss {
        match s {
            Stmt::Trap => {}
            Stmt::Bind { value, .. } | Stmt::Assign { value, .. } | Stmt::Evaluate(value) => {
                arg_staging_expr(value, 0, arities)
            }
            Stmt::Return { value, cleanup } => {
                arg_staging_expr(value, 0, arities);
                for d in cleanup {
                    arg_staging_expr(d, 0, arities);
                }
            }
            Stmt::If { condition, yes, no } => {
                arg_staging_expr(condition, 0, arities);
                arg_staging_stmts(yes, arities);
                arg_staging_stmts(no, arities);
            }
            Stmt::While { condition, body } => {
                arg_staging_expr(condition, 0, arities);
                arg_staging_stmts(body, arities);
            }
            Stmt::Unsafe(ss) => arg_staging_stmts(ss, arities),
            Stmt::Asm(a) => {
                for (_, v) in &a.inputs {
                    arg_staging_expr(v, 0, arities);
                }
            }
        }
    }
}
fn collect_vars(ss: &[Stmt], out: &mut Vec<String>) {
    for s in ss {
        match s {
            Stmt::Bind { name, .. } => out.push(name.clone()),
            Stmt::If { yes, no, .. } => {
                collect_vars(yes, out);
                collect_vars(no, out);
            }
            Stmt::While { body, .. } | Stmt::Unsafe(body) => collect_vars(body, out),
            _ => {}
        }
    }
}
fn layouts(ir: &Module, avr: Option<DeviceLayout>) -> Res<HashMap<String, Frame8>> {
    let mut frames = HashMap::new();
    let mut next: u32 = avr.map_or(0xc000, |m| m.ram.start);
    let stop: u32 = avr.map_or(0xcff0, |m| m.static_limit()); // Reserve high RAM for stack on AVR; C64 scratch is fixed.
    for f in &ir.functions {
        let mut names: Vec<String> = f.params.iter().map(|(n, _)| n.clone()).collect();
        collect_vars(&f.instructions, &mut names);
        let mut vars = HashMap::new();
        for name in names {
            if next + 2 > stop {
                return Err("8-bit static variable budget exhausted".into());
            }
            if vars.insert(name, next as u16).is_some() {
                return Err("8-bit duplicate static variable".into());
            }
            next += 2;
        }
        let mut scratch = Vec::new();
        let d = max_depth(&f.instructions);
        if d > 32 {
            return Err("8-bit expression nesting too deep".into());
        }
        for _ in 0..d {
            if next + 4 > stop {
                return Err("8-bit expression scratch budget exhausted".into());
            }
            scratch.push(next as u16);
            next += 4;
        }
        let mut arities = Vec::new();
        arg_staging_stmts(&f.instructions, &mut arities);
        let mut call_stage = Vec::new();
        for arity in arities {
            let mut slots = Vec::new();
            for _ in 0..arity {
                if next + 2 > stop {
                    return Err("8-bit argument staging budget exhausted".into());
                }
                slots.push(next as u16);
                next += 2;
            }
            call_stage.push(slots);
        }
        frames.insert(
            f.name.clone(),
            Frame8 {
                vars,
                scratch,
                call_stage,
            },
        );
    }
    Ok(frames)
}
fn var(f: &Frame8, name: &str) -> Res<u16> {
    f.vars
        .get(name)
        .copied()
        .ok_or_else(|| format!("missing 8-bit slot: {name}"))
}
fn scratch(f: &Frame8, d: usize) -> Res<u16> {
    f.scratch
        .get(d)
        .copied()
        .ok_or_else(|| "missing 8-bit scratch slot".into())
}

// C64 loads the PRG at $0801. BASIC line `10 SYS2061` enters at $080D.
// Static Mini variables occupy a reserved C64 RAM workspace $C000..$CFFF.
const C64_ENTRY: u16 = 0x080d;
// Helper scratch is private to non-reentrant internal routines, never Mini vars.
const H_LEFT: u16 = 0xcff0;
const H_RIGHT: u16 = 0xcff2;
const H_REM: u16 = 0xcff4; // 17-bit remainder occupies three bytes.
const H_COUNT: u16 = 0xcff7;

const BASIC_STUB: [u8; 12] = [
    0x0b, 0x08, 0x0a, 0x00, 0x9e, b'2', b'0', b'6', b'1', 0, 0, 0,
];
struct C64 {
    code: Vec<u8>,
    labels: HashMap<String, usize>,
    refs: Vec<(usize, String)>,
    seq: u32,
    needs_mul: bool,
    needs_div: bool,
}
impl C64 {
    fn new() -> Self {
        Self {
            code: vec![],
            labels: HashMap::new(),
            refs: vec![],
            seq: 0,
            needs_mul: false,
            needs_div: false,
        }
    }
    fn label(&mut self, n: &str) -> Res<()> {
        if self.labels.insert(n.into(), self.code.len()).is_some() {
            return Err(format!("duplicate 6510 label {n}"));
        }
        Ok(())
    }
    fn fresh(&mut self) -> String {
        let n = self.seq;
        self.seq += 1;
        format!("@c64.{n}")
    }
    fn abs(&mut self, op: u8, addr: u16) {
        self.code.push(op);
        self.code.extend(addr.to_le_bytes());
    }
    fn imm(&mut self, op: u8, value: u8) {
        self.code.extend([op, value]);
    }
    fn jump(&mut self, op: u8, n: &str) {
        self.code.push(op);
        let p = self.code.len();
        self.code.extend([0, 0]);
        self.refs.push((p, n.into()));
    }
    fn load16(&mut self, a: u16) {
        self.abs(0xad, a);
        self.abs(0xae, a + 1);
    }
    fn save16(&mut self, a: u16) {
        self.abs(0x8d, a);
        self.abs(0x8e, a + 1);
    }
    fn literal(&mut self, n: u16) {
        self.imm(0xa9, n as u8);
        self.imm(0xa2, (n >> 8) as u8);
    }
    fn branch_jmp(&mut self, branch: u8, label: &str) {
        self.code.extend([branch, 3]);
        self.jump(0x4c, label);
    }
    fn trap_on_carry(&mut self, trap_carry: bool) {
        self.branch_jmp(if trap_carry { 0x90 } else { 0xb0 }, "@mini.trap");
    }
    fn asm(
        &mut self,
        a: &AsmIr,
        fr: &Frame8,
        frames: &HashMap<String, Frame8>,
        fs: &HashMap<String, &Function>,
    ) -> Res<()> {
        for (reg, v) in &a.inputs {
            if reg != "a" {
                return Err("C64 asm requires accumulator a".into());
            }
            self.expr(v, fr, frames, fs, 0)?;
            // CHECK before narrowing usize -> u8, trap rather than truncate.
            self.imm(0xe0, 0); // CPX #0 (result high byte)
            self.branch_jmp(0xf0, "@mini.trap"); // BEQ -> skip trap; branch_jmp jumps when NOT equal
        }
        for i in &a.instructions {
            match i.as_str() {
                "nop" => self.code.push(0xea),
                "sei" => self.code.push(0x78),
                "cli" => self.code.push(0x58),
                s if s.starts_with("sta:") => {
                    let addr = s[4..]
                        .parse::<u16>()
                        .map_err(|_| "invalid C64 STA address")?;
                    self.abs(0x8d, addr);
                }
                _ => return Err(format!("invalid C64 opcode {i}")),
            }
        }
        Ok(())
    }
    fn expr(
        &mut self,
        e: &Expr,
        fr: &Frame8,
        frames: &HashMap<String, Frame8>,
        fs: &HashMap<String, &Function>,
        d: usize,
    ) -> Res<()> {
        match &e.kind {
            ExprKind::ConstI64(n) => {
                let n = u16::try_from(*n).map_err(|_| "constant outside 16-bit range")?;
                self.literal(n);
            }
            ExprKind::ConstBool(b) => self.literal(if *b { 1 } else { 0 }),
            ExprKind::Local(n) => self.load16(var(fr, n)?),
            ExprKind::Call { name, arguments } => {
                let callee = frames.get(name).ok_or("missing callee frame")?;
                let sig = fs.get(name).ok_or("missing callee")?;
                let stage = fr
                    .call_stage
                    .get(d)
                    .ok_or("C64 missing argument staging level")?;
                for (i, arg) in arguments.iter().enumerate() {
                    self.expr(arg, fr, frames, fs, d + 1)?;
                    self.save16(*stage.get(i).ok_or("C64 staging size error")?);
                }
                for (i, (param, _)) in sig.params.iter().enumerate() {
                    self.load16(stage[i]);
                    self.save16(var(callee, param)?);
                }
                self.jump(0x20, name);
            }
            ExprKind::Binary { op, left, right } => {
                let tmp = scratch(fr, d)?;
                self.expr(left, fr, frames, fs, d + 1)?;
                self.save16(tmp);
                self.expr(right, fr, frames, fs, d + 1)?;
                self.save16(tmp + 2);
                if matches!(op.as_str(), "+" | "-") {
                    self.load16(tmp);
                    self.code.push(if op == "+" { 0x18 } else { 0x38 }); // CLC/SEC
                    self.abs(if op == "+" { 0x6d } else { 0xed }, tmp + 2); // ADC/SBC low
                    self.code.push(0xa8); // TAY (save result low)
                    self.code.push(0x8a); // TXA (high byte)
                    self.abs(if op == "+" { 0x6d } else { 0xed }, tmp + 3);
                    self.code.push(0xaa);
                    self.code.push(0x98); // TAX; TYA
                    self.trap_on_carry(op == "+");
                } else if op == "*" || op == "/" {
                    if op == "*" {
                        self.needs_mul = true;
                    } else {
                        self.needs_div = true;
                    }
                    self.load16(tmp);
                    self.save16(H_LEFT);
                    self.load16(tmp + 2);
                    self.save16(H_RIGHT);
                    self.jump(0x20, if op == "*" { "@mini.mul" } else { "@mini.div" });
                } else {
                    // 16-bit unsigned compare: compare high; compare low if high equal.
                    // Reverse for '>' and '<=' to reuse carry-only branch conditions.
                    let reverse = matches!(op.as_str(), ">" | "<=");
                    let (l, r) = if reverse {
                        (tmp + 2, tmp)
                    } else {
                        (tmp, tmp + 2)
                    };
                    self.abs(0xad, l + 1);
                    self.abs(0xcd, r + 1); // LDA high; CMP high
                    self.code.extend([0xd0, 6]); // BNE +6: preserve high compare flags
                    self.abs(0xad, l);
                    self.abs(0xcd, r);
                    let skip_jump_if_false = match op.as_str() {
                        "<" | ">" => 0xb0,
                        ">=" | "<=" => 0x90,
                        "==" => 0xd0,
                        "!=" => 0xf0,
                        _ => return Err("unknown 6510 compare".into()),
                    };
                    let yes = self.fresh();
                    let end = self.fresh();
                    self.branch_jmp(skip_jump_if_false, &yes);
                    self.literal(0);
                    self.jump(0x4c, &end);
                    self.label(&yes)?;
                    self.literal(1);
                    self.label(&end)?;
                }
            }
            _ => return Err("unsupported C64 IR expression".into()),
        }
        Ok(())
    }
    fn stmts(
        &mut self,
        body: &[Stmt],
        fr: &Frame8,
        frames: &HashMap<String, Frame8>,
        fs: &HashMap<String, &Function>,
    ) -> Res<()> {
        for s in body {
            match s {
                Stmt::Trap => self.jump(0x4c, "@mini.trap"),
                Stmt::Bind { name, value, .. } | Stmt::Assign { name, value } => {
                    self.expr(value, fr, frames, fs, 0)?;
                    self.save16(var(fr, name)?);
                }
                Stmt::Return { value: e, cleanup } => {
                    if !cleanup.is_empty() {
                        return Err("C64 destructors require struct support".into());
                    }
                    self.expr(e, fr, frames, fs, 0)?;
                    self.code.push(0x60);
                }
                Stmt::Evaluate(e) => self.expr(e, fr, frames, fs, 0)?,
                Stmt::If { condition, yes, no } => {
                    let lfalse = self.fresh();
                    let lend = self.fresh();
                    self.expr(condition, fr, frames, fs, 0)?;
                    self.imm(0xc9, 0); // CMP #0; bool canonical 0/1
                    self.branch_jmp(0xd0, &lfalse); // BNE skips JMP to false
                    self.stmts(yes, fr, frames, fs)?;
                    self.jump(0x4c, &lend);
                    self.label(&lfalse)?;
                    self.stmts(no, fr, frames, fs)?;
                    self.label(&lend)?;
                }
                Stmt::While { condition, body } => {
                    let top = self.fresh();
                    let end = self.fresh();
                    self.label(&top)?;
                    self.expr(condition, fr, frames, fs, 0)?;
                    self.imm(0xc9, 0);
                    self.branch_jmp(0xd0, &end);
                    self.stmts(body, fr, frames, fs)?;
                    self.jump(0x4c, &top);
                    self.label(&end)?;
                }
                Stmt::Unsafe(ss) => self.stmts(ss, fr, frames, fs)?,
                Stmt::Asm(a) => self.asm(a, fr, frames, fs)?,
            }
        }
        Ok(())
    }
    // Unsigned checked 16-bit shift/add. Parameters in H_LEFT/H_RIGHT;
    // result A/X. No helper calls into Mini; static ABI remains non-reentrant.
    fn mul_helper(&mut self) -> Res<()> {
        self.label("@mini.mul")?;
        self.literal(0);
        self.save16(H_REM); // accumulator
        let top = self.fresh();
        let no_add = self.fresh();
        let done = self.fresh();
        self.label(&top)?;
        self.abs(0xad, H_RIGHT);
        self.imm(0x29, 1); // LDA rhs.lo; AND #1
        self.branch_jmp(0xd0, &no_add); // if zero skip addition
        self.abs(0xad, H_REM);
        self.code.push(0x18); // LDA sum.lo; CLC
        self.abs(0x6d, H_LEFT);
        self.abs(0x8d, H_REM); // ADC lhs.lo; STA
        self.abs(0xad, H_REM + 1);
        self.abs(0x6d, H_LEFT + 1);
        self.abs(0x8d, H_REM + 1);
        self.branch_jmp(0x90, "@mini.trap"); // BCC skips trap; 16-bit carry = overflow
        self.label(&no_add)?;
        self.abs(0x4e, H_RIGHT + 1);
        self.abs(0x6e, H_RIGHT); // LSR rhs.hi / ROR rhs.lo
        self.abs(0xad, H_RIGHT);
        self.abs(0x0d, H_RIGHT + 1); // ORA rhs.hi
        self.branch_jmp(0xd0, &done); // rhs == 0 => done
        self.abs(0xad, H_LEFT + 1);
        self.imm(0x29, 0x80);
        self.branch_jmp(0xf0, "@mini.trap"); // cannot shift away a set 16th bit
        self.abs(0x0e, H_LEFT);
        self.abs(0x2e, H_LEFT + 1); // ASL/ROL lhs
        self.jump(0x4c, &top);
        self.label(&done)?;
        self.load16(H_REM);
        self.code.push(0x60);
        Ok(())
    }
    // Sixteen-step restoring unsigned division, using a 17-bit remainder.
    // H_LEFT is shifted into the quotient; H_RIGHT is the divisor.
    fn div_helper(&mut self) -> Res<()> {
        self.label("@mini.div")?;
        self.abs(0xad, H_RIGHT);
        self.abs(0x0d, H_RIGHT + 1);
        self.branch_jmp(0xd0, "@mini.trap"); // divide by zero => trap
        self.imm(0xa9, 0);
        for a in [H_REM, H_REM + 1, H_REM + 2] {
            self.abs(0x8d, a);
        }
        self.imm(0xa9, 16);
        self.abs(0x8d, H_COUNT);
        let top = self.fresh();
        let sub = self.fresh();
        let skip = self.fresh();
        self.label(&top)?;
        self.abs(0x0e, H_LEFT);
        self.abs(0x2e, H_LEFT + 1); // ASL/ROL quotient
        for a in [H_REM, H_REM + 1, H_REM + 2] {
            self.abs(0x2e, a);
        } // shift in dividend bit
        self.abs(0xad, H_REM + 2);
        self.branch_jmp(0xf0, &sub); // top byte nonzero => subtract
        self.abs(0xad, H_REM + 1);
        self.abs(0xcd, H_RIGHT + 1);
        self.branch_jmp(0xb0, &skip); // high byte less => do not subtract
        self.branch_jmp(0xf0, &sub); // high byte greater => subtract
        self.abs(0xad, H_REM);
        self.abs(0xcd, H_RIGHT);
        self.branch_jmp(0x90, &sub); // low byte >= => subtract
        self.jump(0x4c, &skip);
        self.label(&sub)?;
        self.code.push(0x38); // SEC
        self.abs(0xad, H_REM);
        self.abs(0xed, H_RIGHT);
        self.abs(0x8d, H_REM);
        self.abs(0xad, H_REM + 1);
        self.abs(0xed, H_RIGHT + 1);
        self.abs(0x8d, H_REM + 1);
        self.abs(0xad, H_REM + 2);
        self.imm(0xe9, 0);
        self.abs(0x8d, H_REM + 2);
        self.abs(0xad, H_LEFT);
        self.imm(0x09, 1);
        self.abs(0x8d, H_LEFT);
        self.label(&skip)?;
        self.abs(0xce, H_COUNT); // DEC counter
        self.branch_jmp(0xf0, &top); // BNE? BEQ skips JMP when count==0
        self.load16(H_LEFT);
        self.code.push(0x60);
        Ok(())
    }
    fn finish(mut self) -> Res<Vec<u8>> {
        if C64_ENTRY as usize + self.code.len() > 0xa000 {
            return Err("C64 code overlaps BASIC ROM window $A000".into());
        }
        for (at, name) in self.refs {
            let off = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("unknown C64 label {name}"))?;
            let addr = (C64_ENTRY as usize)
                .checked_add(off)
                .ok_or("C64 address overflow")?;
            if addr >= 0xa000 {
                return Err("C64 jump outside program region".into());
            }
            self.code[at..at + 2].copy_from_slice(&(addr as u16).to_le_bytes());
        }
        let mut prg = Vec::with_capacity(self.code.len() + 14);
        prg.extend(0x0801u16.to_le_bytes());
        prg.extend(BASIC_STUB);
        prg.extend(self.code);
        Ok(prg)
    }
}

struct Avr {
    code: Vec<u8>,
    labels: HashMap<String, usize>,
    refs: Vec<(usize, String)>,
    seq: u32,
    needs_mul: bool,
    needs_div: bool,
    layout: DeviceLayout,
}
impl Avr {
    fn new(layout: DeviceLayout) -> Self {
        Self {
            code: vec![],
            labels: HashMap::new(),
            refs: vec![],
            seq: 0,
            needs_mul: false,
            needs_div: false,
            layout,
        }
    }
    fn word(&mut self, w: u16) {
        self.code.extend(w.to_le_bytes());
    }
    fn label(&mut self, n: &str) -> Res<()> {
        if self.labels.insert(n.into(), self.code.len()).is_some() {
            return Err(format!("duplicate AVR label {n}"));
        }
        Ok(())
    }
    fn fresh(&mut self) -> String {
        let n = self.seq;
        self.seq += 1;
        format!("@avr.{n}")
    }
    fn jump(&mut self, call: bool, name: &str) {
        self.word(if call { 0x940e } else { 0x940c });
        let p = self.code.len();
        self.word(0);
        self.refs.push((p, name.into()));
    }
    fn ldi(&mut self, r: u8, v: u8) -> Res<()> {
        if !(16..=31).contains(&r) {
            return Err("AVR LDI requires r16..r31".into());
        }
        self.word(
            0xe000 | (((v as u16) & 0xf0) << 4) | (((r as u16) - 16) << 4) | ((v as u16) & 0x0f),
        );
        Ok(())
    }
    fn andi(&mut self, r: u8, v: u8) -> Res<()> {
        if !(16..=31).contains(&r) {
            return Err("AVR ANDI requires r16..r31".into());
        }
        self.word(
            0x7000 | (((v as u16) & 0xf0) << 4) | (((r as u16) - 16) << 4) | ((v as u16) & 15),
        );
        Ok(())
    }
    fn ori(&mut self, r: u8, v: u8) -> Res<()> {
        if !(16..=31).contains(&r) {
            return Err("AVR ORI requires r16..r31".into());
        }
        self.word(
            0x6000 | (((v as u16) & 0xf0) << 4) | (((r as u16) - 16) << 4) | ((v as u16) & 15),
        );
        Ok(())
    }
    fn single(&mut self, opcode: u16, r: u8) {
        self.word(opcode | ((r as u16) << 4));
    }
    fn out(&mut self, port: u8, r: u8) -> Res<()> {
        if port > 63 || r > 31 {
            return Err("invalid AVR OUT".into());
        }
        self.word(
            0xb800 | (((port as u16) & 0x30) << 5) | ((r as u16) << 4) | ((port as u16) & 0xf),
        );
        Ok(())
    }
    fn sts(&mut self, addr: u16, r: u8) -> Res<()> {
        if r > 31 || u32::from(addr) >= self.layout.ram.end {
            return Err("AVR STS outside configured data space".into());
        }
        self.word(0x9200 | ((r as u16) << 4));
        self.word(addr);
        Ok(())
    }
    fn lds(&mut self, r: u8, addr: u16) -> Res<()> {
        if r > 31 || u32::from(addr) >= self.layout.ram.end {
            return Err("AVR LDS outside configured data space".into());
        }
        self.word(0x9000 | ((r as u16) << 4));
        self.word(addr);
        Ok(())
    }
    fn rr(&mut self, op: u16, d: u8, r: u8) {
        self.word(op | (((r as u16) & 16) << 5) | ((d as u16) << 4) | ((r as u16) & 15));
    }
    fn save16(&mut self, a: u16) -> Res<()> {
        self.sts(a, 24)?;
        self.sts(a + 1, 25)
    }
    fn load16(&mut self, a: u16) -> Res<()> {
        self.lds(24, a)?;
        self.lds(25, a + 1)
    }
    fn literal(&mut self, n: u16) -> Res<()> {
        self.ldi(24, n as u8)?;
        self.ldi(25, (n >> 8) as u8)
    }
    // Fixed-size conditional branch across an absolute 2-word JMP: +2 words.
    fn br_skip_jump(&mut self, condition: u16, target: &str) {
        self.word(condition | (2 << 3));
        self.jump(false, target);
    }
    fn check_high_zero(&mut self) -> Res<()> {
        self.ldi(20, 0)?;
        self.rr(0x1400, 25, 20); // CP r25,r20
        self.br_skip_jump(0xf001, "@mini.trap"); // BREQ +2, else JMP trap
        Ok(())
    }
    fn asm(
        &mut self,
        a: &AsmIr,
        fr: &Frame8,
        frames: &HashMap<String, Frame8>,
        fs: &HashMap<String, &Function>,
    ) -> Res<()> {
        for (reg, value) in &a.inputs {
            let r = reg
                .strip_prefix('r')
                .and_then(|s| s.parse::<u8>().ok())
                .ok_or("invalid AVR asm register")?;
            self.expr(value, fr, frames, fs, 0)?;
            self.check_high_zero()?;
            if r != 24 {
                self.rr(0x2c00, r, 24);
            } // MOV r,r24
        }
        for op in &a.instructions {
            match op.as_str() {
                "nop" => self.word(0x0000),
                "sei" => self.word(0x9478),
                "cli" => self.word(0x94f8),
                v if v.starts_with("outio:") || v.starts_with("sts:") => {
                    let p: Vec<_> = v.split(':').collect();
                    if p.len() != 3 {
                        return Err("malformed AVR store".into());
                    }
                    let addr = p[1].parse::<u16>().map_err(|_| "bad AVR store address")?;
                    let r = p[2]
                        .strip_prefix('r')
                        .and_then(|s| s.parse::<u8>().ok())
                        .ok_or("bad AVR register")?;
                    if p[0] == "outio" {
                        self.out(u8::try_from(addr).map_err(|_| "AVR I/O range")?, r)?;
                    } else {
                        self.sts(addr, r)?;
                    }
                }
                _ => return Err(format!("unknown AVR opcode {op}")),
            }
        }
        Ok(())
    }
    fn expr(
        &mut self,
        e: &Expr,
        fr: &Frame8,
        frames: &HashMap<String, Frame8>,
        fs: &HashMap<String, &Function>,
        d: usize,
    ) -> Res<()> {
        match &e.kind {
            ExprKind::ConstI64(n) => {
                self.literal(u16::try_from(*n).map_err(|_| "AVR constant outside usize")?)?
            }
            ExprKind::ConstBool(b) => self.literal(if *b { 1 } else { 0 })?,
            ExprKind::Local(n) => self.load16(var(fr, n)?)?,
            ExprKind::Call { name, arguments } => {
                let callee = frames.get(name).ok_or("AVR missing callee frame")?;
                let sig = fs.get(name).ok_or("AVR missing callee")?;
                let stage = fr
                    .call_stage
                    .get(d)
                    .ok_or("AVR missing argument staging level")?;
                for (i, arg) in arguments.iter().enumerate() {
                    self.expr(arg, fr, frames, fs, d + 1)?;
                    self.save16(*stage.get(i).ok_or("AVR staging size error")?)?;
                }
                for (i, (param, _)) in sig.params.iter().enumerate() {
                    self.load16(stage[i])?;
                    self.save16(var(callee, param)?)?;
                }
                self.jump(true, name);
            }
            ExprKind::Binary { op, left, right } => {
                let tmp = scratch(fr, d)?;
                self.expr(left, fr, frames, fs, d + 1)?;
                self.save16(tmp)?;
                self.expr(right, fr, frames, fs, d + 1)?;
                self.save16(tmp + 2)?;
                self.load16(tmp)?;
                self.lds(20, tmp + 2)?;
                self.lds(21, tmp + 3)?;
                if op == "+" || op == "-" {
                    if op == "+" {
                        self.rr(0x0c00, 24, 20);
                        self.rr(0x1c00, 25, 21); // ADD / ADC
                        self.br_skip_jump(0xf400, "@mini.trap"); // BRCC skip trap
                    } else {
                        self.rr(0x1800, 24, 20);
                        self.rr(0x0800, 25, 21); // SUB / SBC
                        self.br_skip_jump(0xf400, "@mini.trap"); // BRCC skip trap
                    }
                } else if op == "*" || op == "/" {
                    if op == "*" {
                        self.needs_mul = true;
                    } else {
                        self.needs_div = true;
                    }
                    self.load16(tmp)?;
                    self.lds(20, tmp + 2)?;
                    self.lds(21, tmp + 3)?;
                    self.jump(true, if op == "*" { "@mini.mul" } else { "@mini.div" });
                } else {
                    if matches!(op.as_str(), ">" | "<=") {
                        self.rr(0x1400, 20, 24);
                        self.rr(0x0400, 21, 25);
                    } else {
                        self.rr(0x1400, 24, 20);
                        self.rr(0x0400, 25, 21);
                    }
                    let skip_jump_if_false = match op.as_str() {
                        "<" | ">" => 0xf400,   // BRCC
                        ">=" | "<=" => 0xf000, // BRCS
                        "==" => 0xf401,        // BRNE
                        "!=" => 0xf001,        // BREQ
                        _ => return Err("unknown AVR comparison".into()),
                    };
                    let yes = self.fresh();
                    let end = self.fresh();
                    self.br_skip_jump(skip_jump_if_false, &yes);
                    self.literal(0)?;
                    self.jump(false, &end);
                    self.label(&yes)?;
                    self.literal(1)?;
                    self.label(&end)?;
                }
            }
            _ => return Err("unsupported AVR IR expression".into()),
        }
        Ok(())
    }
    fn stmts(
        &mut self,
        body: &[Stmt],
        fr: &Frame8,
        frames: &HashMap<String, Frame8>,
        fs: &HashMap<String, &Function>,
    ) -> Res<()> {
        for s in body {
            match s {
                Stmt::Trap => self.jump(false, "@mini.trap"),
                Stmt::Bind { name, value, .. } | Stmt::Assign { name, value } => {
                    self.expr(value, fr, frames, fs, 0)?;
                    self.save16(var(fr, name)?)?;
                }
                Stmt::Return { value: e, cleanup } => {
                    if !cleanup.is_empty() {
                        return Err("AVR destructors require struct support".into());
                    }
                    self.expr(e, fr, frames, fs, 0)?;
                    self.word(0x9508);
                }
                Stmt::Evaluate(e) => self.expr(e, fr, frames, fs, 0)?,
                Stmt::If { condition, yes, no } => {
                    let lfalse = self.fresh();
                    let lend = self.fresh();
                    self.expr(condition, fr, frames, fs, 0)?;
                    self.ldi(20, 0)?;
                    self.rr(0x1400, 24, 20); // CP low with zero
                    self.br_skip_jump(0xf401, &lfalse); // BRNE +2, else JMP false
                    self.stmts(yes, fr, frames, fs)?;
                    self.jump(false, &lend);
                    self.label(&lfalse)?;
                    self.stmts(no, fr, frames, fs)?;
                    self.label(&lend)?;
                }
                Stmt::While { condition, body } => {
                    let top = self.fresh();
                    let end = self.fresh();
                    self.label(&top)?;
                    self.expr(condition, fr, frames, fs, 0)?;
                    self.ldi(20, 0)?;
                    self.rr(0x1400, 24, 20);
                    self.br_skip_jump(0xf401, &end);
                    self.stmts(body, fr, frames, fs)?;
                    self.jump(false, &top);
                    self.label(&end)?;
                }
                Stmt::Unsafe(ss) => self.stmts(ss, fr, frames, fs)?,
                Stmt::Asm(a) => self.asm(a, fr, frames, fs)?,
            }
        }
        Ok(())
    }
    // Both helpers take lhs r24:r25, rhs r20:r21, result r24:r25.
    // Their private registers may be clobbered; Mini's private ABI does not
    // preserve registers across calls. Neither helper invokes a Mini function.
    fn mul_helper(&mut self) -> Res<()> {
        self.label("@mini.mul")?;
        self.rr(0x2c00, 18, 24);
        self.rr(0x2c00, 19, 25);
        self.ldi(24, 0)?;
        self.ldi(25, 0)?;
        let top = self.fresh();
        let after = self.fresh();
        let done = self.fresh();
        self.label(&top)?;
        self.rr(0x2c00, 16, 20);
        self.andi(16, 1)?;
        self.br_skip_jump(0xf401, &after); // BRNE skip JMP, add only if bit=1
        self.rr(0x0c00, 24, 18);
        self.rr(0x1c00, 25, 19);
        self.br_skip_jump(0xf400, "@mini.trap"); // BRCC => no overflow
        self.label(&after)?;
        self.single(0x9406, 21);
        self.single(0x9407, 20); // LSR rhs.hi; ROR rhs.lo
        self.rr(0x2c00, 16, 20);
        self.rr(0x2800, 16, 21);
        self.br_skip_jump(0xf401, &done); // BRNE => continue; zero => done
        self.rr(0x2c00, 16, 19);
        self.andi(16, 0x80)?;
        self.br_skip_jump(0xf001, "@mini.trap"); // BREQ => safe to shift
        self.rr(0x0c00, 18, 18);
        self.rr(0x1c00, 19, 19); // LSL/ROL lhs
        self.jump(false, &top);
        self.label(&done)?;
        self.word(0x9508);
        Ok(())
    }
    fn div_helper(&mut self) -> Res<()> {
        self.label("@mini.div")?;
        self.rr(0x2c00, 16, 20);
        self.rr(0x2800, 16, 21);
        self.br_skip_jump(0xf401, "@mini.trap"); // BRNE => nonzero
        self.ldi(22, 0)?;
        self.ldi(23, 0)?;
        self.ldi(18, 0)?;
        self.ldi(19, 0)?;
        self.ldi(17, 16)?;
        let top = self.fresh();
        let subtract = self.fresh();
        let skip = self.fresh();
        self.label(&top)?;
        self.rr(0x0c00, 24, 24);
        self.rr(0x1c00, 25, 25); // shift dividend/quotient
        self.rr(0x1c00, 22, 22);
        self.rr(0x1c00, 23, 23);
        self.rr(0x1c00, 18, 18); // 17th remainder bit
        self.rr(0x2c00, 16, 18);
        self.rr(0x2800, 16, 16);
        self.br_skip_jump(0xf001, &subtract); // if high remainder bit nonzero
        self.rr(0x1400, 22, 20);
        self.rr(0x0400, 23, 21);
        self.br_skip_jump(0xf000, &subtract); // BRCS skip subtract when rem < div
        self.jump(false, &skip);
        self.label(&subtract)?;
        self.rr(0x1800, 22, 20);
        self.rr(0x0800, 23, 21);
        self.rr(0x0800, 18, 19); // 17th bit -= borrow; r19 is zero
        self.ori(24, 1)?;
        self.label(&skip)?;
        self.single(0x940a, 17); // DEC iterations
        self.br_skip_jump(0xf001, &top); // BREQ => done; else JMP loop
        self.word(0x9508);
        Ok(())
    }
    fn finish(mut self) -> Res<Vec<u8>> {
        if self.code.len() > self.layout.flash.len() as usize {
            return Err("AVR code exceeds configured flash region".into());
        }
        for (at, name) in self.refs {
            let addr = *self
                .labels
                .get(&name)
                .ok_or_else(|| format!("unknown AVR label {name}"))?;
            if addr & 1 != 0 {
                return Err("unaligned AVR jump target".into());
            }
            if addr >= self.layout.flash.len() as usize {
                return Err("AVR jump outside configured flash".into());
            }
            self.code[at..at + 2].copy_from_slice(&((addr / 2) as u16).to_le_bytes());
        }
        ihex(&self.code)
    }
}
fn ihex(data: &[u8]) -> Res<Vec<u8>> {
    let mut out = String::new();
    for (block, chunk) in data.chunks(16).enumerate() {
        let addr = block * 16;
        if addr > 65535 {
            return Err("Intel HEX address overflow".into());
        }
        let mut rec = vec![chunk.len() as u8, (addr >> 8) as u8, addr as u8, 0];
        rec.extend(chunk);
        let sum = rec.iter().fold(0u8, |a, b| a.wrapping_add(*b));
        rec.push(0u8.wrapping_sub(sum));
        out.push(':');
        for byte in rec {
            out.push_str(&format!("{byte:02X}"));
        }
        out.push('\n');
    }
    out.push_str(":00000001FF\n");
    Ok(out.into_bytes())
}
pub(crate) fn emit(ir: &Module, target: Target) -> Res<Vec<u8>> {
    check_subset(ir)?;
    if ir.functions.iter().any(|f| !f.end_cleanup.is_empty()) {
        return Err("8-bit destructor calls require struct ABI support".into());
    }
    let avr = target.arch == Arch::Avr8;
    let frames = layouts(
        ir,
        if avr {
            Some(target.layout.ok_or("AVR device map missing")?)
        } else {
            None
        },
    )?;
    let fs: HashMap<String, &Function> = ir.functions.iter().map(|f| (f.name.clone(), f)).collect();
    match target.arch {
        Arch::Mos6510 => {
            let mut e = C64::new();
            e.code.push(0xd8); // CLD: checked ADC/SBC must use binary mode
            e.jump(0x4c, "__main");
            e.label("@mini.trap")?;
            e.jump(0x4c, "@mini.trap");
            for f in &ir.functions {
                e.label(&f.name)?;
                e.stmts(&f.instructions, &frames[&f.name], &frames, &fs)?;
                if !matches!(f.instructions.last(), Some(Stmt::Return { .. })) {
                    e.code.push(0x60);
                }
            }
            if e.needs_mul {
                e.mul_helper()?;
            }
            if e.needs_div {
                e.div_helper()?;
            }
            e.finish()
        }
        Arch::Avr8 => {
            let layout = target.layout.ok_or("AVR device map missing")?;
            let mut e = Avr::new(layout);
            e.jump(false, "@mini.start");
            for _ in 1..layout.vectors {
                e.jump(false, "@mini.trap");
            }
            e.label("@mini.trap")?;
            e.word(0xcfff); // RJMP $ (IRQ/overflow trap)
            e.label("@mini.start")?;
            let stack_top = layout.stack_top();
            e.ldi(16, (stack_top >> 8) as u8)?;
            e.out(0x3e, 16)?;
            e.ldi(16, stack_top as u8)?;
            e.out(0x3d, 16)?;
            e.jump(true, "__main");
            e.label("@mini.idle")?;
            e.word(0xcfff); // application returns here
            for f in &ir.functions {
                e.label(&f.name)?;
                e.stmts(&f.instructions, &frames[&f.name], &frames, &fs)?;
                if !matches!(f.instructions.last(), Some(Stmt::Return { .. })) {
                    e.word(0x9508);
                }
            }
            if e.needs_mul {
                e.mul_helper()?;
            }
            if e.needs_div {
                e.div_helper()?;
            }
            e.finish()
        }
        _ => Err("not an 8-bit codegen target".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{P, check_for, check_usize_literals, lex};
    fn build(src: &str, t: &str) -> Res<Vec<u8>> {
        let target = Target::parse(t).unwrap();
        let ast = P::new(lex(src)?).parse()?;
        let (fs, frames) = check_for(&ast, target)?;
        check_usize_literals(&ast, &fs, &frames, target)?;
        let ir = crate::ir::lower(&ast, &frames, &fs)?;
        emit(&ir, target)
    }
    #[test]
    fn c64_16bit_slots_calls_and_overflow_paths() {
        let src = "fn sum(a: usize, b: usize) -> usize { return a + b } program { let x: usize = 20; let y: usize = 22; let z: usize = sum(x,y); let answer: usize = 42; if z == answer { return 0 } else { return 0 } }";
        let bytes = build(src, "c64-6510").unwrap();
        assert_eq!(
            &bytes[..14],
            &[1, 8, 11, 8, 10, 0, 0x9e, b'2', b'0', b'6', b'1', 0, 0, 0]
        );
        assert!(bytes.windows(3).any(|x| x == [0x8d, 0x00, 0xc0])); // static slots in $C000
        assert!(bytes.contains(&0x6d)); // ADC abs
        assert!(bytes.contains(&0x20)); // JSR
    }
    #[test]
    fn avr_16bit_slots_calls_and_arith() {
        let src = "fn sum(a: usize, b: usize) -> usize { return a + b } program { let x: usize = 20; let y: usize = 22; let z: usize = sum(x,y); let answer: usize = 42; if z == answer { return 0 } else { return 0 } }";
        let bytes = build(src, "avr8-atmega328p").unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains(":00000001FF"));
        assert!(text.len() > 250);
    }
    #[test]
    fn legacy_gpio_and_c64_platform_asm() {
        let c64 = "unsafe fn signal(){unsafe{asm{in a = 6; sta(53280); clobber memory}}} program{unsafe{signal()};return 0}";
        assert!(
            build(c64, "c64-6510")
                .unwrap()
                .windows(2)
                .any(|x| x == [0xa9, 6])
        );
        let avr = "unsafe fn signal(){unsafe{asm{in r16 = 32; outio(5,r16); clobber memory}}} program{unsafe{signal()};return 0}";
        assert!(build(avr, "avr8-atmega328p").is_ok());
    }
    #[test]
    fn scalar_mul_div_only_if_reachable() {
        for target in ["c64-6510", "avr8-atmega328p"] {
            let base = build("program { return 0 }", target).unwrap();
            let unused = build(
                "fn unused(a:usize,b:usize)->usize{return a*b} program { return 0 }",
                target,
            )
            .unwrap();
            assert_eq!(
                base, unused,
                "dead arithmetic helper must not appear in output"
            );
            let product = build(
                "program { let a:usize=257; let b:usize=255; let x:usize=a*b; return 0 }",
                target,
            )
            .unwrap();
            let quotient = build(
                "program { let a:usize=65535; let b:usize=255; let x:usize=a/b; return 0 }",
                target,
            )
            .unwrap();
            assert!(
                product.len() > base.len() + 70,
                "mul code missing: {target}"
            );
            assert!(
                quotient.len() > base.len() + 70,
                "div code missing: {target}"
            );
        }
    }
    #[test]
    fn device_memory_budget_really_changes_codegen() {
        use crate::device_config::DeviceProfile;
        let atmega168 =
            DeviceProfile::parse(include_str!("../targets/devices/atmega168p.target")).unwrap();
        let small = Target::from_device(&atmega168);
        let large = Target::parse("avr8-atmega328p").unwrap();
        let mut src = String::from("program {\n");
        // 385*2 bytes exceeds the smaller chip's 768-byte Mini static budget;
        // the larger chip has 1792 bytes available for Mini slots.
        for i in 0..385 {
            src.push_str(&format!("let slot{i}: usize = 1;\n"));
        }
        src.push_str("return 0\n}");
        let ast = P::new(lex(&src).unwrap()).parse().unwrap();
        let (fs, frames) = check_for(&ast, small).unwrap();
        check_usize_literals(&ast, &fs, &frames, small).unwrap();
        let ir = crate::ir::lower(&ast, &frames, &fs).unwrap();
        assert!(emit(&ir, small).is_err());
        assert!(emit(&ir, large).is_ok());
    }
    #[test]
    fn zero_return_and_signed_runtime_stays_rejected() {
        for target in ["c64-6510", "avr8-atmega328p"] {
            assert!(
                build(
                    "fn zero()->usize{return 0} program{let x:usize=zero();return 0}",
                    target
                )
                .is_ok()
            );
            assert!(build("fn bad(x:i64)->i64{return x*x} program{return 0}", target).is_ok()); // dead
            assert!(
                build(
                    "fn bad(x:i64)->i64{return x*x} program{let a:i64=2;let b:i64=bad(a);return 0}",
                    target
                )
                .is_err()
            );
        }
    }
    #[test]
    fn unsafe_and_capacity_errors() {
        assert!(
            build(
                "unsafe fn x(){unsafe{asm{nop}}} program{x();return 0}",
                "c64-6510"
            )
            .is_err()
        );
        assert!(
            build(
                "fn recur(x:usize)->usize{return recur(x)} program{return 0}",
                "c64-6510"
            )
            .is_ok()
        ); // dead function removed
        assert!(build("fn recur(x:usize)->usize{return recur(x)} program{let x:usize=1;let y:usize=recur(x);return 0}","c64-6510").is_err());
        assert!(build("program{return 1+2}", "c64-6510").is_err());
        assert!(build("program{let x:usize=65536;return 0}", "avr8-atmega328p").is_err());
    }
}
