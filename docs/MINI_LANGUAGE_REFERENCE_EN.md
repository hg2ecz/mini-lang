# Mini Language Reference – From Variables to Structures

**Language version:** Mini Native 4.4.0 (2026-11-21)  
**Source of truth:** inspection of `src/main.rs`, `src/preprocess.rs`, `src/target.rs`, and target backends in the 4.4 source tree.  
**Principle:** *Mini should do less, but do it reliably.*

> This is a practical reference for the **currently implemented language**, not a specification of planned features. Support differs by target architecture. Examples are aligned with the compiler source, but end-to-end execution has not been verified for every example and target. The language does **not** yet provide a general memory-safety guarantee, especially for `unsafe` library code.

## Contents

1. Design and program model
2. Source files, comments, identifiers
3. Primitive types and `usize`
4. Variables and mutability
5. Expressions, operators, overflow
6. Control flow
7. Functions and calling conventions
8. Structures as data objects
9. Methods and missing object features
10. Ownership and `drop`
11. `unsafe`, raw addresses and assembly
12. `#include` and libraries
13. Conditional compilation
14. Targets and device profiles
15. What belongs in libraries?
16. Language and target limitations
17. Compiling, debugging and learning path
18. Quick syntax reference

## 1. Mini's design and program model

Mini is a **small, statically typed, native-compiled** procedural language. Its core consists of simple types, functions, conditional execution, loops, and structures. Platform and operating-system facilities are implemented in Mini source libraries rather than as high-level compiler built-ins.

The smallest program:

```mini
program {
    return 42
}
```

The mandatory `program` block is the entry point and returns `i64`. On Linux, its result can become the process exit status (shells normally expose only the lowest eight bits). Bare-metal targets use different startup and termination rules.

At the top level a compilation unit may contain `struct`, `fn`, `unsafe fn`, and **exactly one** `program` block. Additional declarations are loaded via `#include`. The compiler emits only functions reachable from the entry point and any other mandatory roots.

There is **no built-in** `print`, `File`, `Socket`, `Vector`, GPIO framework or high-level Linux `syscall()` facility. Libraries can build such facilities using language operations and the constrained `unsafe asm` interface.

## 2. Source files, comments and names

Mini sources conventionally end in `.mini`. `//` introduces a comment to the end of the line.

```mini
// A simple function
fn answer() -> i64 {
    return 42
}

program { return answer() }
```

Statements are separated by a newline or `;`. Newlines inside parentheses do not separate statements. Identifiers begin with an ASCII letter or `_` and can then contain digits. **Identifiers cannot begin with digits.**

The current lexer does not support general string literals, character literals, hexadecimal Mini number literals, or block comments. Quoted `"..."` strings are supported by **preprocessor directives**, not by ordinary Mini expressions.

Top-level function declarations are gathered before type checking, so a function need not be textually declared before it is called. There are no general namespaces, `pub` visibility rules, module aliases, or import namespaces: includes bring declarations into a shared namespace.

## 3. Primitive and user-defined types

| Type | Meaning | Note |
|---|---|---|
| `i8`, `i16`, `i32`, `i64` | Signed 8/16/32/64-bit integer | Target dependent |
| `u8`, `u16`, `u32`, `u64` | Unsigned 8/16/32/64-bit integer | Target dependent |
| `f32`, `f64` | 32/64-bit floating point | **Not yet implemented** |
| `usize` | Target-width unsigned integer | Often useful for sizes and addresses |
| `bool` | `true` / `false` | Not interchangeable with an integer |
| `Name` | A declared `struct` type | Currently scalar fields only |
| no return type | A `fn` with no returned value | `void` is not a storable value type |

`usize` uses **16 bits on AVR8 and C64, 32 bits on ARM32/Cortex-M, and 64 bits on x86-64/AArch64**. The selected target determines its width.

```mini
fn sum(a: usize, b: usize) -> usize {
    return a + b
}
program {
    let result: usize = sum(20, 22)
    if result == 42 { return 0 } else { return 1 }
}
```

General implicit `i64`↔`usize` conversions are forbidden. `u8`–`u64` and `i8`–`i64` are supported on x86-64 and partially on ARM Linux. The language does **not** yet implement `f32`, `f64`, `char`, `string`, `ptr`, arrays, or slices as general language types. Low-level libraries may use the existing scalar types with `unsafe asm`, but that is not a substitute for a sound typed pointer or container system.


### 4.4 fixed-width integer support

| Target | Integer types / `as` | Notes |
|---|---|---|
| x86-64 Linux | `i8`–`i64`, `u8`–`u64`, checked `as` | 4.3 implementation path |
| AArch64 Linux | `i8`–`i64`, `u8`–`u64`, checked `as` | 4.4 development path |
| ARMv7-A Linux | `i8`–`i64`, `u8`–`u32`; restricted `u64` storage/casts | `u64` arithmetic/comparison rejected |
| Thumb/Cortex-M, AVR8, C64 | Earlier scalar subset | New widths and `as` unavailable |

**`f32` and `f64` are not implemented on any target.** Integer `as` is checked: out-of-range values trap (intended exit status 101 on Linux). Direct positive integer literals currently do not exceed `i64::MAX`. Rust compilation and end-to-end generated-program execution were not verified during this documentation update.

## 4. Variables: declarations, initialization, limitations

Every local variable requires an **explicit type and initializer**:

```mini
program {
    let a: i64 = 40
    let b: i64 = 2
    let ok: bool = a + b == 42
    if ok { return a + b } else { return 1 }
}
```

**Scalar local reassignment is supported:**

```mini
program {
    let x: i64 = 1
    x = x + 1
    return x
}
```

Reassignment of resource-owning structures remains restricted.

Local identifiers cannot be redeclared within the same function. General shadowing, `const`, `static`, global variables, and `let mut` are absent. Scalars can be used as calculation values; owned structures are subject to stricter copying and dropping rules.

The current checker also rejects `let` declarations directly inside `if`/`while` branches for conservative safety reasons. Declare locals in the function body or move the work to a separate function.

## 5. Expressions and operators

| Category | Operators / forms |
|---|---|
| Arithmetic | `+`, `-`, `*`, `/`, unary `-x` |
| Comparisons | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Grouping | `( ... )` |
| Calls | `name(arg1, arg2)` |
| Field reads | `variable.field` |

```mini
fn evaluate(x: i64) -> i64 {
    return (x + 2) * 3
}
program { return evaluate(12) }
```

Multiplication and division bind more tightly than addition/subtraction; comparisons have lower precedence. Operations are type-checked. Conditions for `if` and `while` must be `bool`.

Supported backends aim to trap on overflow and division by zero. **This must be validated separately on each architecture**: not every backend implements `i64` arithmetic. Mini does not offer arbitrary precision integers ; integer-to-integer `as` is a checked conversion.

There are no general `&&`, `||`, `!`, bitwise `&`, `|`, `^`, shifts, remainder `%`, compound assignment, or ternary expressions in this release.

## 6. Control flow

```mini
fn maximum(a: usize, b: usize) -> usize {
    if a > b { return a } else { return b }
}
program {
    let result: usize = maximum(42, 10)
    if result == 42 { return 0 } else { return 1 }
}
```

```mini
program {
    while false {
        // This body never runs.
    }
    return 0
}
```

Supported: `if`, `else`, `while`, `return`. Scalar counters can be incremented in `while` loops with `i = i + 1`. There is no `for`, `break`, `continue`, `switch`, `match`, `defer`, exception handling, or `try`.

The ownership checker compares the `drop` state of conditional paths, rejecting incompatible states. A `while` body cannot permanently drop an outer owner under the current safety model.

## 7. Functions and return values

```mini
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn noop() {
    // No return value.
}

program {
    noop()
    return add(20, 22)
}
```

Parameters and return values are typed. A `fn` without `-> T` returns no value. The `program` block must return a value. The compiler rejects missing or mismatched returns and unreachable statements immediately after an unconditional `return`.

The full Linux backends support multiple parameters, though targets can impose limits on parameter count, recursion, supported scalar types and calling convention. **Mini's internal ABI is not automatically C ABI-compatible.**

Names identify functions uniquely; there are no overloads, generics, function pointers, lambdas or dynamic dispatch. Unreachable library functions are omitted from the native binary.

## 8. Structures – Mini's data objects

A `struct` declares a named group of fields. Currently it must have **1–32 fields**, and each field must have type `i64`, `usize`, or `bool`.

```mini
struct Point {
    x: i64
    y: i64
}

fn point_sum(p: Point) -> i64 {
    return p.x + p.y
}

program {
    let p: Point = Point(20, 22)
    return point_sum(p)
}
```

`Point(20, 22)` is a positional constructor: arguments follow the declared field order. `p.x` **reads** a field; general `p.x = 5` mutation is not supported. Fields cannot currently contain other structures, vectors, strings, or recursively owned objects. A `struct` is not a heap-allocated class and has no implicit object header.

Structure support is more complete on Linux x86-64, AArch64 and ARM32. **AVR8, C64 and Cortex-M do not provide full structure passing/ownership** and reject unsupported cases. Full backends use a hidden caller-provided result address for aggregate returns.

### Example owned object

```mini
struct Token { number: i64 }

fn __drop_Token(value: Token) {
    // Library-specific cleanup would go here.
}

unsafe fn make_token(number: i64) -> Token {
    unsafe { return Token(number) }
}

program {
    unsafe {
        let token: Token = make_token(42)
        drop token
    }
    return 0
}
```

This demonstrates an ownership contract; the example destructor does not actually release a resource. A real library must ensure that its constructor and destructor agree on resource validity.

## 9. Methods: what is and is not implemented

The parser recognizes the `variable.method(...)` **call shape**, internally translating it to a named function call. But current top-level declarations do **not** support `fn Type.method(...)`, nor is there a language-level `self`/`Self` mechanism. Therefore, **a general user-definable method system is not yet available**.

**Supported alternative:** write an ordinary function taking the structure as its first argument:

```mini
struct Meter { value: i64 }

fn meter_value(m: Meter) -> i64 {
    return m.value
}

program {
    let m: Meter = Meter(42)
    return meter_value(m)
}
```

Not supported: `class`, inheritance, interfaces/traits, virtual methods, generic objects, overloaded constructors, properties, operator overloading, polymorphism, general `self`, or a heap-object model.

## 10. Ownership and `drop`

Mini implements conservative ownership checking for a subset of aggregate values. The objective is to prevent obvious double-release and use-after-drop errors. **It is not a full Rust borrow checker.**

- `drop variable` applies only to structures, not scalar locals.
- Using a variable after `drop` is a compile-time error.
- Dropping the same variable twice is an error.
- A borrowed structure argument cannot be dropped by the callee.
- A destructor for `T` uses a specifically checked `fn __drop_T(value: T)` declaration.
- Full Linux backends insert ordinary destructor calls when owned locals leave their supported lifetime, in reverse declaration order.
- No special destructor opcode is required in the IR: cleanup is an **ordinary function call**.

**Limitations:** no general move expressions, explicit lifetime system, complete borrowing, recursive cleanup of owned nested fields, or universal RAII guarantees across every control-flow path. The `unsafe` library author is responsible for making destructors correct. For safety, the checker deliberately rejects several complicated patterns.

## 11. `unsafe`, raw addresses and assembly

Low-level machine access is exposed through `unsafe asm`, rather than special built-in syscall functions.

```mini
fn pid() -> i64 {
    let result: i64 = -1
    unsafe {
        asm {
            in rax = 39
            syscall
            out result = rax
            clobber rcx
            clobber r11
            clobber memory
        }
    }
    return result
}

program { return pid() }
```

This example targets **x86-64 Linux**. Other architectures use different registers, syscall numbers, and instructions. Calling an `unsafe fn` requires an `unsafe` context. The low-level `raw_addr(...)` facility can expose a raw address and should only be used within the required unsafe boundary.

`asm` is **not a general-purpose inline assembler**. Only instructions, registers, and operand combinations supported by the parser and selected backend are available. `in`, `out`, and `clobber` declare inputs, outputs and side effects, but validation does not establish full memory safety, race freedom, or the correctness of hardware-register accesses.

**Library rule:** Linux syscalls, MMIO, port I/O, interrupts and allocation belong in `.mini` library functions. Exposing such a wrapper as *safe* requires checking all input, memory and ownership preconditions.

## 12. `#include` and libraries

An example library:

`math.mini`:

```mini
fn answer() -> i64 { return 42 }
fn unused() -> i64 { return 100 }
```

`main.mini`:

```mini
#include "math.mini"
program { return answer() }
```

An include path is relative to the **including file**. The preprocessor loads the file, merges declarations into the shared namespace, and processes the same included file at most once per compilation. Circular includes are errors. Since `unused()` is unreachable, it is omitted from the emitted native code. No separate linker is required for this step: Mini performs reachability selection itself.

The 4.4.0 source package contains Linux API libraries for x86-64 (`lib/linux.mini`), AArch64 (`lib/linux_aarch64.mini`) and ARM32 (`lib/linux_armv7.mini`); `lib/linux_sys.mini` chooses based on the target. Raw syscall wrappers generally require `unsafe` contracts.

## 13. Conditional compilation

```mini
#if arch == "avr8"
    #include "drivers/avr_gpio.mini"
#elseif arch == "thumbv7m"
    #include "drivers/thumb_gpio.mini"
#else
    #error Unsupported target
#endif
```

The `drivers/*` paths above are **illustrative**: use actual filenames in your project. The preprocessor **does not open includes in inactive branches**. Conditions support `==` or `!=` with `arch`, `os`, `abi`, `cpu`, `device`, `feature`, and `pointer_bits`. String values must be quoted; `pointer_bits` uses a number.

```mini
#if pointer_bits == 16
    // AVR8 or C64
#elseif pointer_bits == 32
    // ARM32 / Cortex-M
#else
    // x86-64 / AArch64
#endif
```

The `feature` property is constrained to known device capabilities, rather than accepting arbitrary strings. Conditional includes let one high-level API select different per-platform implementations.

## 14. Targets and device descriptors

Example `.target` device profile:

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

The descriptor has **five fields**: `device`, `flash`, `ram`, `stack`, and `features`. It is a minimal memory/CPU profile, not a programmable linker script. The vector table and output format belong to compiler target handling; peripheral drivers remain in Mini libraries. Memory ranges are half-open (`start..end`, excluding the end).

```sh
cargo build --release
./target/release/mini --target x86_64-linux main.mini -o app
./target/release/mini --target aarch64-linux main.mini -o app_a64
./target/release/mini --target armv7-linux-soft main.mini -o app_arm
./target/release/mini --device targets/devices/atmega328p.target main.mini -o app.hex
./target/release/mini --device targets/devices/stm32f103c8.target main.mini -o app.elf
./target/release/mini --target c64-6510 main.mini -o app.prg
./target/release/mini --emit-ir main.mini
```

Choosing a supported target does not mean every Mini operation is supported there. Unknown or unsupported operations should produce a **compilation error**, not silently truncated or incorrect machine code.

## 15. What belongs in libraries?

**Good library candidates:** file and socket I/O, standard input/output, Linux syscall wrappers, GPIO, UART, SPI/I²C, timers, allocators, dynamic containers, string operations, protocols, drivers. Their implementations require the low-level operations and data models actually available on the chosen target.

**Irreducible compiler responsibilities:** lexing/parsing, typing and ownership checks, includes and conditional compilation, calls/returns/ABI, target-aware scalar operations, constrained assembly emission, reachable-function selection, minimal IR, native code and executable/firmware packaging.

**Important distinction:** a feature can *belong in a library architecturally* even if the current Mini language lacks the mutation/data primitives needed to implement it fully today. “Can live in `#include`” is a design boundary, not evidence that every supporting facility is already implemented.

## 16. Current limitations – feature matrix

| Language capability | 4.4.0 status |
|---|---|
| Typed `let`, integer values, `bool` | Available |
| `+ - * /` and comparisons | Available; target-dependent support |
| `if/else`, `while`, `return` | Available |
| General `x = expr` assignment | **Available for scalar locals** |
| `fn`, multi-argument calls | Available; target limitations |
| `struct` positional construction and field reads | Available; more complete on Linux backends |
| General method declarations and `self`/`Self` | **Not available** |
| Ownership-aware `drop` | Partial, conservative |
| Automatic `drop` | Linux backends, with limitations |
| `unsafe asm`, `unsafe fn` | Available, with restricted instruction set |
| `#include` and `#if` | Available |
| General strings, arrays, slices, references, pointer types | **Not available** |
| Classes, inheritance, traits, generic types | **Not available** |
| `for`, `break`, `continue`, logical `&&`/`||` | **Not available** |
| Full borrow checker or unconditional memory-safety guarantees | **Not available** |

**Target summary:** Linux x86-64/AArch64/ARM32 offer the broadest support; AVR8/C64 use 16-bit `usize`, limited scalar operations and assembly but lack full structures/`i64`/`drop`; Cortex-M3/M4 provide a 32-bit scalar subset with parameter limits, not general `i64`/structures; RISC-V has no functioning code-generation backend yet.

## 17. Compilation, debugging and suggested learning order

The source package includes 25 tutorials under `examples/tutorial/` and a `tests/tutorial413.sh` regression script. Suggested progression: `program` → types and `let` → arithmetic → functions → structures → includes → `unsafe` → ownership → cross-compilation.

Examples of intentionally invalid programs:

```mini
// Invalid: a type is mandatory
program { let n = 42; return n }

// Valid: scalar local reassignment
program { let n: i64 = 1; n = 2; return n }

// Invalid: string literals are not supported
program { return "hello" }
```

`cargo build --release` builds the **Mini compiler**, not the `.mini` application. Then compile the Mini source separately, inspect the output format, and execute it on suitable hardware or an emulator. A successful build, a plausible machine-code file, and successful execution are **three separate verification stages**.

## 18. Quick syntax reference

```text
program { statements }
fn name(param: type, ...) -> type { statements }
fn name(...) { statements }
unsafe fn name(...) -> type { statements }
struct Name { field: i64; other: usize }
let variable: type = expression
variable = expression  // scalar local
expression as target_type  // checked integer cast
if condition { ... } else { ... }
while condition { ... }
return expression
drop structure_variable
unsafe { ... }
unsafe { asm { in reg = expr; out variable = reg; clobber reg; instruction } }
#include "relative/path.mini"
#if arch == "x86_64" ... #elseif ... #else ... #endif
```

**Final design rule:** Mini is intentionally small. Rather than making every abstraction a compiler feature, the project aims to strictly verify the few operations it does implement and keep higher-level facilities in ordinary libraries.
