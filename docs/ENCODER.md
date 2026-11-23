# Mini 3.2 code generation contract

The backend is deliberately split into three pieces:

- **IR lowering** (`backend_x86.rs`): local frame layout, expression evaluation, control flow, calling convention; emits only named encoder operations.
- **x86 instruction encoder** (`encode_x86_64.rs`): the sole owner of x86 opcode bytes. The `asm` language and IR codegen call the same encoder.
- **`CodeBuffer`** (`code_buffer.rs`): bytes, symbolic labels and rel32 relocations. No target-specific instruction selection.

The x86 encoder does not accept arbitrary Mini-source byte sequences. Supported `asm` remains the documented restricted dialect; Linux syscall ABI is implemented by `lib/linux.mini` using those instructions.

## Calling convention (internal Mini ABI)

- Every call argument occupies one 8-byte stack word; a struct parameter is a *borrowed address*.
- The caller aligns stack to 16 bytes before `call` and cleans up pushed words after it.
- Scalar results are in `rax`.
- A struct result uses one additional hidden destination pointer, passed as the *last* stack word before `call`.
- The callee owns a 16-aligned local stack frame. Struct fields are currently scalar (`i64`, `bool`, `usize`) stored in 8-byte slots.
- These rules are private Mini ABI conventions, **not** the System V AMD64 function-call ABI.

## Fault behavior

Checked integer arithmetic rejects unsafe cases by executing an illegal instruction (`ud2`, SIGILL, shell status **132**; exit code 101 until 4.8), rather than relying on undefined behavior or unhandled CPU traps. This release adds signed overflow traps for `+`, `-`, `*`, and unary `-`. Division rejects zero and `i64::MIN / -1`.

The unsafe boundary is not a proof of correctness of included system libraries; their buffer lifetime, pointer and file descriptor invariants must be audited and tested separately.

## Porting to other CPUs

For another backend, reuse the typed IR and implement a new instruction encoder plus codegen. Do not add File/Vector/Linux instructions to IR. An ARM/AVR/RISC-V encoder needs its own instruction encodings, relocations and ABI implementation; it must not pretend to support a target until it generates and passes executable tests.
