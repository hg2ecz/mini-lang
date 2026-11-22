> **Historical 4.2 snapshot — not current!** See the [4.4 language reference](MINI_LANGUAGE_REFERENCE_EN.md) and [4.4 scalar support](SCALARS_4_4_EN.md) for current behavior.

# Mini – numeric types and reassignment (implementation status)

Planned built-in scalar types are `u8`, `u16`, `u32`, `u64`; `i8`, `i16`, `i32`, `i64`; `f32`, `f64`; `bool`; and target-dependent unsigned `usize` (16-bit AVR8/C64, 32-bit ARM32/Thumb, 64-bit x86-64/AArch64).

**Status in 4.2.0:** `i64`, `usize`, and `bool` work within the existing target-specific subsets. The new type names are reserved and recognized as numeric types, but use of `u8`–`u64`, `i8`–`i32`, and `f32`/`f64` is **rejected with an explicit not-implemented error** until real data-width and floating-point code generation exists. The `as` conversion syntax is similarly rejected for now, rather than producing an unsafe or incorrect cast.

## Reassignment – implemented

```mini
program {
    let x: i64 = 0
    while x < 42 {
        x = x + 1
    }
    return x
}
```

`let` declares a local, whereas `x = ...` updates an existing scalar local. A mismatch in type, assignment to an undeclared variable, or assignment after `drop` is rejected. Reassigning resource-owning structs is disallowed until correct move/destructor semantics are implemented. Parameters are conservatively treated as immutable.

## Proposed `as` semantics

```mini
let small: u8 = 42
let wide: u64 = small as u64
```

`as` is intended to be explicit and **checked**: a value that cannot be represented by the target type must fail, not silently wrap or truncate. For floating-point-to-integer conversion, NaNs, infinities, out-of-range values, and rounding behavior require explicit rules. None of these conversions are implemented in 4.2.0.

## Work still required

1. Encode signedness and width in typed IR and target ABIs.
2. Implement integer literal range checks and signed/unsigned machine arithmetic per target.
3. Add a typed cast expression to IR and checked backend lowering.
4. Implement IEEE 754 `f32`/`f64` literals and operations, with correct hardware FPU or software fallback.
5. Cover scalar fields, function arguments, returns, and constructor calls on all targets.
6. Run Cargo and native/emulator tests before declaring any backend complete.

Built-in types and conversions are compiler responsibilities; OS and hardware services remain in `#include` libraries.
