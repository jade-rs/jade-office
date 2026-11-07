"""Physical-line ordering must not hide glyph, page or absolute XY changes."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import pymupdf


class WordCoordinateComparisonTests(unittest.TestCase):
    def compare(self, glyphs, page_count=1):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with pymupdf.open() as document:
                page = document.new_page(width=200, height=100)
                # Emit the right cell first and with an almost identical Y.
                # Sorting exact Y before X used to reverse this physical row.
                page.insert_text((100, 39.9999), 'B')
                page.insert_text((20, 40), 'A')
                document.save(root/'probe.pdf')
            (root/'probe.json').write_text(json.dumps(dict(page_count=page_count, glyphs=glyphs)), encoding='utf-8')
            subprocess.run([sys.executable, str(Path(__file__).with_name('compare-word-coordinates.py')), str(root)], check=True, capture_output=True)
            return json.loads((root/'comparison.json').read_text(encoding='utf-8'))[0]

    def test_pdf_baseline_roundoff_keeps_physical_column_order(self):
        result = self.compare([[1, 'A', 20, 40], [1, 'B', 100, 40]])
        self.assertEqual((result['word_glyphs'], result['native_glyphs'], result['matched'], result['within_0_375pt']), (2, 2, 2, 2))

    def test_absolute_horizontal_and_vertical_shifts_still_fail(self):
        for x, y in [(20.376, 40), (20, 40.376)]:
            with self.subTest(x=x, y=y):
                result = self.compare([[1, 'A', x, y], [1, 'B', 100, 40]])
                self.assertLess(result['within_0_375pt'], 2)

    def test_missing_or_substituted_glyph_still_fails(self):
        for glyphs in [[[1, 'A', 20, 40]], [[1, 'A', 20, 40], [1, 'C', 100, 40]]]:
            with self.subTest(glyphs=glyphs):
                result = self.compare(glyphs)
                self.assertLess(result['matched'], 2)

    def test_moved_page_still_fails(self):
        result = self.compare([[2, 'A', 20, 40], [2, 'B', 100, 40]], page_count=2)
        self.assertNotEqual(result['word_pages'], result['native_pages'])
        self.assertEqual(result['matched'], 0)


if __name__ == '__main__':
    unittest.main()
