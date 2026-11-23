> **4.4 clarification:** this is the original 4.1.3 tutorial set; see the [language reference](MINI_LANGUAGE_REFERENCE_EN.md) for new integer types, checked `as`, and reassignment. Not all examples work on all targets.

# Mini example programs – 4.1.3

All examples use **actual Mini 4.1.x syntax**, not pseudocode. The programs in `examples/tutorial` are numbered from basic expressions through functions, structs, includes, target selection and Linux syscalls. Import Linux wrappers using `#include "../../lib/linux_sys.mini"`. Only reachable functions should be emitted to the output binary.

## Run a program

```sh
cargo build --release
./target/release/mini --target x86_64-linux examples/tutorial/08_function.mini -o function-demo
./function-demo
echo $?  # expected: 42
bash tests/tutorial413.sh
```

Files `01`–`16` introduce Mini's scalar language, basic control flow, functions, structures and includes. Files `17`–`24` use Linux-specific syscalls. File `25` deliberately divides by zero and expects a runtime trap (exit 101). Files in `examples/negative/` are expected to fail at compile time. AVR/C64 and Cortex-M backends only implement subsets; the full tutorial targets mature Linux backends, with ARM execution still requiring verification.

## Linux syscall libraries

`lib/linux_sys.mini` chooses one of `linux.mini` (x86-64), `linux_aarch64.mini`, and `linux_armv7.mini` via preprocessor conditions. All three expose the common new functions: `sys_getppid`, `sys_getuid`, `sys_geteuid`, `sys_getgid`, `sys_getegid`, `sys_gettid`, `sys_sched_yield`, `sys_dup`, and `sys_fsync`.

`sys_dup` and `sys_fsync` are **unsafe** because the caller is responsible for descriptor lifetime and ownership. Existing raw memory and pointer syscalls remain unsafe as well. These wrappers return raw kernel results (negative values are `-errno`), not a libc-style `errno`. A valid raw address must match the memory access size and direction required by the syscall. Marking a library function `unsafe` is not itself a memory-safety proof.

## Important limitations

The current Mini subset has no general variable assignment (`x = x + 1`) or built-in string/heap API; there is no counter-based loop in this collection. `while false` only illustrates the grammar. `raw_addr` is only legal in unsafe contexts. No claim is made that all targets or tests have been run.
