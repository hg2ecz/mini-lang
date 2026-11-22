> **4.4.0 documentation update:** scalar reassignment, fixed-width integer types, and checked integer `as` casts are supported on the relevant Linux backends. See [language reference](MINI_LANGUAGE_REFERENCE_EN.md) and [4.4 scalars](SCALARS_4_4_EN.md).

# Mini – User Guide (English)

**Version:** Mini Native 4.4.0 (some tutorial examples originated in 4.1.2)  
**Principle:** *Mini should do less, but do it reliably.*

This guide describes the **language implemented by the current compiler**, not a proposed future version. The examples are based on the checked-in sources; execution has not been verified on every target. Mini does **not** currently guarantee complete memory safety.

## 1. Your first program

Create `hello.mini`:

```mini
program {
    return 42
}
```

Build for x86-64 Linux:

```sh
cargo build --release
./target/release/mini hello.mini -o hello
./hello
echo $?  # expected: 42
```

`program` is the entry block. On Linux its return value is used as an exit status; a shell exposes only the low 8 bits. There is no built-in `print` statement: output is provided by Mini libraries.

## 2. Comments, separators, and types

`//` starts a line comment. Separate statements with newlines or `;`. A `let` binding requires an **explicit type**:

```mini
program {
    let a: i64 = -10
    let b: usize = 20
    let ready: bool = true
    if ready { return a + 52 } else { return 1 }
}
```

Current built-in types:

| Type | Meaning |
|---|---|
| `i64` | Signed 64-bit integer |
| `usize` | Target-dependent unsigned integer |
| `bool` | `true` or `false` |
| `f32`, `f64` | reserved floating-point types; not implemented |
| `struct` | Named aggregate of scalar fields |

`usize` is **16-bit** on AVR8 and C64, **32-bit** on ARM32/Cortex-M, and **64-bit** on x86-64/AArch64. There is no implicit conversion between `i64` and `usize`. Supported arithmetic operations trap on overflow on the appropriate backends, but device execution still needs verification.

### Reassignment and checked integer casts (4.4)

```mini
program {
    let n: u16 = 40
    n = n + 2
    return n as i64
}
```

Works on the relevant x86-64, AArch64, and ARM32 integer subsets. The `as` cast checks representability and never silently truncates. ARM32 `u64` arithmetic, new integer widths on Cortex-M/AVR8/C64, and `f32`/`f64` remain unsupported.

## 3. Expressions and conditions

Available operators are `+`, `-`, `*`, `/`, `==`, `!=`, `<`, `<=`, `>`, `>=`, and unary `-`. Multiplication and division bind more tightly than addition/subtraction. Use parentheses to group expressions.

```mini
fn compute(x: i64) -> i64 {
    return (x + 2) * 3
}
program {
    let y: i64 = compute(12)
    if y == 42 { return 42 } else { return 1 }
}
```

Conditions must have type `bool`; integers are not automatically booleans.

## 4. Functions

```mini
fn sum(a: usize, b: usize) -> usize {
    return a + b
}

fn tick() {
    // no return value
}

program {
    let result: usize = sum(20, 22)
    tick()
    if result == 42 { return 0 } else { return 1 }
}
```

Functions have the form `fn name(parameter: type, ...) -> return_type`. Omitting `->` declares a no-result function. The compiler emits only functions reachable from the program, so unused included functions stay out of the binary.

## 5. Branches and loops

```mini
fn choose(x: usize) -> usize {
    if x > 10 { return x } else { return 10 }
}

program {
    let a: usize = choose(42)
    while false {
        // body never runs
    }
    return 0
}
```

Mini 4.4 supports scalar-local reassignment such as `x = x + 1`, so counting `while` loops can be written. There is no `for`, `break`, `continue`, or `match` in this release. The current ownership checker also restricts declarations of owners inside branches/loops.

## 6. Structures

```mini
struct Pair {
    left: i64
    right: i64
}

fn make_pair(a: i64, b: i64) -> Pair {
    return Pair(a, b)
}

program {
    let p: Pair = make_pair(20, 22)
    return p.left + p.right
}
```

A struct constructor is named after the struct; supply field values in declaration order. Fields may currently contain only `i64`, `usize`, or `bool`. Struct support is principally implemented for 32/64-bit Linux targets; AVR8, C64, and Cortex-M backends reject unsupported aggregate operations.

## 7. Libraries and conditional compilation

File handling, GPIO, networking, and other services belong to **Mini source libraries** loaded through `#include`. Include paths are **relative to the including file**.

```mini
#if arch == "x86_64"
    #include "lib/linux.mini"
#elseif arch == "aarch64"
    #include "lib/linux_aarch64.mini"
#else
    #error This example requires Linux x86-64 or AArch64
#endif

program {
    let pid: i64 = sys_getpid()
    if pid > 0 { return 0 } else { return 1 }
}
```

A condition uses `property == value` or `!=`. Properties: `arch`, `os`, `abi`, `cpu`, `device`, `feature`, `pointer_bits`. String values need quotes; `pointer_bits` uses a number. Supported directives are `#if`, `#elseif`, `#else`, `#endif`, `#include`, and `#error`. Includes in inactive branches are not opened.

Currently `#include` **loads declarations into the program**; it does not create automatic module namespaces or a public/private export system.

## 8. `unsafe` and target-specific assembly

Hardware and system calls remain in libraries. Assembly may occur only inside `unsafe` and uses an **architecture-specific limited instruction subset**.

The following **x86-64 Linux** example follows `lib/linux.mini`:

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
```

`in` supplies a register; `out` stores a result in an existing variable; `clobber` declares required side effects. The frontend **checks these target-specific declarations**; the backends do not accept arbitrary inline assembly. `unsafe` does not make invalid pointers or incorrect machine-level contracts safe.

`raw_addr(variable)` obtains a raw address and is allowed only in `unsafe`. Keep low-level pointer manipulation within carefully reviewed libraries.

## 9. `unsafe fn` and ownership-based `drop`

Calling an `unsafe fn` requires an `unsafe` block. When a struct `T` has a function with the exact signature `fn __drop_T(value: T)`, the compiler treats it as an owned resource with a destructor. The destructor is an ordinary Mini function, **not** a special IR opcode.

```mini
struct Handle { number: i64 }

fn __drop_Handle(value: Handle) {
    // resource cleanup may be implemented here
}

unsafe fn adopt(value: i64) -> Handle {
    unsafe { return Handle(value) }
}

program {
    unsafe {
        let h: Handle = adopt(42)
        drop h
    }
    return 0
}
```

After `drop h`, using `h` again is invalid, and double drops are rejected. On supported Linux targets, owned function locals are dropped automatically on `return` or at the end of a no-result function, in reverse order. **Limitations:** no full borrow checker, arbitrary moves, nested owning fields, automatic drop of all temporaries, or guaranteed cleanup after hardware faults. Resource safety depends on the library contract.

## 10. Targets and building

```sh
# Linux
./target/release/mini --target x86_64-linux input.mini -o app
./target/release/mini --target aarch64-linux input.mini -o app_arm64
./target/release/mini --target armv7-linux-soft input.mini -o app_arm32
./target/release/mini --target armv7-linux-hard input.mini -o app_arm32_hard

# Commodore 64: MOS 6510, PRG
./target/release/mini --target c64-6510 input.mini -o app.prg

# AVR: concrete device profile, Intel HEX
./target/release/mini --device targets/devices/atmega328p.target input.mini -o app.hex

# Cortex-M3 / M4: concrete device profile, ARM ELF32
./target/release/mini --device targets/devices/stm32f103c8.target input.mini -o app.elf

# Validate device profile
./target/release/mini --check-device targets/devices/atmega328p.target

# Inspect IR (default x86-64 target)
./target/release/mini --emit-ir input.mini
```

A `.target` file has exactly five keys: `device`, `flash`, `ram`, `stack`, and `features`:

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

The compiler's reviewed device catalogue supplies the vector table, startup convention, and output format. Invalid memory maps and unknown capabilities are rejected.

## 11. Target-specific limitations

| Target | Current limits |
|---|---|
| Linux x86-64 / AArch64 / ARM32 | Broader `i64` and struct support; private Mini ABI, not automatically C ABI |
| AVR8, C64 | 16-bit `usize`; scalar subset; no general `i64`, aggregates, or ownership `drop` |
| Cortex-M3/M4 | 32-bit `usize`; scalar subset; at most 2 scalar arguments; no `i64`, structs, multiply/divide, recursion, or FPU code |
| RISC-V | Target names exist but no implemented backend |

For Cortex-M, the firmware uses the reset vector; currently end the `program` block with `return 0`. Including an MMIO library does not automatically establish safe hardware access.

## 12. Troubleshooting and learning path

Run `cargo test`, followed by the matching `tests/*.sh` regression script. Good examples in the repository include `examples/usize_ok.mini`, `examples/struct.mini`, `examples/target_linux.mini`, `examples/eightbit_arith.mini`, and `examples/thumb_firmware.mini`.

**Suggested learning order:** `program` → types → functions → `if`/`while` → `struct` → `#include` → device profiles → `unsafe`/assembly → resource ownership. Not every example runs on every target. A successful build is not proof of full memory safety or correct hardware operation.

## 13. Program structure in detail

A Mini source file consists of `struct` declarations, `fn` declarations, and exactly one `program` block. The `program` block is mandatory. The small language core is designed to keep control flow explicit and readable.

```mini
// demo.mini
fn answer() -> i64 {
    return 42
}

program {
    return answer()
}
```

Do not assume that a function must be declared before another function can call it: the compiler gathers declarations for checking. Variables, on the other hand, have initialization and lifetime rules. Local names must be unique within their function; general-purpose variable shadowing is not part of the current model.

**Notation:** `mini` code blocks show Mini programs, `sh` shows terminal commands, and `ini` shows target descriptions. Some legacy examples originate from 4.1.2; use the 4.4 language reference for current feature coverage, not a finalized language specification.

### Why is there no built-in `print`?

Printing, file access, GPIO, and memory allocation are services, not fundamental programming-language constructs. Mini delegates them to platform libraries. That way the compiler does not need special Linux calls or AVR peripheral tables. Library functions are ordinary Mini functions.

## 14. Types, literals, and arithmetic safety

`usize` is not an alias for `i64`. `i64` is a fixed-width signed 64-bit integer; `usize` is target-sized and unsigned. Certain nonnegative numeric literals are accepted where `usize` is expected, but **there is no implicit conversion of arbitrary `i64` variables to `usize`**.

```mini
fn twice(x: usize) -> usize {
    return x + x
}

program {
    let count: usize = 21
    let doubled: usize = twice(count)
    if doubled == 42 { return 0 } else { return 1 }
}
```

The following is **invalid** because the variable types differ:

```mini
program {
    let signed: i64 = 20
    let unsigned: usize = signed  // compile-time error
    return 0
}
```

Distinguish compile-time checks from runtime checks. The compiler rejects known type mismatches. Supported backends emit runtime traps for overflowing arithmetic or division by zero; they do not silently provide a mathematically correct unbounded result. **Successful Rust compilation of the compiler does not prove** that every generated backend instruction is correct on real hardware.

Mini 4.4 supports `u8`–`u64` and `i8`–`i64` integers on x86-64 and partially on ARM Linux. Floating-point `f32`/`f64`, arrays, strings, references and general safe pointers are still unavailable. Do not treat these as implemented language features.

## 15. Current limits of control flow

An `if` or `while` condition must be a `bool`. `while` repeats while the condition is true, and scalar local reassignment (`x = x + 1`) is supported, so conventional incrementing loops are possible.

```mini
fn classify(x: usize) -> usize {
    if x < 10 {
        return 1
    } else {
        return 2
    }
}

program {
    let code: usize = classify(12)
    if code == 2 { return 0 } else { return 1 }
}
```

Use `while` only when the relevant backend and externally changing state make its use meaningful. Do **not** assume `break`, `continue`, `for`, or `&&`/`||` operators exist in this subset.

## 16. Calls, return values, and method-shaped syntax

Argument counts and types are statically checked. The result type is part of the function declaration. `program` is a special entry block, not a `fn main()` declaration.

The parser recognizes calls shaped like `variable.member(...)`, but this is **not** a general object-oriented method, trait, or dispatch system. Such a call must resolve to a function name and argument signature the compiler actually supports. Treat complicated method examples as portable only once verified with the specific compiler release.

`return` exits the function immediately. On supported backends, live owned locals with destructors are cleaned up while the return value is preserved.

## 17. Structures and data representation

A structure currently contains a **fixed set of named scalar fields**, between one and 32 fields. Constructor arguments follow declaration order.

```mini
struct Point {
    x: i64
    y: i64
}

fn origin() -> Point {
    return Point(0, 0)
}

program {
    let p: Point = origin()
    return p.x + p.y
}
```

Fields cannot themselves be general structures, vectors, or strings in this release. Access a field using `p.x`. Full aggregate calling conventions are available only in the relevant Linux backends; AVR8, C64, and the current Thumb backend have stricter restrictions.

## 18. Writing reusable libraries

A library is an ordinary `.mini` file. For example, `lib/calculations.mini`:

```mini
fn triple(x: usize) -> usize {
    return x * 3
}

fn unused_helper(x: usize) -> usize {
    return x + 100
}
```

Application file `example.mini`:

```mini
#include "lib/calculations.mini"

program {
    let answer: usize = triple(14)
    if answer == 42 { return 0 } else { return 1 }
}
```

All processed library declarations are checked, but the emitted machine code is restricted to functions reachable from `program` and their dependencies. `unused_helper` does not become generated machine code. Tests must verify the exact behavior on each backend; this rule does not cover global initializers, because Mini does not provide them.

`#include` is **not a package manager** and does not automatically create namespaces or a `pub` visibility system. Use collision-free function names. Paths are relative to the including file.

## 19. Safe conditional compilation

A small platform-selection library is often useful:

```mini
#if arch == "x86_64"
    #include "lib/linux.mini"
#elseif arch == "aarch64"
    #include "lib/linux_aarch64.mini"
#else
    #error Linux x86-64 or ARM64 required
#endif
```

`#if` is not a runtime `if`; it is resolved **before Mini parsing**. Files in inactive branches are not opened. A `feature` condition checks a named declared CPU capability, for example `feature == "fpv4_sp"`. This does not imply that Mini has implemented floating-point values or FPU code generation.

**Safety recommendation:** include an explicit error branch for unsupported platforms rather than accidentally using the registers of another architecture.

## 20. Device profiles and memory layout

Bare-metal device profiles are deliberately strict five-key data files. Example:

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

`flash` and `ram` use **half-open ranges** (`start..end`): `start` belongs to the range, `end` does not. `stack` specifies reserved bytes. `features` lists only known CPU instruction capabilities valid for that device.

The compiler checks that code and allocated data fit the profile's limits. Peripheral register addresses and behavior, by contrast, are **not scripts in the target description**; they belong to platform-specific `#include` libraries.

**Important:** the actual device data sheet still rules. A valid `.target` file alone does not guarantee correct clocks, pin wiring, or memory-mapped register usage.

## 21. Ownership and resource lifetimes

Mini's basic ownership aim is to let exactly one owner release a resource. Within the supported structured cases, the compiler tracks whether an owned local is live, and rejects use after `drop`.

Under the `__drop_T` convention, a matching Mini function acts as the destructor for `T`. The compiler does not know how to release that resource: **the library implements it**. `drop handle` explicitly releases the owner; at function exit, a destructor call can be inserted automatically.

Do **not** yet rely on: a complete reference/borrow lifetime checker; nested owning fields; exceptions; guaranteed destructor calls on hardware reset; or uniform automatic `drop` across all targets. Public safe resource APIs require independent review.

## 22. Rules for correct `unsafe asm`

`unsafe` means the author is responsible for demonstrating the hardware-level preconditions. It **does not excuse invalid addresses, incorrect registers, or omitted memory effects**.

- Keep assembly inside very small wrapper functions with narrowly defined contracts.
- Do not assume general assembler syntax; Mini accepts only a checked target-specific instruction subset.
- `clobber` declarations are validated in the current compiler **before IR lowering**, as part of assembly and type checking. The small IR therefore does not retain a redundant `clobbers` field. This is not permission for arbitrary register mutation.
- Raw pointers and lengths are especially sensitive: a valid `syscall` opcode does not make an invalid user-supplied pointer safe.
- Exercise library code with target-specific negative tests and hardware tests where possible.

System calls, port I/O, and MMIO should remain ordinary library wrappers rather than new specialized IR opcodes. The architecture is **small language + small encoder + checked libraries**.

## 23. Troubleshooting guide

| Message or symptom | Likely cause | Action |
|---|---|---|
| `unknown variable` | A local name does not exist | Check spelling and declaration |
| `arithmetic/comparison requires matching ...` | Incompatible operand types | Use matching `i64` or `usize` operands |
| `use after drop` | An owned value was released | Do not reference it again |
| `unsupported ... asm` | Opcode missing from the backend subset | Use supported instructions or a different target |
| `backend unavailable` | No encoder exists for the target | Do not force a misleading binary output |
| `missing program block` | Entry block omitted | Add exactly one `program { ... }` |
| Firmware does not start | Target map, reset vector, stack, or peripheral issue | Check ELF/HEX/PRG format, then device behavior |

Debug in layers: (1) source and includes, (2) target profile, (3) `cargo test`, (4) running the Mini compiler, (5) inspecting output using `readelf` or HEX/PRG tools, (6) running on the actual target. Rust compilation of the compiler is only an initial milestone.

## 24. Exercises and release checklist

**Beginner:** write a function that adds two `usize` values and call it from `program`. Write a second function using a `bool` condition to choose between results. Verify its output through a Linux exit status.

**Intermediate:** move arithmetic functions into a separate `.mini` file, include it, and verify that an uncalled helper does not appear in the generated binary.

**Advanced:** write one platform-selection wrapper using `#if arch`. Use only the supported `unsafe asm` subset, mark raw-resource functions `unsafe fn`, and add negative tests for invalid assembly and use after `drop`.

**Release checklist:** target explicitly selected; code uses only supported types; includes are target-specific; unsafe surface is small; ownership invariants are checked; negative tests are rejected; output format is correct; runtime behavior is independently verified on the target device.

---

*This guide documents the Mini Native 4.4.0 development prototype. The language and backends are evolving; examples illustrate source syntax and are not a guarantee of successful execution on every architecture.*


## 13. Worked example: a small calculator

This example needs no library and is suitable for learning the basics on **Linux x86-64**:

```mini
fn square(n: i64) -> i64 {
    return n * n
}

fn plus_one(n: i64) -> i64 {
    return n + 1
}

program {
    let a: i64 = square(6)
    let b: i64 = plus_one(a)
    if b == 37 { return 0 } else { return 1 }
}
```

Save as `calculator.mini` and run:

```sh
./target/release/mini --target x86_64-linux calculator.mini -o calculator
./calculator
printf 'Exit status: %s\n' "$?"
```

**Expected exit status: 0.** A Linux process exit code is not a general-purpose printing facility.

## 14. Worked example: your own library

Create two files in the same directory.

`math.mini`:

```mini
fn twice(x: i64) -> i64 {
    return x + x
}

fn unused_helper(x: i64) -> i64 {
    return x * 100
}
```

`main.mini`:

```mini
#include "math.mini"

program {
    let result: i64 = twice(21)
    if result == 42 { return 0 } else { return 1 }
}
```

The compiler normally emits `twice()` but omits `unused_helper()` through **call-graph reachability**. Dead-code elimination reduces binary size; it is not a permission or security boundary. Included source still participates in compilation.

## 15. Target-dependent source selection

Conditional compilation chooses source **at build time**, not at runtime:

```mini
#if arch == "avr8"
    #include "lib/eightbit/avr_portb_328_168.mini"
#elseif arch == "x86_64"
    #include "lib/linux.mini"
#else
    #error Unsupported platform in this example
#endif
```

Include paths are relative to the including file. Call only functions defined by the selected branch. Memory regions and instruction features come from the `.target` profile; peripheral register operations remain in Mini libraries.

**Important:** `#if feature == "fpv4_sp"` merely tests whether the device profile advertises the feature. It does not mean Mini 4.4.0 can generate floating-point instructions.

## 16. Frequent compiler errors

| Symptom | Likely reason | What to do |
|---|---|---|
| `unknown function` | Missing declaration or include | Check the name and active `#include` branch |
| Type mismatch | Mixing `i64`, `usize`, and `bool` | Use matching types; do not expect automatic conversion |
| `use after drop` | Access to a released owner | Do not reference the binding after `drop` |
| `unsafe` error | Raw operation in safe code | Restrict `unsafe` to reviewed library implementations |
| Unsupported target operation | Backend implements a smaller subset | Simplify the program or select a supported target |
| `unsupported clobber` | Invalid or undeclared assembly effect | Review the target backend's assembly contract |
| Firmware too large | Flash, SRAM, or stack bound | Recheck `.target` and program size |
| `#if` / `#endif` error | Unbalanced preprocessor directives | Close each `#if` with a matching `#endif` |

Exact diagnostic wording may change between releases. **Do not use `unsafe` to bypass an unsupported operation:** first verify that the target backend can generate correct machine code for it.

## 17. Writing a safe Mini library

When developing a platform library:

1. A public safe function should **not accept unchecked raw pointers**. Use `unsafe fn` if the caller must uphold invariants the compiler cannot check.
2. Validate argument bounds, lifetimes, and resource state before system calls or MMIO wherever the current language supports such validation.
3. State who owns a returned resource and which `__drop_T` routine closes it.
4. Use only assembly instructions supported by the selected backend, and declare required clobbers.
5. Do not assume automatic volatile semantics, atomics, interrupt safety, or complete pointer validation.
6. Test each library on every target device you intend to support.

Mini's current ownership checker does **not** prove the internals of an `unsafe` library correct. A high-level interface is only as safe as the invariants established by its implementation and tests.

## 18. Which example should I use?

| Learning goal | File | Suggested target |
|---|---|---|
| Hello program and exit status | `examples/hello.mini` | x86-64 Linux |
| Functions and structures | `examples/struct.mini` | x86-64 Linux |
| `usize` arithmetic | `examples/usize_ok.mini` | x86-64 Linux |
| Linux-specific libraries | `examples/target_linux.mini` | x86-64 / AArch64 Linux |
| Ownership and cleanup | `examples/ownership_linux.mini` | Supported Linux backend |
| 16-bit values on 8-bit CPUs | `examples/eightbit_arith.mini` | AVR8 / C64 |
| Basic Cortex-M firmware | `examples/thumb_firmware.mini` | STM32F103C8 / F401CC |

Some files are *negative tests* deliberately containing invalid Mini code. Do not expect `unsafe_rejected.mini`, `obsolete_rejected.mini`, or overflow-test examples to behave like ordinary successful applications.

## 19. Compatibility and project status

Mini 4.4.0 is an **experimental compiler**; backend capabilities differ. Do not assume that a structure operation accepted on Linux also compiles for AVR8 or Cortex-M. Memory and ownership safeguards are incomplete, and there is no general guarantee of threading, full atomics, or C ABI compatibility.

Select the intended target first and stay within that backend's documented subset. Future guide revisions should track actual compiler and device test results rather than planned language features.
