# Compiler / library boundary

## Belongs in the compiler
- parsing, type checking, `unsafe` checking, conservative ownership checks
- target-aware `usize` (32-bit on ARMv7, 64-bit on supported 64-bit targets)
- typed IR, calling convention including hidden output pointer, code emission, executable/firmware writer
- restricted inline assembly with checked operands/effects
- an address-of primitive for local storage (`raw_addr` currently only `i64`)
- `#include` (literal relative paths, single-load), dead function elimination

## Belongs in included Mini libraries
- Linux `read`, `write`, `openat`, `close`, etc. as `asm` wrappers
- allocators, vectors, strings, buffers, files, network, peripherals, port I/O helpers
- higher-level checked APIs built atop trusted unsafe operations

## Critical qualification
Libraries cannot create safety guarantees from an unchecked pointer alone. Safe-facing wrappers require a compiler-enforced ownership/lifetime mechanism and verified implementation of unsafe internals. The restricted 3.8 owner/drop hook is a step forward but does not provide a general borrow checker or verified safe libraries. See `OWNERSHIP.md`.

Target-specific assembly still needs a real machine-code backend for each target; a library cannot provide AVR instruction encoding to an x86-only code generator.

## 3.3: AArch64 Linux backend

A fordítóban most már található AArch64 utasításkódoló és ELF64 emitter. Az
ARM64 Linux rendszerszolgáltatásai továbbra is kizárólag Mini könyvtári
függvények (`lib/linux_aarch64.mini`) `unsafe asm` blokkjain át érhetők el.
## 3.4: ARMv7 Linux backend

Az ARMv7 A32 encoder és ELF32 Linux backend 32 bites `usize`-t használ,
a 64 bites `i64` értékeket pedig két 32 bites regiszterben kezeli.
A Linux szolgáltatások itt is `lib/linux_armv7.mini`-ben maradnak.
A két hard/soft-float változat csak az ELF-ABI-jelölésben különbözik:
a Mini egyelőre nem támogat lebegőpontos adattípusokat és külső C ABI-t.
A Rust-fordítás és az ARMv7 futtatási tesztek még végrehajtandók.
