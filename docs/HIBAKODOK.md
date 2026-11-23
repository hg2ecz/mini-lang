# Mini hibakódok / Error codes

> Ezt a fájlt a fordító generálja: `mini --print-errors > docs/HIBAKODOK.md`.
> Egy kód magyarázata parancssorból: `mini --explain E0301`.

| Kód | Jelentés | Meaning |
|---|---|---|
| [E0000](#e0000) | Egyéb hiba | Other error |
| [E0001](#e0001) | Érvénytelen karakter vagy zárójelezés | Invalid character or bracket |
| [E0002](#e0002) | Szintaktikai hiba | Syntax error |
| [E0003](#e0003) | Előfeldolgozási hiba | Preprocessor error |
| [E0101](#e0101) | Típushiba | Type error |
| [E0102](#e0102) | Ismeretlen vagy ismételt név | Unknown or duplicate name |
| [E0103](#e0103) | Literál a típus tartományán kívül | Literal out of range |
| [E0104](#e0104) | Érvénytelen konverzió | Invalid conversion |
| [E0201](#e0201) | `unsafe` szükséges | `unsafe` required |
| [E0202](#e0202) | Érvénytelen `asm` blokk | Invalid `asm` block |
| [E0301](#e0301) | Használat felszabadítás után | Use after drop |
| [E0302](#e0302) | Kétszeres felszabadítás | Double drop |
| [E0303](#e0303) | Ágfüggő tulajdonjog | Path-dependent ownership |
| [E0304](#e0304) | Tulajdonjog- vagy destruktorszabály | Ownership or destructor rule |
| [E0305](#e0305) | Ágon vagy cikluson belüli `let` | Branch- or loop-local `let` |
| [E0401](#e0401) | Tömbindex a határon kívül (fordításkor) | Array index out of bounds (compile time) |
| [E0402](#e0402) | Tömbhasználati korlát | Array usage limit |
| [E0501](#e0501) | A célplatform ezt nem támogatja | Not supported on this target |
| [E0502](#e0502) | Hibás eszközprofil | Invalid device profile |
| [E0601](#e0601) | Méret- vagy veremkorlát | Size or stack limit |

## E0000

**Egyéb hiba** / *Other error*

```text
Ehhez az üzenethez még nincs külön hibakód. Ha gyakran találkozol vele, jelezd: kap saját kódot és magyarázatot.
```

## E0001

**Érvénytelen karakter vagy zárójelezés** / *Invalid character or bracket*

```text
A forrás olyan karaktert tartalmaz, amelyet a Mini nem ismer, vagy a zárójelek nem párosak. Az egész literál legfeljebb 9223372036854775807 lehet.

Rossz:  return (1 + 2
Jó:     return (1 + 2)
```

## E0002

**Szintaktikai hiba** / *Syntax error*

```text
A parser mást várt az adott helyen. Gyakori okok: két utasítás egy sorban `;` nélkül, hiányzó `}` vagy foglalt név használata.
A `return` után ugyanabban a blokkban nem állhat több utasítás.

Rossz:  let a: i64 = 1 let b: i64 = 2
Jó:     let a: i64 = 1; let b: i64 = 2
```

## E0003

**Előfeldolgozási hiba** / *Preprocessor error*

```text
Az `#if`, `#elseif`, `#else`, `#endif`, `#include` vagy `#error` sor hibás, vagy egy `#error` ág aktív lett a választott célra.
A feltételek csak `arch`, `os`, `abi`, `cpu`, `device`, `feature` és `pointer_bits` tulajdonságot vizsgálhatnak, idézőjeles értékkel.

Rossz:  #if arch == x86_64
Jó:     #if arch == "x86_64"
```

## E0101

**Típushiba** / *Type error*

```text
A kifejezés típusa nem az, amit a hely megkövetel. A Mini nem végez implicit konverziót: `i64` és `usize` nem keverhető, `bool` nem egész szám, az `if`/`while` feltétele `bool`.

Rossz:  let n: usize = 5
        return n
Jó:     return n as i64
```

## E0102

**Ismeretlen vagy ismételt név** / *Unknown or duplicate name*

```text
A név nincs deklarálva, vagy ugyanabban a hatókörben kétszer szerepel. Függvényt `#include`-dal is behozhatsz; a változót használat előtt `let`-tel kell deklarálni.

Rossz:  return y
Jó:     let y: i64 = 1
        return y
```

## E0103

**Literál a típus tartományán kívül** / *Literal out of range*

```text
A számliterál nem fér el a várt típusban az adott célon (pl. `usize` 32 bites ARMv7-en és Cortex-M-en, 16 bites AVR-en).

Rossz:  let x: usize = 70000     // AVR-en
Jó:     let x: usize = 60000
```

## E0104

**Érvénytelen konverzió** / *Invalid conversion*

```text
Az `as` jelenleg csak egész típusok között működik, és futásidőben ellenőrzött: ha az érték nem fér el, a program trappel.

Rossz:  let b: bool = 1 as bool
Jó:     let b: bool = x != 0
```

## E0201

**`unsafe` szükséges** / *`unsafe` required*

```text
A művelet megkerülheti a Mini biztonsági ellenőrzéseit, ezért csak `unsafe` blokkban szabad: `asm`, `raw_addr`, `unsafe fn` hívása, illetve destruktoros (erőforrás-tulajdonos) struct közvetlen létrehozása.
Az alkalmazáskódban inkább a könyvtár biztonságos függvényét használd.

Rossz:  let fd: OwnedFd = OwnedFd(3)
Jó:     let fd: OwnedFd = unsafe { adopt_fd(3) }   // csak ha valóban te birtoklod a leírót
```

## E0202

**Érvénytelen `asm` blokk** / *Invalid `asm` block*

```text
A Mini csak szűk, célonként ellenőrzött assembly-részhalmazt enged: engedélyezett utasítások, deklarált bemeneti/kimeneti regiszterek és kötelező `clobber` lista. A pontos szabályok a célplatform dokumentációjában vannak (docs/AARCH64.md, docs/ARM32.md, docs/EIGHTBIT.md).

Rossz:  unsafe { asm { syscall } }
Jó:     unsafe { asm { in rax = 39; syscall; out p = rax; clobber rcx; clobber r11; clobber memory } }
```

## E0301

**Használat felszabadítás után** / *Use after drop*

```text
A változót már felszabadítottad (`drop`), utána nem olvasható, nem írható és nem adható át. A felszabadított erőforrás már nem érvényes.

Rossz:  drop f
        return f.fd
Jó:     let fd: i64 = f.fd
        drop f
        return fd
```

## E0302

**Kétszeres felszabadítás** / *Double drop*

```text
Ugyanazt a tulajdonos változót kétszer szabadítanád fel (`drop`). Egy erőforrás (pl. fájlleíró) kétszeri lezárása súlyos hiba, ezért a Mini fordításkor tiltja.

Rossz:  drop f
        drop f
Jó:     drop f
```

## E0303

**Ágfüggő tulajdonjog** / *Path-dependent ownership*

```text
Az `if` két ága után a tulajdonos változók állapotának egyeznie kell (vagy az egyik ágnak `return`-nel kell végződnie), és ciklusban nem változhat tulajdonjog. Így a fordító minden úton pontosan tudja, mit kell felszabadítani.

Rossz:  if c { drop f }
Jó:     if c { drop f } else { drop f }
```

## E0304

**Tulajdonjog- vagy destruktorszabály** / *Ownership or destructor rule*

```text
A destruktor (`__drop_T`) pontos alakja `fn __drop_T(value: T)`, nem lehet `unsafe`, nincs visszatérési értéke, és közvetlenül nem hívható – helyette `drop változó`.
Kölcsönzött (paraméterként kapott) structot nem szabadíthatsz fel és nem írhatsz felül; tulajdonos structot nem másolhatsz másik változóba.

Rossz:  __drop_File(f)
Jó:     drop f
```

## E0305

**Ágon vagy cikluson belüli `let`** / *Branch- or loop-local `let`*

```text
Ebben a verzióban `let` csak a függvény legfelső szintjén állhat, `if`, `while` és `unsafe` blokkon belül nem. Deklaráld a változót a blokk előtt.

Rossz:  if c { let x: i64 = 1 }
Jó:     let x: i64 = 0
        if c { x = 1 }
```

## E0401

**Tömbindex a határon kívül (fordításkor)** / *Array index out of bounds (compile time)*

```text
A konstans index bizonyíthatóan kívül esik a tömbön. Az érvényes indexek: 0 .. hossz-1.

Rossz:  let a: [i64; 4] = [0; 4]
        return a[4]
Jó:     return a[3]
```

## E0402

**Tömbhasználati korlát** / *Array usage limit*

```text
Változó indexszel jelenleg legfeljebb 16 elemű tömb indexelhető, az index típusa `usize`, és `while` feltételében nem lehet tömbelérés. A tömb hossza 1..1024 konstans, a kezdőérték konstans.

Rossz:  while a[i] == 0 { i = i + 1 }
Jó:     let v: usize = a[i]
        while v == 0 { i = i + 1; v = a[i] }
```

## E0501

**A célplatform ezt nem támogatja** / *Not supported on this target*

```text
A program helyes Mini, de a választott cél kódgenerátora ezt a műveletet vagy típust még nem tudja biztonságosan lefordítani, ezért a fordító megtagadja (fail-closed). A támogatási mátrix a kézikönyvben található.

Példa: Cortex-M-en jelenleg csak `usize` és `bool` kifejezések, legfeljebb két paraméter.
```

## E0502

**Hibás eszközprofil** / *Invalid device profile*

```text
A `--device` kapcsolóval megadott `.target` fájl hibás: ismeretlen kulcs, rossz számformátum, üres vagy fordított memóriatartomány, vagy olyan CPU-képesség, amelyet a chip nem tud.
A kulcsok: `device`, `flash`, `ram`, `stack`, `features` (lásd docs/DEVICE_MAP.md).

Rossz:  flash = 0x08010000..0x08000000
Jó:     flash = 0x08000000..0x08010000
```

## E0601

**Méret- vagy veremkorlát** / *Size or stack limit*

```text
A program túllépi a célplatform egy korlátját: veremméret, Flash-méret, paraméterszám, vagy MCU-n tiltott rekurzió.
Mikrovezérlőn a fordító előre kiszámolja a legrosszabb veremigényt, és ha nem fér el, fordítási hibát ad.
```
