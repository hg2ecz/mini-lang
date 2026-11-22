# Mini Linux file owner library – 4.5.1

`lib/io/file.mini` supplies a **minimal owning file object**. Linux system calls remain ordinary `#include`-d Mini functions; no `File` primitive or new IR opcode was added to the compiler.

**Targets:** x86-64 Linux, AArch64 Linux, ARM32 Linux (soft/hard target profiles). This Linux library is not available for AVR8, C64, or Cortex-M.

## API

| Function | Safety | Purpose |
|---|---|---|
| `file_open_readonly(path: i64) -> i64` | `unsafe` | Linux `openat(AT_FDCWD, path, O_RDONLY, 0)`; fd or negative errno |
| `file_openat(dirfd, path, flags, mode) -> i64` | `unsafe` | Raw Linux `openat`; fd or negative errno |
| `file_adopt(fd: i64) -> File` | `unsafe` | Takes unique ownership of a valid, open descriptor |
| `file_read(file: File, address: i64, count: i64) -> i64` | `unsafe` | Borrows `File`; requires valid writable buffer |
| `file_write(file: File, address: i64, count: i64) -> i64` | `unsafe` | Borrows `File`; requires valid readable buffer |
| `drop file` | statement | Explicitly invokes `__drop_File` to close the descriptor |

The compiler automatically calls `__drop_File` at the end of the owner's lifetime. Passing `File` as an argument borrows it in the current restricted Mini ABI; it does not transfer ownership.

## Example 1 – automatic close

`examples/file_auto_drop.mini` opens the file `a` in the working directory, reads one byte, writes it to standard output, and automatically closes the file when returning.

```mini
#include "../lib/io/file.mini"
program {
    let path: i64 = 97
    let buffer: i64 = 0
    unsafe {
        let fd: i64 = file_open_readonly(raw_addr(path))
        if fd < 0 { return 1 }
        let file: File = file_adopt(fd)
        let count: i64 = file_read(file, raw_addr(buffer), 1)
        if count != 1 { return 2 }
        sys_write(1, raw_addr(buffer), 1)
        return 0
    }
}
```

On the three supported *little-endian* Linux targets, the representation of `i64 = 97` begins with `a\0`. This is only a minimal demonstration, **not a general string/buffer API**.

## Example 2 – explicit close

`examples/file_explicit_drop.mini` performs the same open/read operation, then executes:

```mini
drop file
```

The owner cannot be used afterward and is not automatically closed a second time.

## Safety limitations

1. The caller must validate raw pointers and byte counts; these operations are intentionally `unsafe`.
2. Check the `openat` result before passing it to `file_adopt` (never adopt a negative errno).
3. Never adopt a descriptor twice or call `sys_close` independently while `File` owns it. The current Mini checker cannot prove these unsafe contracts.
4. `File.fd` is not private in the current language; this is **not yet a fully safe abstraction**.
5. Destructor errors from `close` are ignored. Retrying `close` after failure is not automatically safe.
6. Full borrow checking and string/slice types are not implemented; 8-bit and Cortex-M targets do not support this Linux file library.
7. Do not separately include `linux_sys.mini` in the same source: it is already included by `file.mini`.

## Tests

```sh
cargo build --release
bash tests/file_io46.sh
```

The test compiles both examples for four Linux target profiles, checks four unsafe/ownership negative cases, and runs the file examples on a native x86-64 Linux host. ARM execution remains a separate target-machine test.
