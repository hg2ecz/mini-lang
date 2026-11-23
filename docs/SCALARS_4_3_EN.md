> **4.3 historical release:** 4.4 also adds ARM Linux integer support. See [4.4](SCALARS_4_4_EN.md).

# Mini 4.3 – Fixed-width integers and checked `as`

This is a **development release**. The first width-checked integer implementation targets **x86-64 Linux**. ARM, Thumb, AVR8 and C64 explicitly reject the new integer features rather than silently generating incorrect binaries. Cargo compilation and end-to-end ELF execution have not been verified in the release environment.

## Types

- Unsigned: `u8`, `u16`, `u32`, `u64`.
- Signed: `i8`, `i16`, `i32`, `i64`.
- Target-sized: `usize` (AVR8/C64: 16 bits; ARM32/Thumb: 32 bits; x86-64/AArch64: 64 bits).
- Boolean: `bool`.
- **Not yet implemented:** `f32` and `f64`; compiling programs using them fails.

On x86-64, smaller integers currently occupy **8-byte internal Mini ABI slots**, with their value ranges checked. This does not imply C ABI compatibility or compact byte-exact struct layouts.

## Example

```mini
fn increment(x: u8) -> u8 {
    return x + 1
}
program {
    let n: u8 = 41
    n = increment(n)
    return n as i64
}
```

`let` introduces a local variable and `n = ...` reassigns it. Implicit conversion across numeric types is prohibited, except for simple integer literals that fit the expected type.

## Checked casts

```mini
program {
    let a: i64 = 200
    let b: u8 = a as u8
    return b as i64
}
```

`as` checks whether the integer fits the destination type. A failed conversion triggers the Mini trap on x86-64 Linux (SIGILL, shell status 132 since 4.9; exit code 101 before); values are never silently truncated. Boolean and floating-point conversions are not supported yet. Narrow integer arithmetic checks its result range, and divide-by-zero triggers a checked failure. Comparisons respect signedness.

## Current limitations

1. The lexer supports positive decimal literals up to `i64::MAX`. The upper half of `u64` cannot yet be written directly as a decimal literal.
2. The `-9223372036854775808` literal also cannot yet be written directly.
3. Struct fields remain restricted to the previous scalar subset; compact fixed-width aggregate layouts are not implemented.
4. Floating-point parsing, IR and code generation are not implemented.
5. New integer and cast code generation is currently x86-64 Linux only.
6. Rust compilation and execution of generated ELF binaries remain **unverified in the release environment**.

## Test commands

```sh
cargo test
cargo build --release
bash tests/scalars43.sh
```

On x86-64 Linux, `cargo test` includes tests that execute generated ELF files and check exit codes.
