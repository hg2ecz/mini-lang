> **Történeti 4.7-es ismertető.** A Mini 4.8 korlátozott, ellenőrzött változóindexelését lásd: [ARRAYS_4_8_EN.md](ARRAYS_4_8_EN.md).

# Mini 4.7 – Fixed-size arrays (first deliberately restricted implementation)

Syntax: `let name: [T; N] = [constant; N]`. Length `N` is in 1..1024. `T` is a supported scalar integer type or `bool`. Arrays use local storage, not a heap. The compiler **lowers each element into an existing scalar local**, without changing the IR opcode set or adding backend primitives.

```mini
program {
    let values: [usize; 3] = [0; 3]
    values[0] = 40
    values[1] = 2
    return (values[0] + values[1]) as i64
}
```

## Safety and limitations

- Indices must currently be **nonnegative integer literals** (`values[1]`), checked at compile time. Variable indices such as `values[i]` are explicitly rejected.
- Repeated initializers are limited to literals (`0`, `true`, `false`, `-1`), so effectful expressions cannot be inadvertently evaluated multiple times.
- Arrays cannot yet be function parameters or returns; slices, copying, runtime indexing and `raw_addr(array)` are unsupported.
- AVR8/C64 backends **do not yet support `u8` arrays**; use scalar element types supported by each backend (such as `usize` or `bool`). Each `usize` consumes the backend's existing 16-bit local slot; this is NOT packed byte storage.
- Existing frame and device RAM checks still apply. Microcontroller storage is statically planned with recursion prohibited; Linux locals use stack frames.
- Linux heap allocation remains in `lib/memory/linux_buffer.mini` using `mmap`/`munmap`. Fully safe `get`/`set` operations are not yet available.

Next: runtime bounds checking, packed element storage, and safe borrowed array/slice parameters.
