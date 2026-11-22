#!/usr/bin/env python3
"""Formatting regressions, including all packaged Mini examples/libraries."""
import importlib.machinery
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
loader = importlib.machinery.SourceFileLoader('minifmt', str(ROOT / 'tools' / 'minifmt'))
spec = importlib.util.spec_from_loader(loader.name, loader)
minifmt = importlib.util.module_from_spec(spec)
loader.exec_module(minifmt)


class MiniFmtTests(unittest.TestCase):
    def test_inline_blocks_and_semicolons(self):
        original = 'fn f(x:u8)->u8{let y:u8=x+1;return y}\nprogram{return f(41) as i64}\n'
        actual = minifmt.format_source(original)
        self.assertIn('    let y: u8 = x + 1\n', actual)
        self.assertIn('    return y\n', actual)
        self.assertEqual(minifmt.format_source(actual), actual)

    def test_preprocessor_and_strings_preserved(self):
        original = '#if arch == "x86_64"\n#include "some dir/a.mini"\n#else\n#error invalid target\n#endif\nprogram{return 0}\n'
        actual = minifmt.format_source(original)
        for row in original.splitlines()[:5]:
            self.assertIn(row, actual)
        self.assertEqual(minifmt.format_source(actual), actual)

    def test_comments_and_asm(self):
        original = 'program { // hello\nunsafe { asm { in rax = 1; syscall; clobber rcx; clobber r11; clobber memory } }\nreturn 0}\n'
        actual = minifmt.format_source(original)
        self.assertIn('    // hello', actual)
        self.assertIn('            syscall\n', actual)
        self.assertEqual(minifmt.format_source(actual), actual)

    def test_does_not_write_invalid_input(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'bad.mini'
            path.write_text('program { return 1\n', encoding='utf-8')
            result = subprocess.run([sys.executable, str(ROOT/'tools/minifmt'), '-w', str(path)], capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(path.read_text(), 'program { return 1\n')

    def test_check_write(self):
        with tempfile.TemporaryDirectory() as d:
            path = Path(d) / 'test.mini'
            path.write_text('program{return 42}\n')
            tool = [sys.executable, str(ROOT/'tools/minifmt')]
            self.assertEqual(subprocess.run(tool + ['--check', str(path)], capture_output=True).returncode, 1)
            self.assertEqual(subprocess.run(tool + ['--write', str(path)], capture_output=True).returncode, 0)
            self.assertEqual(subprocess.run(tool + ['--check', str(path)], capture_output=True).returncode, 0)

    def test_packaged_sources_idempotent(self):
        paths = sorted((ROOT/'examples').rglob('*.mini')) + sorted((ROOT/'lib').rglob('*.mini'))
        self.assertGreater(len(paths), 25)
        for path in paths:
            with self.subTest(path=str(path.relative_to(ROOT))):
                content = path.read_text(encoding='utf-8')
                once = minifmt.format_source(content)
                twice = minifmt.format_source(once)
                self.assertEqual(once, twice)


if __name__ == '__main__':
    unittest.main(verbosity=2)
