# Mini editor tooling — minifmt, Vim and mcedit

These developer tools do not add language features or complexity to the Mini compiler.

## minifmt

`tools/minifmt` is a conservative formatter written in Python 3 using only the standard library. It does not require Rust or import the Mini compiler AST. It normalizes indentation, braces, statement line breaks, and simple operator spacing for the current Mini syntax.

```sh
python3 tools/minifmt --stdout examples/hello.mini
python3 tools/minifmt --check examples/hello.mini
python3 tools/minifmt --write examples/hello.mini
python3 tools/minifmt --write examples/tutorial/*.mini
```

`--stdout` prints one file without modifying it. `--check` exits **0** when already formatted, **1** when changes are needed, **2** on errors. `--write` / `-w` changes files in place; back up or commit them first. The default mode only reports files that would change.

Complete preprocessor directive lines (`#include`, `#if`, etc.), string contents and comments are preserved. `unsafe asm` blocks are indented, without transforming instruction content.

**Limitations:** this is not a full parser-based equivalent of `rustfmt`, does not prove semantic equivalence, and may reject future Mini syntax. Unbalanced delimiters cause an error without writing changes. Always compile/test after formatting.

## Vim

From the project root:

```sh
mkdir -p ~/.vim/syntax ~/.vim/ftdetect ~/.vim/ftplugin
cp editor/vim/syntax/mini.vim ~/.vim/syntax/
cp editor/vim/ftdetect/mini.vim ~/.vim/ftdetect/
cp editor/vim/ftplugin/mini.vim ~/.vim/ftplugin/
```

Enable `syntax on` and `filetype plugin on` in `~/.vimrc`. Open a `.mini` file and use `:set filetype?` to confirm the detected type is `mini`.

## Midnight Commander (mcedit)

```sh
mkdir -p ~/.local/share/mc/syntax
cp editor/mcedit/mini.syntax ~/.local/share/mc/syntax/
```

Insert these lines into `~/.config/mc/mcedit/Syntax`, **before the catch-all rule**:

```text
file \.mini$ Mini\sSource
include mini.syntax
```

If your user-level `Syntax` does not exist yet, copy the system-wide configuration first (often `/usr/share/mc/syntax/Syntax`) and edit it there. Paths vary by installation. Restart mcedit.

## Tests

```sh
python3 tests/test_minifmt.py
bash tests/editor_tooling45.sh
```

The tests exercise formatter idempotence and the installed Vim runtime if available. They do not claim to validate mcedit parsing unless Midnight Commander is actually installed.
