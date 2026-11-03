"""Integrity checks for versioned captures; all fixtures here are synthetic."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


def load_script(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), Path(__file__).parent/(name+'.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


archiver = load_script('archive-word-windows-capture')
extractor = load_script('extract-word-windows-reference')


class VersionCaptureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root/'source'
        self.source.mkdir()
        self.evidence = self.root/'evidence'
        docx, pdf = self.source/'probe.docx', self.source/'probe.pdf'
        docx.write_bytes(b'synthetic docx for integrity checks')
        pdf.write_bytes(b'synthetic pdf for integrity checks')
        self.capture = dict(schema=2, platform='windows', status='complete',
                            capture_id='20261101T010203456Z-1234abcd',
                            word=dict(product_version='16.0.19127.20860'), documents=[dict(
                                name='probe', source=str(docx), pdf='probe.pdf', pages=1,
                                sha256=hashlib.sha256(docx.read_bytes()).hexdigest(),
                                pdf_sha256=hashlib.sha256(pdf.read_bytes()).hexdigest())])
        self.manifest = self.source/'capture.json'
        self.write(self.manifest, self.capture)
        reference = dict(word_product_version='16.0.19127.20860', capture_id=self.capture['capture_id'],
                         fixtures=[dict(name='probe', glyphs=[[1,'A',72,84]])])
        self.write(self.source/'word-reference.json', reference)
        self.comparison = [dict(fixture='probe', word_pages=1, native_pages=1, word_glyphs=1,
                                native_glyphs=1, matched=1, within_0_375pt=1,
                                pdf_sha256=self.capture['documents'][0]['pdf_sha256'])]
        self.write(self.source/'comparison.json', self.comparison)

    @staticmethod
    def write(path, value):
        path.write_text(json.dumps(value), encoding='utf-8')

    def test_full_version_directory_and_no_overwrite(self):
        with patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
            target = archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertEqual(target.parent.name, 'word-16.0.19127.20860')
        original = (self.evidence/'index.json').read_bytes()
        with self.assertRaises(FileExistsError):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertEqual((self.evidence/'index.json').read_bytes(), original)

    def test_mismatch_cannot_be_promoted(self):
        self.comparison[0]['within_0_375pt'] = 0
        self.write(self.source/'comparison.json', self.comparison)
        with self.assertRaisesRegex(ValueError, 'Refusing to promote'):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertFalse(self.evidence.exists())

    def test_tampered_pdf_cannot_be_archived(self):
        (self.source/'probe.pdf').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'PDF changed'):
            archiver.archive(self.manifest, self.evidence, ['probe'])

    def test_reference_cannot_be_mixed_between_word_versions(self):
        self.capture['word']['product_version'] = '16.0.20000.10000'
        self.write(self.manifest, self.capture)
        with self.assertRaisesRegex(ValueError, 'different version'):
            archiver.archive(self.manifest, self.evidence, ['probe'])

    def test_stale_comparison_cannot_promote_another_word_pdf(self):
        self.comparison[0]['pdf_sha256'] = 'different-pdf'
        self.write(self.source/'comparison.json', self.comparison)
        with self.assertRaisesRegex(ValueError, 'different Word PDF'):
            archiver.archive(self.manifest, self.evidence, ['probe'])

    def test_missing_native_comparison_cannot_be_archived(self):
        self.write(self.source/'comparison.json', [])
        with self.assertRaisesRegex(ValueError, 'Incomplete'):
            archiver.archive(self.manifest, self.evidence, [])

    def add_metafile(self):
        data = b'raw drawing bytes (extraction is tested independently in jade-core)'
        (self.source/'probe.page-001.emf').write_bytes(data)
        page = dict(page=1, file='probe.page-001.emf', sha256=hashlib.sha256(data).hexdigest(),
                    width_pt=612, height_pt=792)
        self.capture['documents'][0]['metafiles'] = [page]
        self.write(self.manifest, self.capture)
        reference = json.loads((self.source/'word-reference.json').read_text(encoding='utf-8'))
        reference['coordinate_source'] = 'Word Page.EnhMetaFileBits / MS-EMF'
        reference['fixtures'][0]['metafiles'] = [page]
        self.write(self.source/'word-emf-reference.json', reference)
        comparison = [dict(self.comparison[0], metafile_sha256=[page['sha256']])]
        self.write(self.source/'emf-comparison.json', comparison)
        return reference, comparison

    def test_metafiles_and_separate_strict_reference_are_preserved(self):
        reference, _ = self.add_metafile()
        with patch.object(archiver, 'extract_emf_capture', return_value=reference), \
                patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
            target = archiver.archive(self.manifest, self.evidence, ['probe'], 'emf')
        self.assertEqual((target/'probe.page-001.emf').read_bytes(),
                         (self.source/'probe.page-001.emf').read_bytes())
        entry = json.loads((self.evidence/'index.json').read_text(encoding='utf-8'))['references'][0]
        self.assertEqual(entry['reference'], 'strict-emf.json')
        self.assertTrue((target/'word-reference.json').exists(), 'retain independent PDF evidence')

    def test_changed_metafile_cannot_be_archived(self):
        self.add_metafile()
        (self.source/'probe.page-001.emf').write_bytes(b'changed drawing')
        with self.assertRaisesRegex(ValueError, 'metafile changed'):
            archiver.archive(self.manifest, self.evidence, ['probe'], 'emf')

    def test_stale_drawing_comparison_and_forged_origins_cannot_promote(self):
        reference, comparison = self.add_metafile()
        comparison[0]['metafile_sha256'] = ['stale drawing hash']
        self.write(self.source/'emf-comparison.json', comparison)
        with self.assertRaisesRegex(ValueError, 'different Word metafiles'):
            archiver.archive(self.manifest, self.evidence, ['probe'], 'emf')
        comparison[0]['metafile_sha256'] = [reference['fixtures'][0]['metafiles'][0]['sha256']]
        self.write(self.source/'emf-comparison.json', comparison)
        with patch.object(archiver, 'extract_emf_capture', return_value=dict(reference, fixtures=[])):
            with self.assertRaisesRegex(ValueError, 'differs from raw Word'):
                archiver.archive(self.manifest, self.evidence, ['probe'], 'emf')

    def test_partial_capture_or_short_version_cannot_be_extracted(self):
        self.capture['status'] = 'failed'
        self.write(self.manifest, self.capture)
        with self.assertRaisesRegex(ValueError, 'complete versioned'):
            extractor.extract(self.manifest)
        self.capture['status'] = 'complete'
        self.capture['word']['product_version'] = '16.0.19127'
        self.write(self.manifest, self.capture)
        with self.assertRaisesRegex(ValueError, 'complete build'):
            extractor.extract(self.manifest)


if __name__ == '__main__':
    unittest.main()
