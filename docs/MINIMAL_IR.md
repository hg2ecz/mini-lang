# Mini Core: minimal typed IR and library boundary

Motto: **A Mini inkább tudjon kevesebbet, de azt megbízhatóan.**

## IR essentials

- Typed scalar literals, local values, fields and aggregate constructors.
- Arithmetic/comparison, branches/loops, calls, return with cleanup calls, and drop invalidation.
- Scoped unsafe and assembly operands/effects (current x86-64 restricted subset).
- No dedicated Linux syscall, file, socket, allocator, vector or string operations.

## Dead code elimination

All included source files are parsed and semantically checked. IR lowering computes
transitive reachability starting from `__main` and removes unused functions
**before** code generation. It walks nested control flow, expression trees and
assembly input operands. Recursion terminates using a visited set.

The compiler still embeds fixed ELF metadata and the program entry/exit stub.
Eliminating unused *functions* does not mean identical output size to a raw
single-instruction program. Unreachable compile-time declarations may still be
validated and consume compiler memory; they do not become native code.

## Safety and scope

This is not a proven memory-safe compiler. The restricted Mini 3.8 local-owner
cleanup model lowers `__drop_T` to ordinary typed calls; arbitrary moves,
borrows, nested resources and 8-bit aggregate destructors are unsupported. The `asm`
interface is deliberately restricted. Static calls only; indirect calls and
function-pointer reachability are not supported. Added libraries can contain
unsafe code; safety of their wrappers needs review and tests.

Target support: x86_64, AArch64 and ARMv7 Linux; AVR8 (ATmega328P)
and C64 (MOS 6510) have scalar-only native subsets. RISC-V is not implemented.
