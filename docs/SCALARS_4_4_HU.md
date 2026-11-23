# Mini 4.4 – Egész típusok ARM Linuxon

A Mini az `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `usize` és `bool` típusokat kezeli. Az `as` **ellenőrzött** egész konverzió: ha az érték nem fér el a céltípusban, a futó program trappel leáll (Linuxon 4.9 óta SIGILL, a shellben 132). A számolás sem szűkíthet csendben.

```mini
fn increment(x: u8) -> u8 {
    return x + 1
}
program {
    let n: u8 = 40
    n = increment(n)
    return (n + 1) as i64
}
```

## Támogatottság

| Cél | Egész típusok | `usize` | Állapot |
| --- | --- | --- | --- |
| x86-64 Linux | 8/16/32/64 bites egész típusok | 64 bit | Korábbi implementáció |
| AArch64 Linux | 8/16/32/64 bites egész típusok | 64 bit | Új, tesztelendő backend |
| ARMv7-A Linux | 8/16/32 bites egész típusok, `i64`; `u64` tárolás/konverzió korlátozott | 32 bit | Új, tesztelendő backend |
| Cortex-M, AVR8, C64 | Régebbi részhalmazok | célfüggő | Új egész típusok még nincsenek |

ARM32-n `u64` aritmetika/összehasonlítás jelenleg **fordítási hiba**, nem 32 bitesre csonkolt eredmény. A 64 bites unsigned literálok teljes tartományának beolvasása hiányzik. Az `f32` és `f64` még nem működik; külön FPU/soft-float kódgenerálás és konverziók szükségesek.

A `tests/arm_integers44.sh` a cross-fordítást ellenőrzi, és csak akkor futtatja a binárisokat, ha natív ARM Linux vagy megfelelő QEMU érhető el. A csomag itt nem esett át Cargo-fordításon vagy natív futtatáson.
