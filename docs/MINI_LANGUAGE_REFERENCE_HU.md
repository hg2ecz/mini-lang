# Mini nyelvi referencia – a változóktól a struktúrákig

**Nyelvi állapot:** Mini Native 4.4.0 (2026-11-21)  
**Alap:** a 4.4-es `src/main.rs`, `src/preprocess.rs`, `src/target.rs` és a célgép-backendek vizsgálata.  
**Mottó:** *A Mini inkább tudjon kevesebbet, de azt megbízhatóan.*

> Ez a dokumentum a **jelenleg megvalósított nyelv** részletes, használatközpontú leírása, nem a jövőbeli Mini tervezett specifikációja. A fordító elkészülhet, de a támogatás célgépenként eltérhet. A példák egy részét a forráskód alapján állítottuk össze; nincs minden példára és célgépre igazolt futtatási eredmény. Az `unsafe` könyvtári kód memóriabiztonsága nem garantált.

## Tartalom

1. A Mini szemlélete és programmodellje
2. Forrásfájl, megjegyzések, azonosítók
3. Alaptípusok és `usize`
4. Változók és változtathatóság
5. Kifejezések, operátorok, túlcsordulás
6. Elágazás és ciklus
7. Függvények, argumentumok, visszatérés
8. Struktúrák: a Mini adatobjektumai
9. Metódusok és hiányzó objektumfunkciók
10. Ownership és `drop`
11. `unsafe`, nyers címek és assembly
12. `#include` és platformkönyvtárak
13. Feltételes fordítás
14. Célgépek és `.target` fájlok
15. Mit építhetünk könyvtárból?
16. Nyelvi és célgépkorlátok
17. Fordítás, hibakeresés, tanulási út
18. Gyors szintaktikai referencia

## 1. A Mini szemlélete és programmodellje

A Mini **kisméretű, statikusan típusos, natívra fordított** eljárásorientált nyelv. A nyelv középpontjában egyszerű adattípusok, függvények, elágazások, ciklusok és struktúrák állnak. A hardverspecifikus és operációsrendszer-funkciók forrásnyelvi könyvtárakban vannak.

A minimális program:

```mini
program {
    return 42
}
```

A `program` blokk a belépési pont, visszatérési típusa `i64`. Linux alatt a program visszatérési értéke kilépési státuszként használatos (a shell általában csak a legalacsonyabb nyolc bitet jelzi). Bare-metal célgépeken az indítás és a leállás másként működhet.

Egy fordítási egység felső szintjén `struct`, `fn`, `unsafe fn` és **pontosan egy** `program` blokk szerepelhet. A szükséges más fájlokat a `#include` előfeldolgozó tölti be. A fordító csak a belépési pontból és más kötelező gyökerekből elérhető függvények gépi kódját tartja meg.

**Nincsenek beépített** `print`, `File`, `Socket`, `Vector`, GPIO vagy Linux `syscall()` magas szintű szolgáltatások. Ezeket a Mini-könyvtárak valósítják meg az alapműveletekből és az `unsafe asm`-ből.

## 2. Forrásfájlok, megjegyzések, nevek

A forráskiterjesztés `.mini`. A megjegyzés `//` jeltől a sor végéig tart.

```mini
// Egy egyszerű függvény
fn answer() -> i64 {
    return 42
}

program { return answer() }
```

Az utasításokat új sor vagy `;` választja el. Kerek zárójel belsejében a sorvégek nem választják szét az utasítást. Az azonosító betűvel vagy `_` jellel kezdődik; utána szám is lehet. **Nem kezdődhet számmal.**

A jelenlegi lexer nem kezel általános sztringliterálokat, karakterliterálokat, hexadecimális Mini-számliterálokat vagy blokkmegjegyzéseket. A `"..."` idézett szöveg **csak az előfeldolgozó direktíváiban** használható, nem normál Mini-kifejezésként.

A top-level deklarációk sorrendjétől függetlenül hívható egy másik függvény, ha a neve és típusa ismert. Nincs általános névtér-, `pub`-, modul- vagy importalias-rendszer: a `#include` a deklarációkat egy közös névtérbe gyűjti.

## 3. Alaptípusok

| Típus | Jelentés | Megjegyzés |
|---|---|---|
| `i8`, `i16`, `i32`, `i64` | 8/16/32/64 bites előjeles egész | Célgépenként eltérő backend-támogatás |
| `u8`, `u16`, `u32`, `u64` | 8/16/32/64 bites előjel nélküli egész | Célgépenként eltérő backend-támogatás |
| `f32`, `f64` | Lebegőpontos, **még nem támogatott** | A fordító elutasítja |
| `usize` | Célgépfüggő szélességű, előjel nélküli egész | Méret/címzés jellegű számításokhoz |
| `bool` | `true` vagy `false` | Nem cserélhető fel egész számmal |
| `Név` | Felhasználói `struct` típus | Jelenleg csak skalár mezőkkel |
| nincs érték | Visszatérési típus nélküli `fn` | Nem tárolható `void` változó |

A `usize` szélessége: **AVR8 és C64: 16 bit; ARM32 és Cortex-M: 32 bit; x86-64 és AArch64: 64 bit**. A célgép határozza meg, nem a forrásban rögzített szám.

```mini
fn sum(a: usize, b: usize) -> usize {
    return a + b
}
program {
    let result: usize = sum(20, 22)
    if result == 42 { return 0 } else { return 1 }
}
```

Az `i64` és `usize` közötti általános implicit konverzió tiltott. **Az `u8`–`u64` és `i8`–`i64` egész típusok már támogatottak x86-64-en és részben ARM Linuxon. Az `f32`, `f64`, `char`, `string`, `ptr`, tömb és slice továbbra sem általánosan támogatott nyelvi típus.** Hardverközeli adatot az `unsafe` könyvtárak a meglévő típusokra és az assembly interfészre építhetnek, de ez nem helyettesít egy kész, biztonságos típusrendszert.


### 4.4 számtípus-támogatottság

| Célgép | Egész típusok / `as` | Megjegyzés |
|---|---|---|
| x86-64 Linux | `i8`–`i64`, `u8`–`u64`, ellenőrzött `as` | 4.3-as útvonal |
| AArch64 Linux | `i8`–`i64`, `u8`–`u64`, ellenőrzött `as` | 4.4-es fejlesztési útvonal |
| ARMv7-A Linux | `i8`–`i64`, `u8`–`u32`; korlátozott `u64` tárolás/konverzió | `u64` aritmetika és összehasonlítás tiltott |
| Thumb/Cortex-M, AVR8, C64 | Korábbi skalár részhalmaz | Új szélességes típusok és `as` nem támogatottak |

Az **`f32`/`f64` még egyik célgépen sem működik**. Az egész `as` ellenőrzött: a nem ábrázolható érték csapdát eredményez (Linux célokon tervezett 101-es kilépési kód). Közvetlen pozitív egész literál jelenleg legfeljebb `i64::MAX` lehet. A Rust-fordítást és a generált programok végponttól végpontig futását ezen dokumentációfrissítés során nem igazoltuk.

## 4. Változók: létrehozás és korlátok

A változóhoz **kötelező típus és kezdőérték**:

```mini
program {
    let a: i64 = 40
    let b: i64 = 2
    let ok: bool = a + b == 42
    if ok { return a + b } else { return 1 }
}
```

**A skalár változó újraértékadása támogatott:**

```mini
program {
    let x: i64 = 1
    x = x + 1
    return x
}
```

Tulajdonos struktúrák általános újraértékadása még korlátozott.

A lokális változó neve nem deklarálható újra ugyanabban a függvényben. Nincs általános shadowing, `const`, `static`, globális változó vagy `let mut`. A skalár értékek számításokban használhatók; tulajdonos struktúráknál a másolás és felszabadítás szigorúbb.

A jelenlegi statikus ellenőrzés biztonsági okból elutasítja a `let` deklarációkat közvetlen `if`/`while` ágakban; deklarálj inkább a függvény szintjén, vagy szervezd külön függvénybe a logikát.

## 5. Kifejezések és operátorok

| Csoport | Műveletek |
|---|---|
| Számítás | `+`, `-`, `*`, `/`, előjelváltás `-x` |
| Összehasonlítás | `==`, `!=`, `<`, `<=`, `>`, `>=` |
| Csoportosítás | `( ... )` |
| Hívás | `név(arg1, arg2)` |
| Mezőolvasás | `változó.mező` |

```mini
fn evaluate(x: i64) -> i64 {
    return (x + 2) * 3
}
program { return evaluate(12) }
```

A `*`, `/` erősebben köt, mint `+`, `-`; az összehasonlítás gyengébben. A műveletek típusellenőrzöttek. `bool` feltétel szükséges az `if` és `while` utasításnál.

A támogatott backendek ellenőrzött túlcsordulást és nullával osztás elleni csapdát céloznak meg. **A biztonságosságot architektúránként tesztelni kell**: az `i64` műveletek nem támogatottak minden célon. A Mini nem kínál automatikus tetszőleges pontosságú egész számokat, illetve lebegőpontos konverziót; az egész–egész `as` ellenőrzött.

Jelenleg nincs `&&`, `||`, `!`, bitművelet (`&`, `|`, `^`, eltolás), maradék (`%`), összetett értékadás vagy feltételes operátor.

## 6. Vezérlési szerkezetek

```mini
fn maximum(a: usize, b: usize) -> usize {
    if a > b { return a } else { return b }
}
program {
    let result: usize = maximum(42, 10)
    if result == 42 { return 0 } else { return 1 }
}
```

```mini
program {
    while false {
        // A törzs nem fut le.
    }
    return 0
}
```

Támogatott: `if`, `else`, `while`, `return`. A `while` jelenleg főként olyan algoritmusokhoz használható, amelyek feltétele a meglévő változó- és memóriamodell mellett kifejezhető; a hagyományos `i = i + 1` számláló nincs implementálva. Nincs `for`, `break`, `continue`, `switch`, `match`, `defer`, kivételkezelés vagy `try`.

Az ownership-ellenőrző a feltételes `drop` állapotát összefésüli, és elutasítja az eltérő tulajdonosi állapotot. `while` törzséből nem lehet tulajdonost végleg eldobni a jelenlegi modellben.

## 7. Függvények

```mini
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn noop() {
    // Nincs visszatérési értéke.
}

program {
    noop()
    return add(20, 22)
}
```

A paramétereknek és a visszatérési értéknek típusuk van. A `-> T` nélküli függvény nem tér vissza adattal. A főprogramnak visszatérési értéket kell adnia. A fordító elutasítja a hiányzó vagy rossz típusú visszatérést és az elérhetetlen utasítást közvetlen `return` után.

Több argumentum támogatott a teljes Linux backendeken. Célgépfüggő korlát lehet a paraméterszám, rekurzió, skalártípus vagy függvény-ABI. A Mini saját belső ABI-ja **nem egyenlő automatikusan a C ABI-val**.

A `fn` név az egyetlen függvényazonosító; nincs túlterhelés, generikus függvény, névtér, függvénymutató, lambda vagy dinamikus dispatch. A nem használt könyvtári függvények kimaradnak a binárisból.

## 8. Struktúrák – a Mini objektumai

A `struct` névvel ellátott adatcsoport. Jelenleg **1–32 mező**, és a mezők csak `i64`, `usize`, `bool` típusúak lehetnek.

```mini
struct Point {
    x: i64
    y: i64
}

fn point_sum(p: Point) -> i64 {
    return p.x + p.y
}

program {
    let p: Point = Point(20, 22)
    return point_sum(p)
}
```

A `Point(20, 22)` pozicionális konstruktor: az értékeket a mezők deklarációs sorrendjében adja át. A `p.x` **mezőolvasás**; általános `p.x = 5` mezőírás nincs. A struktúra nem tartalmazhat jelenleg más struktúrát, vektort, sztringet vagy beágyazott tulajdonosi objektumot. A `struct` nem osztály: nincs automatikus heapallokáció és nincs objektumfejléc.

A struktúrák kezelése az x86-64, AArch64 és ARM32 Linux backendeken fejlettebb; **AVR8, C64 és Cortex-M esetén a struktúraátadás/ownership nem teljes, egyes esetek fordítási hibák**. Az összetett visszatéréshez a teljes backendek rejtett eredménycímet használnak.

### Tulajdonosi objektum példa

```mini
struct Token { number: i64 }

fn __drop_Token(value: Token) {
    // Ide kerülne a könyvtári felszabadítás.
}

unsafe fn make_token(number: i64) -> Token {
    unsafe { return Token(number) }
}

program {
    unsafe {
        let token: Token = make_token(42)
        drop token
    }
    return 0
}
```

Ez az ownership-szerződést szemlélteti; a `__drop_Token` jelen példában nem csinál erőforrásműveletet. Éles könyvtárban a konstruktor és destruktor együtt felelnek az erőforrás érvényességéért.

## 9. Metódusok: ami van, és ami még nincs

A parser felismeri a `változó.metódus(...)` **hívási formát**, és belül névalapú függvényhívássá alakítja. A jelenlegi felső szintű függvénydeklaráció azonban nem támogatja a `fn T.metódus(...)` névalakot, és nincs `self`/`Self` nyelvi mechanizmus. Ezért **nem áll rendelkezésre teljes, felhasználó által deklarálható metódusrendszer**.

**Javasolt, ma működő minta:** írj normál függvényt, első paraméterként a struktúrával:

```mini
struct Meter { value: i64 }

fn meter_value(m: Meter) -> i64 {
    return m.value
}

program {
    let m: Meter = Meter(42)
    return meter_value(m)
}
```

**Nincs**: `class`, öröklés, `interface`/`trait`, virtuális metódus, generikus objektum, konstruktor-túlterhelés, tulajdonság (`property`), operátor-túlterhelés, polimorfizmus, általános `self` vagy heapobjektum-modell.

## 10. Ownership és `drop`

A Mini tulajdonosi ellenőrzése konzervatív. A cél az, hogy a kezelt objektumoknál megakadályozza a nyilvánvaló dupla felszabadítást és a felszabadítás utáni használatot. Nem teljes Rust borrow checker.

- `drop változó` csak struktúrára alkalmazható; skalárra nem.
- A `drop` után a változó használata fordítási hiba.
- Kétszeri `drop` fordítási hiba.
- Kölcsönkapott struktúraparamétert nem szabad felszabadítani.
- Egy `T` típus destruktora egy speciálisan ellenőrzött `fn __drop_T(value: T)` függvény.
- A teljes Linux backendek az élettartam végén közönséges destruktorhívásokat szúrnak be, fordított lokális sorrendben.
- A destruktor nem külön IR-művelet: **normál függvényhívás**.

**Korlátok:** nincs általános move-kifejezés, külön élettartamjelölés, teljes borrowing, struktúrába ágyazott birtokolt mezők rekurzív felszabadítása, RAII-garancia minden lehetséges control-flow helyzetre. A destruktor helyességét az `unsafe` könyvtárnak kell biztosítania. A statikus ellenőrzés jelenleg szándékosan tilt több összetett vezérlési mintát.

## 11. `unsafe`, címek és assembly

A nyers gépi műveletek nem külön rendszerhívás-primitívek, hanem `unsafe asm` blokkok.

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

program { return pid() }
```

Ez **x86-64 Linuxra** példa. Más architektúrán más regiszterek, syscall-számok és utasítások kellenek. `unsafe fn` hívása csak `unsafe` környezetben megengedett. A `raw_addr(...)` nyers címet képez; kizárólag `unsafe` használat indokolt.

Az `asm` **nem általános beágyazott assembler**. Csak a parserben és a kiválasztott backendben engedélyezett utasítások, regiszterek és operandusok támogatottak. Az `in`, `out`, `clobber` leírja a bemeneteket, eredményeket és módosító hatásokat, de az ellenőrzés nem bizonyítja a teljes memóriabiztonságot, az adatverseny-mentességet vagy a hardverregiszter helyességét.

**Könyvtárszabály:** nyers Linux syscall, MMIO, port I/O, megszakítás és memóriafoglalás kerüljön `.mini` könyvtári függvényekbe. Egy könyvtári burkoló csak akkor legyen biztonságos API, ha a bemeneti és tulajdonosi előfeltételeit ténylegesen ellenőrzi.

## 12. `#include` és könyvtárak

Példa fájlpár:

`math.mini`:

```mini
fn answer() -> i64 { return 42 }
fn unused() -> i64 { return 100 }
```

`main.mini`:

```mini
#include "math.mini"
program { return answer() }
```

A relatív útvonal a **hivatkozó fájl könyvtárából** indul. A preprocessor a fájlt beolvassa, a deklarációkat azonos névtérbe helyezi, és a betöltött fájlt az aktuális fordítás során csak egyszer dolgozza fel; a ciklikus include hibás. A nem használt `unused()` nem kerül a gépi kódba. Ez nem külön linker: a függvényelérhetőségi elemzés a Mini fordítójában történik.

A 4.4.0 Linux API-k három architektúraspecifikus könyvtárban érhetők el (`lib/linux.mini`, `lib/linux_aarch64.mini`, `lib/linux_armv7.mini`), a `lib/linux_sys.mini` pedig céltól függően választ közülük. A nyers syscall-függvények általában `unsafe` szerződéseket igényelnek.

## 13. Feltételes fordítás

```mini
#if arch == "avr8"
    #include "drivers/avr_gpio.mini"
#elseif arch == "thumbv7m"
    #include "drivers/thumb_gpio.mini"
#else
    #error Unsupported target
#endif
```

A fenti `drivers/*` fájlnevek **szemléltetők**, a projektben lévő tényleges fájlhoz igazítsd őket. A preprocessor az **inaktív ágból nem nyit meg include-ot**. Feltételek: `==`, `!=`; ismert változók `arch`, `os`, `abi`, `cpu`, `device`, `feature`, `pointer_bits`. A stringérték idézőjeles, a `pointer_bits` szám.

```mini
#if pointer_bits == 16
    // AVR8 vagy C64
#elseif pointer_bits == 32
    // ARM32 / Cortex-M
#else
    // x86-64 / AArch64
#endif
```

A `feature` jelenleg célgépprofilból kiolvasott képesség, nem tetszőleges szöveg. A feltételes include segít ugyanazon magas szintű API-hoz eltérő platformmegvalósítást választani.

## 14. Célgépek és konfiguráció

Példa eszközleíró (`.target`):

```ini
device = atmega328p
flash = 0x0000..0x8000
ram = 0x0100..0x0900
stack = 256
features = jmp,call,mul,lpm
```

A leíró **öt mezője**: `device`, `flash`, `ram`, `stack`, `features`. Nem teljes linker-szkript, nincs végrehajtható logika. A vektortábla és a kimeneti formátum a fordító célgépadatbázisában található; a perifériakód a Mini-könyvtárban. A profil memóriatartománya félig nyitott (`kezdő..vég`, a vég nem része a tartománynak).

```sh
cargo build --release
./target/release/mini --target x86_64-linux main.mini -o app
./target/release/mini --target aarch64-linux main.mini -o app_a64
./target/release/mini --target armv7-linux-soft main.mini -o app_arm
./target/release/mini --device targets/devices/atmega328p.target main.mini -o app.hex
./target/release/mini --device targets/devices/stm32f103c8.target main.mini -o app.elf
./target/release/mini --target c64-6510 main.mini -o app.prg
./target/release/mini --emit-ir main.mini
```

A célgép megválasztása önmagában nem garantálja, hogy a program minden IR-művelete támogatott. Az ismeretlen vagy nem támogatott műveletet **fordítási hibával** kell elutasítani.

## 15. Mit építhetünk könyvtárból?

**Jó könyvtári jelöltek:** fájl- és socketkezelés, szabványos I/O, Linux syscall burkolók, GPIO, UART, SPI/I²C, időzítés, memóriakezelő, dinamikus konténerek, sztringműveletek, protokollok, driverek. Ezek megvalósításához a ténylegesen támogatott alacsony szintű műveletek és adatmodellek szükségesek.

**A fordító elengedhetetlen feladatai:** lexer/parser, típus- és ownership-ellenőrzés, `#include` és fordítási feltételek, hívás/return/ABI, célgépfüggő skalárműveletek, ellenőrzött assembly, elérhető függvények kiválasztása, IR, gépikód és futtatható/firmware-fájl.

**Lényeges különbség:** egy szolgáltatás **elvileg könyvtárba való** akkor is, ha a jelenlegi Mini adat- és mutációs modellje még nem elegendő egy teljes értékű implementációjához. A „mehet `#include`-ba” nem jelenti automatikusan, hogy ma már minden szükséges primitív rendelkezésre áll.

## 16. Jelenlegi korlátok – ellenőrző mátrix

| Nyelvi képesség | 4.4.0 állapot |
|---|---|
| Típusos `let`, számok, `bool` | Van |
| `+ - * /` és összehasonlítás | Van, célgépfüggő támogatással |
| `if/else`, `while`, `return` | Van |
| Általános `x = expr` | **Van, skalár lokálisokra** |
| `fn`, többparaméteres hívás | Van; célgépfüggő korlátok |
| `struct` pozicionális konstruktor, mezőolvasás | Van; teljesebb Linux backendeken |
| Általános metódusdeklaráció, `self`/`Self` | **Nincs** |
| Tulajdonosi `drop` | Részleges, konzervatív |
| Automatikus `drop` | Linux backendeken, korlátozásokkal |
| `unsafe asm`, `unsafe fn` | Van, korlátozott utasításkészlettel |
| `#include` és `#if` | Van |
| Sztring, tömb, slice, referencia, pointertípus | **Nincs** általános nyelvi támogatás |
| `class`, öröklés, trait, generikus típus | **Nincs** |
| `for`, `break`, `continue`, logikai `&&`/`||` | **Nincs** |
| Teljes ownership/borrow-checker, garantált memóriabiztonság | **Nincs** |

**Célgép-összefoglaló:** Linux x86-64/AArch64/ARM32: a legteljesebb támogatás; AVR8 és C64: 16 bites `usize`, korlátozott skalárok és `unsafe asm`, nincs általános struktúra/`i64`/`drop`; Cortex-M3/M4: 32 bites skalár részhalmaz, korlátozott paraméterezés, nincs teljes `i64` vagy struct; RISC-V: még nincs működő backend.

## 17. Fordítás, hibakeresés, tanulási út

A forráscsomagban található `examples/tutorial/` 25 tanulópéldája és `tests/tutorial413.sh` tesztje jó kiindulás. Ajánlott sorrend: `program` → típusok és `let` → számítások → függvények → struktúrák → include → `unsafe` → ownership → több architektúra.

Fordítási hibák, amelyekkel szándékosan találkozhatsz:

```mini
// Hibás: hiányzik a típus
program { let n = 42; return n }

// Helyes: skalár újraértékadás
program { let n: i64 = 1; n = 2; return n }

// Hibás: nincs sztringliterál
program { return "hello" }
```

A `cargo build --release` csak a **fordítót** állítja elő. Ezután a `.mini` program fordítását külön kell ellenőrizni, végül a generált programot a megfelelő hardveren vagy emulátorban tesztelni. A build, a gépikódformátum és a tényleges futás **három külön bizonyítási lépés**.

## 18. Gyors referencia

```text
program { utasítások }
fn név(param: típus, ...) -> típus { utasítások }
fn név(...) { utasítások }
unsafe fn név(...) -> típus { utasítások }
struct Név { mező: i64; másik: usize }
let változó: típus = kifejezés
változó = kifejezés  // skalár lokális
kifejezés as céltípus  // ellenőrzött egész konverzió
if feltétel { ... } else { ... }
while feltétel { ... }
return kifejezés
drop struktúraváltozó
unsafe { ... }
unsafe { asm { in reg = expr; out változó = reg; clobber reg; utasítás } }
#include "relatív/út.mini"
#if arch == "x86_64" ... #elseif ... #else ... #endif
```

**Tervezési zárszó:** a Mini tudatosan kicsi. A cél nem az, hogy minden lehetséges absztrakció beépített legyen, hanem hogy amit a fordító támogat, azt szigorúan ellenőrizze, és a maradék szolgáltatások könyvtárakban éljenek.
