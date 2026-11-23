# Mini – fejlesztési terv (4.8 → 6.0), 2. változat

> **Mottó:** 1. a biztonságos Mini nyelv, 2. az egyszerű, könnyen tanulható Mini nyelv, 3. az egyszerű, de jó fordító, amely új architektúrákra könnyen bővíthető.
> Mindehhez a programozót a lehető legjobban segítő kézikönyv tartozik, amely **a termék része, nem utólagos melléklet**.

**Újdonságok az 1. változathoz képest:**

1. **Könyvtár először:** ami `#include`-olt könyvtárban, `unsafe asm` segítségével megvalósítható, az nem kerül a fordítóba (1. fejezet).
2. **A futási sebesség a fordító dolga:** saját optimalizációs sáv, közös középrészben, a biztonság gyengítése nélkül (6. fejezet).
3. **Tanulságok más nyelvekből és fordítókból** (2. fejezet).
4. **Tanulhatóság** mint önálló szempont (4. fejezet).

A terv a `mini_native_v4_8_checked_index` forráskódjának (`src/main.rs`, `src/ir.rs`, `backend_*.rs`, `tiny8.rs`) és dokumentációjának átnézésén alapul.

---

## 0. Döntési sorrend és kapuőr kérdések

Ha két cél ütközik, **mindig a magasabb prioritású nyer**: biztonság → egyszerűség és tanulhatóság → fordító (ezen belül: helyesség → futási sebesség → fordítási sebesség).

| # | Kérdés | Elvárt válasz |
|---|---|---|
| K1 | Milyen `unsafe` használatot vált ki, vagy milyen hibaosztályt zár ki? | Konkrét példa |
| K2 | **Megvalósítható könyvtárban** (`#include` + `unsafe asm`)? | Ha igen, oda kerül, nem a fordítóba |
| K3 | Hány új fogalmat kell megtanulni hozzá? Elmagyarázható egy kézikönyvoldalon? | Legfeljebb egy új fogalom, egy oldal |
| K4 | Hány backendet kell módosítani? | Ideálisan 0 |
| K5 | Van pozitív teszt, negatív teszt és kézikönyv-fejezet? | Igen, ugyanabban a változtatásban |
| K6 | Mit veszünk el vagy mit nem teszünk be cserébe? | Tudatos döntés, rögzítve |

---

## 1. Munkamegosztás: mi a fordítóé, mi a könyvtáré

A Mini eddigi legjobb döntése, hogy a rendszerhívások, fájlok, pufferek és perifériák nem a fordítóban, hanem `.mini` könyvtárakban élnek, `unsafe asm` blokkokkal. Ezt az elvet általánosítjuk.

**Szabály:** a fordítóba csak az kerül, amit könyvtár **nem tud** megcsinálni:

- nyelvi szintaxis és típusrendszer;
- biztonsági ellenőrzések (típus, tulajdonjog, határ, túlcsordulás, `unsafe` határ) – ezeket könyvtár nem tudja kikényszeríteni;
- optimalizáció, regiszterkiosztás, inlining;
- utasításkódolás és objektumformátum.

**Miért nem lassít a könyvtár-először elv?** Mert a fordító **inline-olja** a kis könyvtári függvényeket, az `asm`-burkolókat is. Az `asm` blokkok már most deklarálják bemeneteiket, kimeneteiket és clobber-listájukat, így az optimalizáló pontosan tudja, mit érint egy blokk. Egy `#include`-olt `wrapping_add` vagy `sys_write` ezért ugyanolyan gyors lehet, mint egy beépített művelet.

**Hook-konvenció:** a meglévő `__drop_T` mintájára a fordító jól definiált hookokat hív, amelyeket a célplatform könyvtára valósít meg.

| Funkció | Hova kerül | Megjegyzés |
|---|---|---|
| Szintaxis, típusok, `for`, `enum`, `match` | Fordító | |
| Határ-, túlcsordulás-, tulajdonjog-, `unsafe`-ellenőrzés | Fordító | Könyvtár nem tudja kikényszeríteni |
| Optimalizáció, inlining, regiszterkiosztás | Fordító | 6. fejezet |
| Utasításkódolás, ELF/firmware/PRG kiírás | Fordító | |
| **Trap kezelése** (üzenet, LED, kilépés) | Könyvtár: `__mini_trap(ok, sor)` hook | Fordítói tartalék: illegális utasítás. A hook nem térhet vissza. |
| Rendszerhívások, fájl, puffer, allokátor, `Vec` | Könyvtár | Mint eddig |
| Körbeforduló aritmetika, bitforgatás, bájtsorrend-csere | Könyvtár (`asm`, inline-olva) | Nem kell hozzá új nyelvi elem |
| Kiírás, formázás, szövegkezelés | Könyvtár | |
| Perifériák, MMIO, megszakítás-tiltás | Könyvtár | |
| Képesség-tokenek (capability) | Könyvtár, privát mezőkkel | Egyetlen fordítói szabály kell hozzá (2. fejezet, Austral) |
| `test` blokk, `assert` | Fordító (minimális) | A hely (fájl:sor) csak a fordító ismeri |

---

## 2. Tanulságok más nyelvekből és fordítókból

### 2.1 Nyelvek

| Nyelv | Mit tanulunk belőle | Mini döntés |
|---|---|---|
| **C** | Mit kerüljünk: definiálatlan viselkedés, implicit konverziók, null, tömb→mutató romlás, makrók | Megerősíti: a biztonságos Mini-ben nincs definiálatlan viselkedés |
| **Pascal, Modula-2, Oberon-07** (Wirth) | Az Oberon-07 teljes nyelvi leírása 17 oldal, ebből kb. 1,5 oldal a nyelvtan; számlálós `FOR`; tömbhatár-ellenőrzés; kicsi, egymenetes fordítók; a nyelv generációról generációra *egyszerűbb* lett | **Átvesszük:** oldalkeret a nyelvi szabványra (≤ 20 oldal), `for` ciklus, „egy funkció elhagyása is fejlesztés” |
| **Ada / SPARK** | Tartomány-altípusok (`range 0..100`), szerződések, futásidejű hibák hiányának bizonyítása | **Később:** tartománytípusok, ha a `for` ciklus nem elég. Teljes bizonyítás: nem (túl bonyolult), könnyű tartományelemzés: igen |
| **Go** | Kis specifikáció; egyetlen formázási stílus (`gofmt`); egyetlen eszköz (`go build/test/fmt/vet`); gyors fordítás | **Átvesszük:** `mini` egyetlen eszköz (`build`, `run`, `test`, `fmt`, `doc`, `explain`); a formázó a fordító saját parserét használja |
| **Zig** | Nincs rejtett vezérlés és allokáció; biztonsági módban trap a definiálatlan viselkedés helyett; `test` blokkok; `defer`; van ellenőrzéseket kikapcsoló kiadási mód | **Átvesszük:** nincs rejtett vezérlés (a destruktorhívás az egyetlen, dokumentált kivétel), `test` blokk. **Nem vesszük át:** ellenőrzéseket kikapcsoló mód; `defer` (a destruktor lefedi) |
| **Rust** | `unsafe` határ, tulajdonjog, kimerítő `match`, `--explain`, jó hibaüzenetek | Részben megvan. Borrow checker és élettartam-annotáció: nem |
| **Swift** | Túlcsordulás alapból trap; kizárólagossági szabály a mutáló hozzáférésekre | Megerősíti a mostani irányt és a `mut` aliasing-szabályt |
| **Hylo** | Mutable value semantics: a referenciák másodrendűek, csak függvényhatáron jönnek létre, nem tárolhatók; nincs élettartam-annotáció | **Átvesszük:** ez a `T` / `mut T` / `own T` paraméterkonvenció elméleti alapja |
| **Austral** | „Fejben elfér” egyszerűség (a teljes nyelv megérthető a specifikációból); lineáris típusok; képesség-alapú biztonság, ahol a gyökér-képességet a program belépési pontja kapja; hibánál azonnali leállás | **Átvesszük:** egyszerűségi mérce, fail-stop. **Kísérleti:** képesség-tokenek könyvtárban. Lineáris típusok helyett marad az automatikus destruktor, mert tanulónak egyszerűbb |
| **Hare, C3, Odin** | Kicsi, modern, C-szerű nyelvek; a Hare a kicsi QBE backendet használja és hosszú távú stabilitást ígér; C3 szerződések | **Átvesszük:** stabilitási ígéret (editions). Szerződések (`requires`): talán később, futásidejű ellenőrzésként |
| **Elm, Gleam** | Barátságos, tanító hibaüzenetek; egy dologra egy megoldás; nincs null | **Átvesszük:** hibaüzenet-stílus (mi a baj, miért, hogyan javítsd, példával) |
| **Lua** | Kicsi nyelv, rövid kézikönyv, könnyen beágyazható | A kézikönyv terjedelme is mérce |
| **Python** | Olvashatóság; `doctest` | A kézikönyv példái tesztként futnak |
| **BASIC, Logo** | Azonnali visszajelzés a tanulónak | Böngészős játszótér (a fordító WebAssemblyre fordítva + LIR-interpreter) |
| **WebAssembly** | Strukturált vezérlés, gyorsan validálható kód | LIR-ellenőrző (verifier); `wasm32` mint opcionális cél |

### 2.2 Fordítók és technikák

| Projekt / technika | Mit tanulunk belőle | Mini döntés |
|---|---|---|
| **QBE** | Célja az ipari fordítók teljesítményének 70%-a a kód 10%-ából; SSA-alapú IR; az 1.3 (2026) GVN-t, globális kódmozgatást és ciklusoptimalizációkat hozott, az utasításkiválasztáshoz pedig deklaratív mintákból kódot generál; az inliningot a folyamatos (függvényenkénti) feldolgozás miatt elhalasztották | **Átvesszük:** SSA-alapú LIR; deklaratív mintatáblák; a „70% / 10%” mint mérce. **Eltérés:** a Mini inline-ol, mert a könyvtár-először elv ezt megköveteli. A QBE nem lesz függőség (saját kódolók maradnak, külső eszközlánc nélkül) |
| **Go fordító** | Architektúránkénti átíró szabályfájlok, amelyekből kód generálódik | Egy backend = szabálytábla + utasításkódoló + ABI-leírás |
| **Wirth fordítói** | Kicsi, egymenetes, gyors fordítók | A fordítási sebesség is mért cél |
| **Braun és tsai. (2013)** | SSA-építés közvetlenül az AST-ből, dominancia-számítás nélkül | A LIR ezzel épül közvetlenül a HIR-ből |
| **Lineáris pásztázó regiszterkiosztás** (Poletto–Sarkar) | Egyszerű, gyors, jó minőségű | Első regiszterallokátor; a regiszterek célonként adatként leírva |
| **Cranelift** | IR-ellenőrző, kiterjedt fuzzing; e-gráfok | Ellenőrző és fuzzing: átvesszük. E-gráf: nem (túl bonyolult) |
| **Csmith, YARPGen** | Véletlen programokkal végzett differenciális tesztelés | Mini-programgenerátor: `-O0` vs. `-O` vs. interpreter |

---

## 3. Tervezési alapelvek

| # | Alapelv | Gyakorlati jelentés |
|---|---|---|
| P0 | **Könyvtár először** | Ami könyvtárban megvalósítható, oda kerül; a fordító inliningja teszi ezt költségmentessé |
| P1 | **Fail-closed** | Amit a fordító nem tud bizonyítottan biztonságosan lefordítani, azt érthető hibával elutasítja |
| P2 | **A biztonság egy helyen él** | Minden ellenőrzés a frontendben és a közös loweringban készül; a backend csak végrehajt |
| P3 | **Egyetlen hibamodell** | Minden futásidejű hiba egy `Trap(ok, hely)` útvonalon megy, a normál kilépéstől megkülönböztethetően |
| P4 | **Nincs implicit viselkedés** | Nincs implicit konverzió, null, kivétel, rejtett másolás, rejtett allokáció |
| P5 | **A biztonságos Mini-ben nincs definiálatlan viselkedés** | Nyelvi szabály; kivétel csak `unsafe` alatt |
| P6 | **Az `unsafe` szűk, látható, auditálható** | Az alkalmazáskódban tiltható; a fordító listázza |
| P7 | **Ami le van írva, az ellenőrzött** | Kód, teszt és kézikönyv együtt frissül; a táblázatokat a fordító generálja |
| P8 | **A sebesség a fordító dolga, de soha nem a biztonság rovására** | Nincs ellenőrzéseket kikapcsoló mód; az optimalizáló csak bizonyítottan felesleges ellenőrzést hagyhat el |
| P9 | **Tanulhatóság** | Egy dologra egy mód; kevés kulcsszó; fokozatos felfedés; azonnali visszajelzés |

---

## 4. Tanulhatóság

### 4.1 Nyelvi rétegek

A kézikönyv és a tutorial négy rétegben vezeti a tanulót. Minden réteg önmagában is hasznos programokat enged írni.

| Réteg | Tartalom | Kinek |
|---|---|---|
| 1. Alapok | egész típusok, `let`, `if`, `while`, `for`, függvények | mindenki, első nap |
| 2. Adatok | `struct`, tömbök, szelet-paraméterek, `enum` + `match` | első hét |
| 3. Erőforrások | tulajdonjog, `own`/`mut`, destruktorok | aki fájllal, pufferrel dolgozik |
| 4. Könyvtárírás | `unsafe`, `asm`, hookok, eszközprofilok | könyvtár- és platformírók |

A 4. réteg az alkalmazáskódban a `--forbid-unsafe` kapcsolóval kizárható, így egy tanuló vagy egy alkalmazásfejlesztő soha nem találkozik vele véletlenül.

### 4.2 Szabályok

- **Kulcsszókeret:** legfeljebb 30 kulcsszó; a nyelvi szabvány legfeljebb 20 oldal (az Oberon-07 a mérce).
- **Egy forma:** a `mini fmt` kötelező stílust ad, nincs stílusvita.
- **Azonnali visszajelzés:** `mini run prog.mini` egy lépésben fordít és futtat; böngészős játszótér a kézikönyv minden példájához.
- **Tanító hibaüzenetek** (Elm és Rust mintájára):

```
hiba[E0305]: az `f` fájlt már átadtad
  --> main.mini:9:10
   |
 7 |     close(f)
   |           - itt adtad át a tulajdonjogot (`own` paraméter)
 9 |     read(f, mut buf)
   |          ^ itt már nem használhatod
   |
   = miért: lezárt fájlból olvasni hiba; a Mini ezt fordításkor kiszűri
   = javítás: olvass a `close` előtt
   = részletek: mini --explain E0305
```

---

## 5. Kiindulási helyzet (4.8)

### 5.1 Erősségek

| Erősség | Hol látszik a kódban |
|---|---|
| Fail-closed kultúra | `compile_for`: nem támogatott célon elutasítás; `tiny8.rs` „fail closed before machine code is emitted” |
| Explicit, ellenőrzött `unsafe` határ | `validate_unsafe_expr` |
| Túlcsordulás, nullával osztás, konverzió ellenőrzése | `trap_unless(...)`, `check_integer_range` minden fő backendben |
| Konzervatív tulajdonjog-ellenőrzés | `dead` halmaz: `double drop`, `use after drop`, ágak egyezése |
| Könyvtár-először: OS-szolgáltatások `.mini`-ben | `lib/`, `BOUNDARY.md` |
| Deklarált `asm` bemenetek, kimenetek, clobberek | `check_asm*` – ez teszi lehetővé a későbbi inliningot |
| Saját kódolók, külső eszközlánc nélkül | `encode_*.rs`, `code_buffer.rs` |

### 5.2 Gyengeségek

| # | Probléma | Hol |
|---|---|---|
| G1 | **A fordító és a tesztek soha nem futottak** | `VERIFICATION.md`, minden `RELEASE_*.md` |
| G2 | A trap Linuxon `exit(101)`, megkülönböztethetetlen egy `return 101`-től | `fn trap() { linux_exit(101) }` |
| G3 | Nem egységes hibamodell: `exit(101)`, végtelen ciklus (tömbindex), `BKPT` + ciklus (Thumb) | backendek, `main.rs` |
| G4 | Hibaüzenetek pozíció nélkül; a tokenek nem tárolnak sort/oszlopot | `enum T` |
| G5 | 1381 soros, sűrű `main.rs` minden fordítási fázissal | `src/main.rs` |
| G6 | A backendek AST-t, frame-eket és függvénytáblát is kapnak | `Gen::new(&ast,&frames,&fs,&hir)` |
| G7 | A biztonsági logika öt backendben duplikált | `backend_*`, `tiny8` |
| G8 | Tömbök skalár slotokra bontva, O(N) dinamikus index, ≤ 16 elem | `array_slot`, `dispatch` |
| G9 | Minden slot 8 bájt | `ty_size` |
| G10 | **Nincs optimalizáció**: minden érték veremslotban, nincs regiszterkiosztás, nincs inlining | backendek |
| G11 | Csak függvényszintű tulajdonjog; nincs mozgatás, blokkszint, privát mező | `OWNERSHIP.md` |
| G12 | Nincs összeg típus, nincs bitművelet, nincs `for` ciklus | `Ty`, lexer |
| G13 | Szétszórt, ellentmondó verziójú dokumentáció | gyökér, `docs/` |

---

## 6. Optimalizáció – a fordító feladata

### 6.1 Elvek

1. **Közös középrész:** minden optimalizáció a cél-független LIR-en fut. Egy új backend így „ingyen” kapja a sebességet.
2. **Trap-egyenértékűség:** az optimalizált program pontosan akkor és ugyanazzal az okkal áll le, mint az optimalizálatlan, és előtte ugyanazokat a megfigyelhető hatásokat produkálja.
3. **Ellenőrzést csak bizonyítással lehet elhagyni:** például ha a `for i in 0..len(buf)` ciklusban az index bizonyítottan a tartományban van.
4. **Az `asm` átlátszatlan, de deklarált:** az optimalizáló a deklarált bemenetek, kimenetek és clobberek alapján dolgozik; a `clobber memory` gát. Az MMIO-hozzáférés így természetesen „volatile”.
5. **Mérés alapján:** egy optimalizáció akkor kerül be, ha a benchmark-készleten mérhetően javít, és átmegy a differenciális teszteken. A középrész kódmérete keretet kap (kb. 5000 sor), a QBE „70% a kód 10%-ából” szemlélete szerint.

### 6.2 Optimalizációs szintek

| Szint | Cél | Biztonság |
|---|---|---|
| `-O0` | Gyors fordítás, könnyű hibakeresés | Teljes |
| `-O` (alapértelmezett) | Futási sebesség | Teljes |
| `-Os` | Kódméret (AVR, Cortex-M flash) | Teljes |

Nincs olyan szint, amely ellenőrzést kapcsolna ki (ellentétben a Zig `ReleaseFast` módjával).

### 6.3 Optimalizációk sorrendje

| Lépcső | Optimalizációk | Miért ebben a sorrendben |
|---|---|---|
| **O1** | Konstans-hajtogatás és -propagáció, halott kód eltávolítása, másolás-propagáció, lineáris pásztázó regiszterkiosztás, peephole | A legnagyobb nyereség a legkisebb kóddal; a mostani „minden a veremben” modell a fő lassító |
| **O2** | Inlining (kis függvények és `asm`-burkolók), felesleges határ- és túlcsordulás-ellenőrzés elhagyása (`for` ciklus, tartományelemzés), ellenőrzés kiemelése a ciklus elé, ismételt ellenőrzések összevonása | A könyvtár-először elv és a biztonsági ellenőrzések ára itt térül meg |
| **O3** | Globális értékszámozás (GVN), ciklusinvariáns kód kiemelése, `if`-elimináció, farokhívás | A QBE 1.3 tapasztalata szerint ezek adják a következő nagy lépést |
| **Os** | Közös epilógusok, kisebb utasításformák választása, inlining-küszöb csökkentése | MCU-k flash-mérete |

### 6.4 A backend szerepe

- **Utasításkiválasztás mintatáblával:** minden cél ugyanazzal a deklaratív mintanyelvvel írja le, mely LIR-mintákból milyen gépi utasítás lesz (a QBE és a Go fordító mintájára).
- **Regiszterleírás adatként:** regiszterlista, hívó/hívott által mentett regiszterek, argumentumregiszterek.
- A backend így három részből áll: **mintatábla + utasításkódoló + ABI-leírás**. Az optimalizáció és a biztonság a közös részben van.

### 6.5 Mérés és helyesség

- **Benchmark-készlet:** szita, CRC32 (táblás és bitenkénti), rendezés, mátrixszorzás, puffer-másolás, AVR időzítő-hurok; később CoreMark. Az eredményeket a CI követi.
- **Célok:** x86-64-en a `gcc -O2` teljesítményének 50%-a az O2 végére, 70%-a az O3 végére; MCU-n a kódméret követése.
- **Helyesség:** differenciális tesztelés (`-O0` / `-O` / `-Os` / interpreter), véletlen Mini-programgenerátor, LIR-ellenőrző minden optimalizációs lépés után a teszt-buildekben.

---

## 7. Fázisok

A sorrend magyarázata: a 4.9 az ellenőrizhetőséget, az 5.0 a közös IR-t teszi rendbe, mielőtt új biztonsági és optimalizációs funkciók jönnének. Ha ezeket a mostani szerkezetben vezetnénk be, öt backendben kellene ötször megírni (G7), ami maga is biztonsági kockázat.

### Fázis 0 – Alapozás (4.9)

| Feladat | Részletek |
|---|---|
| 0.1 CI | `cargo build`, `cargo test`, `clippy -D warnings`; kiadás csak zöld CI-ből |
| 0.2 Futtatás emulátoron | x86-64 natívan; AArch64, ARMv7 `qemu-user`; Cortex-M `qemu-system-arm`; AVR `simavr`; C64 `tests/emulate8.py` vagy VICE |
| 0.3 Egységes tesztformátum | `.mini` fájlok fejléccel: `// expect: exit 42`, `// expect-error: E0301`, `// expect-trap: overflow`, `// targets: ...`; egy Rust futtató a 28 shell-szkript helyett |
| 0.4 `Span` és hibakódok | `fájl:sor:oszlop` minden hibánál, `#include`-on át is; stabil `E0xxx` kódok |
| 0.5 Egységes trap | `Trap(ok, hely)` → `__mini_trap` hook, ha a célkönyvtár megadja; különben illegális utasítás. A tömbindex végtelen ciklusa is ide kerül |
| 0.6 Dokumentáció rendbetétele | régi kiadási fájlok → `docs/history/`; `CHANGELOG.md`; egy verziószám |

### Fázis 1 – Biztonsági mag és nyelv (5.1–5.4)

| # | Elem | Lényeg | Hova |
|---|---|---|---|
| 1.1 | **Valódi tömbök** | Összefüggő tárolás, típus szerinti elemméret, O(1) ellenőrzött index, nincs 16 elemes korlát | Fordító (közös lowering) |
| 1.2 | **Kölcsönzött nézetek** (Hylo) | Paraméterek: `x: T` olvasás, `x: mut T` írás, `x: own T` tulajdon-átadás; szeletek `[u8]`, `mut [u8]`; referenciák nem tárolhatók és nem adhatók vissza → nincs élettartam-annotáció; egy `mut` argumentum nem lehet ugyanaz, mint egy másik | Fordító |
| 1.3 | **`for` ciklus** (Wirth) | `for i in 0..n { }`; az index ciklusonként csak olvasható. Könnyebb tanulni, nincs „egyel elcsúszás”, és az index-ellenőrzés bizonyíthatóan elhagyható | Fordító |
| 1.4 | **Bitműveletek, bájtsztring** | `& \| ^ << >> %` definiált szemantikával; `b"szöveg"` csak olvasható statikus tömb | Fordító. A körbeforduló aritmetika és a bitforgatás **könyvtár** |
| 1.5 | **Teljesebb tulajdonjog** | Mozgatás (`return x`, `own`), blokkszintű élettartam, tulajdonos mezők | Fordító |
| 1.6 | **Privát mezők, fájlmodulok** | Fájl = modul; mezők alapból privátak, `pub` kulcsszóval nyilvánosak | Fordító |
| 1.7 | **`enum` + kimerítő `match`** | Generikusok nélkül; biztonságos `File` és `Buffer` API | Fordító + könyvtár |
| 1.8 | **`unsafe` audit** | `--unsafe-report`, `--forbid-unsafe`, `--trust lib/`, kötelező `// SAFETY:` megjegyzés | Fordító |
| 1.9 | **`test` blokk, `assert`** (Zig, Go) | `test "név" { }`, `mini test`; `assert` hibája `Trap(Assert, hely)` | Fordító (minimális) |
| 1.10 | **Veremkorlát MCU-n** | Fordítási idejű veremhasználat-számítás, `--stack-report`, túllépés = fordítási hiba | Fordító |
| 1.11 | **Képesség-tokenek** (Austral, kísérleti) | A `program` egy gyökér-tokent kap; a fájl- és hálózati könyvtárak ebből származtatott tokent kérnek; így a függvény szignatúrája mutatja, mihez fér hozzá | Könyvtár + egy fordítói szabály |

Példa az 5.3 végére elérendő állapotra, `unsafe` nélkül:

```mini
#include "lib/io/file.mini"

program {
    let buf: [u8; 64] = [0; 64]
    match open_read(b"adat.txt") {
        Ok(f) => {
            let n: usize = read(f, mut buf)
            let sum: u64 = 0
            for i in 0..n {
                sum = sum + buf[i] as u64   // határellenőrzés bizonyítottan elhagyható
            }
            return sum as i64
        }
        Err(code) => return 1
    }
}
```

### Fázis 2 – Egyszerű nyelv (folyamatos)

| Feladat | Részletek |
|---|---|
| 2.1 Nyelvi szabvány | EBNF + szemantika, ≤ 20 oldal; az egyetlen hiteles forrás |
| 2.2 Nem-célok | Nincs: implicit konverzió, null, kivétel, szemétgyűjtés, makró (csak `#include`, `#if`), operátor-túlterhelés, öröklés, lezárás, generikus típus (amíg nem indokolt), rejtett allokáció, ellenőrzéseket kikapcsoló mód |
| 2.3 Méretkeret | ≤ 30 kulcsszó; minden új kulcsszó a K1–K6 kérdéseken megy át |
| 2.4 Változáskezelés | Editions: nem kompatibilis változás csak új szinttel, migrációs útmutatóval |

### Fázis 3 – Egyszerű, bővíthető fordító (5.0, majd folyamatosan)

| Feladat | Részletek |
|---|---|
| 3.1 Modulokra bontás | `driver`, `diagnostics`, `lexer`, `parser`, `ast`, `check/` (types, unsafe, ownership, stack), `hir`, `lir`, `lower`, `opt/`, `interp`, `backend/`, `encode/`, `object/`; `rustfmt` |
| 3.2 SSA-alapú LIR | Braun-algoritmussal közvetlenül a HIR-ből; explicit `Trap`, destruktorhívások, határellenőrzések. Már az elejétől SSA, hogy az optimalizációhoz ne kelljen új IR |
| 3.3 Naiv lowering először | Az 5.0-ban minden SSA-érték kap egy veremslotot: helyes és egyszerű. A sebesség az O-lépcsőkben jön, a backend módosítása nélkül |
| 3.4 Backend = mintatábla + kódoló + ABI-leírás | A célspecifikus `asm`-ellenőrzés is a backendbe kerül (`AsmDialect`), a `main.rs`-ből ki |
| 3.5 LIR-ellenőrző és interpreter | Az ellenőrző minden lépés után fut a teszt-buildekben; az interpreter a végrehajtható specifikáció (`mini run --interp`). `asm`-et tartalmazó könyvtári függvényeknek `#if target interp` alatti alternatív megvalósítása lehet |
| 3.6 Fuzzing | Lexer, parser, ellenőrző: ne legyen pánik; aritmetikai differenciális fuzzing |
| 3.7 Kódolók ellenőrzése | Automatikus összevetés `objdump` / `llvm-mc` kimenetével a CI-ben |
| 3.8 RISC-V backend (5.5) | Csak mintatábla, kódoló és ABI-leírás; 0 módosított fájl a `backend/`-en és a céltáblán kívül |

### Fázis 4 – Kézikönyv (minden kiadással együtt)

| Rész | Tartalom |
|---|---|
| 1. Első lépések és tutorial | Telepítés; első program; futtatás QEMU-n, MCU-n és a játszótéren; a 25 tutorial fejezetekként, a 4.1 rétegei szerint |
| 2. Útmutatók | Fájl olvasása; LED villogtatása AVR-en; MMIO-regiszter írása; saját biztonságos könyvtár írása; új eszközprofil; **új architektúra hozzáadása** |
| 3. Referencia | Nyelvi szabvány; standard könyvtár; parancssor; generált célplatform-mátrix |
| 4. Magyarázatok | Biztonsági modell; tulajdonjog és kölcsönzés; trap és fail-stop; az `unsafe` szerződés; **„Hogyan írj gyors Mini kódot?”** (mit optimalizál a fordító, mit nem) |
| 5. Hibakatalógus | Minden `E0xxx`: mi, miért, rossz és jó példa; `mini --explain` |

Programozót segítő elemek: egyoldalas puskalap; „C-ből és Rustból érkezőknek” fejezet; magyar–angol szójegyzék; minden fejezet elején „ehhez ezt kell tudnod”, a végén feladat; minden fejezetben „mi nem működik még” doboz. A kódblokkok CI-ben futnak; a magyar a mesterpéldány, az angol ugyanabban a változtatásban frissül.

---

## 8. Ütemterv

| Verzió | Nyelv és biztonság | Optimalizáció | Kilépési feltétel |
|---|---|---|---|
| **4.9** | CI, emulátorok, tesztformátum, `Span`, hibakódok, trap-hook, doksi rendbetétele | – | Minden teszt zöld 3 célon; trap ≠ normál kilépés |
| **5.0** | Modulok, SSA-LIR naiv loweringgal, ellenőrző, interpreter, backend-interfész. Nincs nyelvi változás | Alapmérés (benchmark-készlet felállítása) | Azonos viselkedés; a backendek csak LIR-t látnak |
| **5.1** | Tömbök, nézetek, `for`, bitműveletek, bájtsztring | **O1** | „Hello, world” `unsafe` nélkül; mérhető gyorsulás |
| **5.2** | Tulajdonjog, privát mezők, modulok | **O2a**: inlining, `asm`-burkolók is | A könyvtári `sys_write` hívás inline-olódik |
| **5.3** | `enum` + `match`, biztonságos könyvtárak, `unsafe` audit, `test` blokk | **O2b**: ellenőrzések elhagyása | 0 `unsafe` a példaprogramokban; `for`-ciklusban nincs index-ellenőrzés |
| **5.4** | Veremkorlát, fixed-width minden célon, struct AVR-en | **Os** | Túl nagy veremigény = fordítási hiba |
| **5.5** | RISC-V backend | – (ingyen kapja) | 0 módosított fájl a backenden kívül |
| **5.6** | Képesség-tokenek (kísérleti) | **O3**: GVN, LICM | x86-64-en ≥ 70% a `gcc -O2`-höz képest a benchmarkon |
| **6.0** | Stabil edition, ≤ 20 oldalas szabvány, teljes kézikönyv, játszótér | Stabil szintek | Minden kézikönyvpélda CI-ben fut |

---

## 9. Kész-definíció minden változtatásra

- [ ] Pozitív és negatív teszt, `expect` annotációval, minden érintett célon futtatva
- [ ] Interpreter és natív eredmény egyezik, minden optimalizációs szinten
- [ ] Hibaüzenet pozícióval, hibakóddal és katalógusoldallal
- [ ] Kézikönyv-frissítés magyarul és angolul, futó példával
- [ ] Nincs benchmark-regresszió (vagy indokolt és rögzített)
- [ ] `CHANGELOG.md` bejegyzés az ismert korlátokkal
- [ ] A K1–K6 válaszok rögzítve; a K2-re adott válasz külön indokolva, ha a funkció a fordítóba kerül

---

## 10. Mérőszámok

| Mérőszám | Cél |
|---|---|
| `unsafe` a példaprogramokban (a `lib/`-en kívül) | 0 (5.3-tól) |
| Ténylegesen futtatott tesztek aránya backendenként | 100% |
| Pozícióval ellátott hibaüzenetek | 100% (4.9-től) |
| Kulcsszavak száma / nyelvi szabvány terjedelme | ≤ 30 / ≤ 20 oldal |
| Biztonsági ellenőrzést tartalmazó backend-kódsorok | 0 (5.0-tól) |
| Új backendhez módosított fájlok a `backend/`-en kívül | 0 |
| Futási sebesség a `gcc -O2`-höz képest (x86-64 benchmark) | ≥ 50% (5.3), ≥ 70% (5.6) |
| Középrész (optimalizáló) mérete | ≤ kb. 5000 sor |
| Fordítási idő a teljes tesztkészletre | követve, nem nőhet indokolatlanul |
| Pánik a fordítóban fuzzing alatt | 0 |

---

## 11. Kockázatok

| Kockázat | Kezelés |
|---|---|
| A nyelv „Rust-másolattá” hízik | Kapuőr kérdések, nem-célok, kulcsszó- és oldalkeret |
| Optimalizálási hiba eltüntet egy ellenőrzést | Trap-egyenértékűségi szabály, LIR-ellenőrző, differenciális és véletlen programos tesztelés |
| Az optimalizáló bonyolultsága elszabadul | Kódméret-keret; csak mérhető nyereségű lépések; e-gráf és hasonlók kizárva |
| Ismét futtatás nélküli kiadások | Kiadás csak zöld CI-ből |
| Az 5.0-s átépítés regressziót okoz | Backendenként, lépésenként; a régi út kapcsoló mögött, amíg a tesztek egyeznek |
| A könyvtár-először elv lassú kódot ad | Inlining (O2a) az elv előfeltétele; benchmark mutatja |
| Kétnyelvű kézikönyv elcsúszik | Magyar mesterpéldány, CI-ellenőrzés a fejezetszerkezetre |

---

## 12. Első konkrét lépések (2–4 hét)

1. CI felállítása; a meglévő tesztek első valódi futtatása és a hibák javítása.
2. QEMU-user beállítása AArch64-re és ARMv7-re; a generált ELF-ek futtatása.
3. Egységes `.mini` tesztformátum és Rust tesztfuttató.
4. `Span` a lexerben és a parserben; hibakódok kiosztása; az első 10 katalógusoldal.
5. Egységes trap a `__mini_trap` hookkal; Linux-könyvtári megvalósítás stderr-üzenettel.
6. Benchmark-készlet összeállítása és alapmérés a mostani fordítóval.
7. Dokumentáció rendbetétele: `docs/history/`, `CHANGELOG.md`, egy verziószám.
8. Tervezési dokumentumok: `docs/design/LIR.md` (SSA-LIR, ellenőrző), `docs/design/BACKEND.md` (mintatábla, ABI-leírás), `docs/design/OPTIMIZER.md` (trap-egyenértékűség, lépcsők).

---

## 13. Források

- Hylo: https://github.com/hylo-lang/hylo-lang.github.io ; Racordon és tsai.: *Native Implementation of Mutable Value Semantics*, https://arxiv.org/abs/2106.12678
- Austral specifikáció: https://austral.github.io/spec/spec.html ; képesség-alapú biztonság: https://austral.github.io/tutorial/capability-based-security
- Oberon-07 nyelvi leírás (17 oldal): https://hokstad.com/oberon
- Zig áttekintés (build módok, `defer`, nincs rejtett vezérlés): https://ziglang.org/learn/overview/
- QBE: https://c9x.me/compile/ ; QBE 1.3 (LWN, 2026. július): https://lwn.net/Articles/1080519
- Braun és tsai.: *Simple and Efficient Construction of SSA Form* (CC 2013): https://compilers.cs.uni-saarland.de/projects/ssaconstr/
