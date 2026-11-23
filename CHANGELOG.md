# Változásnapló / Changelog

## 4.9.1 – Hibaüzenetek (Diagnostics)

### Új

- **Forráspozíció minden szintaktikai és ellenőrzési hibánál**, `#include`-olt fájlokon át is: az előfeldolgozó sortérképet készít, a lexer sor/oszlop pozíciót rögzít, a parser minden utasítás elé helyjelölőt tesz (`St::At`), amelyet az ellenőrző követ. A hibaüzenet a hibás forrássort és – ha ismert – oszlopjelölőt (`^`) is mutat.
- **Stabil hibakódok** (`E0001`–`E0601`, 20 kód) egyetlen táblázatban (`src/diagnostics.rs`). Unit teszt ellenőrzi, hogy a fordító minden hibaüzenete kódot kap.
- `mini --explain E0xxx`: magyarázat rossz és jó példával. `mini --print-errors`: Markdown katalógus; a `docs/HIBAKODOK.md` ebből készül, és a teszt elbukik, ha elavult.
- **Cortex-M:** a normál programvég `BKPT #1`, a trap `BKPT #0` – eddig mindkettő ugyanaz volt. A váratlan kivételek vektorai kifejezetten a trapre mutatnak.

### Ismert korlátok

- A célplatform-kódgenerátorok (backendek) hibái még hely nélkül jelennek meg.
- A trap okát a program még nem írja ki (tervezett `__mini_trap` könyvtári hook).

## 4.9.0 – Alapozás (Foundation)

Az első kiadás, amelyet valódi eszközlánccal fordítottak és teszteltek (Rust 1.100, LLVM 23, QEMU 8.2, Unicorn 2.1). A korábbi kiadások tesztjei soha nem futottak; ez a kiadás ezért **hibajavítás-központú**. Nyelvi újdonság nincs.

### Javított hibák (valódi futtatás során találva)

- **Thumb `BL` kódolás (Cortex-M):** az utasítás alsó 11 bitjébe a bájt-eltolás került a félszavas helyett, így szinte minden függvényhívás rossz címre ugrott. Javítva; regressziós teszt 9 LLVM-referenciaeltolással (`encode_thumb::tests::thumb_bl_matches_llvm_reference`).
- **Thumb lokális változók (Cortex-M):** bináris műveletek és két argumentumú hívások közben az ideiglenesen verembe tolt érték után a lokális változók `sp`-relatív címe elcsúszott, így a második operandus rossz helyről töltődött be (pl. `sum(20, 22)` ≠ 42, a túlcsordulás-ellenőrzés hatástalan). Javítva veremmélység-követéssel; végrehajtási teszt: `tests/thumb_exec49.sh`.
- **Parser regresszió (4.8):** `else` nélküli `if` után következő utasítás „expected statement separator” hibát adott. Ez alapvető programokat és a `lib/io/file.mini`, `lib/memory/linux_buffer.mini` könyvtárakat is érintette.
- **IR literáltípusok:** az egész literálok minden helyzetben `i64` típust kaptak; most a várt típust kapják (paraméter, `let`, értékadás, visszatérés, másik operandus). Ez tette fordíthatóvá a Thumb-példát.
- **Fordítói pánik:** a `mini --check-device … | grep -q …` és hasonló hívások lezárt csővezetéknél pánikoltak. A kimenet most csőbiztos.
- **Könyvtárak nyelvi hibái:** `lib/arm/mmio.mini` nem létező `pub` kulcsszót, `lib/memory/linux_buffer.mini` nem támogatott `bool == false` összehasonlítást használt.
- **Tesztkód:** a unit tesztek egy `Option`/`Result` hiba miatt le sem fordultak; több teszt elavult szabályokat feltételezett (nem létező célnév, `usize` visszatérés `i64` helyett, HIR-dump formátum).

### Viselkedésváltozás

- **Egységes trap:** Linux célokon a futásidejű hiba `exit(101)` helyett illegális utasítás (x86-64 `ud2`, AArch64/ARMv7 `udf #0`): a folyamat SIGILL-lel áll le (shell: 132). Így egy `return 101` és egy túlcsordulás többé nem összetéveszthető.
- **Tömbhatár:** a futásidejű indexhiba végtelen ciklus helyett ugyanezt a trapet hajtja végre (új IR-utasítás: `Stmt::Trap`).

### Tesztelés és eszközök

- `tests/run_all.sh`: egy belépési pont (build, unit tesztek, minden szkript) – ez a CI alapja.
- `tests/foundation49.sh`: a fenti javítások regressziós tesztjei x86-64-en, AArch64-en és ARMv7-en.
- `tests/thumb_exec49.sh` és `tools/run_cortexm.py`: Cortex-M firmware valódi végrehajtása Unicorn emulátorban, MMIO-naplóval.
- `tests/ownership/branch_return_state.mini`: a korábbi „branch_state” teszt helyes (biztonságos) programot ellenőrzött; pozitív teszt lett, és új negatív teszt került mellé.

### Dokumentáció

- Régi kiadási fájlok: `docs/history/`. Új rövid README, ez a változásnapló, valós ellenőrzési mátrix (`docs/VERIFICATION.md`), fejlesztési terv (`docs/MINI_FEJLESZTESI_TERV.md`).
- A 101-es trapkód említései frissítve a kézikönyvekben.

### Ismert korlátok (a következő lépések)

- ~~A hibaüzenetek még nem tartalmaznak sor/oszlop pozíciót és hibakódot.~~ (4.9.1-ben megoldva)
- A trap okát (túlcsordulás, határ, osztás) a program még nem írja ki; tervezett `__mini_trap` könyvtári hook.
- AVR8 és C64: a generált kód végrehajtása emulátorban még nincs automatizálva (csak a meglévő `tests/emulate8.py`).
- ~~A Thumb-backend a normál végén és trapnél is ugyanazt a `BKPT`-t hajtja végre.~~ (4.9.1-ben megoldva)
- Hiányzó nyelvi elemek, amelyek könyvtárakban kerülőutat igényelnek: `not`, `bool` egyenlőség.
