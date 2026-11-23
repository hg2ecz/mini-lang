//! Deliberately small Cortex-M3/M4 Thumb backend. Firmware starts with an
//! architectural vector table; unsupported Mini operations are rejected.
//! Private Mini ABI: r0/r1 scalar args (max 2), r0 scalar result; functions
//! use aligned stack frames; no struct/i64 arithmetic, interrupts or recursion.
use crate::device_config::DeviceLayout;
use crate::encode_thumb::Encoder;
use crate::ir::{Expr, ExprKind, Function, Module, Stmt};
use crate::target::Target;
use crate::{AsmBlock, Ast, Res, Ty, referenced_vars, type_expr, validate_unsafe_expr};
use std::collections::{HashMap, HashSet};

pub(crate) fn check_asm(
    a: &AsmBlock,
    vars: &HashMap<String, Ty>,
    dead: &HashSet<String>,
    ast: &Ast,
    fs: &HashMap<String, usize>,
) -> Res<()> {
    if a.instructions.is_empty() || a.instructions.len() > 64 {
        return Err("Thumb asm requires 1..64 instructions".into());
    }
    let memory = a
        .instructions
        .iter()
        .any(|i| matches!(i.as_str(), "dmb" | "isb" | "ldr" | "str"));
    if a.instructions
        .iter()
        .any(|i| !matches!(i.as_str(), "nop" | "yield" | "dmb" | "isb" | "ldr" | "str"))
    {
        return Err("unsupported Thumb asm instruction".into());
    }
    if a.clobbers.len() > 1
        || (a.clobbers.len() == 1 && a.clobbers[0] != "memory")
        || (memory && !a.clobbers.iter().any(|c| c == "memory"))
    {
        return Err("Thumb MMIO/barriers require exactly clobber memory".into());
    }
    let mut registers = HashSet::new();
    for (reg, e) in &a.inputs {
        if !matches!(reg.as_str(), "r0" | "r1") || !registers.insert(reg.as_str()) {
            return Err("Thumb asm accepts distinct r0/r1 inputs only".into());
        }
        if type_expr(e, vars, ast, fs)? != Ty::Usize {
            return Err("Thumb MMIO inputs must have usize type".into());
        }
        validate_unsafe_expr(e, true, ast, fs)?;
        for n in referenced_vars(e) {
            if dead.contains(&n) {
                return Err(format!("use after drop: {n}"));
            }
        }
    }
    if a.outputs.len() > 1
        || a.outputs.iter().any(|(name, reg)| {
            reg != "r0" || vars.get(name) != Some(&Ty::Usize) || dead.contains(name)
        })
    {
        return Err("Thumb asm outputs must use an existing live usize variable from r0".into());
    }
    let has_load = a.instructions.iter().any(|i| i == "ldr");
    let has_store = a.instructions.iter().any(|i| i == "str");
    if has_load && has_store {
        return Err("Thumb asm cannot mix memory reads and writes in a single block".into());
    }
    if has_load {
        if a.instructions
            .iter()
            .filter(|i| i.as_str() == "ldr")
            .count()
            != 1
            || registers != HashSet::from(["r0"])
            || a.outputs.len() != 1
        {
            return Err(
                "Thumb ldr requires in r0 = address; out variable = r0; clobber memory".into(),
            );
        }
    } else if has_store {
        if a.instructions
            .iter()
            .filter(|i| i.as_str() == "str")
            .count()
            != 1
            || registers != HashSet::from(["r0", "r1"])
            || !a.outputs.is_empty()
        {
            return Err("Thumb str requires in r0 = address; in r1 = value; clobber memory".into());
        }
    } else if !a.inputs.is_empty() || !a.outputs.is_empty() {
        return Err("Thumb register bindings are permitted only for ldr/str".into());
    }
    Ok(())
}

fn validate_expr(expr: &Expr, funcs: &HashMap<&str, &Function>) -> Res<()> {
    if !matches!(expr.ty, Ty::Usize | Ty::Bool | Ty::Void) {
        return Err("Thumb subset supports usize/bool expressions only; i64 and aggregates require a separate correct backend".into());
    }
    match &expr.kind {
        ExprKind::ConstI64(v) if (0..=u32::MAX as i64).contains(v) => Ok(()),
        ExprKind::ConstI64(_) => Err("Thumb 32-bit literal out of range".into()),
        ExprKind::ConstBool(_) | ExprKind::Local(_) => Ok(()),
        ExprKind::Binary { op, left, right } => {
            if !matches!(
                op.as_str(),
                "+" | "-" | "==" | "!=" | "<" | "<=" | ">" | ">="
            ) {
                return Err(format!("Thumb operation {op} unsupported"));
            }
            let literal_rhs =
                matches!(&right.kind,ExprKind::ConstI64(n) if (0..=u32::MAX as i64).contains(n));
            if left.ty != Ty::Usize || !(right.ty == Ty::Usize || literal_rhs) {
                return Err(
                    "Thumb arithmetic/comparisons require usize operands or a fitting literal"
                        .into(),
                );
            }
            validate_expr(left, funcs)?;
            if !literal_rhs {
                validate_expr(right, funcs)?;
            }
            Ok(())
        }
        ExprKind::Call { name, arguments } => {
            let f = funcs
                .get(name.as_str())
                .ok_or_else(|| format!("Thumb unknown function {name}"))?;
            if f.params.len() != arguments.len() || arguments.len() > 2 {
                return Err("Thumb ABI supports up to two scalar arguments".into());
            }
            if !matches!(f.result, Ty::Void | Ty::Usize | Ty::Bool) {
                return Err("Thumb aggregate/i64 return unsupported".into());
            }
            for (arg, (_, ty)) in arguments.iter().zip(&f.params) {
                if !matches!(ty, Ty::Usize | Ty::Bool) {
                    return Err("Thumb aggregate/i64 argument unsupported".into());
                }
                validate_expr(arg, funcs)?;
            }
            Ok(())
        }
        _ => Err(
            "Thumb unsupported expression (struct, raw address, signed arithmetic, negation)"
                .into(),
        ),
    }
}
fn validate_stmts(body: &[Stmt], f: &Function, funcs: &HashMap<&str, &Function>) -> Res<()> {
    for item in body {
        match item {
            Stmt::Trap => {}
            Stmt::Assign { value, .. } => validate_expr(value, funcs)?,
            Stmt::Bind { ty, value, .. } => {
                if !matches!(ty, Ty::Usize | Ty::Bool) {
                    return Err("Thumb locals must be usize or bool".into());
                }
                validate_expr(value, funcs)?;
            }
            Stmt::Return { value, cleanup } => {
                if !cleanup.is_empty() {
                    return Err("Thumb destructors not implemented".into());
                }
                if f.name == "__main" {
                    if !matches!(&value.kind, ExprKind::ConstI64(0)) {
                        return Err("bare-metal Thumb program must return literal 0".into());
                    }
                } else {
                    validate_expr(value, funcs)?;
                }
            }
            Stmt::Evaluate(expr) => validate_expr(expr, funcs)?,
            Stmt::If { condition, yes, no } => {
                if condition.ty != Ty::Bool {
                    return Err("Thumb if condition must be bool".into());
                }
                validate_expr(condition, funcs)?;
                validate_stmts(yes, f, funcs)?;
                validate_stmts(no, f, funcs)?;
            }
            Stmt::While { condition, body } => {
                if condition.ty != Ty::Bool {
                    return Err("Thumb while condition must be bool".into());
                }
                validate_expr(condition, funcs)?;
                validate_stmts(body, f, funcs)?;
            }
            Stmt::Unsafe(body) => validate_stmts(body, f, funcs)?,
            Stmt::Asm(a) => {
                if a.instructions
                    .iter()
                    .any(|s| !matches!(s.as_str(), "nop" | "yield" | "dmb" | "isb" | "ldr" | "str"))
                {
                    return Err("Thumb asm opcode unsupported".into());
                }
                for (_, expr) in &a.inputs {
                    validate_expr(expr, funcs)?;
                }
            }
        }
    }
    Ok(())
}
fn calls_expr(e: &Expr, calls: &mut Vec<String>) {
    match &e.kind {
        ExprKind::Call { name, arguments } => {
            calls.push(name.clone());
            for e in arguments {
                calls_expr(e, calls);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            calls_expr(left, calls);
            calls_expr(right, calls);
        }
        ExprKind::Neg(x) => calls_expr(x, calls),
        _ => {}
    }
}
fn calls_stmts(body: &[Stmt], calls: &mut Vec<String>) {
    for s in body {
        match s {
            Stmt::Trap => {}
            Stmt::Bind { value, .. } | Stmt::Assign { value, .. } | Stmt::Evaluate(value) => {
                calls_expr(value, calls)
            }
            Stmt::Return { value, cleanup } => {
                calls_expr(value, calls);
                for e in cleanup {
                    calls_expr(e, calls);
                }
            }
            Stmt::If { condition, yes, no } => {
                calls_expr(condition, calls);
                calls_stmts(yes, calls);
                calls_stmts(no, calls);
            }
            Stmt::While { condition, body } => {
                calls_expr(condition, calls);
                calls_stmts(body, calls);
            }
            Stmt::Unsafe(body) => calls_stmts(body, calls),
            Stmt::Asm(a) => {
                for (_, e) in &a.inputs {
                    calls_expr(e, calls)
                }
            }
        }
    }
}
fn frame_size(f: &Function) -> Res<u16> {
    if f.params.len() > 2 {
        return Err("Thumb ABI supports at most two parameters".into());
    }
    let mut count = f.params.len();
    fn bindings(b: &[Stmt], count: &mut usize) {
        for s in b {
            match s {
                Stmt::Bind { .. } => *count += 1,
                Stmt::Unsafe(b) => bindings(b, count),
                Stmt::If { yes, no, .. } => {
                    bindings(yes, count);
                    bindings(no, count);
                }
                Stmt::While { body, .. } => bindings(body, count),
                _ => {}
            }
        }
    }
    bindings(&f.instructions, &mut count);
    let bytes = count.checked_mul(4).ok_or("Thumb local-size overflow")?;
    let aligned = (bytes + 7) & !7;
    if aligned > 504 {
        return Err(format!(
            "Thumb function {} has too many locals (max 504 bytes)",
            f.name
        ));
    }
    Ok(aligned as u16)
}
fn peak_expr(e: &Expr) -> u32 {
    match &e.kind {
        ExprKind::Binary { left, right, .. } => peak_expr(left).max(8 + peak_expr(right)),
        ExprKind::Call { arguments, .. } => {
            if arguments.len() == 2 {
                peak_expr(&arguments[0]).max(8 + peak_expr(&arguments[1]))
            } else {
                arguments.first().map_or(0, peak_expr)
            }
        }
        ExprKind::Neg(x) => peak_expr(x),
        _ => 0,
    }
}
fn peak_stmts(body: &[Stmt]) -> u32 {
    body.iter()
        .map(|s| match s {
            Stmt::Trap => 0,
            Stmt::Bind { value, .. } | Stmt::Assign { value, .. } | Stmt::Evaluate(value) => {
                peak_expr(value)
            }
            Stmt::Return { value, cleanup } => {
                peak_expr(value).max(cleanup.iter().map(peak_expr).max().unwrap_or(0))
            }
            Stmt::While { condition, body } => peak_expr(condition).max(peak_stmts(body)),
            Stmt::If { condition, yes, no } => peak_expr(condition)
                .max(peak_stmts(yes))
                .max(peak_stmts(no)),
            Stmt::Unsafe(body) => peak_stmts(body),
            Stmt::Asm(a) => {
                a.inputs
                    .iter()
                    .enumerate()
                    .map(|(i, (_, e))| (i as u32) * 8 + peak_expr(e))
                    .max()
                    .unwrap_or(0)
                    + 8 * u32::from(!a.inputs.is_empty())
            }
        })
        .max()
        .unwrap_or(0)
}
fn validate(ir: &Module, layout: DeviceLayout) -> Res<()> {
    if !layout.features.has("thumb2") {
        return Err("Cortex-M Thumb backend requires thumb2 instruction subset".into());
    }
    let funcs: HashMap<&str, &Function> =
        ir.functions.iter().map(|f| (f.name.as_str(), f)).collect();
    if !funcs.contains_key("__main") {
        return Err("Thumb firmware needs __main".into());
    }
    let mut edges = HashMap::new();
    for f in &ir.functions {
        if f.name != "__main"
            && (!matches!(f.result, Ty::Usize | Ty::Bool | Ty::Void)
                || f.params
                    .iter()
                    .any(|(_, t)| !matches!(t, Ty::Usize | Ty::Bool)))
        {
            return Err(format!(
                "Thumb function {} has unsupported ABI type",
                f.name
            ));
        }
        validate_stmts(&f.instructions, f, &funcs)?;
        let mut calls = Vec::new();
        calls_stmts(&f.instructions, &mut calls);
        if calls.iter().any(|n| !funcs.contains_key(n.as_str())) {
            return Err(format!("Thumb function {} calls unknown function", f.name));
        }
        edges.insert(f.name.as_str(), calls);
    }
    fn walk<'a>(
        name: &'a str,
        edges: &HashMap<&'a str, Vec<String>>,
        active: &mut HashSet<String>,
        seen: &mut HashMap<String, u32>,
        frames: &HashMap<String, u32>,
    ) -> Res<u32> {
        if active.contains(name) {
            return Err(format!("Thumb recursion forbidden: {name}"));
        }
        if let Some(v) = seen.get(name) {
            return Ok(*v);
        }
        active.insert(name.to_owned());
        let mut max_child = 0;
        for other in &edges[name] {
            max_child = max_child.max(walk(other, edges, active, seen, frames)?);
        }
        active.remove(name);
        // Temporary expression staging uses up to 8 bytes per nested expression.
        // Conservative 64 byte guard per stack frame; reject if map too small.
        let bytes = frames[name] + 8 + max_child;
        seen.insert(name.to_owned(), bytes);
        Ok(bytes)
    }
    let frames: HashMap<String, u32> = ir
        .functions
        .iter()
        .map(|f| {
            Ok((
                f.name.clone(),
                u32::from(frame_size(f)?) + peak_stmts(&f.instructions) + 16,
            ))
        })
        .collect::<Res<_>>()?;
    let maximum = walk(
        "__main",
        &edges,
        &mut HashSet::new(),
        &mut HashMap::new(),
        &frames,
    )?;
    if maximum > layout.stack_reserve {
        return Err(format!(
            "Thumb call stack estimate {maximum} exceeds reserved {} bytes",
            layout.stack_reserve
        ));
    }
    Ok(())
}
/// `depth` counts bytes pushed for expression temporaries (`push {r0,r2}`).
/// Locals live at fixed offsets from the frame's SP, so every SP-relative
/// access must add `depth` while temporaries are on the stack.
struct Gen {
    enc: Encoder,
    depth: u16,
}
impl Gen {
    fn new(_ir: &Module) -> Self {
        Self {
            enc: Encoder::new(),
            depth: 0,
        }
    }
    fn save(&mut self) {
        self.enc.save0();
        self.depth += 8;
    }
    fn restore(&mut self) {
        self.enc.restore0();
        self.depth -= 8;
    }
    fn load_local(&mut self, slot: u16) -> Res<()> {
        self.enc.load(0, slot + self.depth)
    }
    fn store_local(&mut self, reg: u8, slot: u16) -> Res<()> {
        self.enc.store(reg, slot + self.depth)
    }
    fn scalar(&mut self, e: &Expr, slots: &HashMap<String, u16>) -> Res<()> {
        match &e.kind {
            ExprKind::ConstI64(v) => self.enc.imm32(*v as u32),
            ExprKind::ConstBool(v) => self.enc.movs(0, u8::from(*v)),
            ExprKind::Local(name) => {
                self.load_local(*slots.get(name).ok_or("missing Thumb local slot")?)?
            }
            ExprKind::Binary { op, left, right } => {
                self.scalar(left, slots)?;
                self.save();
                self.scalar(right, slots)?;
                self.enc.move_r1_r0();
                self.restore();
                match op.as_str() {
                    "+" => {
                        self.enc.h(0x1840);
                        let ok = self.enc.cond(3);
                        self.enc.trap();
                        self.enc.patch_cond(ok, self.enc.pos())?;
                    } // BCC
                    "-" => {
                        self.enc.h(0x1A40);
                        let ok = self.enc.cond(2);
                        self.enc.trap();
                        self.enc.patch_cond(ok, self.enc.pos())?;
                    } // BCS
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                        self.enc.h(0x4288); // CMP r0, r1 (unsigned for usize)
                        let cc = match op.as_str() {
                            "==" => 0,
                            "!=" => 1,
                            "<" => 3,
                            "<=" => 9,
                            ">" => 8,
                            ">=" => 2,
                            _ => unreachable!(),
                        };
                        let yes = self.enc.cond(cc);
                        self.enc.movs(0, 0);
                        let end = self.enc.branch();
                        self.enc.patch_cond(yes, self.enc.pos())?;
                        self.enc.movs(0, 1);
                        self.enc.patch_b(end, self.enc.pos())?;
                    }
                    _ => return Err("unsupported Thumb operator".into()),
                }
            }
            ExprKind::Call { name, arguments } => {
                if arguments.len() == 2 {
                    self.scalar(&arguments[0], slots)?;
                    self.save();
                    self.scalar(&arguments[1], slots)?;
                    self.enc.move_r1_r0();
                    self.restore();
                } else if arguments.len() == 1 {
                    self.scalar(&arguments[0], slots)?;
                }
                self.enc.call(name);
            }
            _ => return Err("invalid Thumb scalar expression".into()),
        }
        Ok(())
    }
    fn stmts(&mut self, body: &[Stmt], slots: &HashMap<String, u16>, bytes: u16) -> Res<()> {
        for s in body {
            match s {
                Stmt::Trap => self.enc.trap(),
                Stmt::Bind { name, value, .. } | Stmt::Assign { name, value } => {
                    self.scalar(value, slots)?;
                    self.store_local(0, slots[name])?;
                }
                Stmt::Evaluate(e) => self.scalar(e, slots)?,
                Stmt::Return { value, .. } => {
                    if !matches!(value.ty, Ty::I64) {
                        self.scalar(value, slots)?;
                    } else {
                        self.enc.movs(0, 0);
                    }
                    self.enc.add_sp(bytes)?;
                    self.enc.pop_pc();
                }
                Stmt::Unsafe(stmts) => self.stmts(stmts, slots, bytes)?,
                Stmt::If { condition, yes, no } => {
                    self.scalar(condition, slots)?;
                    self.enc.h(0x2800); // cmp r0,#0
                    let fail = self.enc.cond(0);
                    self.stmts(yes, slots, bytes)?;
                    let done = self.enc.branch();
                    self.enc.patch_cond(fail, self.enc.pos())?;
                    self.stmts(no, slots, bytes)?;
                    self.enc.patch_b(done, self.enc.pos())?;
                }
                Stmt::While { condition, body } => {
                    let start = self.enc.pos();
                    self.scalar(condition, slots)?;
                    self.enc.h(0x2800);
                    let done = self.enc.cond(0);
                    self.stmts(body, slots, bytes)?;
                    let back = self.enc.branch();
                    self.enc.patch_b(back, start)?;
                    self.enc.patch_cond(done, self.enc.pos())?;
                }
                Stmt::Asm(a) => {
                    // Stage operands before assigning registers; nested calls must not
                    // destroy an already evaluated earlier argument.
                    for (_, expr) in &a.inputs {
                        self.scalar(expr, slots)?;
                        self.save();
                    }
                    for (reg, _) in a.inputs.iter().rev() {
                        match reg.as_str() {
                            "r0" => self.enc.h(0xBC05),
                            "r1" => self.enc.h(0xBC06),
                            _ => return Err("invalid Thumb asm register".into()),
                        }
                        self.depth -= 8;
                    }
                    for ins in &a.instructions {
                        match ins.as_str() {
                            "nop" => self.enc.h(0xBF00),
                            "yield" => self.enc.h(0xBF10),
                            "dmb" => self.enc.w(0xF3BF, 0x8F5F),
                            "isb" => self.enc.w(0xF3BF, 0x8F6F),
                            "ldr" => self.enc.h(0x6800), // ldr r0,[r0]
                            "str" => self.enc.h(0x6001), // str r1,[r0]
                            _ => return Err("unsupported Thumb instruction".into()),
                        }
                    }
                    for (name, reg) in &a.outputs {
                        if reg != "r0" {
                            return Err("unsupported Thumb asm output".into());
                        }
                        self.store_local(0, slots[name])?;
                    }
                }
            }
            debug_assert_eq!(
                self.depth, 0,
                "Thumb temporaries must be popped at statement end"
            );
        }
        Ok(())
    }
    fn function(&mut self, f: &Function) -> Res<()> {
        self.enc.label(&f.name)?;
        let mut names: Vec<String> = f.params.iter().map(|(n, _)| n.clone()).collect();
        fn collect(body: &[Stmt], names: &mut Vec<String>) {
            for s in body {
                match s {
                    Stmt::Bind { name, .. } => names.push(name.clone()),
                    Stmt::Unsafe(body) | Stmt::While { body, .. } => collect(body, names),
                    Stmt::If { yes, no, .. } => {
                        collect(yes, names);
                        collect(no, names);
                    }
                    _ => {}
                }
            }
        }
        collect(&f.instructions, &mut names);
        let slots: HashMap<String, u16> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), (i * 4) as u16))
            .collect();
        let bytes = frame_size(f)?;
        self.enc.push_lr();
        self.enc.sub_sp(bytes)?;
        for (i, (n, _)) in f.params.iter().enumerate() {
            self.enc.store(i as u8, slots[n])?;
        }
        self.stmts(&f.instructions, &slots, bytes)?;
        if matches!(f.result, Ty::Void) {
            self.enc.movs(0, 0);
            self.enc.add_sp(bytes)?;
            self.enc.pop_pc();
        }
        Ok(())
    }
}
/// Bare-metal ELF32 with a single FLASH PT_LOAD segment, C-M vector table at
/// offset zero in that segment, and Thumb reset handler immediately afterwards.
/// Endianness is little; e_entry carries Thumb bit 0.
fn elf32_firmware(code: &[u8], layout: DeviceLayout, reset_offset: u32) -> Res<Vec<u8>> {
    let file_offset = 0x100usize;
    let load_addr = layout.flash.start;
    let mut data = vec![0u8; file_offset];
    data[0..16].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    data[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    data[18..20].copy_from_slice(&40u16.to_le_bytes()); // EM_ARM
    data[20..24].copy_from_slice(&1u32.to_le_bytes());
    data[24..28].copy_from_slice(&(load_addr + reset_offset + 1).to_le_bytes());
    data[28..32].copy_from_slice(&52u32.to_le_bytes());
    data[36..40].copy_from_slice(&0x0500_0000u32.to_le_bytes()); // EABI5
    data[40..42].copy_from_slice(&52u16.to_le_bytes());
    data[42..44].copy_from_slice(&32u16.to_le_bytes());
    data[44..46].copy_from_slice(&1u16.to_le_bytes());
    data[52..56].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
    data[56..60].copy_from_slice(&(file_offset as u32).to_le_bytes());
    data[60..64].copy_from_slice(&load_addr.to_le_bytes());
    data[64..68].copy_from_slice(&load_addr.to_le_bytes());
    data[68..72].copy_from_slice(&(code.len() as u32).to_le_bytes());
    data[72..76].copy_from_slice(&(code.len() as u32).to_le_bytes());
    data[76..80].copy_from_slice(&5u32.to_le_bytes()); // read+execute
    data[80..84].copy_from_slice(&0x100u32.to_le_bytes()); // alignment
    data.extend_from_slice(code);
    Ok(data)
}
pub(crate) fn emit(ir: &Module, target: Target) -> Res<Vec<u8>> {
    let layout = target
        .layout
        .ok_or("Thumb target requires a selected device")?;
    validate(ir, layout)?;
    let vectors = usize::from(layout.vectors);
    let base = vectors.checked_mul(4).ok_or("Thumb vector size overflow")?;
    let reset_offset = base as u32;
    let mut generator = Gen::new(ir);
    // Reset runs on the vector-provided MSP. Call main; on return stop with BKPT #1 (normal end).
    generator.enc.call("__main");
    generator.enc.halt();
    // Shared target for unexpected exceptions (vectors 2..): the runtime trap.
    generator.enc.trap();
    // Main and all reachable functions come from the already pruned IR.
    for f in &ir.functions {
        generator.function(f)?;
    }
    let code = generator.enc.finish()?;
    let size = base
        .checked_add(code.len())
        .ok_or("Thumb flash size overflow")?;
    if size > layout.flash.len() as usize {
        return Err("Thumb firmware exceeds configured Flash".into());
    }
    let mut image = vec![0u8; base];
    let top = layout.ram.end;
    image[0..4].copy_from_slice(&top.to_le_bytes());
    image[4..8].copy_from_slice(&(layout.flash.start + reset_offset + 1).to_le_bytes());
    // Other vectors deliberately point to a safe Thumb trap loop, rather than
    // leaving zero values that would branch outside defined Flash.
    // Unexpected exceptions land on the trap (BKPT #0) emitted after the halt sequence.
    let trap_addr = layout.flash.start + reset_offset + 4 + 4 + 1;
    for i in 2..vectors {
        image[4 * i..4 * i + 4].copy_from_slice(&trap_addr.to_le_bytes());
    }
    image.extend_from_slice(&code);
    elf32_firmware(&image, layout, reset_offset)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_and_elf_header() {
        let layout = crate::device_config::DeviceProfile::parse(include_str!(
            "../targets/devices/stm32f103c8.target"
        ))
        .unwrap()
        .layout;
        let ir = Module {
            functions: vec![Function {
                name: "__main".into(),
                params: vec![],
                result: Ty::I64,
                instructions: vec![Stmt::Return {
                    value: Expr {
                        ty: Ty::I64,
                        kind: ExprKind::ConstI64(0),
                    },
                    cleanup: vec![],
                }],
                end_cleanup: vec![],
            }],
        };
        let t = Target::from_device(
            &crate::device_config::DeviceProfile::parse(include_str!(
                "../targets/devices/stm32f103c8.target"
            ))
            .unwrap(),
        );
        let out = emit(&ir, t).unwrap();
        let off = 0x100;
        assert_eq!(&out[18..20], &40u16.to_le_bytes());
        assert_eq!(&out[off..off + 4], &layout.ram.end.to_le_bytes());
        assert_eq!(
            &out[off + 4..off + 8],
            &(layout.flash.start + u32::from(layout.vectors) * 4 + 1).to_le_bytes()
        );
        assert!(out.len() < 0x100 + layout.flash.len() as usize);
        // Reset: BL __main; BKPT #1; B .   Exception vectors: BKPT #0; B .
        let reset = off + usize::from(layout.vectors) * 4;
        assert_eq!(
            &out[reset + 4..reset + 6],
            &[0x01, 0xbe],
            "normal end is BKPT #1"
        );
        assert_eq!(
            &out[reset + 8..reset + 10],
            &[0x00, 0xbe],
            "exception target is BKPT #0"
        );
        let vector2 = u32::from_le_bytes(out[off + 8..off + 12].try_into().unwrap());
        assert_eq!(
            vector2,
            layout.flash.start + u32::from(layout.vectors) * 4 + 8 + 1,
            "vector 2 points at the trap"
        );
    }
}
