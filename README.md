# Mini Native 4.9.1 – Alapozás / Foundation

**A Mini inkább tudjon kevesebbet, de azt megbízhatóan.** / **Mini should do less, but do it reliably.**

Kísérleti, Rust 2024 nyelven írt fordító a Mini nyelvhez. Célok: x86-64 Linux, AArch64 Linux, ARMv7 Linux, Cortex-M (Thumb), AVR8, Commodore 64.
An experimental Rust 2024 compiler for the Mini language.

A 4.9 az első kiadás, amelynek fordítóját és tesztjeit **ténylegesen lefordították és lefuttatták**, a generált programokat pedig x86-64-en natívan, AArch64-en és ARMv7-en QEMU alatt, Cortex-M-en Unicorn emulátorban végrehajtották. Részletek: [CHANGELOG.md](CHANGELOG.md), [docs/VERIFICATION.md](docs/VERIFICATION.md).

## Fordítás és tesztek / Build and test

```sh
cargo build --release
bash tests/run_all.sh        # build + unit tests + every tests/*.sh
```

Opcionális futtatók / optional runners: `qemu-aarch64`, `qemu-arm` (Linux ARM célok), `python3 -m pip install unicorn` (Cortex-M). Ha hiányoznak, az érintett tesztek „SKIPPED” jelzést adnak.

## Használat / Usage

```sh
./target/release/mini program.mini -o program                       # x86-64 Linux
./target/release/mini --target aarch64-linux program.mini -o program
./target/release/mini --device targets/devices/stm32f103c8.target fw.mini -o fw.elf
./target/release/mini --emit-ir program.mini                         # typed IR dump
```

## Futásidejű hibák / Runtime errors

Túlcsordulás, nullával osztás, sikertelen `as` konverzió és tömbön kívüli index esetén a program **trappel**: Linuxon illegális utasítással (SIGILL, a shellben 132-es állapot), Cortex-M-en `BKPT #0` utasítással (a normál vég `BKPT #1`). A trap így soha nem téveszthető össze egy normál visszatérési értékkel.
Overflow, division by zero, failed `as` conversions and out-of-bounds indices trap (SIGILL on Linux, `BKPT` on Cortex-M).

## Hibaüzenetek / Error messages

Minden fordítási hiba stabil kódot és – ahol ismert – `fájl:sor:oszlop` helyet kap, a hibás forrássorral együtt. Magyarázat: `mini --explain E0301`; teljes lista: [docs/HIBAKODOK.md](docs/HIBAKODOK.md).

## Dokumentáció / Documentation

- Kezdőpont / start here: [docs/README.md](docs/README.md)
- [Magyar nyelvi referencia](docs/MINI_LANGUAGE_REFERENCE_HU.md) · [English reference](docs/MINI_LANGUAGE_REFERENCE_EN.md)
- [Magyar felhasználói kézikönyv](docs/MINI_USER_GUIDE_HU.md) · [English user guide](docs/MINI_USER_GUIDE_EN.md)
- Fejlesztési terv / roadmap: [docs/MINI_FEJLESZTESI_TERV.md](docs/MINI_FEJLESZTESI_TERV.md)
- Korábbi kiadási jegyzetek / release history: `docs/history/`

**Figyelem:** a Mini még nem ad általános memóriabiztonsági garanciát; az `unsafe` könyvtári kód helyessége a könyvtár írójának felelőssége.
