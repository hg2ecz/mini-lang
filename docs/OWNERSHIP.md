# Mini 3.8 — owned locals and library-defined destruction

**A Mini inkább tudjon kevesebbet, de azt megbízhatóan.**

This release adds automatic destruction for function-local, scalar-field
structures on x86-64 Linux, AArch64 Linux and ARMv7 Linux. No File/Socket/Vec,
OS service, memory manager or destructor opcode is added to the IR. A destructor
is an ordinary Mini function from `#include`-loaded source.

## Basic rule

A `struct T` is an owning value when a function named `__drop_T` exists.
The compiler checks **exactly this signature**:

```
struct T { resource: i64 }
fn __drop_T(value: T) {
    // resource-specific cleanup in Mini, with unsafe { ... } when needed
}
```

The destructor is safe (not `unsafe fn`), receives one *borrowed* `T`, and
returns no value. User-written calls of `__drop_T` are forbidden. `drop x`
releases an owning local immediately. Otherwise a local `x` is released
**exactly once** on a reachable `return` or at the end of a `void` function.
Multiple locals are destroyed in reverse declaration order. Return values
are evaluated before cleanup and preserved across normal destructor calls.

Borrowed struct parameters are **never** automatically destroyed. The only
values the compiler owns in this version are struct-typed `let` locals. Copying
an owned struct by assigning one local to another is rejected. `return x` is
not yet an allowed move for a struct local; constructors or function calls can
return aggregates via hidden output storage. The compiler verifies both
branches of `if` have compatible ownership states, and rejects an ownership
change in a `while` loop. Branch/loop-local `let` declarations (including
through `unsafe`) are prohibited until proper block-scoped storage is present.

## Protecting resource invariants

For types with `__drop_T`, calling `T(...)` **requires `unsafe`**. This
prevents safe application code from creating a fake owner, such as an `OwnedFd`
for a descriptor already owned by another object. A safe Mini library API may
return a `T` only after it has validated and established the resource's
ownership invariant; the underlying `unsafe` constructor remains the library
writer's responsibility. An included file is not inherently trusted.

Example (`lib/ownership/linux_fd.mini`):

```
struct OwnedFd { fd: i64 }
fn __drop_OwnedFd(value: OwnedFd) {
    unsafe { sys_close(value.fd) }
}
unsafe fn adopt_fd(fd: i64) -> OwnedFd {
    unsafe { return OwnedFd(fd) }
}
```

This is an **unsafe low-level wrapper**, not a complete safe FD API. It has no
`open` factory that validates pathname/rights, no way to distinguish `-EBADF`
from success, and no retry policy; those belong in Mini libraries. In particular
**never** adopt an FD still owned elsewhere. The current language has no field
privacy, so the safe interface is incomplete.

## What IR/backends do

The front end validates ownership. The existing typed `Call` IR instruction
executes destructors. `Return` carries its ordinary value plus an ordered
list of cleanup calls; void functions have cleanup calls at normal function
end. There is no special OS, allocator, file or destructor operation in the
backend. Dead-code elimination regards implicit cleanup calls as edges in its
call graph. If `__drop_T` is unused, it does not enter the output image.

Backends preserve x86-64 `rax`, AArch64 `x0`, or ARM32 `r0:r1` over cleanup
calls on scalar returns. Struct returns use the existing caller-allocated
result area.

## Explicitly *not* supported or guaranteed

- AVR8 and MOS 6510 do not yet support struct values or resource destructors.
  They reject aggregate programs and continue to support scalar-only programs.
- No nested structs, owning fields, generic Drop traits, moves between owner
  variables, returning a local struct by move, destructors on discarded
  temporaries, exception unwinding, dynamic ownership flags or async cleanup.
- This restricted implementation has function-lifetime owned locals. There is
  no general lexical/block-local ownership yet, and no borrow checker for
  pointer aliasing, nor ownership transfer between threads or callbacks.
- A process exit, hardware fault, trap, interrupt, `sys_exit`, or a destructor
  that does not return bypasses other pending destructors. Destructors cannot
  be proven not to fail. Raw `asm`, raw pointers and incorrect unsafe library
  contracts can still cause memory corruption or double-closing.
- No proof of general memory safety. The Rust crate must still be compiled
  and exercised on the target hardware.

## Tests

```
cargo test
cargo build --release
bash tests/drop38.sh
```

The x86-64 Linux smoke runner compiles and runs separate programs testing
automatic destruction on `return`, `void` function completion, early `return`,
explicit destruction, LIFO order, and scalar return preservation. It also
expects compile-time errors for double drops, fabricated owners, direct
hook calls, invalid hook signatures, divergent branch state and hidden
branch-local declarations. It compiles the Linux library example for all
64/32-bit Linux backends. The tests need a local Rust toolchain; they have
**not** been executed in this workspace.
