> **Történeti 4.7-es ismertető.** A Mini 4.8 korlátozott, ellenőrzött változóindexelését lásd: [ARRAYS_4_8_HU.md](ARRAYS_4_8_HU.md).

# Mini 4.7 – fix méretű tömbök (első, szándékosan korlátozott változat)

A bevezetett alak: `let nev: [T; N] = [konstans; N]`, ahol `N` 1..1024, és `T` egy meglévő skalár egész vagy `bool` típus. A tömb lokális, nem kerül a heapre. A fordító **egy-egy skalár lokális helyre bontja a tömb elemeit**; nincs új IR-opkód vagy új speciális backend.

```mini
program {
    let values: [usize; 3] = [0; 3]
    values[0] = 40
    values[1] = 2
    return (values[0] + values[1]) as i64
}
```

## Biztonság és korlátok

- Az index egyelőre **csak nemnegatív egész literál** lehet (`values[1]`); a fordító azonnal elutasítja a határon túli indexet. A `values[i]` még nem támogatott; nincs hamis biztonsági ígéret.
- Az ismétlő inicializáló jelenleg csak literál (`0`, `true`, `false`, `-1`), mellékhatásos kifejezést nem sokszorozunk meg.
- A tömbök nem adhatók át függvényargumentumként, nem térhetnek vissza függvényből, nincs slice, tömbmásolás, dinamikus indexelés vagy `raw_addr(tömb)`.
- A `u8` elemeket AVR8/C64 backendhez **még nem** támogatjuk: ott továbbra is csak az adott backend által elfogadott skalártípus működik (pl. `usize`, `bool`). A 8 bites gépen az elemenkénti tárolás **16 bites** `usize` helyeket használ, tehát ez nem tömör bájttömb.
- A méret- és RAM-korlátokat a meglévő célgép- és keretellenőrzés érvényesíti. A statikus 8 bites lokális kiosztás rekurziómentes; a Linux-backend lokális stackkeretet használ.
- Linux dinamikus heap: továbbra is a `lib/memory/linux_buffer.mini` `unsafe`-alapú `mmap`/`munmap` könyvtárban. A teljesen biztonságos `get`/`set` még nincs kész.

A következő szükséges lépés a **futásidejű indexek tartományellenőrzése**, a valódi, byte-sűrű tárolás és a kölcsönzött tömb-/slice-paraméterek megvalósítása.
