# Mini 4.4 – Integer types on ARM Linux

Mini recognizes `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `usize`, and `bool`. Integer `as` casts are **checked**: out-of-range conversions trap (SIGILL on Linux, shell status 132 since 4.9). Narrow arithmetic must not silently truncate.

```mini
fn increment(x: u8) -> u8 {
    return x + 1
}
program {
    let n: u8 = 40
    n = increment(n)
    return (n + 1) as i64
}
```

## Backend coverage

| Target | Integer types | `usize` | Status |
| --- | --- | --- | --- |
| x86-64 Linux | 8/16/32/64-bit integers | 64-bit | Previous implementation |
| AArch64 Linux | 8/16/32/64-bit integers | 64-bit | New, unverified emitter |
| ARMv7-A Linux | 8/16/32-bit integers and `i64`; limited `u64` storage/casts | 32-bit | New, unverified emitter |
| Cortex-M, AVR8, C64 | Earlier subsets | target dependent | New integer types unavailable |

ARM32 unsigned `u64` arithmetic/comparisons produce a **compile error**, not truncated output. Decimal literals above the signed `i64` maximum remain unavailable. `f32` and `f64` are **not implemented**; floating-point parsing, the type/IR path, FPU/soft-float code generation, and casts still require work.

`tests/arm_integers44.sh` cross-compiles and runs outputs only if native ARM Linux or an appropriate QEMU user-mode runner is available. The archive has not been Cargo-compiled or hardware-tested in this environment.
