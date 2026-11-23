# Mini Native 4.5.1 – Linux file owner library + developer tooling

**Mini should do less, but do it reliably.** / **A Mini inkább tudjon kevesebbet, de azt megbízhatóan.**

This is an experimental Rust 2024 compiler. Feature support depends on the target; the source tree is **not** a guarantee of successful compilation or execution on all architectures.

## Documentation (current)

Start with [docs/README.md](docs/README.md). The authoritative language descriptions are:

- [Magyar nyelvi referencia](docs/MINI_LANGUAGE_REFERENCE_HU.md)
- [English language reference](docs/MINI_LANGUAGE_REFERENCE_EN.md)
- [Magyar felhasználói kézikönyv](docs/MINI_USER_GUIDE_HU.md)
- [English user guide](docs/MINI_USER_GUIDE_EN.md)

The fixed-width integer and ARM coverage in 4.4 is documented in `docs/SCALARS_4_4_HU.md` and `docs/SCALARS_4_4_EN.md`. Float (`f32`, `f64`) support is **not implemented**. See [RELEASE_4_4.md](RELEASE_4_4.md) and [PROJECT_HISTORY.md](PROJECT_HISTORY.md) for historical release details.

## Developer tools (new in 4.5)

- **`tools/minifmt`**: Python 3 Mini formatter, with `--check`, `--stdout` and `--write`.
- **`editor/vim/`**: `.mini` filetype detection, syntax highlighting, 4-space indentation defaults.
- **`editor/mcedit/`**: Midnight Commander highlighting rules and `Syntax` registration example.
- [Magyar telepítés és használat](docs/EDITOR_TOOLS_HU.md) / [English setup and usage](docs/EDITOR_TOOLS_EN.md).

The formatter is an **independent developer tool**, not a compiler feature. It conservatively preserves directives, strings and comments; semantic preservation has not been proven for all Mini programs. See its tests and limitations.

## Linux file ownership library (4.5.1)

- `lib/io/file.mini` – owning `File`, `openat` wrappers, borrowed `read`/`write`, automatic and explicit `drop`/`close`.
- [Magyar fájlkezelési útmutató](docs/FILE_IO_HU.md) / [English file I/O guide](docs/FILE_IO_EN.md).
- `examples/file_auto_drop.mini`, `examples/file_explicit_drop.mini`.
- `bash tests/file_io46.sh` (requires a built Mini compiler).

**Caution:** the current language lacks validated slices, private fields, and a checked `Result<File, Error>` type, so raw paths/buffers and descriptor adoption remain `unsafe`. This is not a generally safe file API.

## Build and tests

```sh
python3 tests/test_minifmt.py
bash tests/editor_tooling45.sh
cargo build --release
cargo test
bash tests/docs_consistency44.sh
bash tests/arm_integers44.sh
```

For platform libraries look under `lib/`. For educational examples see `examples/tutorial/` and `docs/EXAMPLES_HU.md` / `docs/EXAMPLES_EN.md`.

**Verification:** formatter unit tests and idempotence across packaged Mini sources, Bash syntax, and Vim filetype/syntax loading were checked here. mcedit execution and a Cargo build/generated-program execution were not verified in this environment.


## Mini 4.6 – target-specific memory model (experimental)

- Bare metal: **no heap**. Fixed-size arrays inside functions are a design requirement, **not yet supported by the current compiler**.
- Linux x86-64 / AArch64: `lib/memory/linux_buffer.mini` implements an owning `mmap` buffer with `__drop_Buffer` / `munmap`; safe capacity/range queries and `unsafe` raw-fd I/O.
- No new compiler IR opcode or allocator builtin is introduced.
- The library is gated using target-aware `#if os` / `#if arch`.
- See [Hungarian memory model](docs/MEMORY_MODEL_HU.md) / [English memory model](docs/MEMORY_MODEL_EN.md).
- Run `bash tests/memory46.sh` (Cargo tests run only if available).

This is **not** a fully memory-safe, general-purpose buffer API yet: fixed arrays, checked byte indexing, and a fully verified ownership checker are pending.

## 4.7: Fixed-size arrays

See [magyar leírás](docs/ARRAYS_HU.md) / [English reference](docs/ARRAYS_EN.md). Literal-indexed fixed local arrays are parsed into existing scalar locals. Dynamic indexes and packed byte arrays are not implemented yet. Run `bash tests/arrays47.sh` on a Rust-enabled machine.

## Mini 4.8 (experimental): checked array indices

See [Hungarian guide](docs/ARRAYS_4_8_HU.md), [English guide](docs/ARRAYS_4_8_EN.md)
and [release notes](RELEASE_4_8.md). The prototype uses an O(N) bounds-checked
branch chain for arrays up to 16 elements. Literal arithmetic indices are
resolved at compile time with no runtime bounds check. The loop-range optimizer,
contiguous indexed storage and runtime fault reporting are not yet implemented.
