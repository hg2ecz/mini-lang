> **4.3-as kiadástörténet:** a 4.4 már ARM Linux egészszám-támogatást is tartalmaz. Lásd [4.4](SCALARS_4_4_HU.md).

# Mini 4.3 – Egész típusok és ellenőrzött `as`

Ez a kiadás **fejlesztési változat**. A forráskód alapján a szélességhelyes egészműveletek első megvalósítása **x86-64 Linuxon** található; ARM, Thumb, AVR8 és C64 célokon az új típusok használata egyértelmű fordítási hiba. A teljes Cargo-fordítást és a keletkező ELF-ek futását a csomag készítésekor nem tudtuk ellenőrizni.

## Típusok

- Előjel nélkül: `u8`, `u16`, `u32`, `u64`.
- Előjelesen: `i8`, `i16`, `i32`, `i64`.
- Célgépfüggően: `usize` (AVR8 és C64: 16 bit; ARM32/Thumb: 32 bit; x86-64/AArch64: 64 bit).
- Logikai: `bool`.
- **Még nem megvalósított:** `f32`, `f64`; a fordító elutasítja a használatukat.

Az x86-64 backend a kis egész típusokat jelenleg **8 bájtos belső Mini ABI-helyeken** tárolja, de az értéktartományukat ellenőrzi. Ez NEM kompatibilis a szabványos C ABI-val, és nem jelenti a memóriahatékony, bájtpontos struktúra-elrendezés megvalósítását.

## Példák

```mini
fn increment(x: u8) -> u8 {
    return x + 1
}

program {
    let n: u8 = 41
    n = increment(n)
    return n as i64
}
```

A `let` változót hoz létre; a `n = ...` új értéket rendel hozzá. Az implicit, típusok közötti konverzió nincs megengedve, **kivéve** az adott típus tartományába férő egyszerű egész literálokat.

```mini
program {
    let a: i64 = 200
    let b: u8 = a as u8
    return b as i64
}
```

## Az `as` jelentése

Az `as` **ellenőrzött egész-egész konverzió**; ha a szám nem fér el a céltípusban, az x86-64 Linux program 101-es kilépési kóddal leáll. Nem csonkolja észrevétlenül a biteket. A `bool` és a lebegőpontos konverziók még nem támogatottak.

A szűk egész típusokon az összeadás, kivonás, szorzás, osztás és negálás után tartományellenőrzés következik. A nullával osztás szintén ellenőrzött leállást vált ki. Az összehasonlítások a típustól függően előjeles vagy előjel nélküli módot használnak.

## Korlátok

1. A lexer jelenleg `i64` tartományú **pozitív decimális literálokat** képes beolvasni: `u64` értéket tárolhatunk, de a `9223372036854775808`–`18446744073709551615` tartományú konstansok közvetlen szintaxisa még hiányzik.
2. A `-9223372036854775808` literál közvetlen beolvasása is hiányzik; más számítások útján az érték előállhat.
3. A kis fix szélességű egész típusokkal rendelkező struktúrák még nem általánosan támogatottak; a struktúramezők továbbra is a régi skalár részhalmazra korlátozottak.
4. Az `f32`/`f64` nem működik; nincs még lebegőpontos lexer, IR és FPU-kódgenerálás.
5. Az új integer és cast utak egyelőre csak x86-64 Linuxra készültek. Nem szabad az ARM/AVR/C64 állapotát késznek tekinteni.
6. **A Cargo és a generált ELF-ek futtatási tesztje nincs igazolva a kiadáskor**; a mellékelt Rust-tesztek a célgépen futtatandók.

## Tesztelés

```sh
cargo test
cargo build --release
bash tests/scalars43.sh
```

A regressziós tesztek tartalmaznak x86-64 Linuxon generált ELF-eket futtató ellenőrzést is; azt `cargo test` végzi azon a platformon.
