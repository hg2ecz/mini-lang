//! Source positions, stable error codes and the error catalogue.
//!
//! Errors travel through the compiler as plain `String`s. A message that has a
//! position in the preprocessed (merged) source carries an internal prefix
//! `@@LINE:COL@@`; `render` turns it into `file:line:col` with the
//! preprocessor's line map and adds a stable error code (`E0xxx`).
//! The codes are assigned here, in one table, so the catalogue printed by
//! `mini --explain` and `docs/HIBAKODOK.md` can never drift from the compiler.
use std::cell::Cell;

thread_local! {
    /// Line (in the merged source) of the statement the checker is visiting.
    /// The checker fails fast, so the line set last belongs to the failing
    /// statement. Single-threaded compiler: a thread-local is sufficient.
    static LINE: Cell<u32> = const { Cell::new(0) };
}
pub(crate) fn set_line(line: u32) {
    LINE.with(|cell| cell.set(line));
}
pub(crate) fn line() -> u32 {
    LINE.with(|cell| cell.get())
}

/// Attach a merged-source position unless the message already has one.
pub(crate) fn at(line: u32, col: u32, message: &str) -> String {
    if line == 0 || message.starts_with("@@") {
        message.to_string()
    } else {
        format!("@@{line}:{col}@@{message}")
    }
}

/// Merged-source line (index 0 = line 1) → (file name, original line).
pub(crate) struct SourceMap {
    pub(crate) lines: Vec<(String, u32)>,
}

fn split(message: &str) -> (Option<(u32, u32)>, &str) {
    if let Some(rest) = message.strip_prefix("@@") {
        if let Some((position, text)) = rest.split_once("@@") {
            if let Some((l, c)) = position.split_once(':') {
                if let (Ok(l), Ok(c)) = (l.parse(), c.parse()) {
                    return (Some((l, c)), text);
                }
            }
        }
    }
    (None, message)
}

pub(crate) struct Entry {
    pub(crate) code: &'static str,
    pub(crate) title_hu: &'static str,
    pub(crate) title_en: &'static str,
    /// Lower-case substrings; the first entry with a matching pattern wins.
    patterns: &'static [&'static str],
    explanation: &'static str,
}

pub(crate) const CATALOGUE: &[Entry] = &[
    Entry {
        code: "E0003",
        title_hu: "Előfeldolgozási hiba",
        title_en: "Preprocessor error",
        patterns: &[
            "#",
            "include",
            "conditional property",
            "pointer_bits",
            " value: ",
            "preprocessor",
        ],
        explanation: "Az `#if`, `#elseif`, `#else`, `#endif`, `#include` vagy `#error` sor hibás, vagy egy `#error` ág aktív lett a választott célra.\nA feltételek csak `arch`, `os`, `abi`, `cpu`, `device`, `feature` és `pointer_bits` tulajdonságot vizsgálhatnak, idézőjeles értékkel.\n\nRossz:  #if arch == x86_64\nJó:     #if arch == \"x86_64\"",
    },
    Entry {
        code: "E0502",
        title_hu: "Hibás eszközprofil",
        title_en: "Invalid device profile",
        patterns: &[
            "device",
            "region",
            "unknown device key",
            "missing/invalid value",
            "invalid decimal number",
            "invalid hex number",
            "invalid unsigned number",
            "too long",
        ],
        explanation: "A `--device` kapcsolóval megadott `.target` fájl hibás: ismeretlen kulcs, rossz számformátum, üres vagy fordított memóriatartomány, vagy olyan CPU-képesség, amelyet a chip nem tud.\nA kulcsok: `device`, `flash`, `ram`, `stack`, `features` (lásd docs/DEVICE_MAP.md).\n\nRossz:  flash = 0x08010000..0x08000000\nJó:     flash = 0x08000000..0x08010000",
    },
    Entry {
        code: "E0001",
        title_hu: "Érvénytelen karakter vagy zárójelezés",
        title_en: "Invalid character or bracket",
        patterns: &[
            "invalid character",
            "unmatched",
            "unclosed parentheses",
            "unclosed array bracket",
            "integer literal out of range",
        ],
        explanation: "A forrás olyan karaktert tartalmaz, amelyet a Mini nem ismer, vagy a zárójelek nem párosak. Az egész literál legfeljebb 9223372036854775807 lehet.\n\nRossz:  return (1 + 2\nJó:     return (1 + 2)",
    },
    Entry {
        code: "E0002",
        title_hu: "Szintaktikai hiba",
        title_en: "Syntax error",
        patterns: &[
            "expected '",
            "expected expression",
            "expected identifier",
            "expected separator",
            "expected statement separator",
            "expected struct, fn or program",
            "unclosed",
            "duplicate program",
            "missing program",
            "reserved",
            "unreachable statement",
        ],
        explanation: "A parser mást várt az adott helyen. Gyakori okok: két utasítás egy sorban `;` nélkül, hiányzó `}` vagy foglalt név használata.\nA `return` után ugyanabban a blokkban nem állhat több utasítás.\n\nRossz:  let a: i64 = 1 let b: i64 = 2\nJó:     let a: i64 = 1; let b: i64 = 2",
    },
    Entry {
        code: "E0201",
        title_hu: "`unsafe` szükséges",
        title_en: "`unsafe` required",
        patterns: &["requires unsafe", "requires an unsafe block"],
        explanation: "A művelet megkerülheti a Mini biztonsági ellenőrzéseit, ezért csak `unsafe` blokkban szabad: `asm`, `raw_addr`, `unsafe fn` hívása, illetve destruktoros (erőforrás-tulajdonos) struct közvetlen létrehozása.\nAz alkalmazáskódban inkább a könyvtár biztonságos függvényét használd.\n\nRossz:  let fd: OwnedFd = OwnedFd(3)\nJó:     let fd: OwnedFd = unsafe { adopt_fd(3) }   // csak ha valóban te birtoklod a leírót",
    },
    Entry {
        code: "E0302",
        title_hu: "Kétszeres felszabadítás",
        title_en: "Double drop",
        patterns: &["double drop"],
        explanation: "Ugyanazt a tulajdonos változót kétszer szabadítanád fel (`drop`). Egy erőforrás (pl. fájlleíró) kétszeri lezárása súlyos hiba, ezért a Mini fordításkor tiltja.\n\nRossz:  drop f\n        drop f\nJó:     drop f",
    },
    Entry {
        code: "E0301",
        title_hu: "Használat felszabadítás után",
        title_en: "Use after drop",
        patterns: &["after drop", "was dropped"],
        explanation: "A változót már felszabadítottad (`drop`), utána nem olvasható, nem írható és nem adható át. A felszabadított erőforrás már nem érvényes.\n\nRossz:  drop f\n        return f.fd\nJó:     let fd: i64 = f.fd\n        drop f\n        return fd",
    },
    Entry {
        code: "E0303",
        title_hu: "Ágfüggő tulajdonjog",
        title_en: "Path-dependent ownership",
        patterns: &["conditional drop", "drop in while", "divergent ownership"],
        explanation: "Az `if` két ága után a tulajdonos változók állapotának egyeznie kell (vagy az egyik ágnak `return`-nel kell végződnie), és ciklusban nem változhat tulajdonjog. Így a fordító minden úton pontosan tudja, mit kell felszabadítani.\n\nRossz:  if c { drop f }\nJó:     if c { drop f } else { drop f }",
    },
    Entry {
        code: "E0305",
        title_hu: "Ágon vagy cikluson belüli `let`",
        title_en: "Branch- or loop-local `let`",
        patterns: &["branch-local let", "loop-local let"],
        explanation: "Ebben a verzióban `let` csak a függvény legfelső szintjén állhat, `if`, `while` és `unsafe` blokkon belül nem. Deklaráld a változót a blokk előtt.\n\nRossz:  if c { let x: i64 = 1 }\nJó:     let x: i64 = 0\n        if c { x = 1 }",
    },
    Entry {
        code: "E0304",
        title_hu: "Tulajdonjog- vagy destruktorszabály",
        title_en: "Ownership or destructor rule",
        patterns: &[
            "destructor",
            "cannot drop borrowed",
            "drop requires",
            "owned aggregate copying",
            "aggregate reassignment",
            "borrowed parameter",
            "aggregate return",
            "aggregate parameter",
            "copy forbidden",
            "copying forbidden",
            "aggregate argument",
            "borrowed struct",
            "unused aggregate",
        ],
        explanation: "A destruktor (`__drop_T`) pontos alakja `fn __drop_T(value: T)`, nem lehet `unsafe`, nincs visszatérési értéke, és közvetlenül nem hívható – helyette `drop változó`.\nKölcsönzött (paraméterként kapott) structot nem szabadíthatsz fel és nem írhatsz felül; tulajdonos structot nem másolhatsz másik változóba.\n\nRossz:  __drop_File(f)\nJó:     drop f",
    },
    Entry {
        code: "E0202",
        title_hu: "Érvénytelen `asm` blokk",
        title_en: "Invalid `asm` block",
        patterns: &[
            "asm",
            "svc",
            "syscall",
            "clobber",
            "register operands",
            "register bindings",
            "assembly",
            "outio",
            "sta address",
        ],
        explanation: "A Mini csak szűk, célonként ellenőrzött assembly-részhalmazt enged: engedélyezett utasítások, deklarált bemeneti/kimeneti regiszterek és kötelező `clobber` lista. A pontos szabályok a célplatform dokumentációjában vannak (docs/AARCH64.md, docs/ARM32.md, docs/EIGHTBIT.md).\n\nRossz:  unsafe { asm { syscall } }\nJó:     unsafe { asm { in rax = 39; syscall; out p = rax; clobber rcx; clobber r11; clobber memory } }",
    },
    Entry {
        code: "E0401",
        title_hu: "Tömbindex a határon kívül (fordításkor)",
        title_en: "Array index out of bounds (compile time)",
        patterns: &["out of bounds"],
        explanation: "A konstans index bizonyíthatóan kívül esik a tömbön. Az érvényes indexek: 0 .. hossz-1.\n\nRossz:  let a: [i64; 4] = [0; 4]\n        return a[4]\nJó:     return a[3]",
    },
    Entry {
        code: "E0402",
        title_hu: "Tömbhasználati korlát",
        title_en: "Array usage limit",
        patterns: &["dynamic indexing", "dynamic array access", "array"],
        explanation: "Változó indexszel jelenleg legfeljebb 16 elemű tömb indexelhető, az index típusa `usize`, és `while` feltételében nem lehet tömbelérés. A tömb hossza 1..1024 konstans, a kezdőérték konstans.\n\nRossz:  while a[i] == 0 { i = i + 1 }\nJó:     let v: usize = a[i]\n        while v == 0 { i = i + 1; v = a[i] }",
    },
    Entry {
        code: "E0104",
        title_hu: "Érvénytelen konverzió",
        title_en: "Invalid conversion",
        patterns: &["'as'"],
        explanation: "Az `as` jelenleg csak egész típusok között működik, és futásidőben ellenőrzött: ha az érték nem fér el, a program trappel.\n\nRossz:  let b: bool = 1 as bool\nJó:     let b: bool = x != 0",
    },
    Entry {
        code: "E0103",
        title_hu: "Literál a típus tartományán kívül",
        title_en: "Literal out of range",
        patterns: &["exceeds target range", "literal"],
        explanation: "A számliterál nem fér el a várt típusban az adott célon (pl. `usize` 32 bites ARMv7-en és Cortex-M-en, 16 bites AVR-en).\n\nRossz:  let x: usize = 70000     // AVR-en\nJó:     let x: usize = 60000",
    },
    Entry {
        code: "E0101",
        title_hu: "Típushiba",
        title_en: "Type error",
        patterns: &[
            "mismatch",
            "requires bool",
            "integer operands",
            "matching integer",
            ": expected",
            "field access requires struct",
            "unary '-'",
            "void is not a storage",
            "struct fields must",
            "missing return",
            "arguments",
            "argument type",
            "aggregate in scalar",
            "struct in scalar",
            "expected scalar",
            "expected struct",
            "not struct",
            "void assignment",
            "void destination",
            "constructor arity",
            "empty struct",
            "struct needs",
            "invalid aggregate",
            "struct call requires",
            "raw_addr",
        ],
        explanation: "A kifejezés típusa nem az, amit a hely megkövetel. A Mini nem végez implicit konverziót: `i64` és `usize` nem keverhető, `bool` nem egész szám, az `if`/`while` feltétele `bool`.\n\nRossz:  let n: usize = 5\n        return n\nJó:     return n as i64",
    },
    Entry {
        code: "E0102",
        title_hu: "Ismeretlen vagy ismételt név",
        title_en: "Unknown or duplicate name",
        patterns: &[
            "unknown function",
            "unknown array",
            "unknown variable",
            "unknown local",
            "duplicate",
            "collision",
            "missing entry",
        ],
        explanation: "A név nincs deklarálva, vagy ugyanabban a hatókörben kétszer szerepel. Függvényt `#include`-dal is behozhatsz; a változót használat előtt `let`-tel kell deklarálni.\n\nRossz:  return y\nJó:     let y: i64 = 1\n        return y",
    },
    Entry {
        code: "E0601",
        title_hu: "Méret- vagy veremkorlát",
        title_en: "Size or stack limit",
        patterns: &[
            "recursion",
            "stack",
            "too many",
            "exceeds",
            "too large",
            "flash",
            "1 mib",
        ],
        explanation: "A program túllépi a célplatform egy korlátját: veremméret, Flash-méret, paraméterszám, vagy MCU-n tiltott rekurzió.\nMikrovezérlőn a fordító előre kiszámolja a legrosszabb veremigényt, és ha nem fér el, fordítási hibát ad.",
    },
    Entry {
        code: "E0501",
        title_hu: "A célplatform ezt nem támogatja",
        title_en: "Not supported on this target",
        patterns: &[
            "not implemented",
            "backend",
            "target",
            "thumb",
            "8-bit",
            "c64",
            "avr",
            "aarch64",
            "arm32",
            "x86_64",
            "cortex",
            "mcu",
            "numeric type",
            "unsupported comparison",
            "unsupported operator",
            "unknown operator",
            "6510",
            "intel hex",
            "relocation",
            "unsupported struct return",
        ],
        explanation: "A program helyes Mini, de a választott cél kódgenerátora ezt a műveletet vagy típust még nem tudja biztonságosan lefordítani, ezért a fordító megtagadja (fail-closed). A támogatási mátrix a kézikönyvben található.\n\nPélda: Cortex-M-en jelenleg csak `usize` és `bool` kifejezések, legfeljebb két paraméter.",
    },
    Entry {
        code: "E0000",
        title_hu: "Egyéb hiba",
        title_en: "Other error",
        patterns: &[],
        explanation: "Ehhez az üzenethez még nincs külön hibakód. Ha gyakran találkozol vele, jelezd: kap saját kódot és magyarázatot.",
    },
];

pub(crate) fn classify(message: &str) -> &'static Entry {
    let lower = split(message).1.to_lowercase();
    CATALOGUE
        .iter()
        .find(|e| e.patterns.iter().any(|p| lower.contains(p)))
        .unwrap_or_else(|| CATALOGUE.last().expect("catalogue has E0000"))
}

/// The text printed for a compiler error on stderr.
pub(crate) fn render(message: &str, map: Option<&SourceMap>) -> String {
    let entry = classify(message);
    let (position, text) = split(message);
    let mut out = format!("mini: error[{}]: {text}", entry.code);
    if let Some((line, col)) = position {
        let (file, original) = map
            .and_then(|m| m.lines.get(line as usize - 1).cloned())
            .unwrap_or_else(|| ("<forrás>".to_string(), line));
        let location = if col > 0 {
            format!("{file}:{original}:{col}")
        } else {
            format!("{file}:{original}")
        };
        out.push_str(&format!("\n  --> {location}"));
        // Show the offending source line, with a caret under the column if known.
        if let Some(source) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|text| text.lines().nth(original as usize - 1).map(str::to_string))
        {
            let number = original.to_string();
            let pad = " ".repeat(number.len());
            out.push_str(&format!("\n {pad} |\n {number} | {source}"));
            if col > 0 {
                let indent: String = source
                    .chars()
                    .take(col as usize - 1)
                    .map(|c| if c == '\t' { '\t' } else { ' ' })
                    .collect();
                out.push_str(&format!("\n {pad} | {indent}^"));
            }
        }
    }
    if entry.code != "E0000" {
        out.push_str(&format!(
            "\n  = részletek / details: mini --explain {}",
            entry.code
        ));
    }
    out
}

pub(crate) fn explain(code: &str) -> Option<String> {
    CATALOGUE
        .iter()
        .find(|e| e.code.eq_ignore_ascii_case(code))
        .map(|e| {
            format!(
                "{} – {} / {}\n\n{}\n",
                e.code, e.title_hu, e.title_en, e.explanation
            )
        })
}

/// Markdown error catalogue; `docs/HIBAKODOK.md` must equal this output.
pub(crate) fn catalogue_markdown() -> String {
    let mut out = String::from(
        "# Mini hibakódok / Error codes\n\n> Ezt a fájlt a fordító generálja: `mini --print-errors > docs/HIBAKODOK.md`.\n> Egy kód magyarázata parancssorból: `mini --explain E0301`.\n\n| Kód | Jelentés | Meaning |\n|---|---|---|\n",
    );
    let mut sorted: Vec<&Entry> = CATALOGUE.iter().collect();
    sorted.sort_by_key(|e| e.code);
    for e in &sorted {
        out.push_str(&format!(
            "| [{}](#{}) | {} | {} |\n",
            e.code,
            e.code.to_lowercase(),
            e.title_hu,
            e.title_en
        ));
    }
    for e in &sorted {
        out.push_str(&format!(
            "\n## {}\n\n**{}** / *{}*\n\n```text\n{}\n```\n",
            e.code, e.title_hu, e.title_en, e.explanation
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_round_trip() {
        let m = SourceMap {
            lines: vec![("a.mini".into(), 1), ("lib/b.mini".into(), 7)],
        };
        let r = render(&at(2, 5, "double drop: f"), Some(&m));
        assert!(r.starts_with("mini: error[E0302]: double drop: f"), "{r}");
        assert!(r.contains("--> lib/b.mini:7:5"), "{r}");
        assert_eq!(
            at(3, 1, &at(2, 5, "x")),
            at(2, 5, "x"),
            "first position wins"
        );
    }
    #[test]
    fn representative_messages_get_codes() {
        for (message, code) in [
            ("use after drop: g", "E0301"),
            ("double drop: g", "E0302"),
            ("call to unsafe fn danger requires unsafe block", "E0201"),
            ("expected statement separator, got Id(\"return\")", "E0002"),
            ("return type mismatch: Usize vs I64", "E0101"),
            ("array index 4 out of bounds for a (length 4)", "E0401"),
            ("conditional drop must agree on both branches", "E0303"),
            ("asm output after drop: p", "E0301"),
            ("/x/y.mini:3: #error requires an 8-bit target", "E0003"),
            ("Thumb recursion forbidden: f", "E0601"),
            ("backend not implemented for Riscv64", "E0501"),
        ] {
            assert_eq!(classify(message).code, code, "{message}");
        }
    }
    /// Every error message literal in the compiler sources should map to a
    /// specific code. Only internal errors and driver usage errors may stay E0000.
    #[test]
    fn compiler_messages_are_classified() {
        let sources = [
            include_str!("main.rs"),
            include_str!("ir.rs"),
            include_str!("preprocess.rs"),
            include_str!("device_config.rs"),
            include_str!("target.rs"),
            include_str!("tiny8.rs"),
            include_str!("backend_x86.rs"),
            include_str!("backend_aarch64.rs"),
            include_str!("backend_arm32.rs"),
            include_str!("backend_thumb.rs"),
            include_str!("code_buffer.rs"),
            include_str!("encode_thumb.rs"),
            include_str!("encode_x86_64.rs"),
            include_str!("encode_aarch64.rs"),
            include_str!("encode_arm32.rs"),
        ];
        let mut unclassified = Vec::new();
        for source in sources {
            for prefix in ["Err(\"", "Err(format!(\""] {
                for (start, _) in source.match_indices(prefix) {
                    let text = &source[start + prefix.len()..];
                    let Some(end) = text.find('"') else { continue };
                    let message = &text[..end];
                    if message.len() < 4 || message.starts_with("internal") {
                        continue;
                    }
                    if classify(message).code == "E0000" {
                        unclassified.push(message.to_string());
                    }
                }
            }
        }
        unclassified.sort();
        unclassified.dedup();
        let allowed = ["Usage: mini", "cannot write output", "unknown error code"];
        unclassified.retain(|m| !allowed.iter().any(|a| m.starts_with(a)));
        assert!(
            unclassified.is_empty(),
            "messages without an error code: {unclassified:#?}"
        );
    }
    #[test]
    fn every_code_is_unique_and_explained() {
        let mut seen = std::collections::HashSet::new();
        for e in CATALOGUE {
            assert!(seen.insert(e.code));
            assert!(explain(e.code).is_some());
        }
    }
}
