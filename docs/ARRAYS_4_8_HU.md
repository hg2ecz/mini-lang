# Mini 4.8 — Biztonságos indexelés (fejlesztési változat)

A Mini 4.8 egyelőre **kis méretű fix tömbökben** enged változóval indexelni.

```mini
program {
    let data: [usize; 4] = [0; 4]
    let i: usize = 0
    while i < 4 {
        data[i] = i + 39
        i = i + 1
    }
    return data[3] as i64
}
```

**Fordítási időben bizonyítható:** a `data[1 + 2]` közvetlenül a 3. elemre fordul.
Ha például `data[2 + 2]` szerepel, a fordító hibát jelez, és nem készít binárist.

**Nem bizonyítható:** a `data[i]` esetén a fordító egyszer kiértékeli az indexet,
majd egy ellenőrzött feltételláncban megvizsgálja, hogy az a `0..N-1` tartományba
esik-e. Ha egyetlen elem sem felel meg, a kód csapdahurokban megáll; nem történik
határon kívüli memóriahozzáférés.

## Jelenlegi korlátok

- A dinamikus indexű tömb legfeljebb **16 elemből** állhat; konstans indexszel a
  4.7-es legfeljebb 1024 elem továbbra is használható.
- Indexként `usize` változó használható; az `i64` nem konvertálódik automatikusan.
- A változóindexelés **O(N) feltétellánc**, nem egyetlen indexelt memóriautasítás.
- A `while data[i] == ...` alak még tiltott, mert a ciklusfeltételt minden
  iteráció előtt biztonságosan kell újra kiértékelni. A ciklus törzsében lévő
  `data[i]` hozzáférés használható.
- A változó értéktartományának ciklusból történő bizonyítása **még nincs kész**.
  Így a ciklusban a fordító egyelőre **nem hagyja el** a dinamikus ellenőrzést.
- A hibás index csapdahurokba vezet, és nem ad vissza hibakódot.
- AVR8/C64 esetén a lineáris ellenőrzés drága lehet; a tényleges fordítást és
  célgépfutást ellenőrizni kell.

A későbbi fejlesztés feladata a tömör, összefüggő tömbtárolás, a hatékony O(1)
indexelés, az intervallumelemzés és az ellenőrzések bizonyított elhagyása.
A dinamikus memóriafoglalás Linuxon továbbra is `#include`-olt könyvtári feladat.
