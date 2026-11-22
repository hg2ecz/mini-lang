> **4.4.0 dokumentációs frissítés:** A skalár értékadás, a rögzített szélességű egész típusok és az egész–egész ellenőrzött `as` már támogatottak a megfelelő Linux backendeken. Részletek: [nyelvi referencia](MINI_LANGUAGE_REFERENCE_HU.md), [4.4 számok](SCALARS_4_4_HU.md).

# Mini – felhasználói kézikönyv (magyar)

**Verzió:** Mini Native 4.4.0 (a korábbi példák részben 4.1.2-esek)  
**Alapelv:** *A Mini inkább tudjon kevesebbet, de azt megbízhatóan.*

Ez a dokumentum a **jelenlegi fordító által felismert** nyelvet mutatja be, nem egy jövőbeli nyelvtervet. A forrásokban lévő példákat felhasználtuk, de az összes célgépen való futást nem igazoltuk. A Mini jelenleg **nem garantál teljes memóriabiztonságot**.

## 1. Első program

Hozz létre egy `hello.mini` fájlt:

```mini
program {
    return 42
}
```

Fordítás x86-64 Linuxra:

```sh
cargo build --release
./target/release/mini hello.mini -o hello
./hello
echo $?  # elvárt: 42
```

A `program` az induló programblokk. Linuxon a visszatérési érték kilépési kódként használható; héjban csak az alsó 8 bit látszik. A Mini nem rendelkezik beépített `print` paranccsal: a kiírás Mini-könyvtár feladata.

## 2. Megjegyzések, sorvégek, típusok

`//` egysoros megjegyzést kezd. Az utasításokat új sorral vagy `;` jellel választhatod el. A `let` változót hoz létre, **kifejezett típussal**:

```mini
program {
    let a: i64 = -10
    let b: usize = 20
    let ready: bool = true
    if ready { return a + 52 } else { return 1 }
}
```

A jelenlegi alaptípusok:

| Típus | Jelentés |
|---|---|
| `i8`, `i16`, `i32`, `i64` | előjeles 8/16/32/64 bites egész (célgépenként eltérő) |
| `u8`, `u16`, `u32`, `u64` | előjel nélküli 8/16/32/64 bites egész (célgépenként eltérő) |
| `usize` | előjel nélküli célgépfüggő egész |
| `bool` | `true` vagy `false` |
| `f32`, `f64` | fenntartott, még nem megvalósított lebegőpontos típusok |
| `struct` | névvel ellátott, skalár mezőkből álló struktúra |

`usize`: AVR8 és C64 **16 bit**, ARM32/Cortex-M **32 bit**, x86-64/AArch64 **64 bit**. Nem engedélyezett az automatikus `i64` ↔ `usize` átalakítás. A túlcsorduló számításokhoz a támogatott backendeken csapda tartozik; ez nem helyettesíti a hardveres tesztelést.

### Újraértékadás és egész konverzió (4.4)

```mini
program {
    let n: u16 = 40
    n = n + 2
    return n as i64
}
```

Ez x86-64-en, AArch64-en és az ARM32 támogatott egész részhalmazán használható. Az `as` ellenőrzött; nem férő értéknél nem történik csendes csonkolás. ARM32 `u64` aritmetika, Cortex-M/AVR8/C64 új fix szélességű egészműveletei, valamint `f32`/`f64` még nem támogatottak.

## 3. Kifejezések és feltételek

A támogatott műveletek: `+`, `-`, `*`, `/`, `==`, `!=`, `<`, `<=`, `>`, `>=`, valamint a szám elé írt `-`. A szorzás és osztás erősebben köt, mint az összeadás/kivonás. Zárójel használható.

```mini
fn compute(x: i64) -> i64 {
    return (x + 2) * 3
}
program {
    let y: i64 = compute(12)
    if y == 42 { return 42 } else { return 1 }
}
```

A `bool` nem cserélhető fel egész számmal. A feltétel típusa `bool`.

## 4. Függvények

```mini
fn sum(a: usize, b: usize) -> usize {
    return a + b
}

fn tick() {
    // nincs visszatérési érték
}

program {
    let result: usize = sum(20, 22)
    tick()
    if result == 42 { return 0 } else { return 1 }
}
```

`fn név(paraméter: típus, ...) -> eredménytípus` a függvény alakja. A `->` elhagyásakor nincs visszatérési érték. A fordító a főprogramból elérhető függvényeket tartja meg; egy include-olt könyvtár használaton kívüli függvényei kimaradnak a binárisból.

## 5. Elágazás és ciklus

```mini
fn choose(x: usize) -> usize {
    if x > 10 {
        return x
    } else {
        return 10
    }
}

program {
    let a: usize = choose(42)
    while false {
        // ez a törzs nem fut le
    }
    return 0
}
```

A Mini 4.4 támogatja a skalár lokális újraértékadást (`x = x + 1`), ezért hagyományos számlálós `while` ciklus írható. Nincs `break`, `continue`, `for` vagy `match` ebben a kiadásban. Az egyszerűség miatt a jelenlegi ownership-ellenőrzés korlátozza az ágak és ciklusok belsejében létrehozott tulajdonosokat is.

## 6. Struktúrák

```mini
struct Pair {
    left: i64
    right: i64
}

fn make_pair(a: i64, b: i64) -> Pair {
    return Pair(a, b)
}

program {
    let p: Pair = make_pair(20, 22)
    return p.left + p.right
}
```

A struktúrakonstruktor neve a struktúra neve. A mezőértékek a deklarált sorrendben adandók meg. A struktúrák mezői jelenleg csak `i64`, `usize`, `bool` típusúak lehetnek. A struktúratámogatás a Linux 32/64 bites backendekre korlátozódik; AVR8, C64 és Cortex-M esetén bizonyos struktúraműveleteket a fordító elutasít.

## 7. Könyvtárak és feltételes fordítás

A Mini nem épít be fájlkezelést, GPIO-t vagy hálózatot: mindez `#include`-olt Mini-függvényként valósulhat meg. Az include útvonala **a hivatkozó fájlhoz képest relatív**.

```mini
#if arch == "x86_64"
    #include "lib/linux.mini"
#elseif arch == "aarch64"
    #include "lib/linux_aarch64.mini"
#else
    #error This example requires Linux x86-64 or AArch64
#endif

program {
    let pid: i64 = sys_getpid()
    if pid > 0 { return 0 } else { return 1 }
}
```

A `#if` feltételek formája `tulajdonság == érték` vagy `!=`. Támogatott tulajdonságok: `arch`, `os`, `abi`, `cpu`, `device`, `feature`, `pointer_bits`. Az értékek szövegként idézőjelet kapnak, a `pointer_bits` szám. Direktívák: `#if`, `#elseif`, `#else`, `#endif`, `#include`, `#error`. Az inaktív ág include-ját nem nyitja meg a fordító.

A jelenlegi `#include` **szövegesen betölti a deklarációkat**, nem hoz létre automatikusan modulnévteret vagy publikus/rejtett láthatósági rendszert.

## 8. `unsafe` és célgépfüggő assembly

A hardver- és rendszerhívások könyvtárban maradnak. Az assembly csak `unsafe` blokkban használható, architektúránként **korlátozott utasításrészhalmazzal**.

Az alábbi **x86-64 Linux** példa a `lib/linux.mini` könyvtár mintájára:

```mini
fn pid() -> i64 {
    let result: i64 = -1
    unsafe {
        asm {
            in rax = 39
            syscall
            out result = rax
            clobber rcx
            clobber r11
            clobber memory
        }
    }
    return result
}
```

`in`: bemenő regiszter, `out`: létező eredményváltozó, `clobber`: az assembly módosító hatásának kötelező jelzése. A frontend **célgépfüggően ellenőrzi** ezeket; a backend nem enged tetszőleges assemblyt. Az `unsafe` nem tesz biztonságossá egy hibás mutatót vagy egy érvénytelen regiszterhasználatot.

A `raw_addr(változó)` nyers címet képez és csak `unsafe` környezetben használható. Az alacsony szintű pointerkezelést csak ellenőrzött könyvtárban alkalmazd.

## 9. `unsafe fn` és tulajdonosi `drop`

A `unsafe fn` hívása `unsafe` blokkot igényel. Ha egy `T` struktúrához létezik pontosan `fn __drop_T(value: T)` alakú függvény, a Mini tulajdonosi destruktorként kezeli. A destruktor Mini-ben írt függvény, nem új IR-opkód.

```mini
struct Handle { number: i64 }

fn __drop_Handle(value: Handle) {
    // ide kerülhet a könyvtári felszabadítás
}

unsafe fn adopt(value: i64) -> Handle {
    unsafe { return Handle(value) }
}

program {
    unsafe {
        let h: Handle = adopt(42)
        drop h
    }
    return 0
}
```

A `drop h` után `h` nem használható; kétszeri `drop` hibás. A támogatott Linux backendeken a függvény tulajdonosi lokálisai automatikusan felszabadulnak `return`-nél vagy a void függvény végén, fordított sorrendben. **Fontos korlátok:** nincs teljes kölcsönzésellenőrzés, általános move, egymásba ágyazott tulajdonosmező, temporális destruktor vagy garantált felszabadítás hardverhiba esetén. A könyvtári szerződés helyessége az író felelőssége.

## 10. Célgépek és fordítás

```sh
# Linux
./target/release/mini --target x86_64-linux input.mini -o app
./target/release/mini --target aarch64-linux input.mini -o app_arm64
./target/release/mini --target armv7-linux-soft input.mini -o app_arm32
./target/release/mini --target armv7-linux-hard input.mini -o app_arm32_hard

# Commodore 64: 6510, PRG
./target/release/mini --target c64-6510 input.mini -o app.prg

# AVR: konkrét eszközprofil, Intel HEX
./target/release/mini --device targets/devices/atmega328p.target input.mini -o app.hex

# Cortex-M3 / M4: konkrét eszközprofil, ARM ELF32
./target/release/mini --device targets/devices/stm32f103c8.target input.mini -o app.elf

# Eszközprofil ellenőrzése
./target/release/mini --check-device targets/devices/atmega328p.target

# IR megtekintése (alapértelmezett x86-64 célra)
./target/release/mini --emit-ir input.mini
```

A `.target` leíró pontosan öt mezőt használ: `device`, `flash`, `ram`, `stack`, `features`:

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

A vektortábla, az indítási elrendezés és a kimeneti formátum a fordító célgép-katalógusában található. Ismeretlen képességre vagy memóriatartományra a fordító hibát ad.

## 11. Fontos célgép-korlátok

| Cél | Jelenlegi korlát |
|---|---|
| Linux x86-64 / AArch64 / ARM32 | Szélesebb `i64` és struktúratámogatás; Mini-belső ABI, nem automatikusan C ABI |
| AVR8, C64 | `usize` 16 bit; skalár részhalmaz; nincs általános `i64`, struktúra vagy ownership-`drop` |
| Cortex-M3/M4 | `usize` 32 bit, egyszerű skalár részhalmaz, max. 2 skalár paraméter, nincs `i64`, struktúra, szorzás/osztás, rekurzió vagy FPU-kód |
| RISC-V | Célgéptípusok szerepelnek, de nincs kész backend |

A Cortex-M firmware a resetvektorra épül; a `program` blokk jelenleg `return 0` utasítással záruljon. Egy platformkönyvtári MMIO-függvény sem lesz automatikusan biztonságos attól, hogy include-olható.

## 12. Hibakeresés és tanulási út

Futtasd a `cargo test` parancsot, majd a megfelelő `tests/*.sh` regressziós ellenőrzést. A legjobb minták: `examples/usize_ok.mini`, `examples/struct.mini`, `examples/target_linux.mini`, `examples/eightbit_arith.mini` és `examples/thumb_firmware.mini`.

**Javasolt sorrend:** `program` → típusok → függvények → `if`/`while` → `struct` → `#include` → célgép-profil → `unsafe`/assembly → erőforrástulajdon. Egyes példák csak a megfelelő célgépen működnek; a build sikeressége nem bizonyítja a teljes memóriabiztonságot vagy a firmware hardveres helyességét.

## 13. A Mini program felépítése részletesen

A Mini forrás egy vagy több `struct` és `fn` deklarációból, valamint pontosan egy `program` blokkból áll. A `program` kötelező. Az egyszerű nyelvi mag célja az, hogy a program viselkedése a forrásból világosan követhető legyen.

```mini
// demo.mini
fn answer() -> i64 {
    return 42
}

program {
    return answer()
}
```

A függvények deklarációjának sorrendjére ne építs olyan feltételezést, hogy csak előre definiált függvény hívható: a fordító az összes deklarációt összegyűjti az ellenőrzéshez. Ezzel szemben a **változók** használatakor az inicializálás és az élettartam számít. A lokális névnek egy függvényen belül egyedinek kell lennie; a változó-árnyékolást a jelenlegi modell nem kínálja általános szolgáltatásként.

**Jelölések:** a kézikönyvben a `mini` blokkok Mini-forráskódot, a `sh` blokkok terminálparancsokat, az `ini` blokkok adatleírókat jelölnek. A felsorolt utasítások részben korábbi változatokból származnak; az aktuális képességeket a 4.4 nyelvi referencia írja le, nem végleges nyelvi szabványra.

### Miért nincs `print` a nyelvben?

A `print`, `open`, GPIO-beállítás és a memóriafoglalás szolgáltatás, nem az általános programozási nyelvtan része. A Mini ezeket a platformkönyvtárakra bízza. A fordító így nem tartalmaz külön Linux-kódot vagy AVR-perifériaregiszter-táblát. A könyvtári függvény ugyanolyan Mini-függvény, mint amit a felhasználó ír.

## 14. Típusok, literálok és számtani biztonság

A `usize` nem egyszerűen az `i64` másik neve. A szerepük eltér: `i64` rögzített, előjeles 64 bites egész, a `usize` pedig a célgéphez igazodó előjel nélküli egész. Egyes pozitív számliterálokat a fordító `usize`-ként elfogad ott, ahol a várt típus egyértelmű; **tetszőleges `i64` változót nem alakít át automatikusan**.

```mini
fn twice(x: usize) -> usize {
    return x + x
}

program {
    let count: usize = 21
    let doubled: usize = twice(count)
    if doubled == 42 { return 0 } else { return 1 }
}
```

A következő **hibás**: a két változó típusa eltér.

```mini
program {
    let signed: i64 = 20
    let unsigned: usize = signed  // fordítási hiba
    return 0
}
```

Számításnál különbséget kell tenni a fordítási és futásidejű biztonság között. Az ismert típushibát a fordító utasítja el. A túlcsordulást a megfelelő backend által kibocsátott gépi kód ellenőrzi; egy ilyen program nem kap automatikusan helyes matematikai eredményt. A nulla osztó kezelése szintén a támogatott műveletek biztonsági szerződésének része. **A fordító sikeres lefordulása önmagában nem bizonyítja**, hogy valamennyi célgép backendje minden hibahatárt helyesen kezel.

A 4.4-ben `u8`–`u64` és `i8`–`i64` egész típusok már támogatottak x86-64-en és részben ARM Linuxon; lebegőpontos `f32`/`f64`, tömb, sztring, referencia vagy általános biztonságos pointer még nincs. A dokumentációban ezeket nem szabad működő nyelvi elemként feltüntetni.

## 15. A vezérlés jelenlegi határai

Az `if` feltétele és a `while` feltétele logikai érték. A `while` addig ismétli a törzset, amíg a feltétel igaz, és a skalár lokálisok **újraértékadásosak**: az `x = x + 1` támogatott. Ezért a klasszikus számlálós ciklus egyelőre nem a Mini erőssége.

```mini
fn classify(x: usize) -> usize {
    if x < 10 {
        return 1
    } else {
        return 2
    }
}

program {
    let code: usize = classify(12)
    if code == 2 { return 0 } else { return 1 }
}
```

`while`-t elsősorban akkor használj, ha a program külső állapot vagy platformfüggő művelet köré szervezett, és az adott backend ezt támogatja. **Ne feltételezd**, hogy létezik `break`, `continue`, `for` vagy logikai `&&`/`||`: ebben a részhalmazban nincs ilyen operátor.

## 16. Függvényhívás, visszatérés, módszeralak

Az argumentumok száma és típusa fordításkor ellenőrzött. A függvény visszatérési típusa szintén a deklaráció része. A `program` speciális induló blokk, nem közönséges `fn main()` deklaráció.

A parser felismeri a `változó.tag(...)` alakú hívást, de ez **nem általános objektumorientált metódus- vagy traitrendszer**. Csak olyan, a fordító által feloldható névvel és paraméterezéssel használható, amely ténylegesen deklarálva van. Egy összetett, helyes metóduspéldát csak az adott fordítóváltozattal ellenőrizve tekints hordozhatónak.

A `return` azonnal lezárja a függvényt. Amennyiben a függvény élő, destruktorral rendelkező lokálisokat birtokol, a támogatott backendek a visszatérési érték megőrzése mellett a megfelelő felszabadító hívásokat is kibocsátják.

## 17. Struktúrák és adatmodell

A struktúra jelenleg **névvel ellátott, rögzített számú skalár mező**. A mezőlista legalább egy, legfeljebb 32 elemet tartalmazhat. A konstruktor argumentumai a deklaráció sorrendjét követik.

```mini
struct Point {
    x: i64
    y: i64
}

fn origin() -> Point {
    return Point(0, 0)
}

program {
    let p: Point = origin()
    return p.x + p.y
}
```

A struktúramezők között ma még nem lehet közvetlenül újabb struktúrát, vektort vagy sztringet deklarálni. A mezőkhöz `p.x` alakban férsz hozzá. A teljes struktúra-ABI csak a támogatott Linux backendeken értelmezhető; AVR8, C64 és a jelenlegi Thumb backend korlátozásait külön kell figyelembe venni.

## 18. Könyvtár készítése és újrafelhasználása

A saját könyvtár egy közönséges `.mini` fájl. Például `lib/calculations.mini`:

```mini
fn triple(x: usize) -> usize {
    return x * 3
}

fn unused_helper(x: usize) -> usize {
    return x + 100
}
```

A felhasználó programja, `example.mini`:

```mini
#include "lib/calculations.mini"

program {
    let answer: usize = triple(14)
    if answer == 42 { return 0 } else { return 1 }
}
```

A fordító a könyvtárban szereplő **összes feldolgozott deklarációt ellenőrzi**, de gépi kódba csak a `program` blokkból elérhető függvényeket és azok függőségeit írja. A fenti `unused_helper` nem válik bináris kóddá. Ennek pontos eredményét célgépenként ellenőrző teszt igazolja majd; a szabály célja nem a mellékhatással bíró globális inicializálás kezelése, mert ilyen jelenleg nincs.

A `#include` **nem csomagkezelő**, és nem hoz létre automatikusan `pub` láthatóságot vagy modulkülönbséget. Használj egyedi függvényneveket; a névütközés fordítási hibát eredményezhet. Az útvonalat az include-ot tartalmazó fájl helyéhez képest kell megadni.

## 19. Feltételes fordítás biztonságosan

A platformválasztást érdemes egyetlen, kis átirányító könyvtárban elvégezni:

```mini
#if arch == "x86_64"
    #include "lib/linux.mini"
#elseif arch == "aarch64"
    #include "lib/linux_aarch64.mini"
#else
    #error Linux x86-64 or ARM64 required
#endif
```

Az `#if` nem futásidejű `if`: a feltétel **még a Mini parser előtt** dől el. Az inaktív ágban levő könyvtárat a fordító nem nyitja meg. A `feature` feltétel egy pontosan megnevezett képességet vizsgál; például `feature == "fpv4_sp"` csak azt jelenti, hogy a célprofil jelzi ezt a CPU-képességet. Nem jelenti, hogy a Mini már támogat lebegőpontos típusokat vagy FPU-s kódgenerálást.

**Biztonsági tanács:** mindig legyen hibaág a nem támogatott platformokra. Ez világosabb, mint egy olyan include, amely ugyan lefordulhat, de a másik architektúra regisztereit feltételezi.

## 20. Eszközprofilok és memóriaelrendezés

Bare-metal célra az eszközprofil egy szigorú, ötkulcsos adatfájl. Példa:

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

A `flash` és `ram` tartomány **félig nyílt** (`kezdet..vég`): a kezdőcím beletartozik, a végcím már nem. A `stack` a fenntartandó bájtok számát adja meg. A `features` felsorolás csak az ismert és az eszközhöz érvényes utasításképességeket tartalmazhatja.

A profilból a fordító ellenőrzi, hogy a kód és a lefoglalt adatok megfelelnek-e a méretkorlátoknak. A konkrét perifériák címei és működése ezzel szemben **nem profilnyelvi programok**, hanem a megfelelő `#include` könyvtárak részei.

**Fontos:** a hardveres adatlap felülírhat minden feltételezést; egy helyes `.target` fájl még nem garantál működő órajel-konfigurációt, csatlakozóbekötést vagy memóriatérképet.

## 21. Ownership: erőforrások élettartama

A Mini egyszerű ownership-célja: egy erőforrást pontosan egy tulajdonos szabadítson fel. A fordító a támogatott strukturált esetekben nyomon követi, hogy egy lokális érték él-e, és nem engedi a `drop` utáni használatot.

A `__drop_T` konvencióhoz a `T` típushoz egy megfelelő aláírású Mini-függvényt kell megírni. A fordító a tényleges felszabadítás részleteit nem ismeri; **ezek könyvtári feladatok**. A `drop handle` explicit felszabadítás, a függvényből kilépésnél pedig szükség esetén automatikus hívás keletkezik.

Amire most **nem szabad számítani**: általános referencia- és kölcsönzés-élettartamrendszer; összetett, egymásba ágyazott tulajdonosstruktúrák; kivételkezelés; hardveres reset előtti garantált destruktor; minden platformon automatikus `drop`. Fájl- vagy memóriakezelő könyvtár publikus biztonságos API-ját külön auditálni kell.

## 22. Az `unsafe asm` helyes használatának szabályai

A `unsafe` azt jelenti, hogy a szerző vállalja a hardverközeli előfeltételek bizonyítását. **Nem mentesít a helyes címzés, a megfelelő regiszterek és a memóriahatások leírása alól.**

- Írj apró, jól meghatározott feladatú burkolófüggvényt, és a belső `asm`-ot tartsd röviden.
- Ne feltételezz általános assembly-szintaxist. A Mini csak az ellenőrzött, célgépfüggő utasításrészhalmazt fogadja el.
- A `clobber` ellenőrzése a jelenlegi változatban a **típus- és assembly-validálási szakaszban**, az IR létrehozása előtt történik. A minimális IR ezért nem tárol redundáns `clobbers` mezőt. Ez nem jelent tetszőleges regiszterfelülírási jogot.
- A nyers cím és a méret különösen veszélyes: egy szabályos `syscall` utasítás még nem teszi szabályossá a hozzá átadott érvénytelen pointert.
- A könyvtárat külön célgépteszttel és lehetőleg hardveren is ellenőrizd.

A rendszerhívásokat, portműveleteket és MMIO-t ne tedd vissza speciális IR-opkódba: a cél a **minimális nyelv + kicsi gépikódoló + ellenőrzött könyvtár**.

## 23. Hibajegyzék és javítási módszer

| Üzenet vagy tünet | Valószínű ok | Teendő |
|---|---|---|
| `unknown variable` | Nincs ilyen lokális név | Ellenőrizd a nevet és a deklarációt |
| `arithmetic/comparison requires matching ...` | Eltérő típusok | Használj egyező `i64` vagy `usize` típusokat |
| `use after drop` | Már felszabadított tulajdonos | Ne hivatkozz rá többet |
| `unsupported ... asm` | A backend nem ismeri az utasítást | Használj támogatott részhalmazt vagy válts célgépet |
| `backend unavailable` | A célgéphez nincs kész encoder | Ne erőltesd a hibás kimenetet; várd meg a backend támogatását |
| `missing program block` | Hiányzik a belépési blokk | Adj hozzá pontosan egy `program { ... }` blokkot |
| Firmware nem indul | Hibás eszközprofil, reset, stack vagy periféria | Ellenőrizd az ELF/HEX/PRG formátumot, majd a konkrét hardvert |

Hibakereséskor haladj rétegenként: (1) forrás és include-ok, (2) célgép-profil, (3) `cargo test`, (4) a Mini fordító futtatása, (5) kimenet vizsgálata `readelf`/HEX/PRG eszközökkel, (6) valódi célgépes futtatás. A fordító Rust-ból való lefordulása csak az első lépés.

## 24. Gyakorlófeladatok és ellenőrzőlista

**Kezdő:** írj függvényt, amely két `usize` értéket összead, és `program` blokkból hívd meg. Készíts egy másik függvényt, amely `bool` feltétellel két érték közül választ. A kapott eredményt a Linux kilépési kóddal ellenőrizd.

**Középhaladó:** szervezd külön `.mini` fájlba az aritmetikai függvényeket, add hozzá `#include`-dal, és ellenőrizd, hogy a nem használt segédfüggvény valóban kimarad-e a binárisból.

**Haladó:** készíts egyetlen platformfüggő burkolókönyvtárat `#if arch` alapján. Csak a valóban támogatott `unsafe asm`-részhalmazt használd, és a nyers erőforrás-API-t jelöld `unsafe fn`-nek. Adj negatív tesztet az érvénytelen assemblyhez és a `drop` utáni használathoz.

**Kiadási ellenőrzőlista:** a célgép pontos; a forrás csak támogatott típusokat használ; az include-ok célgépfüggők; az `unsafe` felület kicsi; az ownership-szabályok teljesülnek; a negatív tesztek valóban elutasítódnak; a bináris formátuma helyes; a célgépes működés külön ellenőrzött.

---

*Ez a kézikönyv a Mini Native 4.4.0 fejlesztői változatához tartozik. A nyelv és a backendek még fejlődnek; a példák forrásszintű illusztrációk, és nem jelentenek minden architektúrára vonatkozó futtatási garanciát.*


## 13. Gyakorlati példa: kis számolóprogram

Ez a példa nem használ könyvtárat, ezért alkalmas a nyelvi alapok gyakorlására **Linux x86-64 célgépen**:

```mini
fn square(n: i64) -> i64 {
    return n * n
}

fn plus_one(n: i64) -> i64 {
    return n + 1
}

program {
    let a: i64 = square(6)
    let b: i64 = plus_one(a)
    if b == 37 {
        return 0
    } else {
        return 1
    }
}
```

Mentsd `calculator.mini` néven, majd:

```sh
./target/release/mini --target x86_64-linux calculator.mini -o calculator
./calculator
printf 'Kilépési kód: %s\n' "$?"
```

**Várható kilépési kód: 0.** Linuxon a program `return` értékét a kilépési státuszhoz használjuk; az nem általános képernyőre írás.

## 14. Gyakorlati példa: könyvtári függvény hívása

A következő két fájlt helyezd ugyanabba a könyvtárba:

`math.mini`:

```mini
fn twice(x: i64) -> i64 {
    return x + x
}

fn unused_helper(x: i64) -> i64 {
    return x * 100
}
```

`main.mini`:

```mini
#include "math.mini"

program {
    let result: i64 = twice(21)
    if result == 42 { return 0 } else { return 1 }
}
```

A `twice()` fordítódik bele az eredménybe, míg a `unused_helper()` normál esetben kimarad a **hívásgráf alapján**. Ez kódméret-megtakarítás, nem jogosultság- vagy biztonsági határ: az include-olt forrás továbbra is a fordítás része.

## 15. Példa célgépfüggő programra

A feltételes fordítás **nem** a futó programban ágazik el: a fordító a célgép adatai alapján választ ki egy forrásrészletet.

```mini
#if arch == "avr8"
    #include "lib/eightbit/avr_portb_328_168.mini"
#elseif arch == "x86_64"
    #include "lib/linux.mini"
#else
    #error Unsupported platform in this example
#endif
```

A példában az include-útvonalakat az aktuális fájl helyétől kell értelmezni. Csak olyan platformfüggő függvényt hívj, amely a kiválasztott ágban valóban rendelkezésre áll. Az egyes céleszközök memória- és utasításképességeit a `.target` fájl adja meg, a periféria-regisztereket pedig a Mini-könyvtár.

**Fontos különbség:** `#if feature == "fpv4_sp"` annyit jelent, hogy a célgép-profil hirdeti ezt a képességet. Nem jelenti, hogy a Mini 4.4.0 már képes hardveres lebegőpontos kódot generálni.

## 16. Gyakori fordítási hibák

| Jelenség | Valószínű ok | Teendő |
|---|---|---|
| `unknown function` | Nincs deklaráció vagy hiányzó include | Ellenőrizd a függvény nevét és az aktív `#include` ágat |
| Típusegyezési hiba | `i64`, `usize`, `bool` keverése | Használj egyező típusokat; ne várj automatikus konverziót |
| `use after drop` | Már felszabadított objektum használata | A `drop` után ne hivatkozz a változóra |
| `unsafe` hiba | Nyers művelet biztonságos blokkban | Csak ellenőrzött könyvtári kódban használj `unsafe`-ot |
| Nem támogatott célgép-művelet | A backend skalár részhalmaza szűk | Egyszerűsítsd a kódot, vagy használj támogatott célt |
| `unsupported clobber` | Tiltott / nem deklarált assembly-hatás | Ellenőrizd az adott backend assembly-szerződését |
| Firmware túl nagy | Flash, SRAM vagy stack korlát | Ellenőrizd a `.target` adatokat és a program méretét |
| `#if` / `#endif` hiba | Hibás vagy lezáratlan feltételes ág | Minden `#if`-hez kell megfelelő `#endif` |

A fordítóhibák pontos szövege verzióról verzióra változhat. **Elutasított kódot ne próbálj `unsafe`-pal megkerülni**: előbb ellenőrizd, hogy a műveletet az adott backend helyesen meg tudja-e valósítani.

## 17. Hogyan írjunk biztonságos Mini-könyvtárat?

Egy platformkönyvtár készítésekor az alábbi ellenőrzőlista különösen fontos:

1. A publikus, biztonságos függvény **ne fogadjon ellenőrizetlen nyers címet**. Ha a hívó nem tudja teljesíteni a biztonsági feltételeket, a függvény legyen `unsafe fn`.
2. Rendszerhívás vagy MMIO előtt ellenőrizd az argumentumok érvényességét, méretét és élettartamát, amennyire a jelenlegi nyelvi eszközök lehetővé teszik.
3. Ha egy függvény új erőforrást ad vissza, legyen egyértelmű, ki a tulajdonos, és melyik `__drop_T` zárja le.
4. Az assembly csak a célbackend által támogatott utasításokat használja, és tüntesd fel a szükséges clobbereket.
5. Ne feltételezd, hogy a `volatile`, a memóriakorlát-ellenőrzés, a hardveres megszakításbiztonság vagy az atomikusság automatikusan rendelkezésre áll.
6. A könyvtárat külön teszteld az összes célgépen, amelyhez használni szeretnéd.

A Mini egyszerű ownership-ellenőrzése **nem** bizonyítja egy `unsafe` könyvtár belső helyességét. A magasabb szintű API csak akkor nevezhető biztonságosnak, ha a belső invariánsokat az implementáció és a tesztek valóban alátámasztják.

## 18. Melyik példa melyik célgépre való?

| Tanulási cél | Mintafájl | Ajánlott cél |
|---|---|---|
| Első program, kilépési kód | `examples/hello.mini` | x86-64 Linux |
| Függvények és struktúrák | `examples/struct.mini` | x86-64 Linux |
| `usize` | `examples/usize_ok.mini` | x86-64 Linux |
| Linux-könyvtár kiválasztása | `examples/target_linux.mini` | x86-64 / AArch64 Linux |
| Erőforrás-felszabadítás | `examples/ownership_linux.mini` | Támogatott Linux backend |
| 16 bites 8 bites célgépen | `examples/eightbit_arith.mini` | AVR8 / C64 |
| Cortex-M alapfirmware | `examples/thumb_firmware.mini` | STM32F103C8 / F401CC |

A példák egy része negatív teszt: szándékosan hibás, hogy a fordító biztonságos elutasítását ellenőrizze. Ne várd, hogy az `unsafe_rejected.mini`, az `obsolete_rejected.mini` vagy a túlcsordulást vizsgáló minták normál programként lefussanak.

## 19. Korlátok és verziózási szabály

A Mini 4.4.0 **fejlesztői fordító**, és a különböző backendek képességei nem azonosak. A kézikönyv alapján ne következtess arra, hogy egy Linuxon elfogadott struktúrakezelő program AVR8-on vagy Cortex-M-en is támogatott. A memória- és ownership-védelem jelenleg részleges; nincsenek általános szálak, teljes atomikus műveletek vagy C ABI kompatibilitási garanciák.

Új program készítésekor először válassz célt, és csak a backend által támogatott Mini-részhalmazra támaszkodj. Későbbi fordítóverziókban a kézikönyvet a tényleges teszteredményekkel együtt kell frissíteni.
