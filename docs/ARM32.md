# ARMv7 Linux A32 backend — Mini Native 3.4

## Targets

- `armv7-linux-soft`: Linux ELF32, `EM_ARM=40`, ARM EABI5, `EF_ARM_ABI_FLOAT_SOFT=0x200`.
- `armv7-linux-hard`: same A32 instruction encoder, EABI5, `EF_ARM_ABI_FLOAT_HARD=0x400`.
- `usize` is 32 bits, `i64` is 64 bits in **two 32-bit registers**.
- ARMv7-A **ARM state**, not Thumb/M-profile. Little-endian target only.
- No C runtime, external linker, assembler, shared library or dynamic linker.

**Important:** Mini's function calls use a **private Mini ABI**, not the ARM Linux
AAPCS32 ABI. Both target variants accept Mini-to-Mini calls only. In the current
integer-only Mini language there are no floating-point parameters or returns,
so the hard/soft ABI flag does not change generated Mini function call sequences.
It does not make C / foreign-function calls ABI-compatible.

## Core responsibilities

The same architecture-neutral typed IR and reachability pruning as x86-64 and
AArch64. The ARM backend performs native instruction selection and encoding,
private stack calling convention, checked integer arithmetic, ELF32 output.

The compiler does **not** provide built-in syscalls, files, vectors, sockets,
allocators or HALs. They are Mini source libraries. Linux EABI syscalls use
`in r7=number`, arguments in `r0..r6`, `svc`, output from `r0`, and a mandatory
`clobber memory`. `lib/linux_armv7.mini` contains raw read, write, getpid,
openat, close, mmap2 (page-offset form), munmap wrappers. A negative signed
32-bit syscall result is sign-extended to a Mini `i64` output. Raw pointer
arguments are taken from the low 32 bits of their Mini `i64` values.

## Safety boundary

- All ARM32 assembly requires `unsafe { asm { ... } }`.
- The supported assembly subset is `svc`, `nop`, `dmb`; only syscall input
  registers `r0..r7`, output `r0`, and `clobber memory` are currently allowed.
  Unknown and unsafe instructions are rejected.
- `unsafe` syscall wrappers accepting raw pointer values cannot promise memory
  safety. Higher-level safe wrappers will need explicit bounds and ownership.
- Arithmetic traps use an illegal instruction (`udf #0`, SIGILL; shell status 132 – a Linux `exit(101)` syscall until 4.8); code generation is not
  a guarantee of complete language or memory safety.
- This backend uses a fixed-address, non-PIE `ET_EXEC` ELF32. It requests a
  non-executable stack but **does not provide PIE/ASLR for its text segment**.
- **Updated in Mini 3.8:** `drop` and automatic function-exit cleanup call
  checked Mini `__drop_T(value:T)` hooks for scalar-field structures. This is
  not general move/borrow tracking; see `OWNERSHIP.md`.
- No floating point, bare-metal, ARM Thumb, ARMv6, general inline assembly,
  arbitrary C ABI, or interrupt-handler ABI in this release.

## Test on an ARMv7 Linux device

```sh
cargo test
cargo build --release
bash tests/arm32_native.sh
```

When the host is not ARMv7 Linux, `tests/arm32_native.sh` uses `qemu-arm` if
available. Otherwise it verifies compilation/ELF flags/rejections, reporting
execution as skipped. It is also valid to build the Rust compiler on x86-64 and
copy only the generated ELF32 program to an ARMv7 Linux board.

Example:

```sh
./target/release/mini --target armv7-linux-hard examples/hello_arm32.mini -o hello_arm32
readelf -h hello_arm32
./hello_arm32
```

The expected stdout is `A` and exit status is 0 **if** that device can execute
ARMv7-A A32 Linux EABI binaries.
