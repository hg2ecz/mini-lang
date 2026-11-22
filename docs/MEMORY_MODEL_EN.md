# Mini memory model – 4.6 experimental foundation

**Status: partial implementation, not a complete memory-safety guarantee.**

## Target policy

| Target | Allocation | Automatic release | Storage |
|---|---|---|---|
| AVR8, Cortex-M, C64 | No heap | No heap to free; peripheral owners may still have destructors | Compile-time-sized locals and static storage |
| Linux x86-64, AArch64 | `mmap` from a Mini library | `__drop_Buffer` calls `sys_munmap` | Dynamically allocated owning buffer |
| ARM32 Linux | Planned | Not validated | Do not assume the 64-bit wrapper is valid |

## Necessary language primitives – not yet implemented

Fixed-length arrays **are not implemented in the 4.6 compiler**. Proposed syntax:

```mini
fn packet() -> i64 {
    let bytes: [u8; 64] = [0; 64]
    bytes[0] = 42
    return bytes[0] as i64
}
```

The compiler must verify constant lengths, bounds-check indices, lay out the storage and estimate worst-case stack/RAM usage. Existing AVR static locals do not imply full array support. Cortex-M may use fixed-size stack arrays if the configured stack reserve is checked. Recursion and dynamically sized stack allocation are not yet allowed on MCUs.

Checked byte-level access on Linux requires only small compiler primitives: correctly sized typed loads/stores and checked array indexing. No need for special `malloc`, `File` or `Vector` IR operations. Until then, raw-pointer I/O remains `unsafe`.

## Implemented `.mini` library

`lib/memory/linux_buffer.mini` defines an owning `Buffer {address, capacity, valid}`. `buffer_new` performs Linux `mmap`; failures yield an invalid `(0,0,false)` owner. `buffer_valid`, `buffer_capacity`, and `buffer_fits` are safe checks. `buffer_read_fd` and `buffer_write_fd` remain **unsafe** because their file descriptors are raw, though offset/count ranges are checked. `buffer_raw_address` remains unsafe. The ordinary destructor hook performs automatic `munmap`.

## Limitations

- No general `Option`/`Result` aggregate yet: allocation failures are represented by an invalid owner.
- No full borrow/pointer lifetime checker; do not claim general memory safety.
- Ownership handling is conservative and incomplete for complex moves and control flow.
- The Linux buffer is rejected when included for bare-metal targets.
- Full Cargo/native testing was not possible in this environment.

**Next:** implement `[T; N]`, bounds-checked indexing and MCU RAM/stack accounting; afterwards add safe `get`/`set` buffer APIs, then library-level `Vec<T>`.
