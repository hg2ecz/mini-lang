## Fixed arrays / Fix tömbök (4.7)

- [Fix méretű lokális tömbök, támogatottság és korlátok (HU)](ARRAYS_HU.md)
- [Fixed local arrays, support and limitations (EN)](ARRAYS_EN.md)

## Linux File I/O / Linux fájlkezelés

- [Fájlobjektum, open/read, auto és explicit drop (HU)](FILE_IO_HU.md)
- [File owner, open/read, automatic and explicit drop (EN)](FILE_IO_EN.md)

## Editor tooling / Szerkesztői eszközök

- [minifmt, Vim és mcedit (HU)](EDITOR_TOOLS_HU.md)
- [minifmt, Vim and mcedit (EN)](EDITOR_TOOLS_EN.md)

# Mini 4.4 documentation / Dokumentáció

**Aktuális / Current language reference**

| Magyar | English | Purpose |
|---|---|---|
| [Mini nyelvi referencia](MINI_LANGUAGE_REFERENCE_HU.md) | [Mini language reference](MINI_LANGUAGE_REFERENCE_EN.md) | Variables, fixed-width integers, `as`, functions, structures, ownership, assembly, limits |
| [Felhasználói kézikönyv](MINI_USER_GUIDE_HU.md) | [User guide](MINI_USER_GUIDE_EN.md) | Step-by-step tutorial (includes older examples) |
| [Példaprogramok](EXAMPLES_HU.md) | [Examples](EXAMPLES_EN.md) | 25 tutorial programs and negative tests |
| [4.4 egész típusok](SCALARS_4_4_HU.md) | [4.4 integer types](SCALARS_4_4_EN.md) | Target-by-target support and checked casts |

## Technical documentation

- [Minimal IR](MINIMAL_IR.md), [compiler/library boundary](BOUNDARY.md), [encoders](ENCODER.md)
- [Target descriptions](DEVICE_MAP.md), [preprocessor](PREPROCESSOR.md)
- [Ownership and drop](OWNERSHIP.md), [AVR8/C64 limits](EIGHTBIT.md)
- [AArch64](AARCH64.md), [ARM32](ARM32.md), [verification](VERIFICATION.md)

## Historical materials / Korábbi verziók

`NUMERIC_TYPES_HU.md`, `NUMERIC_TYPES_EN.md`, `SCALARS_4_3_HU.md`, `SCALARS_4_3_EN.md`, and `history/RELEASE_NOTES_4_1_2.md` preserve historical context and should **not** override the 4.4 references. See also `../PROJECT_HISTORY.md`.

**Compatibility warning:** FPU features in `.target` files do not imply `f32`/`f64` support; new integer widths/`as` are available only on the documented Linux backend subsets. AVR8/C64 and Cortex-M retain their earlier subsets. Linux syscalls, GPIO, and drivers remain `#include` libraries.

- [Memóriamodell: MCU statikus / Linux dinamikus (HU)](MEMORY_MODEL_HU.md)
- [Memory model: MCU static / Linux dynamic (EN)](MEMORY_MODEL_EN.md)

- [Mini 4.8 — checked array indexing (HU)](ARRAYS_4_8_HU.md)
- [Mini 4.8 — checked array indexing (EN)](ARRAYS_4_8_EN.md)

## Hibakódok / Error codes

[HIBAKODOK.md](HIBAKODOK.md) – a fordító által generált hibakatalógus (`mini --print-errors`, `mini --explain E0xxx`).
