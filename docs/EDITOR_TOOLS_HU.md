# Mini szerkesztői eszközök — minifmt, Vim, mcedit

A Mini célja a kis és ellenőrizhető nyelvi mag. A formázó és a szerkesztők szintaxiskiemelése **külső fejlesztői eszköz**, nem új fordítói képesség.

## minifmt

A `tools/minifmt` Python 3 szabványkönyvtárral működő, konzervatív formázó. Nem igényel telepített Rustot, és **nem használja a Mini fordító AST-jét**. A jelenlegi (4.4) szintaxisban a kapcsos zárójelek alapján behúz, elrendezi a sortöréseket és az egyszerű operátorok szóközeit.

```sh
python3 tools/minifmt --stdout examples/hello.mini
python3 tools/minifmt --check examples/hello.mini
python3 tools/minifmt --write examples/hello.mini
python3 tools/minifmt --write examples/tutorial/*.mini
```

- `--stdout`: a formázott fájl a szabványos kimenetre kerül, a fájl változatlan marad.
- `--check`: visszatérési kód **0**, ha már helyesen formázott; **1**, ha változtatna; **2**, ha nem tudja biztonságosan feldolgozni a bemenetet.
- `--write` / `-w`: helyben módosítja a fájlt. Formázás előtt ajánlott verziókezelőbe menteni.
- Alapértelmezésben csak jelzi a szükséges módosításokat.

A `#if`, `#include` és további előfeldolgozó-direktívák teljes sorát érintetlenül hagyja. A sztringek és kommentek tartalmát nem módosítja. Az `unsafe asm` blokkokat ugyanúgy behúzza, mint a közönséges blokkokat; a bennük levő utasítások szövegét nem írja át.

**Korlát:** ez nem teljes parseralapú `rustfmt`-megfelelő. Nem végez szemantikai bizonyítást, és nem képes minden lehetséges, jövőbeni Mini-szintaxist helyesen formázni. Nem támogatott vagy kiegyensúlyozatlan zárójelek esetén hibát ad, fájlt nem módosít. Fordítási és futtatási tesztet is végezzünk a formázás után.

## Vim

A projektgyökérből:

```sh
mkdir -p ~/.vim/syntax ~/.vim/ftdetect ~/.vim/ftplugin
cp editor/vim/syntax/mini.vim ~/.vim/syntax/
cp editor/vim/ftdetect/mini.vim ~/.vim/ftdetect/
cp editor/vim/ftplugin/mini.vim ~/.vim/ftplugin/
```

A `~/.vimrc` tartalmazza:

```vim
syntax on
filetype plugin on
```

Ezután `vim examples/hello.mini`. Ellenőrzés Vimben: `:set filetype?` → `mini`.

## Midnight Commander / mcedit

Másold a szabályfájlt a felhasználói MC szintaxiskönyvtárba:

```sh
mkdir -p ~/.local/share/mc/syntax
cp editor/mcedit/mini.syntax ~/.local/share/mc/syntax/
```

A `~/.config/mc/mcedit/Syntax` fájlba vedd fel az alábbi két sort, **a mindent elkapó általános szabály előtt**:

```text
file \.mini$ Mini\sSource
include mini.syntax
```

Ha nincs saját `Syntax` fájlod, a rendszeren telepített (például `/usr/share/mc/syntax/Syntax`) mintát másold a fenti helyre, és abba szúrd be a két sort. Az MC telepítési helyei disztribúciónként változhatnak. Ezután indítsd újra az mcedit-et.

Az MC fájlformátuma eltér a Vim szintaxisfájljától, ezért két külön szabályrendszer tartozik a projekthez.

## Ellenőrzés

```sh
python3 tests/test_minifmt.py
bash tests/editor_tooling45.sh
```

A Vim szintaxisbetöltést a teszt automatikusan vizsgálja, ha a `vim` telepítve van. Az mcedit szabályfájl valós futását telepített Midnight Commanderrel kell ellenőrizni; pusztán a fájl létezése nem igazolja a teljes szintaxis-megfelelőséget.
