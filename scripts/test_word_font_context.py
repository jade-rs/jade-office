import hashlib
from io import BytesIO
import json
from pathlib import Path
import tempfile
import unittest

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
import pymupdf

from word_font_context import capture_font_context


class FontContextTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'source.docx').write_bytes(b'unchanged synthetic DOCX')
        self.manifest = self.root / 'capture.json'

    def make_capture(self, version='1.00', embedded=True):
        with pymupdf.open() as pdf:
            page = pdf.new_page()
            if embedded:
                font = FontBuilder(1000, isTTF=True)
                font.setupGlyphOrder(['.notdef', 'A'])
                font.setupCharacterMap({65: 'A'})
                pen = TTGlyphPen(None)
                pen.moveTo((0,0)); pen.lineTo((300,700)); pen.lineTo((600,0)); pen.closePath()
                empty = TTGlyphPen(None).glyph()
                font.setupGlyf({'.notdef': empty, 'A': pen.glyph()})
                font.setupHorizontalMetrics({'.notdef': (600,0), 'A': (600,0)})
                font.setupHorizontalHeader(ascent=800, descent=-200)
                font.setupNameTable(dict(familyName='Captured Test', styleName='Regular',
                                        fullName='Captured Test Regular', psName='CapturedTest-Regular',
                                        version='Version ' + version))
                font.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=800, usWinDescent=200)
                font.setupPost()
                buffer = BytesIO(); font.save(buffer)
                page.insert_font(fontname='captured', fontbuffer=buffer.getvalue())
                page.insert_text((72,84), 'A', fontname='captured')
            else:
                page.insert_text((72,84), 'A')
            data = pdf.tobytes()
        (self.root / 'source.pdf').write_bytes(data)
        self.capture = dict(schema=2, status='complete', platform='windows',
                            capture_id='20261121T010203456Z-1234abcd',
                            word=dict(product_version='16.0.19127.20936', executable_sha256='a'*64),
                            documents=[dict(name='source', source=str(self.root / 'source.docx'),
                                            sha256=hashlib.sha256((self.root / 'source.docx').read_bytes()).hexdigest(),
                                            pdf='source.pdf', pdf_sha256=hashlib.sha256(data).hexdigest(), pages=1)])
        self.save()

    def save(self):
        self.manifest.write_bytes(json.dumps(self.capture).encode())

    def test_same_word_and_source_preserve_different_actual_font_versions(self):
        self.make_capture('1.00')
        before = capture_font_context(self.manifest)
        self.make_capture('2.00')
        after = capture_font_context(self.manifest)
        self.assertEqual(before['word'], after['word'])
        self.assertEqual(before['documents'][0]['source_sha256'], after['documents'][0]['source_sha256'])
        a, b = before['documents'][0]['font_resources'][0], after['documents'][0]['font_resources'][0]
        self.assertEqual((a['family'], a['style'], a['version']), ('Captured Test', 'Regular', 'Version 1.00'))
        self.assertEqual(b['version'], 'Version 2.00')
        self.assertNotEqual(a['program_sha256'], b['program_sha256'])
        self.assertNotEqual(before['context_sha256'], after['context_sha256'])
        self.assertEqual(capture_font_context(self.manifest), after)

    def test_substituted_pdf_and_changed_input_cannot_reuse_context(self):
        self.make_capture()
        original = (self.root / 'source.pdf').read_bytes()
        self.make_capture('2.00')
        (self.root / 'source.pdf').write_bytes(original)
        with self.assertRaisesRegex(ValueError, 'PDF changed'):
            capture_font_context(self.manifest)
        self.make_capture()
        (self.root / 'source.docx').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'DOCX changed'):
            capture_font_context(self.manifest)

    def test_missing_embedded_program_is_explicit(self):
        self.make_capture(embedded=False)
        fact = capture_font_context(self.manifest)['documents'][0]['font_resources'][0]
        self.assertFalse(fact['embedded'])
        self.assertIsNone(fact['version'])
        self.assertIsNone(fact['program_sha256'])
        self.assertEqual(fact['metadata_status'], 'not-embedded')

    def test_wrong_page_count_or_unsafe_path_cannot_create_context(self):
        self.make_capture()
        self.capture['documents'][0]['pages'] = 2; self.save()
        with self.assertRaisesRegex(ValueError, 'page count'):
            capture_font_context(self.manifest)
        self.capture['documents'][0]['pdf'] = '../source.pdf'; self.save()
        with self.assertRaisesRegex(ValueError, 'capture directory'):
            capture_font_context(self.manifest)


if __name__ == '__main__':
    unittest.main()
