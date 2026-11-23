> **Version 4.4.0:** ARM fixed-width integer/checked-cast development update. See [RELEASE_4_4.md](RELEASE_4_4.md) and `docs/SCALARS_4_4_{HU,EN}.md`. Rust/Cargo compilation not verified here.

# Mini Native 4.1.2 – minimal compiler with bilingual user manuals

**Magyar kézikönyv:** [`docs/MINI_USER_GUIDE_HU.md`](docs/MINI_USER_GUIDE_HU.md)  
**English guide:** [`docs/MINI_USER_GUIDE_EN.md`](docs/MINI_USER_GUIDE_EN.md)  
**Changes and verification:** [`docs/RELEASE_NOTES_4_1_2.md`](docs/RELEASE_NOTES_4_1_2.md)

The guides document supported Mini source syntax and known per-target limitations. This release addresses seven Rust compiler warnings without removing target-ASM validation. **Cargo compilation and end-to-end target execution have not been verified in this environment.**

---

## Earlier project notes (may describe previous versions)

# Mini Native 4.1.2 – Warning cleanup + bilingual user guides

**Two complete Markdown manuals:**

- [Magyar felhasználói kézikönyv](docs/MINI_USER_GUIDE_HU.md)
- [English user guide](docs/MINI_USER_GUIDE_EN.md)

**Warnings addressed:** test-only compiler helper (`#[cfg(test)]`); unused
Thumb function map; unused ARM condition enum cases and unused `tst`; test-only
Thumb `bytes`; redundant unused IR `clobbers` field. `clobber` declarations
are still validated **by the frontend for each target**, before conversion
to IR. Regression tests ensure unknown and duplicate x86-64 clobbers fail.
The restricted assembly dialect does not provide arbitrary clobber/register
allocation or a complete inline-assembly safety model.

**Verification:** source/guide checks and shell syntax have been run; a Rust
compiler is unavailable in this packaging environment, so **Cargo builds and
tests have not been executed here**. Your local successful 4.1.1 build is the
baseline; please run `cargo test` and `cargo build --release` for 4.1.2.

---

# Mini Native 4.1 — Cortex-M Thumb backend and consistent Rust names

**Development prototype — Rust compilation and actual MCU execution have not been verified in this environment.**

## Naming convention (enforced by regression script)

- Instruction encoders: `src/encode_x86_64.rs`, `src/encode_aarch64.rs`,
  `src/encode_arm32.rs`, `src/encode_thumb.rs`.
- AArch64 backend: `src/backend_aarch64.rs`.
- Rust test identifiers begin with a letter or underscore. In particular,
  `linux_64_bit_backends_accept_hook_without_new_ir_opcode` and
  `eight_bit_targets_reject_owned_aggregate_safely` replace the invalid
  number-prefixed test names from the 4.0 archive.

## New experimental Thumb target

`--device targets/devices/stm32f103c8.target` (Cortex-M3) and
`--device targets/devices/stm32f401cc.target` (Cortex-M4) now use a restricted
Thumb-2 backend producing an ARM ELF32 file with a Cortex-M vector table.
The subset supports 32-bit `usize`, `bool`, up to two scalar arguments,
addition/subtraction with overflow traps, comparisons, control flow, simple
calls and a small `unsafe asm` whitelist (`nop`, `yield`, `dmb`, `isb`,
`ldr`, `str`). Arithmetic and memory safety beyond that subset is **not
claimed**. Unsupported `i64`, aggregates, recursion, multiplication and
other operations fail at compile time. FPU support is **not implemented**;
`fpv4_sp` is only target metadata.

The IR and reachable-function selection remain shared across all targets.
GPIO and MMIO logic belong to Mini `#include` files, not built-in IR opcodes.

## Commands

```bash
cargo test
cargo build --release
bash tests/source_layout41.sh
bash tests/thumb41.sh
```

`tests/source_layout41.sh` is a source-level naming check; the rest needs
Rust/Cargo and, for hardware validation, the relevant target/emulator.

---

# Historical: Mini Native 4.0 device profiles

**A Mini inkább tudjon kevesebbet, de azt megbízhatóan.**

Source follow-up to the 3.9 compiler prototype. The only new capability is
**strictly declarative MCU target metadata** and its use by the preprocessor.
No new Mini language types, IR operations, OS builtins, peripheral drivers,
linker-script interpreter or runtime dependencies are added.

## A deliberately small `.target` format

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

These are the **only five keys**. The device is a known hardware identity; the
compiler selects CPU family, startup/vector table and output format from a
small reviewed catalogue. Regions are half-open, in bytes; stack bytes are
reserved at the top of RAM. The capability list is a strictly checked subset
of the device's supported instructions. Old keys (`name`, `cpu`, `vectors`,
`format`) are intentionally invalid and must be migrated.

See [docs/DEVICE_MAP.md](docs/DEVICE_MAP.md) for checks, limits and examples.

## Conditional `#include` selection

```mini
#if arch == "avr8"
  #if device == "atmega328p"
    #include "atmega328p_io.mini"
  #elseif device == "atmega168p"
    #include "atmega168p_io.mini"
  #endif
#elseif arch == "thumbv7m"
  #if feature == "fpv4_sp"
    #include "cortex_m4f.mini"
  #else
    #include "cortex_m_soft.mini"
  #endif
#endif
```

In addition to `arch`, `os`, `abi`, `pointer_bits`, the preprocessor now
understands `cpu`, `device` and `feature`. Inactive include files are not
opened. This affects source selection **before** Mini parsing and lowering.
The typed IR remains minimal. Only reachable functions are emitted.

## Known profiles and limits

- ATmega328P, ATmega168P: AVR5, Intel HEX path implemented at source level.
- STM32F103C8: Cortex-M3 without FPU, metadata validation only.
- STM32F401CC: Cortex-M4 with `fpv4_sp`, metadata validation only.
- The bare-metal ARM Thumb emitter is **not yet implemented**; neither ARM MCU
  profile can generate firmware. An FPU capability does **not** enable hard-
  float calling conventions or Mini float types.
- Linux x86-64, AArch64, ARMv7, C64 support from earlier prototypes remains.
  The 8-bit Mini implementation still has scalar/ABI restrictions; read
  [docs/EIGHTBIT.md](docs/EIGHTBIT.md) and [docs/OWNERSHIP.md](docs/OWNERSHIP.md).

## Build, inspect, test

```sh
cargo test
cargo build --release
./target/release/mini --check-device targets/devices/stm32f401cc.target
./target/release/mini --device targets/devices/atmega328p.target examples/device_features.mini -o hello.hex
bash tests/device40.sh
```

**Verification status:** the source and test scripts were prepared, but this
environment has no `cargo` or `rustc`. Rust compilation and generated binary
execution remain **unverified**. This is not a security-certified compiler.
Hardware data needs confirmation against each device's actual datasheet, and
unsafe hardware functions remain the library author's responsibility.


## Mini 4.1.3 tutorial and Linux library extension

Read `docs/EXAMPLES_HU.md` or `docs/EXAMPLES_EN.md`. Examples are under `examples/tutorial/`; platform-independent Linux library entry is `lib/linux_sys.mini`. Run `bash tests/tutorial413.sh` after building the compiler. See `CHANGES_4.1.3.md`.
