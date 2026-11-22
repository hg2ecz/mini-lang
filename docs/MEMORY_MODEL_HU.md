# Mini memóriamodell – 4.6 kísérleti alap

**Állapot: részleges megvalósítás, nem kész memóriabiztonsági garancia.**

## Célonkénti szabály

| Cél | Foglalás | Automatikus felszabadítás | Tárolási modell |
|---|---|---|---|
| AVR8, Cortex-M, C64 | Heap tilos | Nincs heap, a tulajdonosi perifériaerőforrások `drop`-ja külön kérdés | Fordításkor ismert lokális és statikus tárhely |
| Linux x86-64, AArch64 | `mmap` Mini-könyvtárból | `__drop_Buffer` → `sys_munmap` | Dinamikus tulajdonosi puffer |
| ARM32 Linux | Terv | Még nem validált | Ne feltételezzük, hogy az x86-64 változat helyesen működik |

## Kötelező nyelvi alapok – még fejlesztendők

A fix hosszúságú tömbök **nem valósultak meg a 4.6-os fordítóban**. Javasolt egyszerű szintaxis:

```mini
fn packet() -> i64 {
    let bytes: [u8; 64] = [0; 64]
    bytes[0] = 42
    return bytes[0] as i64
}
```

A fordító feladata: a méret fordításkori ellenőrzése, indexelés határellenőrzése, a tároló helyének kijelölése, valamint a maximális stack-/RAM-használat becslése. Az AVR8 jelenlegi statikus lokálishoz rendelése nem jelent általános tömbtámogatást. A Cortex-M esetén a rögzített méretű stacktömb megengedhető, ha a konfigurált stackkorlátot ellenőrizzük. **Rekurzió és dinamikus méretű stackfoglalás** egyelőre nem megengedett MCU-n.

A biztonságos Linux bájtindexeléshez szükséges minimális fordítói műveletek: típusszélességhelyes load/store és ellenőrzött tömbindex. A nyers pointeres I/O-t addig `unsafe`-nek kell hagyni. Nem vezetünk be `malloc`/`File`/`Vector` speciális IR-műveletet.

## Már szereplő `.mini` könyvtár

`lib/memory/linux_buffer.mini` egy egyszerű `Buffer {address, capacity, valid}` tulajdonost biztosít. A `buffer_new` a Linux `mmap`-ot hívja, hibánál érvénytelen `(0,0,false)` objektumot ad, és csak érvényes mappinget szabad használni. `buffer_valid`, `buffer_capacity` és `buffer_fits` biztonságos ellenőrző műveletek. A nyers descriptorral rendelkező `buffer_read_fd` és `buffer_write_fd` **továbbra is `unsafe`**, bár az offset/length ellenőrzött. `buffer_raw_address` szintén `unsafe`. A `__drop_Buffer` automatikus `munmap`-ot ad a már működő destruktor-beillesztésen keresztül.

### Automatikus felszabadítás

```mini
#include "../../lib/memory/linux_buffer.mini"

program {
    let buf: Buffer = buffer_new(4096)
    if buffer_valid(buf) {
        return 42  // __drop_Buffer(buf) a visszatérés előtt
    }
    return 1
}
```

### Explicit felszabadítás

```mini
let buf: Buffer = buffer_new(512)
drop buf
// Ezután a buf felhasználása fordítási hiba.
```

## Korlátok és ismert kockázatok

- Nincs `Option` / `Result` összetett eredménytípus, ezért a létrehozás sikertelen állapotot is reprezentálhat.
- Nincs általános kölcsönzési és pointer-élettartam ellenőrzés; nem állítjuk, hogy a teljes fájl-/memóriakezelés biztonságos.
- Az ownership-elemzés még nem támogatja az összes áthelyezési, ágelágazási és aggregátum-újraértékadási esetet.
- Az AVR8 és a Cortex-M nem fordíthatja be a Linux-pufferkönyvtárat; az előfeldolgozó elutasítja.
- A fordító és a kibocsátott ELF-ek teljes Cargo-/natív tesztje ebben a környezetben nem futott le.

## Folytatás

Elsőként a **`[T; N]` típus, a határellenőrzött indexelés és az MCU stack-/RAM-keret ellenőrzése** készüljön el. Ezután a Linux `Buffer` kaphat biztonságos `get`/`set` és később dinamikus `Vec<T>` könyvtári API-t, anélkül hogy az IR magasabb szintű erőforrásműveletekkel bővülne.
