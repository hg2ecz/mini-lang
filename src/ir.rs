//! Typed, target-neutral high-level IR (HIR).
//! The x86-64 backend consumes this representation, not the parser AST.
use super::{Ast, Ex, Frame, Res, St, Ty, type_expr};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct Expr {
    pub ty: Ty,
    pub kind: ExprKind,
}
#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
    ConstI64(i64),
    ConstBool(bool),
    Local(String),
    Field {
        owner: String,
        field: String,
    },
    Neg(Box<Expr>),
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        arguments: Vec<Expr>,
    },
    Cast {
        value: Box<Expr>,
        to: Ty,
    },
}
#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Bind {
        name: String,
        ty: Ty,
        value: Expr,
    },
    Assign {
        name: String,
        value: Expr,
    },
    Return {
        value: Expr,
        cleanup: Vec<Expr>,
    },
    Evaluate(Expr),
    If {
        condition: Expr,
        yes: Vec<Stmt>,
        no: Vec<Stmt>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    Unsafe(Vec<Stmt>),
    Asm(AsmIr),
}
#[derive(Debug, Clone)]
/// Target-specific clobbers are validated in check_asm() in main.rs
/// before lowering. They are not machine operations and must not be
/// interpreted here as unchecked register-allocation metadata.
pub(crate) struct AsmIr {
    pub inputs: Vec<(String, Expr)>,
    pub outputs: Vec<(String, String)>,
    pub instructions: Vec<String>,
}
#[derive(Debug, Clone)]
pub(crate) struct Function {
    pub name: String,
    pub params: Vec<(String, Ty)>,
    pub result: Ty,
    pub instructions: Vec<Stmt>,
    /// Automatically emitted destructor calls at the end of void functions.
    pub end_cleanup: Vec<Expr>,
}
#[derive(Debug, Clone)]
pub(crate) struct Module {
    pub functions: Vec<Function>,
}

fn expr(e: &Ex, vars: &HashMap<String, Ty>, ast: &Ast, fs: &HashMap<String, usize>) -> Res<Expr> {
    let ty = type_expr(e, vars, ast, fs)?;
    let kind = match e {
        Ex::Num(v) => ExprKind::ConstI64(*v),
        Ex::Bool(v) => ExprKind::ConstBool(*v),
        Ex::Var(name) => ExprKind::Local(name.clone()),
        Ex::Field(owner, field) => ExprKind::Field {
            owner: owner.clone(),
            field: field.clone(),
        },
        Ex::Neg(e) => ExprKind::Neg(Box::new(expr(e, vars, ast, fs)?)),
        Ex::Cast(e, ty) => ExprKind::Cast {
            value: Box::new(expr(e, vars, ast, fs)?),
            to: ty.clone(),
        },
        Ex::Bin(a, op, b) => ExprKind::Binary {
            op: op.clone(),
            left: Box::new(expr(a, vars, ast, fs)?),
            right: Box::new(expr(b, vars, ast, fs)?),
        },
        Ex::Call(name, args) => ExprKind::Call {
            name: name.clone(),
            arguments: args
                .iter()
                .map(|a| expr(a, vars, ast, fs))
                .collect::<Res<Vec<_>>>()?,
        },
    };
    Ok(Expr { ty, kind })
}
/// Flow-sensitive ownership of local struct values. All struct arguments are
/// borrowed in this restricted ABI, so only `let` bindings enter this set.
/// Local names are function-unique (no shadowing).
#[derive(Clone, Debug, Default)]
struct Lifetime {
    initialized: Vec<String>,
    dead: HashSet<String>,
}

fn hook_call(name: &str, ty: &Ty, fs: &HashMap<String, usize>) -> Option<Expr> {
    let Ty::Struct(s) = ty else {
        return None;
    };
    let hook = format!("__drop_{s}");
    if !fs.contains_key(&hook) {
        return None;
    }
    Some(Expr {
        ty: Ty::Void,
        kind: ExprKind::Call {
            name: hook,
            // The destructor's parameter is a *temporary borrow*, never a move.
            arguments: vec![Expr {
                ty: ty.clone(),
                kind: ExprKind::Local(name.to_string()),
            }],
        },
    })
}

fn live_cleanups(
    life: &Lifetime,
    vars: &HashMap<String, Ty>,
    fs: &HashMap<String, usize>,
) -> Vec<Expr> {
    // Last local created is destroyed first. No dynamic owner flags are needed:
    // if/else ownership states are validated, and ownership changes in loops
    // are rejected before lowering.
    life.initialized
        .iter()
        .rev()
        .filter(|n| !life.dead.contains(*n))
        .filter_map(|n| {
            hook_call(
                n.as_str(),
                vars.get(n.as_str()).expect("checked local type"),
                fs,
            )
        })
        .collect()
}

fn always_returns(body: &[St]) -> bool {
    body.iter().any(|s| match s {
        St::Ret(_) => true,
        St::Unsafe(children) => always_returns(children),
        St::If(_, yes, no) => always_returns(yes) && always_returns(no),
        _ => false,
    })
}

fn stmts(
    block: &[St],
    life: &mut Lifetime,
    vars: &HashMap<String, Ty>,
    ast: &Ast,
    fs: &HashMap<String, usize>,
) -> Res<Vec<Stmt>> {
    let mut result = Vec::new();
    for s in block {
        match s {
            St::Let(name, ty, e) => {
                result.push(Stmt::Bind {
                    name: name.clone(),
                    ty: ty.clone(),
                    value: expr(e, vars, ast, fs)?,
                });
                life.initialized.push(name.clone());
            }
            St::Assign(name, value) => result.push(Stmt::Assign {
                name: name.clone(),
                value: expr(value, vars, ast, fs)?,
            }),
            St::Drop(name) => {
                // check_block has already established that this is a live,
                // non-borrowed struct, and rejects any subsequent use.
                if let Some(dtor) = hook_call(name, &vars[name], fs) {
                    result.push(Stmt::Evaluate(dtor));
                }
                // A trivially destructible struct needs no IR operation at all.
                life.dead.insert(name.clone());
            }
            St::Ret(value) => result.push(Stmt::Return {
                value: expr(value, vars, ast, fs)?,
                cleanup: live_cleanups(life, vars, fs),
            }),
            St::Expr(value) => result.push(Stmt::Evaluate(expr(value, vars, ast, fs)?)),
            St::If(condition, yes, no) => {
                let mut left = life.clone();
                let mut right = life.clone();
                let a = stmts(yes, &mut left, vars, ast, fs)?;
                let b = stmts(no, &mut right, vars, ast, fs)?;
                result.push(Stmt::If {
                    condition: expr(condition, vars, ast, fs)?,
                    yes: a,
                    no: b,
                });
                let yes_returns = always_returns(yes);
                let no_returns = always_returns(no);
                if yes_returns && !no_returns {
                    *life = right;
                } else if !yes_returns && no_returns {
                    *life = left;
                } else if !yes_returns && !no_returns {
                    if left.dead != right.dead || left.initialized != right.initialized {
                        return Err("internal: divergent ownership states in IR".into());
                    }
                    *life = left;
                }
            }
            St::While(condition, body) => {
                let mut nested = life.clone();
                let lowered = stmts(body, &mut nested, vars, ast, fs)?;
                // check_block rejects loop ownership changes along continuing
                // paths. A return from inside the loop already has cleanup.
                result.push(Stmt::While {
                    condition: expr(condition, vars, ast, fs)?,
                    body: lowered,
                });
            }
            St::Unsafe(body) => result.push(Stmt::Unsafe(stmts(body, life, vars, ast, fs)?)),
            St::Asm(a) => result.push(Stmt::Asm(AsmIr {
                inputs: a
                    .inputs
                    .iter()
                    .map(|(r, v)| Ok((r.clone(), expr(v, vars, ast, fs)?)))
                    .collect::<Res<Vec<_>>>()?,
                outputs: a.outputs.clone(),
                instructions: a.instructions.clone(),
            })),
        }
    }
    Ok(result)
}

pub(crate) fn lower(
    ast: &Ast,
    frames: &HashMap<String, Frame>,
    fs: &HashMap<String, usize>,
) -> Res<Module> {
    let functions = ast
        .funcs
        .iter()
        .map(|f| {
            let frame = frames.get(&f.name).ok_or("missing checked frame")?;
            let vars: HashMap<String, Ty> = frame
                .vars
                .iter()
                .map(|(n, (_, t))| (n.clone(), t.clone()))
                .collect();
            let mut life = Lifetime::default();
            let instructions = stmts(&f.body, &mut life, &vars, ast, fs)?;
            let end_cleanup = if f.ret == Ty::Void {
                live_cleanups(&life, &vars, fs)
            } else {
                Vec::new()
            };
            Ok(Function {
                name: f.name.clone(),
                params: f.args.clone(),
                result: f.ret.clone(),
                instructions,
                end_cleanup,
            })
        })
        .collect::<Res<Vec<_>>>()?;
    Module { functions }.prune_unreachable("__main")
}

// Traverse all direct call sites, including calls in nested branches,
// unsafe blocks and assembly input operands. Constructors and raw_addr are
// not user functions, so they are not code-generation roots.
fn collect_expr_calls(e: &Expr, calls: &mut Vec<String>, defined: &HashSet<&str>) {
    match &e.kind {
        ExprKind::Call { name, arguments } => {
            if defined.contains(name.as_str()) {
                calls.push(name.clone());
            }
            for a in arguments {
                collect_expr_calls(a, calls, defined);
            }
        }
        ExprKind::Binary { left, right, .. } => {
            collect_expr_calls(left, calls, defined);
            collect_expr_calls(right, calls, defined);
        }
        ExprKind::Neg(value) | ExprKind::Cast { value, .. } => {
            collect_expr_calls(value, calls, defined)
        }
        _ => {}
    }
}
fn collect_stmt_calls(body: &[Stmt], calls: &mut Vec<String>, defined: &HashSet<&str>) {
    for stmt in body {
        match stmt {
            Stmt::Bind { value, .. } | Stmt::Assign { value, .. } | Stmt::Evaluate(value) => {
                collect_expr_calls(value, calls, defined)
            }
            Stmt::Return { value, cleanup } => {
                collect_expr_calls(value, calls, defined);
                for destructor in cleanup {
                    collect_expr_calls(destructor, calls, defined);
                }
            }
            Stmt::If { condition, yes, no } => {
                collect_expr_calls(condition, calls, defined);
                collect_stmt_calls(yes, calls, defined);
                collect_stmt_calls(no, calls, defined);
            }
            Stmt::While { condition, body } => {
                collect_expr_calls(condition, calls, defined);
                collect_stmt_calls(body, calls, defined);
            }
            Stmt::Unsafe(body) => collect_stmt_calls(body, calls, defined),
            Stmt::Asm(asm) => {
                for (_, value) in &asm.inputs {
                    collect_expr_calls(value, calls, defined);
                }
            }
        }
    }
}

impl Module {
    pub(crate) fn prune_unreachable(mut self, root: &str) -> Res<Self> {
        let names: HashSet<&str> = self.functions.iter().map(|f| f.name.as_str()).collect();
        if !names.contains(root) {
            return Err(format!("missing entry function {root}"));
        }
        let mut reachable = HashSet::new();
        let mut pending = vec![root.to_string()];
        while let Some(name) = pending.pop() {
            if !reachable.insert(name.clone()) {
                continue;
            }
            let function = self
                .functions
                .iter()
                .find(|f| f.name == name)
                .ok_or_else(|| format!("missing IR function {name}"))?;
            collect_stmt_calls(&function.instructions, &mut pending, &names);
            for destructor in &function.end_cleanup {
                collect_expr_calls(destructor, &mut pending, &names);
            }
        }
        drop(names);
        self.functions.retain(|f| reachable.contains(&f.name));
        Ok(self)
    }
    pub(crate) fn dump(&self) -> String {
        // This is a human-readable, typed HIR dump, NOT a stable serialization format.
        format!("{self:#?}\n")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{P, check, lex};
    #[test]
    fn lowers_typed_mixed_result() {
        let source = "struct R { value: i64; ok: bool } fn f(x: i64) -> R { return R(x, true) } program { let r: R = f(42); return r.value }";
        let ast = P::new(lex(source).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        let hir = lower(&ast, &frames, &fs).unwrap();
        assert!(hir.dump().contains("ConstBool(true)"));
        assert!(hir.dump().contains("Struct(\"R\")"));
    }
}

#[cfg(test)]
mod reachability_tests {
    use super::*;
    use crate::{P, check, lex};
    fn selected(source: &str) -> Vec<String> {
        let ast = P::new(lex(source).unwrap()).parse().unwrap();
        let (fs, frames) = check(&ast).unwrap();
        lower(&ast, &frames, &fs)
            .unwrap()
            .functions
            .into_iter()
            .map(|f| f.name)
            .collect()
    }
    #[test]
    fn omits_unused_functions_and_preserves_transitive_calls() {
        let names = selected(
            "fn dead() -> i64 { return 99 } fn leaf() -> i64 { return 42 } fn used() -> i64 { return leaf() } program { return used() }",
        );
        assert!(names.contains(&"used".into()));
        assert!(names.contains(&"leaf".into()));
        assert!(names.contains(&"__main".into()));
        assert!(!names.contains(&"dead".into()));
    }
    #[test]
    fn preserves_recursive_cycles_and_branch_references() {
        let names = selected(
            "fn a() -> i64 { return b() } fn b() -> i64 { return a() } fn dead() -> i64 { return 90 } program { if true { return a() } else { return 0 } }",
        );
        assert_eq!(names.len(), 3);
        assert!(!names.contains(&"dead".into()));
    }
    #[test]
    fn includes_functions_called_from_asm_input() {
        let names = selected(
            "fn number() -> i64 { return 39 } fn unused() -> i64 { return 2 } program { let p: i64 = 0; unsafe { asm { in rax = number(); syscall; out p = rax; clobber rcx; clobber r11; clobber memory } }; return p }",
        );
        assert!(names.contains(&"number".into()));
        assert!(!names.contains(&"unused".into()));
    }
}
