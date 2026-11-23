//! Strict, target-aware, line-oriented source preprocessor.
//! An inactive branch does not open files or parse Mini syntax.
use crate::device_config::{Features, known_device};
use crate::target::{Arch, Environment, FloatAbi, Target};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

type Res<T> = Result<T, String>;
#[derive(Clone, Copy, Debug)]
struct Branch {
    parent_active: bool,
    taken: bool,
    else_seen: bool,
}

fn target_arch(t: Target) -> &'static str {
    match t.arch {
        Arch::X86_64 => "x86_64",
        Arch::Aarch64 => "aarch64",
        Arch::Armv7 => "armv7",
        Arch::Thumbv7m => "thumbv7m",
        Arch::Riscv32 => "riscv32",
        Arch::Riscv64 => "riscv64",
        Arch::Avr8 => "avr8",
        Arch::Mos6510 => "mos6510",
    }
}
fn target_os(t: Target) -> &'static str {
    match t.environment {
        Environment::Linux => "linux",
        Environment::BareMetal => "none",
        Environment::Commodore64 => "c64",
    }
}
fn target_abi(t: Target) -> &'static str {
    match t.float_abi {
        FloatAbi::None => "none",
        FloatAbi::Soft => "soft",
        FloatAbi::Hard => "hard",
    }
}
fn condition(expr: &str, t: Target) -> Res<bool> {
    let tokens: Vec<&str> = expr.split_whitespace().collect();
    if tokens.len() != 3 || !["==", "!="].contains(&tokens[1]) {
        return Err(format!(
            "invalid #if condition: {expr}; expected arch/os/abi/cpu/device/feature/pointer_bits == value"
        ));
    }
    let result = match tokens[0] {
        "pointer_bits" => {
            let n: u8 = tokens[2]
                .parse()
                .map_err(|_| format!("invalid pointer_bits value: {}", tokens[2]))?;
            if ![16, 32, 64].contains(&n) {
                return Err("pointer_bits must be 16, 32 or 64".into());
            }
            t.address_bits == n
        }
        name @ ("arch" | "os" | "abi" | "cpu" | "device" | "feature") => {
            let value = tokens[2]
                .strip_prefix('"')
                .and_then(|x| x.strip_suffix('"'))
                .ok_or_else(|| "string condition value must be quoted".to_string())?;
            let known = match name {
                "arch" => [
                    "x86_64", "aarch64", "armv7", "thumbv7m", "riscv32", "riscv64", "avr8",
                    "mos6510",
                ]
                .contains(&value),
                "os" => ["linux", "none", "c64"].contains(&value),
                "abi" => ["none", "soft", "hard"].contains(&value),
                "cpu" => ["none", "avr5", "cortex-m3", "cortex-m4"].contains(&value),
                "feature" => Features::bit(value).is_some(),
                "device" => value == "none" || known_device(value),
                _ => false,
            };
            if !known {
                return Err(format!("unknown {name} value: {value}"));
            }
            match name {
                "arch" => target_arch(t) == value,
                "os" => target_os(t) == value,
                "abi" => target_abi(t) == value,
                "cpu" => t.cpu_name() == value,
                "device" => t.device_name() == value,
                "feature" => t.feature(value),
                _ => false,
            }
        }
        other => return Err(format!("unsupported conditional property: {other}")),
    };
    Ok(if tokens[1] == "==" { result } else { !result })
}

struct Loader {
    target: Target,
    seen: HashSet<PathBuf>,
    active: HashSet<PathBuf>,
    output: String,
    map: Vec<(String, u32)>,
}
impl Loader {
    fn visit(&mut self, filename: &Path) -> Res<()> {
        let path = fs::canonicalize(filename)
            .map_err(|e| format!("include {}: {e}", filename.display()))?;
        if self.active.contains(&path) {
            return Err(format!("cyclic include: {}", path.display()));
        }
        if !self.seen.insert(path.clone()) {
            return Ok(());
        }
        self.active.insert(path.clone());
        let result = self.process(&path);
        self.active.remove(&path);
        result
    }
    fn process(&mut self, path: &Path) -> Res<()> {
        let source = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut stack: Vec<Branch> = Vec::new();
        let mut enabled = true;
        for (i, line) in source.lines().enumerate() {
            let line_no = i + 1;
            let trimmed = line.trim();
            let here = || format!("{}:{}", path.display(), line_no);
            if let Some(cond) = trimmed.strip_prefix("#if ") {
                // Validate even unreachable branches, but only evaluate when parent is active.
                let yes = condition(cond, self.target).map_err(|e| format!("{}: {e}", here()))?;
                stack.push(Branch {
                    parent_active: enabled,
                    taken: enabled && yes,
                    else_seen: false,
                });
                enabled = enabled && yes;
            } else if let Some(cond) = trimmed.strip_prefix("#elseif ") {
                let yes = condition(cond, self.target).map_err(|e| format!("{}: {e}", here()))?;
                let branch = stack
                    .last_mut()
                    .ok_or_else(|| format!("{}: #elseif without #if", here()))?;
                if branch.else_seen {
                    return Err(format!("{}: #elseif after #else", here()));
                }
                enabled = branch.parent_active && !branch.taken && yes;
                branch.taken |= enabled;
            } else if trimmed == "#else" {
                let branch = stack
                    .last_mut()
                    .ok_or_else(|| format!("{}: #else without #if", here()))?;
                if branch.else_seen {
                    return Err(format!("{}: duplicate #else", here()));
                }
                branch.else_seen = true;
                enabled = branch.parent_active && !branch.taken;
                branch.taken |= enabled;
            } else if trimmed == "#endif" {
                let branch = stack
                    .pop()
                    .ok_or_else(|| format!("{}: #endif without #if", here()))?;
                enabled = branch.parent_active;
            } else if trimmed.starts_with('#') {
                if let Some(tail) = trimmed.strip_prefix("#include ") {
                    // An inactive include must not be opened.
                    if enabled {
                        let relative = tail
                            .trim()
                            .strip_prefix('"')
                            .and_then(|x| x.strip_suffix('"'))
                            .ok_or_else(|| format!("{}: invalid #include", here()))?;
                        if relative.is_empty() {
                            return Err(format!("{}: empty include", here()));
                        }
                        let next = path
                            .parent()
                            .ok_or_else(|| format!("{}: missing parent directory", here()))?
                            .join(relative);
                        self.visit(&next)?;
                    }
                } else if let Some(msg) = trimmed.strip_prefix("#error ") {
                    if enabled {
                        return Err(format!("{}: #error {msg}", here()));
                    }
                } else {
                    return Err(format!("{}: unknown preprocessor directive", here()));
                }
            } else if enabled {
                self.output.push_str(line);
                self.output.push('\n');
                self.map.push((path.display().to_string(), line_no as u32));
            }
        }
        if !stack.is_empty() {
            return Err(format!("{}: unterminated #if", path.display()));
        }
        Ok(())
    }
}

pub(crate) fn load_source(path: &Path, target: Target) -> Res<String> {
    Ok(load_source_mapped(path, target)?.0)
}
/// Like `load_source`, plus the original file and line of every output line,
/// so diagnostics can point into included files.
pub(crate) fn load_source_mapped(
    path: &Path,
    target: Target,
) -> Res<(String, crate::diagnostics::SourceMap)> {
    let mut loader = Loader {
        target,
        seen: HashSet::new(),
        active: HashSet::new(),
        output: String::new(),
        map: Vec::new(),
    };
    loader.visit(path)?;
    Ok((
        loader.output,
        crate::diagnostics::SourceMap { lines: loader.map },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn x86() -> Target {
        Target::parse("x86_64-linux").unwrap()
    }
    #[test]
    fn strict_conditions() {
        assert!(condition("arch == \"x86_64\"", x86()).unwrap());
        assert!(condition("pointer_bits == 64", x86()).unwrap());
        assert!(!condition("os == \"none\"", x86()).unwrap());
        assert!(condition("abi != \"soft\"", x86()).unwrap());
        assert!(condition("arch == \"x86_65\"", x86()).is_err());
        assert!(condition("unknown == 12", x86()).is_err());
    }
    #[test]
    fn nested_conditionals_include_and_errors() {
        let base = std::env::temp_dir().join(format!(
            "mini-preproc-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&base).unwrap();
        let main = base.join("main.mini");
        fs::write(
            base.join("helper.mini"),
            "fn forty() -> i64 { return 40 }\n",
        )
        .unwrap();
        fs::write(
            &main,
            r#"
#if arch == "x86_64"
  #if os == "linux"
    #include "helper.mini"
  #else
    #error bad OS
  #endif
#else
  #include "missing.mini"
#endif
program { return forty() + 2 }
"#,
        )
        .unwrap();
        let src = load_source(&main, x86()).unwrap();
        assert!(src.contains("fn forty"));
        assert!(!src.contains("#include"));
        fs::write(&main, "#if pointer_bits == 32\nprogram { return 1 }\n").unwrap();
        assert!(
            load_source(&main, x86())
                .unwrap_err()
                .contains("unterminated")
        );
        fs::write(&main, "#else\n").unwrap();
        assert!(
            load_source(&main, x86())
                .unwrap_err()
                .contains("without #if")
        );
        let _ = fs::remove_dir_all(base);
    }
    #[test]
    fn features_select_includes_and_reject_unknown() {
        let a = crate::device_config::DeviceProfile::parse(include_str!(
            "../targets/devices/atmega328p.target"
        ))
        .unwrap();
        let avr = Target::from_device(&a);
        assert!(condition("cpu == \"avr5\"", avr).unwrap());
        assert!(condition("device == \"atmega328p\"", avr).unwrap());
        assert!(condition("feature == \"mul\"", avr).unwrap());
        assert!(!condition("feature == \"fpv4_sp\"", avr).unwrap());
        assert!(condition("feature == \"typo\"", avr).is_err());
        let arm = crate::device_config::DeviceProfile::parse(include_str!(
            "../targets/devices/stm32f103c8.target"
        ))
        .unwrap();
        let thumb = Target::from_device(&arm);
        assert!(condition("feature == \"thumb2\"", thumb).unwrap());
        assert!(!condition("feature == \"fpv4_sp\"", thumb).unwrap());
        let fpu = crate::device_config::DeviceProfile::parse(include_str!(
            "../targets/devices/stm32f401cc.target"
        ))
        .unwrap();
        let m4 = Target::from_device(&fpu);
        assert!(condition("feature == \"fpv4_sp\"", m4).unwrap());
        assert!(condition("cpu == \"cortex-m4\"", m4).unwrap());
        assert!(condition("device == \"stm32f401cc\"", m4).unwrap());
        // Hardware FPU does not automatically turn on a hard-float calling convention.
        assert!(condition("abi == \"soft\"", m4).unwrap());
        let base = std::env::temp_dir().join(format!(
            "mini-feat-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&base).unwrap();
        fs::write(
            base.join("needed.mini"),
            "fn eligible() -> usize { return 42 }\n",
        )
        .unwrap();
        let main = base.join("program.mini");
        fs::write(
            &main,
            r#"#if arch == "avr8"
#if feature == "mul"
#include "needed.mini"
#else
#include "missing.mini"
#endif
#else
#error Wrong target
#endif
program { return 0 }
"#,
        )
        .unwrap();
        let source = load_source(&main, avr).unwrap();
        assert!(source.contains("fn eligible"));
        assert!(!source.contains("missing.mini"));
        let _ = fs::remove_dir_all(base);
    }
    #[test]
    fn target_specific_avr_path() {
        let avr = Target::parse("avr8-none").unwrap();
        assert!(condition("arch == \"avr8\"", avr).unwrap());
        assert!(condition("pointer_bits == 16", avr).unwrap());
    }
}
