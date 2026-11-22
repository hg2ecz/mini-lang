> **Történeti 4.2-es állapot, nem aktuális!** A 4.4-es támogatási mátrixot a [nyelvi referenciában](MINI_LANGUAGE_REFERENCE_HU.md) és a [4.4-es típusleírásban](SCALARS_4_4_HU.md) találod.

# Mini – szám- és változótípusok (fejlesztési állapot)

A Mini nyelvi magjába tartozó tervezett skalártípusok:

| Típus | Bitek | Jelleg |
|---|---:|---|
| `u8`, `u16`, `u32`, `u64` | 8, 16, 32, 64 | előjel nélküli egész |
| `i8`, `i16`, `i32`, `i64` | 8, 16, 32, 64 | előjeles egész |
| `usize` | célgépfüggő: AVR8/C64 16, ARM32/Thumb 32, x86-64/AArch64 64 | előjel nélküli, adatcímzéshez |
| `f32`, `f64` | 32, 64 | IEEE 754 lebegőpontos szám (tervezett) |
| `bool` | logikai | `true`, `false` |

**Megvalósítási státusz 4.2.0:** az `i64`, `usize`, `bool` használható a már támogatott célgépi részhalmazon. Az `u8`–`u64`, `i8`–`i32`, `f32`, `f64` nevei már elkülönülnek a struktúrák neveitől, de a fordító szándékosan hibával elutasítja használatukat. Az `as` is hibával elutasított, amíg nincs minden érintett célgépen helyes kódgenerálása. Ne tekintsük ezeket már működő típusoknak!

## Újraértékadás – implementált

```mini
program {
    let x: i64 = 0
    while x < 42 {
        x = x + 1
    }
    return x
}
```

A `let` új változót hoz létre, az `x = ...` a már létező skalár lokálist írja felül. A típus nem változhat. Ismeretlen változóra, eltérő típusra és `drop` után értékadásra fordítási hiba keletkezik. Összetett, erőforrást birtokló `struct` értékek újraértékadása jelenleg tiltott, mert ahhoz előbb pontos mozgatási/destruktor-szemantika szükséges. A paraméterek újraértékadása konzervatívan tiltott.

## Az `as` tervezett szemantikája

```mini
let small: u8 = 42
let wide: u64 = small as u64
```

Az explicit `as` konverzió nem lehet rejtett bitvesztés vagy véletlen túlcsordulás. A javasolt **ellenőrzött** konverzió célja, hogy a nem ábrázolható értékek hibát okozzanak, ne csendesen csonkolódjanak. A `f32`/`f64` → egész konverzióknál a NaN, végtelen és tartományon kívüli értékek kezelését külön kell definiálni; a kerekítésnek is dokumentáltnak kell lennie. Ez tervezési döntés, nem kész funkció.

## A még hiányzó megvalósítás

1. Szélesség és előjel rögzítése a közös típusos IR-ben és az ABI-ban.
2. Egész literálok tartományellenőrzése, célgépfüggő aritmetika, előjeles/előjel nélküli összehasonlítás.
3. `as` konverzió külön IR-kifejezésként, ellenőrzött célgépfüggő kódgenerálással.
4. `f32`/`f64` literálok és IEEE 754 műveletek – FPU-s célokon hardverrel, FPU nélkül ellenőrzött szoftveres megvalósítással.
5. Paraméterátadás, strukturamezők, visszatérés és konstruktorok az új típusokkal minden célarchitektúrán.
6. Cargo-alapú végponttól végpontig tesztelés, különösen AVR8-on és Cortex-M-en.

A platformkönyvtárak `#include`-ban maradnak; az **alaptípusok és a konverziók a fordító feladatai**.
