# Minimal architecture-aware preprocessor

Before lexing Mini code, the compiler selects the target and expands only
active conditional includes. This remains independent of the common IR.

A condition is exactly `name == value` or `name != value`, with one of:

| Property | Example |
|---|---|
| `arch` | `arch == "avr8"` |
| `os` | `os == "none"` |
| `abi` | `abi == "soft"` |
| `cpu` | `cpu == "avr5"` or `cpu == "cortex-m4"` |
| `device` | `device == "atmega328p"` |
| `feature` | `feature == "fpv4_sp"` or `feature == "mul"` |
| `pointer_bits` | `pointer_bits == 16` |

The `feature` property is true only if the selected device declares it and the
device parser has confirmed that the chip can support it. An unknown feature
name is an error. A hardware FPU and a hard-float ABI are **different facts**.
For non-device targets (e.g. Linux), device-specific `feature` tests are false.

Directives: `#if`, `#elseif`, `#else`, `#endif`, `#include`, `#error`.
Includes use quoted, relative filenames. Unknown directives, invalid
conditions, unclosed/duplicated branches and include cycles are rejected.
Inactive include paths are not opened or parsed. Dependencies and the
function call graph are resolved after preprocessing.

Example in [`examples/device_features.mini`](../examples/device_features.mini).
