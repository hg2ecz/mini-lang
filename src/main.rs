//! Mini compiler: typed IR is the only input to the x86-64 code emitter.
//! Aggregate returns use a caller-provided hidden output pointer.
mod backend_aarch64;
mod backend_arm32;
mod backend_thumb;
mod backend_x86;
mod code_buffer;
mod device_config;
mod encode_aarch64;
mod encode_arm32;
mod encode_thumb;
mod encode_x86_64;
mod ir;
mod preprocess;
mod target;
mod tiny8;
use std::collections::{HashMap, HashSet};
use std::{env, fs, path::Path};
type Res<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, PartialEq)]
enum T {
    Id(String),
    Num(i64),
    Sym(String),
    End,
}
fn lex(s: &str) -> Res<Vec<T>> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    let mut nesting = 0;
    let mut brackets = 0;
    while i < b.len() {
        let c = b[i];
        if c == b' ' || c == b'\t' || c == b'\r' {
            i += 1;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'\n' || c == b';' {
            if nesting == 0 || c == b';' {
                out.push(T::Sym(";".into()));
            }
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(T::Id(s[start..i].into()));
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n = s[start..i]
                .parse()
                .map_err(|_| "integer literal out of range".to_string())?;
            out.push(T::Num(n));
            continue;
        }
        if c == b'[' {
            brackets += 1;
        }
        if c == b']' {
            if brackets == 0 {
                return Err("unmatched ']'".into());
            }
            brackets -= 1;
        }
        if c == b'(' {
            nesting += 1;
        }
        if c == b')' {
            if nesting == 0 {
                return Err("unmatched ')'".into());
            }
            nesting -= 1;
        }
        if c == b'-' && b.get(i + 1) == Some(&b'>') {
            out.push(T::Sym("->".into()));
            i += 2;
            continue;
        }
        if matches!(c, b'=' | b'!' | b'<' | b'>') && b.get(i + 1) == Some(&b'=') {
            out.push(T::Sym(format!("{}=", c as char)));
            i += 2;
            continue;
        }
        if b"{}[]():,+-*/.=<>".contains(&c) {
            out.push(T::Sym((c as char).to_string()));
            i += 1;
            continue;
        }
        return Err(format!("invalid character at byte {i}: 0x{c:02x}"));
    }
    if nesting != 0 {
        return Err("unclosed parentheses".into());
    }
    if brackets != 0 {
        return Err("unclosed array bracket".into());
    }
    out.push(T::End);
    Ok(out)
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Ty {
    I64,
    Usize,
    Bool,
    Int { bits: u8, signed: bool },
    PendingNumeric(String),
    Struct(String),
    Void,
}
#[derive(Clone, Debug)]
enum Ex {
    Num(i64),
    Bool(bool),
    Var(String),
    Field(String, String),
    Neg(Box<Ex>),
    Bin(Box<Ex>, String, Box<Ex>),
    Call(String, Vec<Ex>),
    Cast(Box<Ex>, Ty),
}
#[derive(Clone, Debug)]
enum St {
    Let(String, Ty, Ex),
    Assign(String, Ex),
    Drop(String),
    Ret(Ex),
    Expr(Ex),
    If(Ex, Vec<St>, Vec<St>),
    While(Ex, Vec<St>),
    Unsafe(Vec<St>),
    Asm(AsmBlock),
}
#[derive(Clone, Debug)]
struct AsmBlock {
    inputs: Vec<(String, Ex)>,
    outputs: Vec<(String, String)>,
    clobbers: Vec<String>,
    instructions: Vec<String>,
}
#[derive(Clone, Debug)]
struct Fun {
    name: String,
    unsafe_fn: bool,
    args: Vec<(String, Ty)>,
    ret: Ty,
    body: Vec<St>,
}
#[derive(Debug)]
struct Ast {
    structs: HashMap<String, Vec<(String, Ty)>>,
    funcs: Vec<Fun>,
}
struct P {
    t: Vec<T>,
    i: usize,
    arrays: HashMap<String, (Ty, usize)>,
    pending: Vec<St>,
    scratch: Vec<St>,
    scratch_id: usize,
}
impl P {
    fn new(t: Vec<T>) -> Self {
        Self {
            t,
            i: 0,
            arrays: HashMap::new(),
            pending: Vec::new(),
            scratch: Vec::new(),
            scratch_id: 0,
        }
    }
    fn cur(&self) -> &T {
        &self.t[self.i]
    }
    fn is(&self, s: &str) -> bool {
        matches!(self.cur(),T::Id(x)|T::Sym(x) if x==s)
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.is(s) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Res<()> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(format!("expected '{s}', got {:?}", self.cur()))
        }
    }
    fn ident(&mut self) -> Res<String> {
        match self.cur().clone() {
            T::Id(x) => {
                if x.starts_with("__mini_arr_") {
                    return Err("reserved internal array name".into());
                }
                self.i += 1;
                Ok(x)
            }
            _ => Err(format!("expected identifier, got {:?}", self.cur())),
        }
    }
    fn sep(&mut self) {
        while self.eat(";") {}
    }
    fn array_slot(&self, name: &str, index: i64) -> Res<String> {
        let Some((_, len)) = self.arrays.get(name) else {
            return Err(format!("unknown array {name}"));
        };
        if index < 0 || index as u64 >= *len as u64 {
            return Err(format!(
                "array index {index} out of bounds for {name} (length {len})"
            ));
        }
        Ok(format!("__mini_arr_{name}_{index}"))
    }
    // A literal or arithmetic constant is resolved at compile time.
    // Do not evaluate variables/functions here: that would be unsound.
    fn constant_index(e: &Ex) -> Option<i64> {
        match e {
            Ex::Num(n) => Some(*n),
            Ex::Neg(a) => Self::constant_index(a)?.checked_neg(),
            Ex::Bin(a, op, b) => {
                let x = Self::constant_index(a)?;
                let y = Self::constant_index(b)?;
                match op.as_str() {
                    "+" => x.checked_add(y),
                    "-" => x.checked_sub(y),
                    "*" => x.checked_mul(y),
                    "/" if y != 0 => x.checked_div(y),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    fn array_index(&mut self, name: &str) -> Res<(Option<String>, Option<Ex>)> {
        let index = self.expr(0)?;
        self.expect("]")?;
        if let Some(i) = Self::constant_index(&index) {
            return Ok((Some(self.array_slot(name, i)?), None));
        }
        let (_, len) = self
            .arrays
            .get(name)
            .ok_or_else(|| format!("unknown array {name}"))?;
        if *len > 16 {
            return Err(format!(
                "dynamic indexing of {name}: at most 16 elements supported in the compact 4.8 lowering"
            ));
        }
        Ok((None, Some(index)))
    }
    fn temp(&mut self, ty: Ty) -> String {
        let n = format!("__mini_arr_tmp_{}", self.scratch_id);
        self.scratch_id += 1;
        let default = if ty == Ty::Bool {
            Ex::Bool(false)
        } else {
            Ex::Num(0)
        };
        self.scratch.push(St::Let(n.clone(), ty, default));
        n
    }
    // A bounded decision chain is a conservative fallback on all backends.
    // The final branch never performs memory access: it traps in a halt loop.
    fn dispatch(&self, array: &str, index: &str, result: &str, write: bool) -> Res<St> {
        let (elem, len) = self
            .arrays
            .get(array)
            .ok_or_else(|| format!("unknown array {array}"))?;
        let _ = elem;
        let mut alternative = vec![St::While(Ex::Bool(true), vec![])];
        for i in (0..*len).rev() {
            let slot = self.array_slot(array, i as i64)?;
            let target = if write {
                slot.clone()
            } else {
                result.to_string()
            };
            let value = if write {
                Ex::Var(result.to_string())
            } else {
                Ex::Var(slot)
            };
            alternative = vec![St::If(
                Ex::Bin(
                    Box::new(Ex::Var(index.to_string())),
                    "==".into(),
                    Box::new(Ex::Num(i as i64)),
                ),
                vec![St::Assign(target, value)],
                alternative,
            )];
        }
        Ok(alternative.remove(0))
    }
    fn dynamic_read(&mut self, name: &str, index: Ex) -> Res<Ex> {
        let ty = self
            .arrays
            .get(name)
            .ok_or_else(|| format!("unknown array {name}"))?
            .0
            .clone();
        let idx = self.temp(Ty::Usize);
        let value = self.temp(ty);
        self.pending.push(St::Assign(idx.clone(), index));
        let guarded = self.dispatch(name, &idx, &value, false)?;
        self.pending.push(guarded);
        Ok(Ex::Var(value))
    }
    fn dynamic_write(&mut self, name: &str, index: Ex, value: Ex) -> Res<()> {
        let ty = self
            .arrays
            .get(name)
            .ok_or_else(|| format!("unknown array {name}"))?
            .0
            .clone();
        let idx = self.temp(Ty::Usize);
        let rhs = self.temp(ty);
        self.pending.push(St::Assign(idx.clone(), index));
        self.pending.push(St::Assign(rhs.clone(), value));
        let guarded = self.dispatch(name, &idx, &rhs, true)?;
        self.pending.push(guarded);
        Ok(())
    }
    fn finish_statement(&mut self, body: &mut Vec<St>, start: usize) -> Res<()> {
        if !self.pending.is_empty() {
            let pending = std::mem::take(&mut self.pending);
            body.splice(start..start, pending);
        }
        Ok(())
    }
    fn array_len(&mut self) -> Res<usize> {
        match self.cur().clone() {
            T::Num(n) if (1..=1024).contains(&n) => {
                self.i += 1;
                Ok(n as usize)
            }
            _ => Err("array length must be a constant in 1..1024".into()),
        }
    }
    fn ty(&mut self) -> Res<Ty> {
        let n = self.ident()?;
        Ok(if n == "i64" {
            Ty::I64
        } else if n == "usize" {
            Ty::Usize
        } else if n == "bool" {
            Ty::Bool
        } else if matches!(
            n.as_str(),
            "u8" | "u16" | "u32" | "u64" | "i8" | "i16" | "i32"
        ) {
            let signed = n.starts_with('i');
            let bits = n[1..].parse::<u8>().map_err(|_| "invalid integer width")?;
            Ty::Int { bits, signed }
        } else if matches!(n.as_str(), "f32" | "f64") {
            Ty::PendingNumeric(n)
        } else {
            Ty::Struct(n)
        })
    }
    fn atom(&mut self) -> Res<Ex> {
        if self.eat("-") {
            return Ok(Ex::Neg(Box::new(self.atom()?)));
        }
        if self.eat("(") {
            let e = self.expr(0)?;
            self.expect(")")?;
            return Ok(e);
        }
        match self.cur().clone() {
            T::Num(n) => {
                self.i += 1;
                Ok(Ex::Num(n))
            }
            T::Id(n) if n == "true" || n == "false" => {
                self.i += 1;
                Ok(Ex::Bool(n == "true"))
            }
            T::Id(n) => {
                self.i += 1;
                if self.eat("[") {
                    let (slot, index) = self.array_index(&n)?;
                    if let Some(slot) = slot {
                        Ok(Ex::Var(slot))
                    } else {
                        self.dynamic_read(&n, index.expect("dynamic index"))
                    }
                } else if self.eat("(") {
                    let mut args = Vec::new();
                    if !self.is(")") {
                        loop {
                            args.push(self.expr(0)?);
                            if !self.eat(",") {
                                break;
                            }
                        }
                    }
                    self.expect(")")?;
                    Ok(Ex::Call(n, args))
                } else if self.eat(".") {
                    let member = self.ident()?;
                    if self.eat("(") {
                        let mut args = vec![Ex::Var(n.clone())];
                        if !self.is(")") {
                            loop {
                                args.push(self.expr(0)?);
                                if !self.eat(",") {
                                    break;
                                }
                            }
                        }
                        self.expect(")")?;
                        Ok(Ex::Call(format!("{n}.{member}"), args))
                    } else {
                        Ok(Ex::Field(n, member))
                    }
                } else {
                    Ok(Ex::Var(n))
                }
            }
            _ => Err(format!("expected expression, got {:?}", self.cur())),
        }
    }
    fn expr(&mut self, min: u8) -> Res<Ex> {
        let mut l = self.atom()?;
        while self.eat("as") {
            let ty = self.ty()?;
            l = Ex::Cast(Box::new(l), ty);
        }
        loop {
            let op = match self.cur() {
                T::Sym(x)
                    if ["+", "-", "*", "/", "<", ">", "==", "!=", "<=", ">="]
                        .contains(&x.as_str()) =>
                {
                    x.clone()
                }
                _ => break,
            };
            let precedence = if op == "*" || op == "/" {
                3
            } else if op == "+" || op == "-" {
                2
            } else {
                1
            };
            if precedence < min {
                break;
            }
            self.i += 1;
            let r = self.expr(precedence + 1)?;
            l = Ex::Bin(Box::new(l), op, Box::new(r));
        }
        Ok(l)
    }
    fn body(&mut self) -> Res<Vec<St>> {
        self.expect("{")?;
        let mut body = Vec::new();
        self.sep();
        while !self.eat("}") {
            if *self.cur() == T::End {
                return Err("unclosed function body".into());
            }
            let statement_start = body.len();
            if self.eat("let") {
                let n = self.ident()?;
                self.expect(":")?;
                if self.eat("[") {
                    if self.arrays.contains_key(&n) {
                        return Err(format!("duplicate array {n}"));
                    }
                    let ty = self.ty()?;
                    if !matches!(ty, Ty::I64 | Ty::Usize | Ty::Bool | Ty::Int { .. }) {
                        return Err("arrays require a scalar integer or bool element type".into());
                    }
                    self.expect(";")?;
                    let len = self.array_len()?;
                    self.expect("]")?;
                    self.expect("=")?;
                    self.expect("[")?;
                    let value = self.expr(0)?;
                    self.expect(";")?;
                    let init_len = self.array_len()?;
                    self.expect("]")?;
                    if init_len != len {
                        return Err(format!(
                            "array {n}: declared length {len}, initialized length {init_len}"
                        ));
                    }
                    if !matches!(&value, Ex::Num(_) | Ex::Bool(_))
                        && !matches!(&value,Ex::Neg(inner) if matches!(inner.as_ref(),Ex::Num(_)))
                    {
                        return Err("array repeat initializer must be a constant; expressions with side effects are not supported".into());
                    }
                    self.arrays.insert(n.clone(), (ty.clone(), len));
                    for i in 0..len {
                        body.push(St::Let(
                            format!("__mini_arr_{n}_{i}"),
                            ty.clone(),
                            value.clone(),
                        ));
                    }
                } else {
                    if self.arrays.contains_key(&n) {
                        return Err(format!("duplicate variable/array {n}"));
                    }
                    let ty = self.ty()?;
                    self.expect("=")?;
                    let e = self.expr(0)?;
                    body.push(St::Let(n, ty, e));
                }
            } else if self.eat("drop") {
                body.push(St::Drop(self.ident()?));
            } else if matches!(self.cur(), T::Id(_))
                && self.t.get(self.i + 1) == Some(&T::Sym("[".into()))
            {
                let name = self.ident()?;
                self.expect("[")?;
                let (slot, index) = self.array_index(&name)?;
                self.expect("=")?;
                let value = self.expr(0)?;
                if let Some(slot) = slot {
                    body.push(St::Assign(slot, value));
                } else {
                    self.dynamic_write(&name, index.expect("dynamic index"), value)?;
                }
            } else if matches!(self.cur(), T::Id(_))
                && self.t.get(self.i + 1) == Some(&T::Sym("=".into()))
            {
                let name = self.ident()?;
                self.expect("=")?;
                let value = self.expr(0)?;
                body.push(St::Assign(name, value));
            } else if self.eat("return") {
                body.push(St::Ret(self.expr(0)?));
            } else if self.eat("if") {
                let cond = self.expr(0)?;
                let condition_checks = std::mem::take(&mut self.pending);
                self.sep();
                let yes = self.body()?;
                self.sep();
                let no = if self.eat("else") {
                    self.sep();
                    self.body()?
                } else {
                    Vec::new()
                };
                body.extend(condition_checks);
                body.push(St::If(cond, yes, no));
            } else if self.eat("while") {
                let cond = self.expr(0)?;
                if !self.pending.is_empty() {
                    return Err("dynamic array access in while condition is not yet supported; use a previously computed scalar".into());
                }
                self.sep();
                let loop_body = self.body()?;
                body.push(St::While(cond, loop_body));
            } else if self.eat("unsafe") {
                self.sep();
                body.push(St::Unsafe(self.body()?));
            } else if self.eat("asm") {
                self.expect("{")?;
                let mut a = AsmBlock {
                    inputs: vec![],
                    outputs: vec![],
                    clobbers: vec![],
                    instructions: vec![],
                };
                self.sep();
                while !self.eat("}") {
                    if *self.cur() == T::End {
                        return Err("unclosed asm block".into());
                    }
                    let item = self.ident()?;
                    match item.as_str() {
                        "in" => {
                            let reg = self.ident()?;
                            self.expect("=")?;
                            let value = self.expr(0)?;
                            a.inputs.push((reg, value));
                        }
                        "out" => {
                            let variable = self.ident()?;
                            self.expect("=")?;
                            let reg = self.ident()?;
                            a.outputs.push((variable, reg));
                        }
                        "clobber" => a.clobbers.push(self.ident()?),
                        "nop" | "pause" | "lfence" | "sfence" | "mfence" | "syscall" | "svc"
                        | "yield" | "dmb" | "isb" | "sei" | "cli" | "ldr" | "str" => {
                            a.instructions.push(item)
                        }
                        "sta" => {
                            self.expect("(")?;
                            let address = match self.cur().clone() {
                                T::Num(v) if (0..=65535).contains(&v) => {
                                    self.i += 1;
                                    v as u16
                                }
                                _ => return Err("sta address must be a literal 0..65535".into()),
                            };
                            self.expect(")")?;
                            a.instructions.push(format!("sta:{address}"));
                        }
                        "outio" | "sts" => {
                            self.expect("(")?;
                            let address = match self.cur().clone() {
                                T::Num(v) if (0..=65535).contains(&v) => {
                                    self.i += 1;
                                    v as u16
                                }
                                _ => {
                                    return Err(
                                        "outio/sts address must be a literal 0..65535".into()
                                    );
                                }
                            };
                            self.expect(",")?;
                            let reg = self.ident()?;
                            self.expect(")")?;
                            a.instructions.push(format!("{item}:{address}:{reg}"));
                        }
                        _ => {
                            return Err(format!(
                                "unsupported assembly instruction/directive: {item}"
                            ));
                        }
                    }
                    if !self.is("}") && !self.is(";") {
                        return Err("expected separator in asm".into());
                    }
                    self.sep();
                }
                body.push(St::Asm(a));
            } else {
                body.push(St::Expr(self.expr(0)?));
            }
            self.finish_statement(&mut body, statement_start)?;
            if !self.is("}") && !self.is(";") {
                return Err(format!(
                    "expected statement separator, got {:?}",
                    self.cur()
                ));
            }
            self.sep();
        }
        Ok(body)
    }
    fn finish_function(&mut self, mut body: Vec<St>) -> Res<Vec<St>> {
        if !self.pending.is_empty() {
            return Err("unflushed array access".into());
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.append(&mut body);
        Ok(scratch)
    }
    fn parse(mut self) -> Res<Ast> {
        let mut structs = HashMap::new();
        let mut funcs = Vec::new();
        let mut main = false;
        self.sep();
        while *self.cur() != T::End {
            if self.eat("struct") {
                let n = self.ident()?;
                if [
                    "i64", "usize", "bool", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "f32",
                    "f64", "__main",
                ]
                .contains(&n.as_str())
                {
                    return Err("reserved type name".into());
                }
                self.expect("{")?;
                let mut fields = Vec::new();
                self.sep();
                while !self.eat("}") {
                    let field = self.ident()?;
                    self.expect(":")?;
                    let fty = self.ty()?;
                    if !matches!(fty, Ty::I64 | Ty::Usize | Ty::Bool) {
                        return Err(
                            "struct fields must be i64, usize or bool in this minimal core".into(),
                        );
                    }
                    if fields.iter().any(|(n, _)| n == &field) {
                        return Err(format!("duplicate field {n}.{field}"));
                    }
                    fields.push((field, fty));
                    if !self.is("}") && !self.is(";") {
                        return Err("expected separator in struct".into());
                    }
                    self.sep();
                }
                if fields.is_empty() || fields.len() > 32 {
                    return Err("struct needs 1..32 scalar fields".into());
                }
                if structs.insert(n.clone(), fields).is_some() {
                    return Err(format!("duplicate struct {n}"));
                }
            } else if self.is("fn") || self.is("unsafe") {
                self.arrays.clear();
                self.scratch.clear();
                self.scratch_id = 0;
                let unsafe_fn = self.eat("unsafe");
                self.expect("fn")?;
                let n = self.ident()?;
                if ["__main", "raw_addr"].contains(&n.as_str()) {
                    return Err("reserved function name".into());
                }
                self.expect("(")?;
                let mut args = Vec::new();
                if !self.is(")") {
                    loop {
                        let a = self.ident()?;
                        self.expect(":")?;
                        let ty = self.ty()?;
                        args.push((a, ty));
                        if !self.eat(",") {
                            break;
                        }
                    }
                }
                self.expect(")")?;
                let ret = if self.eat("->") { self.ty()? } else { Ty::Void };
                self.sep();
                let parsed = self.body()?;
                let body = self.finish_function(parsed)?;
                funcs.push(Fun {
                    name: n,
                    unsafe_fn,
                    args,
                    ret,
                    body,
                });
            } else if self.eat("program") {
                self.arrays.clear();
                self.scratch.clear();
                self.scratch_id = 0;
                if main {
                    return Err("duplicate program".into());
                }
                main = true;
                self.sep();
                funcs.push(Fun {
                    name: "__main".into(),
                    unsafe_fn: false,
                    args: vec![],
                    ret: Ty::I64,
                    body: {
                        let parsed = self.body()?;
                        self.finish_function(parsed)?
                    },
                });
            } else {
                return Err(format!(
                    "expected struct, fn or program, got {:?}",
                    self.cur()
                ));
            }
            self.sep();
        }
        if !main {
            return Err("missing program block".into());
        }
        Ok(Ast { structs, funcs })
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    off: i32,
    size: i32,
}
#[derive(Clone)]
struct Frame {
    vars: HashMap<String, (Slot, Ty)>,
    size: i32,
    sret: Option<Slot>,
}
fn ty_size(t: &Ty, ast: &Ast) -> Res<i32> {
    match t {
        Ty::I64 | Ty::Usize | Ty::Bool | Ty::Int { .. } => Ok(8),
        Ty::PendingNumeric(n) => Err(format!(
            "numeric type {n} is reserved but its code generator is not implemented yet"
        )),
        Ty::Struct(n) => Ok(ast
            .structs
            .get(n)
            .ok_or_else(|| format!("unknown struct {n}"))?
            .len() as i32
            * 8),
        Ty::Void => Err("void is not a storage type".into()),
    }
}
// Only nonnegative integer literals may be used directly as usize. No implicit
// signed-variable conversion: this prevents accidental negative lengths.
fn scalar_integer(ty: &Ty) -> bool {
    matches!(ty, Ty::I64 | Ty::Usize | Ty::Int { .. })
}
fn integer_bounds(ty: &Ty) -> Option<(i128, i128)> {
    match ty {
        Ty::Int { bits, signed: true } => {
            Some((-(1i128 << (*bits - 1)), (1i128 << (*bits - 1)) - 1))
        }
        Ty::Int {
            bits,
            signed: false,
        } => Some((0, (1i128 << *bits) - 1)),
        Ty::I64 => Some((i64::MIN as i128, i64::MAX as i128)),
        Ty::Usize => Some((0, u64::MAX as i128)),
        _ => None,
    }
}
fn fits_literal(e: &Ex, expected: &Ty) -> bool {
    let Some((lo, hi)) = integer_bounds(expected) else {
        return false;
    };
    match e {
        Ex::Num(n) => (*n as i128) >= lo && (*n as i128) <= hi,
        Ex::Neg(x) => matches!(x.as_ref(),Ex::Num(n) if -(*n as i128)>=lo && -(*n as i128)<=hi),
        _ => false,
    }
}
fn fits_expected(e: &Ex, expected: &Ty, actual: &Ty) -> bool {
    actual == expected
        || (scalar_integer(expected) && actual == &Ty::I64 && fits_literal(e, expected))
}
fn type_expr(
    e: &Ex,
    vars: &HashMap<String, Ty>,
    ast: &Ast,
    fs: &HashMap<String, usize>,
) -> Res<Ty> {
    match e {
        Ex::Num(_) => Ok(Ty::I64),
        Ex::Bool(_) => Ok(Ty::Bool),
        Ex::Var(n) => vars
            .get(n)
            .cloned()
            .ok_or_else(|| format!("unknown variable {n}")),
        Ex::Field(n, f) => {
            let ty = vars.get(n).ok_or_else(|| format!("unknown variable {n}"))?;
            let Ty::Struct(s) = ty else {
                return Err("field access requires struct".into());
            };
            ast.structs[s]
                .iter()
                .find(|(name, _)| name == f)
                .map(|(_, ty)| ty.clone())
                .ok_or_else(|| format!("unknown field {s}.{f}"))
        }
        Ex::Neg(a) => {
            let t = type_expr(a, vars, ast, fs)?;
            if !matches!(t, Ty::I64 | Ty::Int { signed: true, .. }) {
                return Err("unary '-' requires a signed integer".into());
            }
            Ok(t)
        }
        Ex::Cast(a, dest) => {
            let from = type_expr(a, vars, ast, fs)?;
            if !scalar_integer(&from) || !scalar_integer(dest) {
                return Err("'as' currently supports integer-to-integer conversion only".into());
            }
            Ok(dest.clone())
        }
        Ex::Bin(a, op, b) => {
            let lhs = type_expr(a, vars, ast, fs)?;
            let rhs = type_expr(b, vars, ast, fs)?;
            let result = if lhs == rhs {
                lhs.clone()
            } else if scalar_integer(&lhs) && fits_literal(b, &lhs) {
                lhs.clone()
            } else if scalar_integer(&rhs) && fits_literal(a, &rhs) {
                rhs.clone()
            } else {
                return Err("arithmetic/comparison requires matching integer types (or fitting integer literals)".into());
            };
            if !scalar_integer(&result) {
                return Err("arithmetic/comparison requires integer operands".into());
            }
            Ok(
                if ["<", ">", "==", "!=", "<=", ">="].contains(&op.as_str()) {
                    Ty::Bool
                } else {
                    result
                },
            )
        }
        Ex::Call(n, args) => {
            if n.starts_with("__drop_") {
                return Err(format!(
                    "destructor {n} cannot be called directly; use 'drop variable'"
                ));
            }
            if n == "raw_addr" {
                if args.len() != 1 {
                    return Err("raw_addr requires one argument".into());
                }
                let Ex::Var(v) = &args[0] else {
                    return Err("raw_addr requires a local variable".into());
                };
                if !matches!(vars.get(v), Some(Ty::I64 | Ty::Usize)) {
                    return Err("raw_addr requires an i64 or usize variable".into());
                }
                return Ok(Ty::I64);
            }
            if let Some(fields) = ast.structs.get(n) {
                if args.len() != fields.len() {
                    return Err(format!(
                        "{n}: expected {} constructor arguments",
                        fields.len()
                    ));
                }
                for (a, (_, field_ty)) in args.iter().zip(fields) {
                    if !fits_expected(a, field_ty, &type_expr(a, vars, ast, fs)?) {
                        return Err(format!("{n}: constructor field type mismatch"));
                    }
                }
                return Ok(Ty::Struct(n.clone()));
            }
            let Some(idx) = fs.get(n) else {
                return Err(format!("unknown function {n}"));
            };
            let f = &ast.funcs[*idx];
            if args.len() != f.args.len() {
                return Err(format!("{n}: expected {} arguments", f.args.len()));
            }
            for (a, (_, ty)) in args.iter().zip(&f.args) {
                if !fits_expected(a, ty, &type_expr(a, vars, ast, fs)?) {
                    return Err(format!("{n}: argument type mismatch"));
                }
                if matches!(ty, Ty::Struct(_)) && !matches!(a, Ex::Var(_)) {
                    return Err(format!(
                        "{n}: aggregate parameter must be an existing variable"
                    ));
                }
            }
            Ok(f.ret.clone())
        }
    }
}
fn referenced_vars(e: &Ex) -> Vec<String> {
    let mut v = Vec::new();
    fn visit(e: &Ex, v: &mut Vec<String>) {
        match e {
            Ex::Var(n) | Ex::Field(n, _) => v.push(n.clone()),
            Ex::Neg(a) | Ex::Cast(a, _) => visit(a, v),
            Ex::Bin(a, _, b) => {
                visit(a, v);
                visit(b, v)
            }
            Ex::Call(_, args) => {
                for a in args {
                    visit(a, v)
                }
            }
            Ex::Num(_) | Ex::Bool(_) => {}
        }
    }
    visit(e, &mut v);
    v
}
fn validate_unsafe_expr(e: &Ex, inside: bool, ast: &Ast, fs: &HashMap<String, usize>) -> Res<()> {
    match e {
        Ex::Call(n, args) => {
            // A type with a destructor can own an external resource. Directly
            // fabricating one from arbitrary scalars is therefore unsafe;
            // safe constructors must establish the resource invariant first.
            if ast.structs.contains_key(n) && fs.contains_key(&format!("__drop_{n}")) && !inside {
                return Err(format!(
                    "construction of owning {n} requires unsafe; call a safe validated factory instead"
                ));
            }
            if n == "raw_addr" && !inside {
                return Err(format!("{n} requires an unsafe block"));
            }
            if let Some(&idx) = fs.get(n) {
                if ast.funcs[idx].unsafe_fn && !inside {
                    return Err(format!("call to unsafe fn {n} requires unsafe block"));
                }
            }
            for a in args {
                validate_unsafe_expr(a, inside, ast, fs)?;
            }
        }
        Ex::Neg(a) | Ex::Cast(a, _) => validate_unsafe_expr(a, inside, ast, fs)?,
        Ex::Bin(a, _, b) => {
            validate_unsafe_expr(a, inside, ast, fs)?;
            validate_unsafe_expr(b, inside, ast, fs)?;
        }
        _ => {}
    }
    Ok(())
}
fn check_asm(
    a: &AsmBlock,
    vars: &HashMap<String, Ty>,
    dead: &HashSet<String>,
    ast: &Ast,
    fs: &HashMap<String, usize>,
    target: target::Target,
) -> Res<()> {
    if matches!(target.arch, target::Arch::Avr8 | target::Arch::Mos6510) {
        return tiny8::check_asm(a, vars, dead, ast, fs, target);
    }
    if target.arch == target::Arch::Thumbv7m {
        return backend_thumb::check_asm(a, vars, dead, ast, fs);
    }
    if target.arch == target::Arch::Aarch64 {
        return check_asm_a64(a, vars, dead, ast, fs);
    }
    if target.arch == target::Arch::Armv7 {
        return check_asm_arm32(a, vars, dead, ast, fs);
    }
    if a.instructions.iter().any(|s| {
        !matches!(
            s.as_str(),
            "nop" | "pause" | "lfence" | "sfence" | "mfence" | "syscall"
        )
    }) {
        return Err("unsupported x86_64 assembly instruction".into());
    }
    // These registers form the Linux x86-64 syscall input ABI; preserve all
    // other compiler state. No arbitrary clobbers / control flow are accepted.
    const INPUTS: &[&str] = &["rax", "rdi", "rsi", "rdx", "r10", "r8", "r9"];
    if a.instructions.is_empty() || a.instructions.len() > 64 {
        return Err("asm requires 1..64 instructions".into());
    }
    let mut seen = HashSet::new();
    for (reg, e) in &a.inputs {
        if !INPUTS.contains(&reg.as_str()) {
            return Err(format!("unsupported asm input register: {reg}"));
        }
        if !seen.insert(reg.as_str()) {
            return Err(format!("duplicate asm input register: {reg}"));
        }
        if !matches!(type_expr(e, vars, ast, fs)?, Ty::I64 | Ty::Usize) {
            return Err("asm input requires i64 or usize".into());
        }
        validate_unsafe_expr(e, true, ast, fs)?;
        for v in referenced_vars(e) {
            if dead.contains(&v) {
                return Err(format!("use after drop: {v}"));
            }
        }
    }
    if a.outputs.len() > 1 {
        return Err("at most one asm output is supported".into());
    }
    for (name, reg) in &a.outputs {
        if reg != "rax" {
            return Err("asm output currently supports only rax".into());
        }
        if !matches!(vars.get(name), Some(Ty::I64 | Ty::Usize)) {
            return Err(format!(
                "asm output {name} must be an existing i64 or usize local"
            ));
        }
        if dead.contains(name) {
            return Err(format!("asm output variable {name} was dropped"));
        }
    }
    let allowed = ["rcx", "r11", "memory"];
    let mut unique = HashSet::new();
    for c in &a.clobbers {
        if !allowed.contains(&c.as_str()) {
            return Err(format!("unsupported clobber: {c}"));
        }
        if !unique.insert(c.as_str()) {
            return Err(format!("duplicate clobber: {c}"));
        }
    }
    let syscall_count = a
        .instructions
        .iter()
        .filter(|x| x.as_str() == "syscall")
        .count();
    if syscall_count > 1 {
        return Err("only one syscall per asm block".into());
    }
    if syscall_count == 1 {
        for c in allowed {
            if !unique.contains(c) {
                return Err(format!("syscall requires clobber {c}"));
            }
        }
        if !seen.contains("rax") {
            return Err("syscall requires in rax = number".into());
        }
        // No instruction may follow the syscall apart from fences/nop; no
        // unsupported register writes are possible in this restricted dialect.
    } else if !a.clobbers.is_empty() || !a.outputs.is_empty() || !a.inputs.is_empty() {
        return Err("register bindings and clobbers require syscall in this asm subset".into());
    }
    Ok(())
}
// Linux ARMv7 EABI: r7 = syscall number, r0..r6 = arguments,
// r0 = signed 32-bit return. This is NOT an arbitrary ARM inline assembler.
fn check_asm_arm32(
    a: &AsmBlock,
    vars: &HashMap<String, Ty>,
    dead: &HashSet<String>,
    ast: &Ast,
    fs: &HashMap<String, usize>,
) -> Res<()> {
    const INPUTS: &[&str] = &["r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7"];
    if a.instructions.is_empty() || a.instructions.len() > 64 {
        return Err("ARM32 asm requires 1..64 instructions".into());
    }
    let mut seen = HashSet::new();
    for (reg, e) in &a.inputs {
        if !INPUTS.contains(&reg.as_str()) {
            return Err(format!("invalid ARM32 asm input: {reg}"));
        }
        if !seen.insert(reg.as_str()) {
            return Err(format!("duplicate ARM32 asm input: {reg}"));
        }
        if !matches!(type_expr(e, vars, ast, fs)?, Ty::I64 | Ty::Usize) {
            return Err("ARM32 asm input must be i64/usize".into());
        }
        validate_unsafe_expr(e, true, ast, fs)?;
        for v in referenced_vars(e) {
            if dead.contains(&v) {
                return Err(format!("use after drop: {v}"));
            }
        }
    }
    if a.outputs.len() > 1 {
        return Err("only one ARM32 asm output is supported".into());
    }
    for (var, reg) in &a.outputs {
        if reg != "r0" {
            return Err("ARM32 asm output must use r0".into());
        }
        if !matches!(vars.get(var), Some(Ty::I64 | Ty::Usize)) {
            return Err(format!("invalid ARM32 asm output local {var}"));
        }
        if dead.contains(var) {
            return Err(format!("asm output after drop: {var}"));
        }
    }
    if a.clobbers.len() > 1 || a.clobbers.iter().any(|c| c != "memory") {
        return Err("ARM32 asm supports only clobber memory".into());
    }
    if a.instructions
        .iter()
        .any(|x| !matches!(x.as_str(), "svc" | "nop" | "dmb"))
    {
        return Err("unsupported ARM32 asm opcode".into());
    }
    let svc_count = a
        .instructions
        .iter()
        .filter(|x| x.as_str() == "svc")
        .count();
    if svc_count > 1 {
        return Err("ARM32 asm supports only one svc".into());
    }
    if svc_count == 1 {
        if !seen.contains("r7") {
            return Err("ARM32 svc requires in r7 = syscall_number".into());
        }
        if !a.clobbers.iter().any(|c| c == "memory") {
            return Err("ARM32 svc requires clobber memory".into());
        }
    } else if !a.inputs.is_empty() || !a.outputs.is_empty() || !a.clobbers.is_empty() {
        return Err("ARM32 asm register operands require svc".into());
    }
    Ok(())
}
// AArch64 Linux syscall boundary: x8 is syscall number, x0..x5 arguments,
// x0 is the result. Library wrapper MUST be unsafe if it accepts raw pointers.
fn check_asm_a64(
    a: &AsmBlock,
    vars: &HashMap<String, Ty>,
    dead: &HashSet<String>,
    ast: &Ast,
    fs: &HashMap<String, usize>,
) -> Res<()> {
    const INPUTS: &[&str] = &["x0", "x1", "x2", "x3", "x4", "x5", "x8"];
    if a.instructions.is_empty() || a.instructions.len() > 64 {
        return Err("asm requires 1..64 instructions".into());
    }
    let mut seen = HashSet::new();
    for (reg, e) in &a.inputs {
        if !INPUTS.contains(&reg.as_str()) {
            return Err(format!("unsupported AArch64 input: {reg}"));
        }
        if !seen.insert(reg.as_str()) {
            return Err(format!("duplicate AArch64 input: {reg}"));
        }
        if !matches!(type_expr(e, vars, ast, fs)?, Ty::I64 | Ty::Usize) {
            return Err("asm input must be i64/usize".into());
        }
        validate_unsafe_expr(e, true, ast, fs)?;
        for v in referenced_vars(e) {
            if dead.contains(&v) {
                return Err(format!("use after drop: {v}"));
            }
        }
    }
    if a.outputs.len() > 1 {
        return Err("only one AArch64 asm output supported".into());
    }
    for (var, reg) in &a.outputs {
        if reg != "x0" {
            return Err("AArch64 asm result must come from x0".into());
        }
        if !matches!(vars.get(var), Some(Ty::I64 | Ty::Usize)) {
            return Err(format!("asm result {var} needs existing scalar local"));
        }
        if dead.contains(var) {
            return Err(format!("asm result {var} was dropped"));
        }
    }
    let mut unique = HashSet::new();
    for c in &a.clobbers {
        if c != "memory" || !unique.insert(c.as_str()) {
            return Err(format!("unsupported/duplicate AArch64 clobber: {c}"));
        }
    }
    for name in &a.instructions {
        if !matches!(name.as_str(), "svc" | "nop" | "yield" | "dmb" | "isb") {
            return Err(format!("unsupported AArch64 instruction: {name}"));
        }
    }
    let svc_count = a
        .instructions
        .iter()
        .filter(|s| s.as_str() == "svc")
        .count();
    if svc_count > 1 {
        return Err("only one svc per asm block".into());
    }
    if svc_count == 1 {
        if !seen.contains("x8") {
            return Err("svc requires in x8 = syscall_number".into());
        }
        if !unique.contains("memory") {
            return Err("svc requires clobber memory".into());
        }
    } else if !a.inputs.is_empty() || !a.outputs.is_empty() || !a.clobbers.is_empty() {
        return Err("AArch64 register operands are supported only for svc".into());
    }
    Ok(())
}
// Branch/loop-local storage needs lexical-scope destruction and conditional
// initialization flags. Until those rules are implemented, forbid it, even
// when a let is hidden inside an unsafe nested block.
fn contains_nested_let(body: &[St]) -> bool {
    body.iter().any(|s| match s {
        St::Let(_, _, _) => true,
        St::Unsafe(children) | St::While(_, children) => contains_nested_let(children),
        St::If(_, yes, no) => contains_nested_let(yes) || contains_nested_let(no),
        _ => false,
    })
}
// Conservative flow-sensitive validation: different ownership states after if are
// rejected, and while may not change ownership state across iterations.
fn check_block(
    stmts: &[St],
    vars: &mut HashMap<String, Ty>,
    slots: &mut HashMap<String, (Slot, Ty)>,
    off: &mut i32,
    dead: &mut HashSet<String>,
    borrowed: &HashSet<String>,
    ret: &Ty,
    ast: &Ast,
    fs: &HashMap<String, usize>,
    unsafe_ctx: bool,
    target: target::Target,
) -> Res<bool> {
    let mut returned = false;
    for st in stmts {
        if returned {
            return Err("unreachable statement after return".into());
        }
        match st {
            St::Asm(a) => {
                if !unsafe_ctx {
                    return Err("asm requires unsafe block".into());
                }
                check_asm(a, vars, dead, ast, fs, target)?;
            }
            St::Unsafe(body) => {
                // Nested unsafe scopes do not create new ownership rules.
                let r = check_block(
                    body, vars, slots, off, dead, borrowed, ret, ast, fs, true, target,
                )?;
                if r {
                    returned = true;
                }
            }
            St::Let(n, t, e) => {
                validate_unsafe_expr(e, unsafe_ctx, ast, fs)?;
                for used in referenced_vars(e) {
                    if dead.contains(&used) {
                        return Err(format!("use after drop: {used}"));
                    }
                }
                if vars.contains_key(n) {
                    return Err(format!("duplicate local {n}"));
                }
                let size = ty_size(t, ast)?;
                let got = type_expr(e, vars, ast, fs)?;
                if !fits_expected(e, t, &got) {
                    return Err(format!("let {n}: expected {t:?}, got {got:?}"));
                }
                if matches!(t, Ty::Struct(_)) && matches!(e, Ex::Var(_)) {
                    return Err("implicit owned aggregate copying is forbidden".into());
                }
                *off += size;
                slots.insert(n.clone(), (Slot { off: *off, size }, t.clone()));
                vars.insert(n.clone(), t.clone());
            }
            St::Assign(n, e) => {
                let ty = vars
                    .get(n)
                    .ok_or_else(|| format!("assignment to unknown local: {n}"))?;
                if dead.contains(n) {
                    return Err(format!("assignment after drop: {n}"));
                }
                if borrowed.contains(n) {
                    return Err(format!("assignment to borrowed parameter: {n}"));
                }
                if matches!(ty, Ty::Struct(_)) {
                    return Err("aggregate reassignment is not yet implemented safely".into());
                }
                validate_unsafe_expr(e, unsafe_ctx, ast, fs)?;
                for used in referenced_vars(e) {
                    if dead.contains(&used) {
                        return Err(format!("use after drop: {used}"));
                    }
                }
                let got = type_expr(e, vars, ast, fs)?;
                if !fits_expected(e, ty, &got) {
                    return Err(format!("assignment to {n}: expected {ty:?}, got {got:?}"));
                }
            }
            St::Drop(n) => {
                let ty = vars
                    .get(n)
                    .ok_or_else(|| format!("unknown drop variable: {n}"))?;
                if !matches!(ty, Ty::Struct(_)) {
                    return Err("drop requires an aggregate".into());
                }
                if borrowed.contains(n) {
                    return Err(format!("cannot drop borrowed parameter: {n}"));
                }
                if !dead.insert(n.clone()) {
                    return Err(format!("double drop: {n}"));
                }
            }
            St::Ret(e) => {
                validate_unsafe_expr(e, unsafe_ctx, ast, fs)?;
                for used in referenced_vars(e) {
                    if dead.contains(&used) {
                        return Err(format!("use after drop: {used}"));
                    }
                }
                let got = type_expr(e, vars, ast, fs)?;
                if !fits_expected(e, ret, &got) {
                    return Err(format!("return type mismatch: {got:?} vs {ret:?}"));
                }
                if matches!(ret, Ty::Struct(_)) && !matches!(e, Ex::Call(_, _)) {
                    return Err("aggregate return must be a constructor or call".into());
                }
                returned = true;
            }
            St::Expr(e) => {
                validate_unsafe_expr(e, unsafe_ctx, ast, fs)?;
                for used in referenced_vars(e) {
                    if dead.contains(&used) {
                        return Err(format!("use after drop: {used}"));
                    }
                }
                type_expr(e, vars, ast, fs)?;
            }
            St::If(cond, yes, no) => {
                validate_unsafe_expr(cond, unsafe_ctx, ast, fs)?;
                for used in referenced_vars(cond) {
                    if dead.contains(&used) {
                        return Err(format!("use after drop: {used}"));
                    }
                }
                if type_expr(cond, vars, ast, fs)? != Ty::Bool {
                    return Err("if requires bool".into());
                }
                if contains_nested_let(yes) || contains_nested_let(no) {
                    return Err("branch-local let is not supported safely yet".into());
                }
                let mut a = dead.clone();
                let mut b = dead.clone();
                let mut av = vars.clone();
                let mut bv = vars.clone();
                let mut aslots = slots.clone();
                let mut bslots = slots.clone();
                let mut ao = *off;
                let mut bo = *off;
                let ar = check_block(
                    yes,
                    &mut av,
                    &mut aslots,
                    &mut ao,
                    &mut a,
                    borrowed,
                    ret,
                    ast,
                    fs,
                    unsafe_ctx,
                    target,
                )?;
                let br = check_block(
                    no,
                    &mut bv,
                    &mut bslots,
                    &mut bo,
                    &mut b,
                    borrowed,
                    ret,
                    ast,
                    fs,
                    unsafe_ctx,
                    target,
                )?;
                // Branch-local declarations cannot escape; slots are still reserved in the frame.
                for (k, v) in aslots {
                    if !slots.contains_key(&k) {
                        slots.insert(k, v);
                    }
                }
                for (k, (mut sl, t)) in bslots {
                    if !slots.contains_key(&k) {
                        if sl.off <= ao {
                            sl.off += ao - *off;
                        }
                        slots.insert(k, (sl, t));
                    }
                }
                *off = ao.max(bo + ao - *off);
                if !ar && !br && a != b {
                    return Err("conditional drop must agree on both branches".into());
                }
                if ar && !br {
                    *dead = b;
                } else if !ar && br {
                    *dead = a;
                } else if !ar && !br {
                    *dead = a;
                }
                returned = ar && br;
            }
            St::While(cond, body) => {
                validate_unsafe_expr(cond, unsafe_ctx, ast, fs)?;
                for used in referenced_vars(cond) {
                    if dead.contains(&used) {
                        return Err(format!("use after drop: {used}"));
                    }
                }
                if type_expr(cond, vars, ast, fs)? != Ty::Bool {
                    return Err("while requires bool".into());
                }
                if contains_nested_let(body) {
                    return Err("loop-local let is not supported safely yet".into());
                }
                let mut inner = dead.clone();
                let mut iv = vars.clone();
                let mut islots = slots.clone();
                let mut io = *off;
                let _ = check_block(
                    body,
                    &mut iv,
                    &mut islots,
                    &mut io,
                    &mut inner,
                    borrowed,
                    ret,
                    ast,
                    fs,
                    unsafe_ctx,
                    target,
                )?;
                if inner != *dead {
                    return Err("drop in while is not supported safely".into());
                }
                for (k, v) in islots {
                    if !slots.contains_key(&k) {
                        slots.insert(k, v);
                    }
                }
                *off = (*off).max(io);
            }
        }
    }
    Ok(returned)
}
fn check(ast: &Ast) -> Res<(HashMap<String, usize>, HashMap<String, Frame>)> {
    check_for(ast, target::Target::parse("x86_64-linux").unwrap())
}
fn check_for(
    ast: &Ast,
    target: target::Target,
) -> Res<(HashMap<String, usize>, HashMap<String, Frame>)> {
    let mut fs = HashMap::new();
    for (i, f) in ast.funcs.iter().enumerate() {
        if fs.insert(f.name.clone(), i).is_some() {
            return Err(format!("duplicate function {}", f.name));
        }
    }
    for (n, fields) in &ast.structs {
        if fs.contains_key(n) {
            return Err(format!("struct/function collision: {n}"));
        }
        if fields.is_empty() {
            return Err(format!("empty struct {n}"));
        }
    }
    // A destructor is an ordinary Mini function whose name follows a reserved hook
    // convention. Its struct argument is BORROWED; callers cannot invoke it
    // directly. The compiler inserts exactly one call per live owning local.
    for f in &ast.funcs {
        if let Some(ty_name) = f.name.strip_prefix("__drop_") {
            if !ast.structs.contains_key(ty_name)
                || f.args.len() != 1
                || f.args[0].1 != Ty::Struct(ty_name.to_string())
                || f.ret != Ty::Void
                || f.unsafe_fn
            {
                return Err(format!(
                    "invalid destructor {}: expected safe fn {}(value: {}) with no return type",
                    f.name, f.name, ty_name
                ));
            }
        }
    }
    let mut frames = HashMap::new();
    for f in &ast.funcs {
        if f.args.len() > 64 {
            return Err("too many arguments".into());
        }
        if f.ret != Ty::Void {
            ty_size(&f.ret, ast)?;
        }
        let mut vars = HashMap::new();
        let mut slots = HashMap::new();
        let mut off = 0i32;
        let sret = if matches!(f.ret, Ty::Struct(_)) {
            off += 8;
            Some(Slot { off, size: 8 })
        } else {
            None
        };
        for (n, t) in &f.args {
            if vars.insert(n.clone(), t.clone()).is_some() {
                return Err(format!("duplicate parameter {n}"));
            }
            off += if matches!(t, Ty::Struct(_)) {
                8
            } else {
                ty_size(t, ast)?
            };
            slots.insert(
                n.clone(),
                (
                    Slot {
                        off,
                        size: if matches!(t, Ty::Struct(_)) { 0 } else { 8 },
                    },
                    t.clone(),
                ),
            );
        }
        // Linear ownership checker on the typed AST. Branch states must agree.
        // Only scalar structs exist in this release; heap/fd ownership is intentionally rejected.
        let args_names: HashSet<String> = f.args.iter().map(|(n, _)| n.clone()).collect();
        let mut dead = HashSet::new();
        let returns = check_block(
            &f.body,
            &mut vars,
            &mut slots,
            &mut off,
            &mut dead,
            &args_names,
            &f.ret,
            ast,
            &fs,
            false,
            target,
        )?;
        if !returns && f.ret != Ty::Void {
            return Err(format!("{}: missing return", f.name));
        }
        if off > 1_000_000 {
            return Err("stack frame exceeds 1 MiB".into());
        }
        frames.insert(
            f.name.clone(),
            Frame {
                vars: slots,
                size: (off + 15) & !15,
                sret,
            },
        );
    }
    Ok((fs, frames))
}

fn elf(code: &[u8]) -> Vec<u8> {
    elf_with_machine(code, 62)
}
fn elf_with_machine(code: &[u8], machine: u16) -> Vec<u8> {
    let mut out = vec![0u8; 4096];
    out[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out[16..18].copy_from_slice(&2u16.to_le_bytes());
    out[18..20].copy_from_slice(&machine.to_le_bytes());
    out[20..24].copy_from_slice(&1u32.to_le_bytes());
    out[24..32].copy_from_slice(&0x401000u64.to_le_bytes());
    out[32..40].copy_from_slice(&64u64.to_le_bytes());
    out[52..54].copy_from_slice(&64u16.to_le_bytes());
    out[54..56].copy_from_slice(&56u16.to_le_bytes());
    out[56..58].copy_from_slice(&2u16.to_le_bytes());
    out[64..68].copy_from_slice(&1u32.to_le_bytes());
    out[68..72].copy_from_slice(&5u32.to_le_bytes());
    out[80..88].copy_from_slice(&0x400000u64.to_le_bytes());
    out[88..96].copy_from_slice(&0x400000u64.to_le_bytes());
    let size = (out.len() + code.len()) as u64;
    out[96..104].copy_from_slice(&size.to_le_bytes());
    out[104..112].copy_from_slice(&size.to_le_bytes());
    out[112..120].copy_from_slice(&4096u64.to_le_bytes());
    // PT_GNU_STACK: explicit RW, never executable. The code PT_LOAD remains RX.
    out[120..124].copy_from_slice(&0x6474_e551u32.to_le_bytes());
    out[124..128].copy_from_slice(&6u32.to_le_bytes());
    out[168..176].copy_from_slice(&16u64.to_le_bytes());
    out.extend_from_slice(code);
    out
}
/// Linker-free ELF32/EM_ARM EABI5 executable for Linux ARM state.
/// Private Mini ABI; no C calls, PLT, interpreter or external symbols.
fn elf32_arm(code: &[u8], float_abi: target::FloatAbi) -> Res<Vec<u8>> {
    if code.len() > u32::MAX as usize - 4096 {
        return Err("ARM32 ELF image too large".into());
    }
    let mut out = vec![0u8; 4096];
    out[..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    fn put16(v: &mut [u8], off: usize, n: u16) {
        v[off..off + 2].copy_from_slice(&n.to_le_bytes());
    }
    fn put32(v: &mut [u8], off: usize, n: u32) {
        v[off..off + 4].copy_from_slice(&n.to_le_bytes());
    }
    let flags = match float_abi {
        target::FloatAbi::Soft => 0x0500_0200,
        target::FloatAbi::Hard => 0x0500_0400,
        _ => return Err("ARM32 Linux requires hard or soft EABI profile".into()),
    };
    put16(&mut out, 16, 2); // ET_EXEC
    put16(&mut out, 18, 40); // EM_ARM
    put32(&mut out, 20, 1); // EV_CURRENT
    put32(&mut out, 24, 0x11000); // entry at base+0x1000
    put32(&mut out, 28, 52); // ELF32 program headers
    put32(&mut out, 36, flags);
    put16(&mut out, 40, 52);
    put16(&mut out, 42, 32);
    put16(&mut out, 44, 2);
    let size = (4096 + code.len()) as u32;
    put32(&mut out, 52, 1); // PT_LOAD
    put32(&mut out, 56, 0); // p_offset
    put32(&mut out, 60, 0x10000);
    put32(&mut out, 64, 0x10000);
    put32(&mut out, 68, size);
    put32(&mut out, 72, size);
    put32(&mut out, 76, 5);
    put32(&mut out, 80, 4096); // RX
    put32(&mut out, 84, 0x6474_e551); // PT_GNU_STACK
    put32(&mut out, 108, 6); // RW stack, no execute
    put32(&mut out, 112, 8); // 8-byte stack alignment
    out.extend_from_slice(code);
    Ok(out)
}

// Target-dependent literal bounds, including literals passed to included functions.
// Large i64 literals remain valid on 32-bit targets; only usize is bounded.
fn check_usize_literals(
    ast: &Ast,
    fs: &HashMap<String, usize>,
    frames: &HashMap<String, Frame>,
    target: target::Target,
) -> Res<()> {
    fn expression(
        e: &Ex,
        expected: Option<&Ty>,
        vars: &HashMap<String, Ty>,
        ast: &Ast,
        fs: &HashMap<String, usize>,
        max: u64,
    ) -> Res<()> {
        match e {
            Ex::Num(n) if expected == Some(&Ty::Usize) => {
                if *n < 0 || *n as u64 > max {
                    return Err(format!("usize literal {n} exceeds target range 0..{max}"));
                }
            }
            Ex::Neg(v) => expression(v, Some(&Ty::I64), vars, ast, fs, max)?,
            Ex::Cast(v, _) => expression(v, None, vars, ast, fs, max)?,
            Ex::Bin(a, _, b) => {
                let ty = type_expr(a, vars, ast, fs)?;
                expression(a, Some(&ty), vars, ast, fs, max)?;
                expression(b, Some(&ty), vars, ast, fs, max)?;
            }
            Ex::Call(name, args) => {
                if let Some(fields) = ast.structs.get(name) {
                    for (arg, (_, ty)) in args.iter().zip(fields) {
                        expression(arg, Some(ty), vars, ast, fs, max)?;
                    }
                } else if let Some(&i) = fs.get(name) {
                    for (arg, (_, ty)) in args.iter().zip(&ast.funcs[i].args) {
                        expression(arg, Some(ty), vars, ast, fs, max)?;
                    }
                } else {
                    for arg in args {
                        expression(arg, None, vars, ast, fs, max)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn block(
        body: &[St],
        ret: &Ty,
        vars: &HashMap<String, Ty>,
        ast: &Ast,
        fs: &HashMap<String, usize>,
        max: u64,
    ) -> Res<()> {
        for st in body {
            match st {
                St::Let(_, ty, e) => expression(e, Some(ty), vars, ast, fs, max)?,
                St::Assign(name, e) => expression(e, vars.get(name), vars, ast, fs, max)?,
                St::Ret(e) => expression(e, Some(ret), vars, ast, fs, max)?,
                St::Expr(e) => expression(e, None, vars, ast, fs, max)?,
                St::If(c, a, b) => {
                    expression(c, None, vars, ast, fs, max)?;
                    block(a, ret, vars, ast, fs, max)?;
                    block(b, ret, vars, ast, fs, max)?;
                }
                St::While(c, b) => {
                    expression(c, None, vars, ast, fs, max)?;
                    block(b, ret, vars, ast, fs, max)?;
                }
                St::Unsafe(b) => block(b, ret, vars, ast, fs, max)?,
                St::Asm(a) => {
                    for (_, e) in &a.inputs {
                        expression(e, None, vars, ast, fs, max)?;
                    }
                }
                St::Drop(_) => {}
            }
        }
        Ok(())
    }
    for f in &ast.funcs {
        let vars = frames[&f.name]
            .vars
            .iter()
            .map(|(k, (_, t))| (k.clone(), t.clone()))
            .collect::<HashMap<_, _>>();
        block(&f.body, &f.ret, &vars, ast, fs, target.usize_max())?;
    }
    Ok(())
}

#[cfg(test)]
fn compile(source: &str) -> Res<Vec<u8>> {
    compile_for(source, target::Target::parse("x86_64-linux").unwrap())
}
fn ast_uses_extended_integer(ast: &Ast) -> bool {
    fn has_ty(t: &Ty) -> bool {
        matches!(t, Ty::Int { .. })
    }
    fn ex(e: &Ex) -> bool {
        match e {
            Ex::Cast(_, _) => true,
            Ex::Neg(x) => ex(x),
            Ex::Bin(a, _, b) => ex(a) || ex(b),
            Ex::Call(_, args) => args.iter().any(ex),
            _ => false,
        }
    }
    fn stmts(body: &[St]) -> bool {
        body.iter().any(|s| match s {
            St::Let(_, t, e) => has_ty(t) || ex(e),
            St::Assign(_, e) | St::Ret(e) | St::Expr(e) => ex(e),
            St::If(c, a, b) => ex(c) || stmts(a) || stmts(b),
            St::While(c, b) => ex(c) || stmts(b),
            St::Unsafe(b) => stmts(b),
            St::Asm(a) => a.inputs.iter().any(|(_, e)| ex(e)),
            St::Drop(_) => false,
        })
    }
    ast.structs.values().flatten().any(|(_, t)| has_ty(t))
        || ast
            .funcs
            .iter()
            .any(|f| has_ty(&f.ret) || f.args.iter().any(|(_, t)| has_ty(t)) || stmts(&f.body))
}

fn compile_for(source: &str, target: target::Target) -> Res<Vec<u8>> {
    if !target.is_implemented() {
        return Err("target backend unavailable or exact MCU not selected".into());
    }
    let ast = P::new(lex(source)?).parse()?;
    // No silent truncation on targets whose width-aware code generator is pending.
    if !matches!(
        target.arch,
        target::Arch::X86_64 | target::Arch::Aarch64 | target::Arch::Armv7
    ) && ast_uses_extended_integer(&ast)
    {
        return Err(
            "fixed-width integer operations and 'as' are not implemented on this target".into(),
        );
    }
    let (fs, frames) = check_for(&ast, target)?;
    check_usize_literals(&ast, &fs, &frames, target)?;
    let hir = ir::lower(&ast, &frames, &fs)?;
    match target.arch {
        target::Arch::X86_64 => backend_x86::Gen::new(&ast, &frames, &fs, &hir).finish(),
        target::Arch::Aarch64 => backend_aarch64::Gen::new(&ast, &frames, &fs, &hir).finish(),
        target::Arch::Armv7 if target.environment == target::Environment::Linux => {
            backend_arm32::Gen::new(&ast, &frames, &fs, &hir, target.float_abi).finish()
        }
        target::Arch::Avr8 | target::Arch::Mos6510 => tiny8::emit(&hir, target),
        target::Arch::Thumbv7m => backend_thumb::emit(&hir, target),
        _ => Err(format!("backend not implemented for {:?}", target.arch)),
    }
}
fn main() {
    let args: Vec<_> = env::args().collect();
    let result: Res<()> = (|| {
        if args.len() == 3 && args[1] == "--emit-ir" {
            let source = preprocess::load_source(
                Path::new(&args[2]),
                target::Target::parse("x86_64-linux").unwrap(),
            )?;
            let ast = P::new(lex(&source)?).parse()?;
            let (funcs, frames) = check(&ast)?;
            print!("{}", ir::lower(&ast, &frames, &funcs)?.dump());
            return Ok(());
        }
        if args.len() == 3 && args[1] == "--check-device" {
            let p = device_config::DeviceProfile::load(Path::new(&args[2]))?;
            let t = target::Target::from_device(&p);
            println!(
                "device: {}\ncpu: {:?}\nflash: 0x{:08X}..0x{:08X}\nram: 0x{:08X}..0x{:08X}\nstack reserve: {} bytes\nfeatures: {}\nvectors (compiler): {}\noutput (compiler): {:?}\nbackend: {}",
                p.name,
                p.layout.cpu,
                p.layout.flash.start,
                p.layout.flash.end,
                p.layout.ram.start,
                p.layout.ram.end,
                p.layout.stack_reserve,
                p.layout.features.names().join(","),
                p.layout.vectors,
                p.layout.output,
                if t.is_implemented() {
                    "available"
                } else {
                    "NOT IMPLEMENTED"
                }
            );
            return Ok(());
        }
        let (target, input, output) = match args.as_slice() {
            [_,input,flag,output] if flag=="-o" => (target::Target::parse("x86_64-linux").unwrap(),input,output),
            [_,flag,t,input,oflag,output] if flag=="--target" && oflag=="-o" =>
                (target::Target::parse(t).ok_or("unknown target triple")?,input,output),
            [_,flag,map,input,oflag,output] if flag=="--device" && oflag=="-o" => {
                let p=device_config::DeviceProfile::load(Path::new(map))?;
                (target::Target::from_device(&p),input,output)
            },
            _ => return Err("Usage: mini [--target TRIPLE | --device FILE.target] INPUT.mini -o OUTPUT; mini --check-device FILE.target; mini --emit-ir INPUT.mini".into())
        };
        if !target.is_implemented() {
            return Err(format!(
                "target {target:?} backend is not implemented or target profile unavailable"
            ));
        }
        let source = preprocess::load_source(Path::new(input), target)?;
        let bytes = compile_for(&source, target)?;
        let path = Path::new(output);
        fs::write(path, bytes).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if matches!(target.environment, target::Environment::Linux) {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("mini: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adding_unused_function_does_not_change_elf() {
        let baseline = compile("program { return 42 }").unwrap();
        let extended =
            compile("fn unused() -> i64 { return 987654321 } program { return 42 }").unwrap();
        assert_eq!(
            baseline, extended,
            "dead functions must not reach native code"
        );
    }
    #[test]
    fn reachable_transitive_library_functions_change_elf() {
        let baseline = compile("program { return 42 }").unwrap();
        let extended = compile("fn helper() -> i64 { return 42 } fn wrapper() -> i64 { return helper() } program { return wrapper() }").unwrap();
        assert_ne!(baseline, extended);
    }
    #[test]
    fn scalar_and_aggregate_abi() {
        assert!(compile("struct R { value: i64; ok: bool } fn f(a: i64, b: bool) -> R { return R(a,b) } program { let r: R = f(42,true); return r.value }").is_ok());
    }
    #[test]
    fn no_special_linux_or_vector_builtin() {
        assert!(compile("program { unsafe { return syscall(39) } }").is_err());
        assert!(compile("program { let v: MiniVector = MiniVector(); return 0 }").is_err());
    }
    #[test]
    fn raw_address_requires_unsafe() {
        assert!(compile("program { let n: i64 = 65; return raw_addr(n) }").is_err());
        assert!(compile("program { let n: i64 = 65; unsafe { return raw_addr(n) } }").is_ok());
    }
    #[test]
    fn elf_header() {
        let elf = compile("program { return 42 }").unwrap();
        assert_eq!(&elf[..4], b"\x7fELF");
        assert_eq!(u16::from_le_bytes([elf[54], elf[55]]), 56);
    }
}

#[cfg(test)]
mod ir_backend_tests {
    use super::*;

    #[test]
    fn machine_code_comes_from_ir_not_ast() {
        let source = "program { return 41 }";
        let ast = P::new(lex(source).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        let original_ir = ir::lower(&ast, &frames, &fs).unwrap();
        let mut changed_ir = original_ir.clone();
        let main = changed_ir
            .functions
            .iter_mut()
            .find(|f| f.name == "__main")
            .unwrap();
        match main.instructions.first_mut().unwrap() {
            ir::Stmt::Return { value, .. } => {
                value.kind = ir::ExprKind::ConstI64(42);
            }
            _ => panic!("expected an IR return"),
        }
        let first = backend_x86::Gen::new(&ast, &frames, &fs, &original_ir)
            .finish()
            .unwrap();
        let second = backend_x86::Gen::new(&ast, &frames, &fs, &changed_ir)
            .finish()
            .unwrap();
        assert_ne!(first, second, "the backend must consume the modified IR");
        assert!(
            second
                .windows(10)
                .any(|bytes| bytes == [0x48, 0xb8, 42, 0, 0, 0, 0, 0, 0, 0])
        );
    }

    #[test]
    fn ir_contains_control_flow_and_typed_calls() {
        let src = "fn f(v: i64) -> i64 { return v + 1 } program { if true { return f(41) } else { return 0 } }";
        let ast = P::new(lex(src).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        let hir = ir::lower(&ast, &frames, &fs).unwrap();
        let dump = hir.dump();
        assert!(dump.contains("If"));
        assert!(dump.contains("Call"));
        assert!(dump.contains("I64"));
    }
}

#[cfg(test)]
mod asm_tests {
    use super::*;
    #[test]
    fn emits_inline_x86_assembly() {
        let elf = compile("fn hint() -> i64 { unsafe { asm { nop; pause; mfence } }; return 42 } program { return hint() }").unwrap();
        assert!(
            elf.windows(6)
                .any(|w| w == [0x90, 0xf3, 0x90, 0x0f, 0xae, 0xf0])
        );
    }
    #[test]
    fn asm_must_be_unsafe() {
        assert!(compile("program { asm { nop }; return 0 }").is_err());
    }
    #[test]
    fn unknown_instruction_is_rejected() {
        assert!(compile("program { unsafe { asm { mov } }; return 0 }").is_err());
    }
}

#[cfg(test)]
mod asm26_tests {
    use super::*;
    const GOOD: &str = "program { let result: i64 = 0; unsafe { asm { in rax = 39; syscall; out result = rax; clobber rcx; clobber r11; clobber memory } }; return result }";
    #[test]
    fn typed_syscall_asm_emits_instruction() {
        let elf = compile(GOOD).unwrap();
        assert!(elf.windows(2).any(|x| x == [0x0f, 0x05]));
    }
    #[test]
    fn rejects_missing_clobber() {
        assert!(compile("program { let v: i64 = 0; unsafe { asm { in rax = 39; syscall; out v = rax; clobber rcx; clobber r11 } }; return v }").is_err());
    }
    #[test]
    fn rejects_untyped_output_or_wrong_register() {
        assert!(compile("program { unsafe { asm { in rax = 39; syscall; out absent = rax; clobber rcx; clobber r11; clobber memory } }; return 0 }").is_err());
        assert!(compile("program { let a: i64 = 1; unsafe { asm { in rsp = a; syscall; clobber rcx; clobber r11; clobber memory } }; return 0 }").is_err());
    }
    #[test]
    fn rejects_unknown_and_duplicate_clobbers_before_ir_lowering() {
        let unknown = "program { let result:i64=0; unsafe { asm { in rax=39; syscall; out result=rax; clobber rcx; clobber r11; clobber memory; clobber rsp } }; return result }";
        assert!(
            compile(unknown)
                .unwrap_err()
                .contains("unsupported clobber")
        );
        let duplicate = "program { let result:i64=0; unsafe { asm { in rax=39; syscall; out result=rax; clobber rcx; clobber r11; clobber memory; clobber memory } }; return result }";
        assert!(
            compile(duplicate)
                .unwrap_err()
                .contains("duplicate clobber")
        );
    }
    #[test]
    fn rejects_duplicate_inputs() {
        assert!(compile("program { unsafe { asm { in rax = 1; in rax = 2; syscall; clobber rcx; clobber r11; clobber memory } }; return 0 }").is_err());
    }
}

#[cfg(test)]
mod unsafe_library_tests {
    use super::*;
    #[test]
    fn unsafe_fn_is_not_callable_from_safe_code() {
        let prefix = "unsafe fn low(n: i64) -> i64 { return n } ";
        assert!(compile(&format!("{prefix}program {{ return low(42) }}")).is_err());
        assert!(
            compile(&format!(
                "{prefix}program {{ unsafe {{ return low(42) }} }}"
            ))
            .is_ok()
        );
    }
}

#[cfg(test)]
mod usize_language_tests {
    use super::*;
    #[test]
    fn accepts_usize_parameters_and_results() {
        let source = "fn add(a: usize, b: usize) -> usize { return a + b } program { let a: usize = 20; let b: usize = 22; let sum: usize = add(a,b); return 0 }";
        assert!(compile(source).is_ok());
    }
    #[test]
    fn rejects_implicit_signed_to_usize_and_mixed_arithmetic() {
        assert!(compile("program { let n: i64 = -1; let m: usize = n; return 0 }").is_err());
        assert!(
            compile("program { let n: usize = 4; let m: i64 = 2; let r: usize = n + m; return 0 }")
                .is_err()
        );
        assert!(compile("program { let n: usize = -1; return 0 }").is_err());
    }
    #[test]
    fn supports_unsigned_comparison() {
        assert!(compile("program { let a: usize = 7; let b: usize = 5; if a > b { return 42 } else { return 1 } }").is_ok());
    }
}

#[cfg(test)]
mod arm64_backend_tests {
    use super::*;
    fn a64() -> target::Target {
        target::Target::parse("aarch64-linux").unwrap()
    }
    #[test]
    fn linux_aarch64_elf64_emission() {
        let image = compile_for("program { return 42 }", a64()).unwrap();
        assert_eq!(&image[..4], b"\x7fELF");
        assert_eq!(image[4], 2); // ELF64
        assert_eq!(u16::from_le_bytes([image[18], image[19]]), 183); // EM_AARCH64
        assert_eq!(
            u64::from_le_bytes(image[24..32].try_into().unwrap()),
            0x401000
        );
        assert_eq!(u16::from_le_bytes(image[56..58].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(image[120..124].try_into().unwrap()),
            0x6474_e551
        ); // PT_GNU_STACK
        assert_eq!(u32::from_le_bytes(image[124..128].try_into().unwrap()), 6); // RW, no execute
        // First instruction is BL __main; exit is the AArch64 svc #0 instruction.
        assert_eq!(image[4099] & 0xfc, 0x94);
        assert!(
            image[4096..]
                .windows(4)
                .any(|s| s == 0xD400_0001u32.to_le_bytes())
        );
    }
    #[test]
    fn a64_keeps_only_reachable_functions() {
        let baseline = compile_for("program { return 42 }", a64()).unwrap();
        let extra = compile_for(
            "fn unused() -> i64 { return 8 } program { return 42 }",
            a64(),
        )
        .unwrap();
        assert_eq!(baseline, extra);
    }
    #[test]
    fn a64_scalar_and_hidden_aggregate_result() {
        let code = "struct R { value:i64; ok:bool } fn make(a:i64,b:bool)->R { return R(a,b) } fn forward(a:i64)->R { return make(a,true) } program { let r:R=forward(42); return r.value }";
        assert!(compile_for(code, a64()).is_ok());
    }
    #[test]
    fn a64_svc_is_target_checked() {
        let okay = "program { let v:i64=0; unsafe { asm { in x8=172; svc; out v=x0; clobber memory } }; return v }";
        assert!(compile_for(okay, a64()).is_ok());
        assert!(compile_for(okay, target::Target::parse("x86_64-linux").unwrap()).is_err());
        assert!(compile_for("program { unsafe { asm { pause } }; return 0 }", a64()).is_err());
        assert!(
            compile_for(
                "program { unsafe { asm { in x8=172; svc } }; return 0 }",
                a64()
            )
            .is_err()
        );
        assert!(
            compile_for(
                "program { unsafe { asm { in x29=172; svc; clobber memory } }; return 0 }",
                a64()
            )
            .is_err()
        );
    }
    #[test]
    fn arm64_library_and_cross_target_include_parse_and_compile() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let hello =
            preprocess::load_source(&root.join("examples/hello_arm64.mini"), a64()).unwrap();
        assert!(
            compile_for(&hello, a64()).is_ok(),
            "ARM64 Linux include should compile"
        );
        for arch in ["aarch64-linux", "x86_64-linux"] {
            let t = target::Target::parse(arch).unwrap();
            let src = preprocess::load_source(&root.join("examples/target_linux.mini"), t).unwrap();
            assert!(
                compile_for(&src, t).is_ok(),
                "conditional Linux library should compile for {arch}"
            );
        }
    }
    #[test]
    fn a64_library_asm_is_not_a_builtin() {
        assert!(compile_for("program { return sys_getpid() }", a64()).is_err());
        assert!(compile_for("program { unsafe { return syscall(172) } }", a64()).is_err());
    }
}

#[cfg(test)]
mod arm32_backend_tests {
    use super::*;
    fn arm(abi: &str) -> target::Target {
        target::Target::parse(abi).unwrap()
    }
    #[test]
    fn arm32_elf_headers_and_abi_flags() {
        for (t, flags) in [
            ("armv7-linux-soft", 0x0500_0200u32),
            ("armv7-linux-hard", 0x0500_0400u32),
        ] {
            let elf = compile_for("program { return 42 }", arm(t)).unwrap();
            assert_eq!(&elf[0..5], b"\x7fELF\x01");
            assert_eq!(u16::from_le_bytes(elf[18..20].try_into().unwrap()), 40);
            assert_eq!(u32::from_le_bytes(elf[36..40].try_into().unwrap()), flags);
            assert_eq!(u32::from_le_bytes(elf[24..28].try_into().unwrap()), 0x11000);
            assert_eq!(
                u32::from_le_bytes(elf[84..88].try_into().unwrap()),
                0x6474_e551
            );
            assert_eq!(u32::from_le_bytes(elf[108..112].try_into().unwrap()), 6);
        }
    }
    #[test]
    fn arm32_rejects_oversize_usize_but_allows_large_i64() {
        let t = arm("armv7-linux-soft");
        assert!(compile_for("program { let x:usize=4294967296; return 0 }", t).is_err());
        assert!(
            compile_for(
                "fn f(x:usize)->i64{return 42} program {return f(4294967296)}",
                t
            )
            .is_err()
        );
        assert!(compile_for("program { let x:i64=4294967296; return 42 }", t).is_ok());
        assert!(compile_for("program { let x:usize=4294967295; return 42 }", t).is_ok());
    }
    #[test]
    fn arm32_rejects_invalid_assembly() {
        let t = arm("armv7-linux-soft");
        assert!(compile_for("program { unsafe { asm { in r7=20; svc } }; return 0 }", t).is_err());
        assert!(
            compile_for(
                "program { unsafe { asm { in r11=20; svc; clobber memory } }; return 0 }",
                t
            )
            .is_err()
        );
        assert!(compile_for("program { unsafe { asm { pause } }; return 0 }", t).is_err());
    }
    #[test]
    fn arm32_minimal_ir_struct_and_signed_arithmetic() {
        let t = arm("armv7-linux-hard");
        let p = "struct R { n:i64; ok:bool } fn make(n:i64)->R{return R(n,true)} fn f(n:i64)->R{return make(n)} program {let r:R=f(42);return r.n}";
        assert!(compile_for(p, t).is_ok());
        let numeric = "program { let x:i64=4294967296; let y:i64=-10; if x/y<0 { return 42 } else { return 1 } }";
        assert!(compile_for(numeric, t).is_ok());
    }
    #[test]
    fn arm32_unused_functions_do_not_change_elf() {
        let t = arm("armv7-linux-soft");
        let a = compile_for("program { return 42 }", t).unwrap();
        let b = compile_for("fn unused()->i64{return 99} program {return 42}", t).unwrap();
        assert_eq!(a, b);
    }
    #[test]
    fn arm32_linux_mini_library_is_resolved() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let t = arm("armv7-linux-soft");
        let hello = preprocess::load_source(&root.join("examples/hello_arm32.mini"), t).unwrap();
        assert!(compile_for(&hello, t).is_ok());
        let selected =
            preprocess::load_source(&root.join("examples/target_linux.mini"), t).unwrap();
        assert!(compile_for(&selected, t).is_ok());
        assert!(compile_for("program { return sys_getpid() }", t).is_err());
    }
}

#[cfg(test)]
mod ownership_drop_tests {
    use super::*;
    const PRELUDE: &str = "struct Guard { code: i64 } \
        fn __drop_Guard(g: Guard) { let seen: i64 = g.code } \
        fn make(n:i64)->Guard { unsafe { return Guard(n) } } ";

    fn lower_checked(src: &str) -> ir::Module {
        let ast = P::new(lex(src).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        ir::lower(&ast, &frames, &fs).unwrap()
    }
    fn main_fun(m: &ir::Module) -> &ir::Function {
        m.functions.iter().find(|f| f.name == "__main").unwrap()
    }
    #[test]
    fn destructor_is_kept_by_dead_code_selection_and_return_cleanup_is_inserted() {
        let ir = lower_checked(&format!(
            "{PRELUDE} program {{let g:Guard=make(73);return g.code}}"
        ));
        assert!(ir.functions.iter().any(|f| f.name == "__drop_Guard"));
        assert!(ir.functions.iter().any(|f| f.name == "make"));
        match main_fun(&ir).instructions.last().unwrap() {
            ir::Stmt::Return { value, cleanup } => {
                assert!(matches!(value.kind, ir::ExprKind::Field { .. }));
                assert_eq!(cleanup.len(), 1);
                assert!(
                    matches!(&cleanup[0].kind,ir::ExprKind::Call{name,..} if name=="__drop_Guard")
                );
            }
            x => panic!("expected return with cleanup: {x:?}"),
        }
    }
    #[test]
    fn explicit_drop_is_once_only() {
        let ir = lower_checked(&format!(
            "{PRELUDE} program {{let g:Guard=make(9);drop g;return 0}}"
        ));
        let main = main_fun(&ir);
        assert!(
            matches!(&main.instructions[1],ir::Stmt::Evaluate(ir::Expr{kind:ir::ExprKind::Call{name,..},..}) if name=="__drop_Guard")
        );
        assert!(matches!(&main.instructions[2],ir::Stmt::Return{cleanup,..} if cleanup.is_empty()));
    }
    #[test]
    fn reverse_order_and_void_end_cleanup() {
        let src = format!(
            "{PRELUDE} fn f() {{let a:Guard=make(1);let b:Guard=make(2)}} program {{f();return 0}}"
        );
        let ir = lower_checked(&src);
        let f = ir.functions.iter().find(|f| f.name == "f").unwrap();
        assert_eq!(f.end_cleanup.len(), 2);
        let locals = f
            .end_cleanup
            .iter()
            .map(|e| match &e.kind {
                ir::ExprKind::Call { arguments, .. } => match &arguments[0].kind {
                    ir::ExprKind::Local(n) => n.as_str(),
                    _ => panic!(),
                },
                _ => panic!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(locals, vec!["b", "a"]);
    }
    #[test]
    fn constructors_need_unsafe_and_destructors_cannot_be_called_directly() {
        let direct = format!("{PRELUDE} program {{ let x:Guard=Guard(9); return 0 }}");
        assert!(compile(&direct).unwrap_err().contains("requires unsafe"));
        let call = format!("{PRELUDE} program {{let x:Guard=make(9);__drop_Guard(x);return 0}}");
        assert!(
            compile(&call)
                .unwrap_err()
                .contains("cannot be called directly")
        );
    }
    #[test]
    fn rejects_double_drop_and_path_dependent_state() {
        assert!(
            compile(&format!(
                "{PRELUDE} program {{let g:Guard=make(1);drop g;drop g;return 0}}"
            ))
            .is_err()
        );
        assert!(
            compile(&format!(
                "{PRELUDE} program {{let g:Guard=make(1);if true {{drop g}} else {{}};return 0}}"
            ))
            .is_err()
        );
    }
    #[test]
    fn destructor_signature_must_be_safe_borrowed_and_void() {
        assert!(compile("struct G{v:i64} fn __drop_G(g:i64){} program{return 0}").is_err());
        assert!(compile("struct G{v:i64} unsafe fn __drop_G(g:G){} program{return 0}").is_err());
        assert!(
            compile("struct G{v:i64} fn __drop_G(g:G)->i64{return 1} program{return 0}").is_err()
        );
    }
    #[test]
    fn linux_64_bit_backends_accept_hook_without_new_ir_opcode() {
        let code = format!("{PRELUDE} program {{let g:Guard=make(73);return g.code}}");
        for triple in [
            "x86_64-linux",
            "aarch64-linux",
            "armv7-linux-soft",
            "armv7-linux-hard",
        ] {
            let target = target::Target::parse(triple).unwrap();
            assert!(compile_for(&code, target).is_ok(), "target {triple}");
        }
    }
    #[test]
    fn eight_bit_targets_reject_owned_aggregate_safely() {
        let code = format!("{PRELUDE} program {{let g:Guard=make(1);return 0}}");
        for triple in ["c64-6510", "avr8-atmega328p"] {
            let target = target::Target::parse(triple).unwrap();
            assert!(compile_for(&code, target).is_err(), "target {triple}");
        }
    }
    #[test]
    fn no_destructor_if_resource_type_is_never_used() {
        let code = "struct Guard { value:i64 } fn __drop_Guard(g:Guard){} program{return 42}";
        assert_eq!(
            compile("program{return 42}").unwrap(),
            compile(code).unwrap()
        );
    }
}

#[cfg(test)]
mod reassignment_42_tests {
    use super::*;
    #[test]
    fn scalar_reassignment_and_loop_lower_to_ir() {
        let source = "program { let x:i64=0; while x < 42 { x = x + 1 }; return x }";
        let ast = P::new(lex(source).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        let m = ir::lower(&ast, &frames, &fs).unwrap();
        assert!(format!("{m:?}").contains("Assign"));
        assert!(compile(source).is_ok());
    }
    #[test]
    fn scalar_reassignment_on_arm_linux() {
        let source = "program { let x:i64=40; x = x + 2; return x }";
        for t in [
            "x86_64-linux",
            "aarch64-linux",
            "armv7-linux-soft",
            "armv7-linux-hard",
        ] {
            assert!(
                compile_for(source, target::Target::parse(t).unwrap()).is_ok(),
                "{t}"
            );
        }
    }
    #[test]
    fn reassignment_rejects_invalid_targets_and_types() {
        for source in [
            "program { x = 1; return 0 }",
            "program { let x:bool=true; x = 10; return 0 }",
            "struct R { n:i64 } program { let r:R=R(0); r = R(1); return 0 }",
            "program { let x:i64=10; x = true; return 0 }",
        ] {
            assert!(compile(source).is_err(), "{source}");
        }
    }
    #[test]
    fn numeric_types_have_explicit_pending_diagnostic() {
        for typ in ["f32", "f64"] {
            let source = format!("program {{ let x:{typ}=1; return 0 }}");
            let result = compile(&source).unwrap_err();
            assert!(result.contains("not implemented"), "{typ}: {result}");
        }
    }
    #[test]
    fn reassignment_on_8bit_and_thumb_subset() {
        let source = "fn count() -> usize { let x:usize=0; while x < 5 { x = x + 1 }; return x } program { count(); return 0 }";
        for t in ["c64-6510", "avr8-atmega328p", "thumbv7m-stm32f103c8"] {
            let Ok(target) = target::Target::parse(t) else {
                continue;
            };
            assert!(compile_for(source, target).is_ok(), "{t}");
        }
    }
}

#[cfg(test)]
mod fixed_width_43_tests {
    use super::*;

    #[test]
    fn integer_types_and_checked_cast_lower_to_ir() {
        let code = "fn add(x:u8, y:u8)->u8 { return x + y } program { let a:u8=40; let b:u8=2; let c:u8=add(a,b); return c as i64 }";
        let ast = P::new(lex(code).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        let hir = ir::lower(&ast, &frames, &fs).unwrap();
        assert!(hir.dump().contains("Cast"));
        assert!(compile(code).is_ok());
    }
    #[test]
    fn signed_narrow_ints_and_unsigned_comparison() {
        for code in [
            "program { let a:i8= -3; let b:i8= 2; let c:i8=a*b; return c as i64 }",
            "program { let a:u16=100; a = a + 1; if a > 100 { return 0 } else { return 1 } }",
            "program { let a:u32=65536; let b:u64=a as u64; return (b / 2) as i64 }",
            "program { let a:i32=42; let b:i64=a as i64; return b }",
        ] {
            assert!(compile(code).is_ok(), "{code}");
        }
    }
    #[test]
    fn type_errors_and_unavailable_targets_rejected() {
        for code in [
            "program { let a:u8=256; return 0 }",
            "program { let a:i8=128; return 0 }",
            "program { let a:u16= -1; return 0 }",
            "program { let a:u8=1; let b:i8=1; return (a+b) as i64 }",
            "program { let a:f32=1; return 0 }",
            "program { let a:i64=1; let b:bool=a as bool; return 0 }",
        ] {
            assert!(compile(code).is_err(), "{code}");
        }
        let code = "program { let a:u8=42; return a as i64 }";
        for t in ["aarch64-linux", "armv7-linux-soft", "armv7-linux-hard"] {
            let target = target::Target::parse(t).unwrap();
            assert!(compile_for(code, target).is_ok(), "{t}");
        }
        for t in ["thumbv7m-stm32f103c8", "avr8-atmega328p", "c64-6510"] {
            let target = target::Target::parse(t).unwrap();
            assert!(compile_for(code, target).is_err(), "{t}");
        }
    }
    #[test]
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn generated_elf_casts_and_bounds_execute() {
        use std::{fs, process::Command};
        let programs = [
            ("program { let a:u8=40; a = a + 2; return a as i64 }", 42),
            (
                "program { let a:i8= -20; let b:i8=2; return (a / b) as i64 }",
                246,
            ),
            ("program { let a:u8=255; a = a + 1; return 0 }", 101),
            ("program { let a:i8=127; a = a + 1; return 0 }", 101),
            (
                "program { let a:i64= -1; let b:u8=a as u8; return b as i64 }",
                101,
            ),
            (
                "program { let a:u16=300; let b:u8=a as u8; return b as i64 }",
                101,
            ),
            (
                "program { let a:i8= -1; if a < 0 {return 42} else {return 2} }",
                42,
            ),
            (
                "program { let a:u8=255; if a > 1 {return 42} else {return 2} }",
                42,
            ),
        ];
        for (i, (code, expected)) in programs.iter().enumerate() {
            let bytes = compile(code).unwrap();
            let path =
                std::env::temp_dir().join(format!("mini_fixed_width_{}_{}", std::process::id(), i));
            fs::write(&path, bytes).unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            let result = Command::new(&path).status();
            let _ = fs::remove_file(&path);
            let status = result.expect("Mini generated ELF could not be executed");
            assert_eq!(status.code(), Some(*expected), "{code}");
        }
    }
}

#[cfg(test)]
mod arm_integer_44_tests {
    use super::*;
    #[test]
    fn arm_linux_accepts_width_checked_scalars() {
        let cases = [
            "program { let a:u8=41; a = a + 1; return a as i64 }",
            "fn add(a:i16,b:i16)->i16 { return a+b } program { return add(20,22) as i64 }",
            "program { let a:i8 = -4; if a < 0 {return 42} else {return 1} }",
            "program { let a:u32=42; let b:i64=a as i64; return b }",
            "program { let a:i64=42; let b:u16=a as u16; return b as i64 }",
        ];
        for t in ["aarch64-linux", "armv7-linux-soft", "armv7-linux-hard"] {
            let target = target::Target::parse(t).unwrap();
            for source in cases {
                assert!(compile_for(source, target).is_ok(), "{t}: {source}");
            }
        }
    }
    #[test]
    fn arm32_rejects_unimplemented_unsigned_64_arithmetic() {
        let code = "program { let a:u64=40; let b:u64=2; let c:u64=a+b; return c as i64 }";
        for t in ["armv7-linux-soft", "armv7-linux-hard"] {
            let err = compile_for(code, target::Target::parse(t).unwrap()).unwrap_err();
            assert!(err.contains("unsigned u64"), "{t}: {err}");
        }
    }
    #[test]
    fn arm_integer_casts_preserve_minimal_ir() {
        let code = "program { let a:u8=42; return a as i64 }";
        let ast = P::new(lex(code).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        let ir = ir::lower(&ast, &frames, &fs).unwrap();
        assert!(ir.dump().contains("Cast"));
    }
}

#[cfg(test)]
mod fixed_array_47_tests {
    use super::*;

    #[test]
    fn array_is_lowered_to_checked_scalar_slots() {
        let source =
            "program { let data: [usize; 4] = [0; 4]; data[2] = 42; return data[2] as i64 }";
        let ast = P::new(lex(source).unwrap()).parse().unwrap();
        let body = &ast.funcs.iter().find(|f| f.name == "__main").unwrap().body;
        assert!(matches!(body[2], St::Let(ref n,_,_) if n == "__mini_arr_data_2"));
        assert!(matches!(body[4], St::Assign(ref n,_) if n == "__mini_arr_data_2"));
        let machine = compile(source).unwrap();
        assert!(machine.starts_with(b"\x7fELF"));
    }

    #[test]
    fn array_bounds_are_checked_during_parsing() {
        for bad in [
            "program { let a: [usize; 4] = [0; 4]; return a[4] as i64 }",
            "program { let a: [usize; 4] = [0; 4]; a[9] = 1; return 0 }",
            "program { let a: [usize; 4] = [0; 3]; return 0 }",
            "program { let a: [usize; 0] = [0; 0]; return 0 }",
            "program { let a: [usize; 4] = [0; 4]; return a[0 - 1] as i64 }",
            "program { let a: [usize; 4] = [foo(); 4]; return 0 }",
        ] {
            assert!(
                P::new(lex(bad).unwrap()).parse().is_err(),
                "should reject: {bad}"
            );
        }
    }

    #[test]
    fn arrays_do_not_add_new_ir_opcodes() {
        let code = "program { let b: [usize; 3] = [0; 3]; b[1] = 42; return b[1] as i64 }";
        let ast = P::new(lex(code).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        let ir = crate::ir::lower(&ast, &frames, &fs).unwrap();
        assert!(
            ir.functions[0]
                .instructions
                .iter()
                .any(|s| matches!(s, crate::ir::Stmt::Assign{name,..} if name=="__mini_arr_b_1"))
        );
    }

    #[test]
    fn fixed_bool_array_is_accepted() {
        let code = "program { let b: [bool; 2] = [false; 2]; b[0] = true; if b[0] { return 42 } else { return 0 } }";
        assert!(compile(code).is_ok());
    }
}

#[cfg(test)]
mod array48_checked_index_tests {
    use super::*;

    #[test]
    fn constant_arithmetic_index_is_resolved_without_runtime_dispatch() {
        let code = "program { let a:[usize;4]=[0;4]; a[1+2]=42; return a[3] as i64 }";
        let ast = P::new(lex(code).unwrap()).parse().unwrap();
        let body = &ast.funcs.last().unwrap().body;
        assert!(!format!("{body:?}").contains("__mini_arr_tmp_"));
        assert!(
            body.iter()
                .any(|s| matches!(s,St::Assign(n,_) if n=="__mini_arr_a_3"))
        );
    }

    #[test]
    fn constant_out_of_bounds_is_a_compile_error() {
        let code = "program { let a:[usize;4]=[0;4]; return a[2+2] as i64 }";
        assert!(P::new(lex(code).unwrap()).parse().is_err());
    }

    #[test]
    fn dynamic_read_is_guarded_and_index_evaluated_once() {
        let code = "program { let a:[usize;3]=[0;3]; let i:usize=2; a[2]=42; return a[i] as i64 }";
        let ast = P::new(lex(code).unwrap()).parse().unwrap();
        let body = &ast.funcs.last().unwrap().body;
        let debug = format!("{body:?}");
        assert!(debug.contains("__mini_arr_tmp_"));
        assert!(
            debug.contains("While(Bool(true), [])"),
            "missing fail-stop: {debug}"
        );
        assert!(debug.contains("__mini_arr_a_2"));
    }

    #[test]
    fn dynamic_store_inside_loop_is_lowered_without_branch_local_declarations() {
        let code = "program { let a:[usize;3]=[0;3]; let i:usize=0; while i < 3 { a[i]=i; i=i+1 } return a[2] as i64 }";
        let ast = P::new(lex(code).unwrap()).parse().unwrap();
        let f = ast.funcs.last().unwrap();
        assert!(f.body.iter().any(|s| matches!(s, St::While(_, _))));
        assert!(
            f.body
                .iter()
                .any(|s| matches!(s,St::Let(n,_,_) if n.starts_with("__mini_arr_tmp_")))
        );
    }

    #[test]
    fn unsupported_index_shapes_are_rejected() {
        for code in [
            "program { let a:[usize;17]=[0;17]; let i:usize=0; return a[i] as i64 }",
            "program { let a:[usize;3]=[0;3]; let i:i64=1; return a[i] as i64 }",
            "program { let a:[usize;3]=[0;3]; let i:usize=1; while a[i] == 0 { i=i+1 } return 0 }",
        ] {
            let rejected = P::new(lex(code).unwrap())
                .parse()
                .and_then(|ast| check(&ast).map(|_| ast));
            assert!(rejected.is_err(), "should reject: {code}");
        }
    }

    #[test]
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn x86_dynamic_index_executes_with_expected_result() {
        use std::os::unix::fs::PermissionsExt;
        use std::{fs, process::Command};
        let code = "program { let a:[usize;4]=[0;4]; let i:usize=0; while i<4 { a[i]=i+39; i=i+1 } return a[3] as i64 }";
        let bytes = compile(code).unwrap();
        let path = std::env::temp_dir().join(format!("mini_array48_{}", std::process::id()));
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let status = Command::new(&path).status();
        let _ = fs::remove_file(&path);
        assert_eq!(
            status.expect("generated x86 ELF failed to launch").code(),
            Some(42)
        );
    }

    #[test]
    fn x86_dynamic_index_generates_elf() {
        let code = "program { let a:[usize;3]=[0;3]; let i:usize=2; a[i]=42; return a[i] as i64 }";
        let bytes = compile(code).unwrap();
        assert!(bytes.starts_with(b"\x7fELF"));
    }
}
