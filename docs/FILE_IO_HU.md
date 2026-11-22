# Mini Linux fájlkezelő könyvtár – 4.5.1

A `lib/io/file.mini` célja egy **egyszerű tulajdonosi fájlobjektum**. A Linux-rendszerhívások továbbra is Mini `#include`-olt könyvtárakban vannak; a fordítóba nem került `File` primitív vagy új IR-utasítás.

**Támogatott környezet:** x86-64 Linux, AArch64 Linux és ARM32 Linux (soft/hard célprofilok). Az AVR8/C64/Cortex-M rendszereken ez a Linux-könyvtár nem használható.

## Publikus függvények

| Függvény | Típus | Jelentés |
|---|---|---|
| `file_open_readonly(path: i64) -> i64` | `unsafe` | Linux `openat(AT_FDCWD, path, O_RDONLY, 0)`; fd vagy negatív errno |
| `file_openat(dirfd, path, flags, mode) -> i64` | `unsafe` | Nyers Linux `openat`; fd vagy negatív errno |
| `file_adopt(fd: i64) -> File` | `unsafe` | Egyetlen tulajdonosként átvesz egy érvényes, nyitott fd-t |
| `file_read(file: File, address: i64, count: i64) -> i64` | `unsafe` | Kölcsönvett `File`, nyers írható puffer |
| `file_write(file: File, address: i64, count: i64) -> i64` | `unsafe` | Kölcsönvett `File`, nyers olvasható puffer |
| `drop file` | nyelvi utasítás | Azonnali lezárás `__drop_File` hívással |

A `__drop_File` destruktort a fordító automatikusan meghívja a tulajdonos élettartamának végén. A `File` argumentum átadása a jelenlegi Mini ABI szerint *kölcsönzés*, nem tulajdonosváltás.

## Példa 1 – automatikus lezárás

A projekt `examples/file_auto_drop.mini` fájlja megnyitja az aktuális könyvtárban található `a` fájlt, beolvas egy bájtot, kiírja, majd a `return` során automatikusan lezárja a fájlt.

```mini
#include "../lib/io/file.mini"
program {
    let path: i64 = 97
    let buffer: i64 = 0
    unsafe {
        let fd: i64 = file_open_readonly(raw_addr(path))
        if fd < 0 { return 1 }
        let file: File = file_adopt(fd)
        let count: i64 = file_read(file, raw_addr(buffer), 1)
        if count != 1 { return 2 }
        sys_write(1, raw_addr(buffer), 1)
        return 0
    }
}
```

Az `i64 = 97` memóriaképe ezen a három *little-endian* Linux-célgépen egy `a\0` útvonal kezdetét adja. Ez csak rövid demonstráció: **nem általános sztring- vagy puffer-API**.

## Példa 2 – explicit lezárás

`examples/file_explicit_drop.mini` ugyanígy megnyitja és olvassa a fájlt, de olvasás után:

```mini
drop file
```

A változó ezután nem használható, és a függvény végén nem záródik le másodszor.

## Biztonsági korlátok

1. A pointerek (`path`, `address`) és a hosszok ellenőrzése **a hívó felelőssége**. Az `unsafe` itt indokolt.
2. Negatív `openat` eredményt soha nem szabad `file_adopt`-nak átadni.
3. Ugyanazt az fd-t nem szabad kétszer adoptálni, illetve a `File` birtoklása közben közvetlenül `sys_close`-zal lezárni. A jelenlegi típusellenőrző ezt az unsafe szerződést nem tudja bizonyítani.
4. Az fd mező jelenleg nem privát; az API ezért **nem tekinthető teljesen biztonságos absztrakciónak**.
5. A destruktor figyelmen kívül hagyja a `close` hibáját; az fd-t `close` hiba után sem biztonságos automatikusan újrapróbálni.
6. A `File` ownership és a `drop` jelenleg a 32/64 bites Linux backendekhez használható. A teljes borrow checker és a sztring/slice típus még hiányzik.
7. Külön `#include "../lib/linux_sys.mini"` ugyanabban az egységben nem szükséges: a `file.mini` már tartalmazza.

## Teszt

```sh
cargo build --release
bash tests/file_io46.sh
```

A teszt négy Linux célprofilra fordítja a két mintát, négy hibás programot elutasíttat, és x86-64 Linuxon ténylegesen beolvassa az `a` fájl egy bájtját. ARM futtatásra a felhasználó saját gépén van szükség.
