> **4.4 pontosítás:** ez a 4.1.3-as példatár; az új egész típusokhoz, `as`-hoz és értékadáshoz lásd a [nyelvi referenciát](MINI_LANGUAGE_REFERENCE_HU.md). Nem minden példa támogatott minden célgépen.

# Mini példaprogram-gyűjtemény – 4.1.3

A gyűjtemény a **tényleges Mini 4.1.x szintaxisát** használja. Minden példa `.mini` forrás, nem pszeudokód. A Linux rendszerhívások az `#include "../../lib/linux_sys.mini"` útján kerülnek be. Az include önmagában nem jelenti az összes függvény gépikódba fordítását.

## Haladási útmutató

| Fájl | Téma | Elvárt kilépési kód |
|---|---|---|
| `examples/tutorial/01_return.mini` | return | 42 |
| `examples/tutorial/02_i64_arithmetic.mini` | i64 arithmetic | 42 |
| `examples/tutorial/03_precedence.mini` | precedence | 42 |
| `examples/tutorial/04_negative.mini` | negative | 42 |
| `examples/tutorial/05_boolean_if.mini` | boolean if | 42 |
| `examples/tutorial/06_comparison.mini` | comparison | 42 |
| `examples/tutorial/07_while_false.mini` | while false | 42 |
| `examples/tutorial/08_function.mini` | function | 42 |
| `examples/tutorial/09_many_parameters.mini` | many parameters | 42 |
| `examples/tutorial/10_nested_calls.mini` | nested calls | 42 |
| `examples/tutorial/11_usize.mini` | usize | 42 |
| `examples/tutorial/12_struct.mini` | struct | 42 |
| `examples/tutorial/13_struct_return.mini` | struct return | 42 |
| `examples/tutorial/14_include_math.mini` | include math | 42 |
| `examples/tutorial/15_dead_code.mini` | dead code | 42 |
| `examples/tutorial/16_arch_conditional.mini` | arch conditional | 42 |
| `examples/tutorial/17_linux_pid.mini` | linux pid | 42 |
| `examples/tutorial/18_linux_ids.mini` | linux ids | 42 |
| `examples/tutorial/19_linux_thread.mini` | linux thread | 42 |
| `examples/tutorial/20_linux_yield.mini` | linux yield | 42 |
| `examples/tutorial/21_linux_write.mini` | linux write | 42, és `A` kiírás |
| `examples/tutorial/22_linux_read_zero.mini` | linux read zero | 42 |
| `examples/tutorial/23_linux_fsync_invalid.mini` | linux fsync invalid | 42 |
| `examples/tutorial/24_linux_dup_invalid.mini` | linux dup invalid | 42 |
| `examples/tutorial/25_runtime_div_zero.mini` | Nullával osztás (futásidejű csapda) | 132 (SIGILL) |

A 01–16. fájlok többsége Linux x86-64, AArch64 és ARM32 környezetben használható; a struktúra- és függvény-ABI működését célgépenként tesztelni kell. A 17–24. példák **Linux-specifikusak**. A 8 bites AVR/C64 és a Cortex-M célokra csak a backend által ténylegesen támogatott részhalmaz fordítható: ezekre nem ígérünk minden példánál működést.

A `examples/negative` könyvtárban fordításkor elutasítandó hibás programok találhatók. A nullával osztás külön futásidejű teszt, nem fordítási hibának minősül.

## Fordítás

```sh
cargo build --release
./target/release/mini --target x86_64-linux examples/tutorial/08_function.mini -o function-demo
./function-demo
echo $?  # 42
bash tests/tutorial413.sh
```

## Linux könyvtár

- `lib/linux_sys.mini`: architektúrafüggő include-útválasztó.
- `lib/linux.mini`: x86-64 Linux.
- `lib/linux_aarch64.mini`: AArch64 Linux.
- `lib/linux_armv7.mini`: ARMv7 Linux EABI.

Közös, új függvények: `sys_getppid`, `sys_getuid`, `sys_geteuid`, `sys_getgid`, `sys_getegid`, `sys_gettid`, `sys_sched_yield`, `sys_dup`, `sys_fsync`.

ARM32 alatt a UID/GID lekérdezések a 32 bites syscall-változatokat használják (`getuid32`, `geteuid32`, `getgid32`, `getegid32`).

`sys_dup` és `sys_fsync` **unsafe**: a nyers fájlleíró érvényességét, tulajdonlását és az esetleges másolatok élettartamát a hívó kezeli. A nyers `sys_write`, `sys_read`, `sys_openat`, `sys_mmap` és `sys_munmap` ugyancsak `unsafe`. A visszatérési érték nyers kernelérték: negatív esetben `-errno`, nem C-könyvtári `errno`.

**Biztonsági figyelmeztetés:** az `unsafe` könyvtárba emelése önmagában nem tesz egy hívást biztonságossá. A `sys_write`-hoz átadott cím és hossz együttesen érvényes olvasható memóriaterületet kell jelöljön, és hasonló feltételek vonatkoznak a többi memóriát érintő hívásra is.

## Korlátok

Az egyszerű Mini még nem támogat általános `x = x + 1` hozzárendelést, így a példák nem használnak számlálós ciklust. A `while false` a ciklusszintaxist mutatja. A példák nem alkalmaznak általános heapet, sztringtípust vagy hálózati absztrakciót; ezek könyvtári megvalósítása külön feladat. A `raw_addr` belső címművelet, használata csak `unsafe` környezetben megengedett.
