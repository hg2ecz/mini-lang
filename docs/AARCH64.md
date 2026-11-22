# Mini AArch64 Linux — explicit implementation contract (3.3)

This is a **native AArch64 Linux ELF64 backend**, not an ARM32/AAPCS32 backend.
The Rust compiler host can be x86_64 or ARM64; running the generated ELF
requires Linux AArch64 or a compatible emulator.

## Common front-end / common typed IR

There is one Mini parser, type checker, target-aware preprocessor, reachable-
function pass and typed high-level IR. Only the reachable functions are
transferred to the backend. No OS service is a special IR instruction.

Both x86-64 and AArch64 use `CodeBuffer` for labels and relocations. The A64
backend emits fixed-width 32-bit instruction words and validates branch ranges.
AArch64 calls use PC-relative signed 26-bit displacements (BL); conditional
branches use 19-bit displacements. Out-of-range branches fail the compilation.

## Mini internal ABI (not AAPCS64 / C ABI)

* Each Mini argument occupies a **16-byte-aligned outgoing stack slot**, with
  one 64-bit data word at the bottom of the slot.
* Parameters are pushed from left to right; on entry, the most recent one is
  nearest the callee stack pointer.
* Aggregate-return functions receive a hidden output pointer as the **last**
  argument slot; borrowed aggregate parameters are represented as pointers.
* The callee saves `x29`, `x30` in a 16-byte stack record, uses `x29` as the
  frame pointer, and allocates a 16-byte-aligned local area.
* Scalar results are returned in `x0`; Mini-to-Mini BL calls do not use the
  external platform C calling convention. No external dynamic linking is used.
* Temporary values in expression evaluation and call arguments are stack
  slots. The stack pointer remains 16-byte-aligned at each memory access.

This is a simple private calling convention, not an ABI for invoking C libraries.
The ELF has an RX text mapping and an explicit RW/non-executable `PT_GNU_STACK`.
The text is fixed-address rather than position-independent (no text ASLR).

## AArch64 Linux syscalls via include

`lib/linux_aarch64.mini` contains wrappers, not compiler builtins. The
restricted `unsafe asm` form uses:

- `x8` — syscall number
- `x0..x5` — positional syscall arguments
- `svc` — encodes `svc #0`
- `x0` — result
- `clobber memory` — mandatory for an `svc` block

The assembler also allows the operand-free instructions `nop`, `yield`, `dmb`
(`dmb ish`) and `isb`. Other instructions/operands are compile errors. An
`unsafe` library function accepting an unvalidated pointer remains unsafe to
call even if its assembly is syntactically valid.

## Integer implementation and safety restrictions

The AArch64 backend uses 64-bit signed `i64`, unsigned `usize` (64 bits on this
target), and one 64-bit slot per `bool`. Checked arithmetic traps via Linux
`exit(101)` on signed/unsigned overflow and division by zero. `i64::MIN / -1`
traps. This is not a guarantee of complete memory or resource safety.

**Updated in Mini 3.8:** owning scalar-field structures can call a Mini-written
`__drop_T(value:T)` destructor on exit; this is not a generic borrow checker.
See `OWNERSHIP.md` for limits.

## Unsupported targets and features

- ARMv7 Linux soft-float / hard-float: target names exist; **no binary emission**.
- ARM Cortex-M, AVR8 and RISC-V: no backend in this release.
- General inline assembler; this release supports only the specified subset.
- ARM64 userspace C ABI calls, ELF shared libraries and dynamically linked files.
- `usize` on ARM32: declared as 32 bits but no 32-bit native backend yet.
- Complete ownership/lifetime safety, strings/vectors as builtin IR ops.

## Acceptance

```bash
cargo test
cargo build --release
bash tests/arm64_native.sh
```

The test script can run from a cross-compilation host; native execution is
performed when the host is ARM64 Linux, or when `qemu-aarch64` is present.
On other hosts it reports `SKIPPED` for execution, rather than claiming that
anything ran.
