# Mini device map v2 — minimal, non-executable

Only five keys are accepted. The map cannot contain code, expressions, aliases,
imports, address arithmetic, sections or linker commands:

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

`device` is a *reviewed hardware identity*, **not a custom CPU name**. The compiler
catalogue binds it to the CPU core and its vector-table layout, and chooses the
output format (AVR -> Intel HEX; Cortex-M -> ELF32, once supported). There is no
`cpu`, `vectors`, `format`, `abi` or `name` key in this file. This intentionally
trades some compiler-internal device metadata for a much smaller and safer
configuration language. Unknown chips are rejected: adding a chip with a new
startup/vector layout requires a catalogue entry in `src/device_config.rs`.

`flash` and `ram` use half-open byte intervals `[start, end)`; AVR has separate
program/data spaces. `stack` reserves bytes at the **high end of RAM**. It is
not a proof of maximum runtime stack use. The compiler rejects invalid
alignment, malformed/overlapping regions, impossible reservation sizes and
unsupported instruction capabilities.

`features` is an explicit **subset** of the chip's checked CPU capabilities.
The presently recognized names are:

| Feature | Meaning |
|---|---|
| `jmp` | AVR absolute JMP instruction |
| `call` | AVR absolute CALL instruction |
| `mul` | AVR hardware integer MUL family available |
| `lpm` | AVR program-memory LPM instruction |
| `thumb2` | Thumb-2 instruction support |
| `udiv` | Hardware integer divide |
| `dsp` | ARM DSP extensions |
| `fpv4_sp` | ARM FPv4 single-precision hardware FPU |

The current AVR5 emitter **requires** `jmp` and `call`; it does not yet support
cores without them. Listing an extra hardware capability doesn't imply the
compiler implements all related instructions. Invalid or duplicated feature
names are rejected, as are impossible device/feature combinations (e.g.
`fpv4_sp` on Cortex-M3). AVR MUL availability is independent of Mini's 16-bit
`*` implementation, which currently uses a software helper.

The Cortex-M4's `fpv4_sp` capability means **hardware FPU exists**, not that
Mini supports floating-point language types or hard-float calling convention.
The bare-metal target's current `abi` remains `soft`; hard-float ABI must be
explicitly implemented and reviewed later. Cortex-M codegen is **not yet
available**; valid Cortex-M profiles can be inspected with `--check-device`
but firmware emission fails closed.

Example for feature-aware library selection:

```mini
#if arch == "avr8"
  #if cpu == "avr5"
    #if device == "atmega328p"
      #include "atmega328p_io.mini"
    #endif
    #if feature == "jmp"
      #include "avr5_system.mini"
    #else
      #error unsupported AVR feature subset
    #endif
  #endif
#elseif arch == "thumbv7m"
  #if feature == "fpv4_sp"
    #include "cortex_m4f.mini"
  #else
    #include "cortex_m_soft.mini"
  #endif
#endif
```

Only an active `#include` file is opened, and the entire resulting Mini source
is type/safety checked. Only reachable functions reach code generation. IRQ
handlers will need explicit reachability roots when interrupt support exists.

Supported device catalogue today: ATmega328P, ATmega168P (AVR5, firmware
prototype) and STM32F103C8 (Cortex-M3, metadata), STM32F401CC (Cortex-M4F,
metadata). Library pin maps, clock setup and peripheral drivers remain
`#include` files.

```sh
mini --check-device targets/devices/stm32f401cc.target
mini --device targets/devices/atmega328p.target examples/device_features.mini -o test.hex
```

Configuration is untrusted input. Validation checks internal consistency and
known instruction subset compatibility, **not** physical chip authenticity,
bootloader placement, clock state, unsafe peripheral address correctness or
full memory safety. The profile's numerical memory boundaries must still be
checked against the manufacturer's datasheet.
