# Ellenőrzés / Verification – Mini 4.9.1

Ez a dokumentum azt rögzíti, **mit futtattunk ténylegesen** ennél a kiadásnál.

## Környezet

| Eszköz | Verzió | Szerep |
|---|---|---|
| Rust / Cargo | 1.100.0 (conda-forge) | a fordító építése és unit tesztjei |
| LLVM `llvm-mc`, `llvm-objdump` | 23.1 | független referencia az utasításkódolókhoz |
| QEMU user mode | 8.2.2 | AArch64 és ARMv7 Linux programok futtatása |
| Unicorn | 2.1.4 | Cortex-M firmware futtatása (`tools/run_cortexm.py`) |

## Eredmények

| Ellenőrzés | Eredmény |
|---|---|
| `cargo build --release` | sikeres, 0 figyelmeztetés |
| `cargo test` | 110 / 110 sikeres |
| `tests/*.sh` (28 szkript, `tests/run_all.sh`) | mind sikeres (a Vim-integráció kihagyva) |
| x86-64 Linux generált programok | natívan futtatva |
| AArch64 Linux generált programok | QEMU alatt futtatva (`arm64_native.sh`, `foundation49.sh`) |
| ARMv7 Linux generált programok | QEMU alatt futtatva (`arm32_native.sh`, `foundation49.sh`) |
| Cortex-M (STM32F103) firmware | Unicorn alatt futtatva (`thumb_exec49.sh`) |
| Thumb `BL` kódolás | 9 eltolás összevetve az LLVM kimenetével |
| AVR8, C64 | csak fordítás és statikus ellenőrzés; végrehajtás nem automatizált |
| Vim, mcedit szerkesztőintegráció | nem futtatva (az eszközök hiányoztak) |

## Reprodukálás

```sh
cargo build --release
PATH=/path/to/qemu:$PATH bash tests/run_all.sh
```
