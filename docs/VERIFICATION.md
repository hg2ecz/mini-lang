# Verification notes – Mini Native 3.2

## Checks performed while assembling this source archive

- Bash syntax: `bash -n tests/native32.sh` passed.
- Static check: `src/backend_x86.rs` does not contain hexadecimal x86 opcode literals and does not directly emit raw bytes.
- The expected critical x86-64 encodings (signed/unsigned compare, multiplication and division, register moves, flags) were checked against the GNU `as`/`objdump` disassembly available in the packaging environment.
- The tarball was verified with `tar -tzf` and `gzip -t`.

## Checks NOT performed

- **No Rust toolchain (`cargo`, `rustc`) is installed in the packaging environment.** Accordingly, `cargo build`, `cargo test`, `tests/native32.sh` and generated ELF execution were **not run**.
- The new signed overflow traps and ABI preserve tests are source-level work awaiting real native execution. They are **not** yet a guarantee of correctness.
- Mini 3.8 adds limited local-owner destructor calls, but general ownership and memory safety remain incomplete. Neither the new Rust code nor the generated binaries were executed in this environment.

## How to certify the package on x86-64 Linux

```sh
cargo --version
cargo test
cargo build --release
bash tests/native32.sh
bash tests/encoder31.sh
bash tests/dead_code.sh
```

Please report failing test output rather than assuming the compiler works. The Mini language is still experimental.
