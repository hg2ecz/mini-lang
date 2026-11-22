# Mini 3.7 — Eight-bit backend specification and safety boundary

## Mini ABI

Both 8-bit codegens share the existing typed IR; no VM or special language
intrinsics have been introduced.

- `usize`: unsigned **16-bit** little-endian, checked `+`, `-`, `*`, checked division by zero for `/`, unsigned comparisons.
- `bool`: canonical 16-bit `0` or `1` in the private Mini ABI.
- `i64`: the `program` entry requires `return 0`; general 64-bit `i64` code is **not supported** on these targets.
- Calls: explicit statically linked Mini functions only, maximum **32 call-depth**;
  no recursion, reentrancy, callbacks, indirect function calls or IRQ handlers.
- Parameters, locals and expression temporaries use disjoint per-function
  static slots. Argument expressions are evaluated into **caller-owned staging
  slots** before updating any callee parameter slot. This prevents overwrites
  in cases such as `f(f(1,2), f(3,4))`.
- Call results: **6502 A(low)/X(high)** or **AVR r24(low)/r25(high)**.
- No Mini C interoperability, exported C ABI or general external calling convention.

This static ABI is much simpler than building stack frames for these 8-bit
microprocessors. Its safety depends on **whole-program direct-call visibility**.
The compiler actively detects call cycles and rejects reachable recursion.

## Memory models

**AVR5 device map**: flash/RAM boundaries, vector count, and reserved stack
are read from `targets/devices/*.target` (see `docs/DEVICE_MAP.md`). The
ATmega328P map has 32 KiB Flash and stack top `$08FF`; the ATmega168P map
has 16 KiB Flash and stack top `$04FF`. Static Mini slots are allocated below
the configured stack reserve. Unsupported interrupts enter a trap loop
(not `RETI`). Only the AVR5 instruction subset is supported; incompatible
AVR CPU variants must be rejected.

**Commodore 64 (MOS 6510)**: PRG at `$0801`, BASIC `10 SYS2061`, native code at
`$080D`, entirely below `$A000` (where BASIC ROM overlays RAM). The compiler
reserves `$C000..$CFEF` for static Mini slots and `$CFF0..$CFFF` for internal arithmetic helpers. Included
libraries and any other software must not overwrite that area. Functions
return through hardware `RTS`; the caller's BASIC stack must have sufficient
space. The compiler does not install interrupt handlers or alter the C64
memory bank configuration.

## `unsafe asm` / security

- 6502: `nop`, `sei`, `cli`, one input register `a`, `sta(<absolute 16-bit address>)`.
- AVR: `nop`, `sei`, `cli`, one input register `r16..r31`, `outio(0..63, reg)`,
  `sts(0..configured_data_end-1, reg)`.
- Dynamic `usize` inputs are supported in `a` on C64 and **`r16` on AVR**. For
  other AVR registers only compile-time 8-bit literals are supported.
- Dynamic input >255 branches to a **target trap loop**, not a truncating store.
- Memory/I/O and interrupt mask effects require `clobber memory`. This clause
  documents compiler-visible effects; it does not prove an unsafe library's
  preconditions or peripheral access safety.
- Arithmetic overflow/underflow likewise enters a trap loop. There is no OS
  or exception handler to report a user-friendly error on these boards.

Everything peripheral-related remains in `lib/eightbit/*.mini`.

## Explicitly not yet implemented

General i64 calculations, bitwise operations,
mutable assignment, aggregates/ownership-driven drop, general `asm` register
constraints or outputs, asynchronous callbacks, IRQ dispatch. Unsupported IR
must fail compilation before writing any object image.

## Test plan

1. `cargo test && cargo build --release` on a Rust 2024 capable machine.
2. `bash tests/eightbit36.sh` for compiler rejection tests, binary structure,
   simulated execution of arithmetic, nested call staging and trap behavior.
3. Run the PRG with VICE / C64 to confirm I/O and BASIC state, and verify
   Intel HEX on an ATmega328P with suitable fuse and board wiring.
4. Only after that consider signed 64-bit runtime helpers and additional ISA backends.

**Status**: reference encoding checks completed; full Cargo build and native
execution have not been run in this environment.

## 3.7 unsigned scalar lowering

The Commodore 64 entrypoint executes `CLD` before arithmetic to guarantee binary rather than BCD carry behavior. Checked `usize` multiplication uses a private shift-add helper, guarding addition and shifts. Unsigned division uses 16 restoring steps and traps on a zero divisor. The emitted helpers are only included when reachable expressions require them; the typed IR requires no additional operators. A normal `return 0` now explicitly clears the 16-bit result registers. Unhandled types remain compilation errors.
