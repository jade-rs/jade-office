"""Integrity checks for versioned captures; all fixtures here are synthetic."""
import hashlib
import importlib.util
import json
import os
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
                            word=dict(product_version='16.0.19127.20860', file_version='16.0.19127.20860',
                                      application_version='16.0', application_build='16.0.19127'),
                            os='Windows test host', os_architecture='X64', documents=[dict(
                                name='probe', source=str(docx), pdf='probe.pdf', pages=1,
                                compatibility_mode=12,
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

    def prepare_unclipped_capture(self):
        import pymupdf
        doc = pymupdf.open()
        page = doc.new_page(width=100, height=100)
        page.insert_text((20,40), 'A', fontsize=12)
        page.insert_text((20,90), 'B', fontsize=12)
        stream = page.get_contents()[-1]
        doc.update_stream(stream, b'q\n0 40 100 60 re W n\n' + doc.xref_stream(stream) + b'\nQ\n')
        pdf = doc.tobytes()
        (self.source/'probe.pdf').write_bytes(pdf)
        self.capture['documents'][0]['pdf_sha256'] = hashlib.sha256(pdf).hexdigest()
        self.capture['captured_at_utc'] = '2026-11-01T01:02:03Z'
        self.capture['table_glyph_projection'] = 'unclipped'
        self.write(self.manifest, self.capture)
        raw = extractor.extract(self.manifest)
        visible = extractor.extract(self.manifest, 'visible')
        self.write(self.source/'word-reference.json', raw)
        self.write(self.source/'word-visible-reference.json', visible)
        rows = [dict(self.comparison[0], word_glyphs=2, native_glyphs=2, matched=2,
                     within_0_375pt=2, table_glyph_projection='unclipped',
                     pdf_sha256=self.capture['documents'][0]['pdf_sha256'])]
        visible_rows = [dict(rows[0], word_glyphs=1, native_glyphs=1, matched=1,
                             within_0_375pt=1, table_glyph_projection='visible')]
        self.write(self.source/'comparison.json', rows)
        self.write(self.source/'visible-comparison.json', visible_rows)
        native_doc = pymupdf.open(stream=pdf, filetype='pdf')
        native_doc.set_metadata(dict(producer='synthetic native renderer'))
        native_pdf = native_doc.tobytes()
        (self.source/'probe.native.pdf').write_bytes(native_pdf)
        self.write(self.source/'painted-comparison.json', [dict(visible_rows[0], coordinate_source='Native vector PDF', native_pdf_sha256=hashlib.sha256(native_pdf).hexdigest())])
        for suffix, reference in [('.json', raw), ('.visible.json', visible)]:
            self.write(self.source/('probe'+suffix), dict(page_count=1,
                       table_glyph_projection=reference['table_glyph_projection'],
                       glyphs=reference['fixtures'][0]['glyphs']))
        return raw, visible, rows, visible_rows

    def test_unclipped_and_visible_pdf_origins_are_preserved_separately(self):
        raw, visible, _, _ = self.prepare_unclipped_capture()
        self.assertEqual(''.join(g[1] for g in raw['fixtures'][0]['glyphs']), 'AB')
        self.assertEqual(''.join(g[1] for g in visible['fixtures'][0]['glyphs']), 'A')
        self.assertEqual(raw['pdf_text_flags'], 135)
        self.assertEqual(visible['pdf_text_flags'], 199)
        with patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
            target = archiver.archive(self.manifest, self.evidence, ['probe'])
        index = json.loads((self.evidence/'index.json').read_text(encoding='utf-8'))['references'][0]
        self.assertEqual(index['glyph_count'], 2)
        self.assertEqual(index['visible_glyph_count'], 1)
        self.assertEqual(index['visible_reference'], 'strict-visible.json')
        self.assertTrue((target/'probe.visible.json').exists())
        self.assertTrue((target/'word-visible-reference.json').exists())

    def test_unclipped_success_cannot_hide_visible_coordinate_failure(self):
        _, _, _, visible_rows = self.prepare_unclipped_capture()
        visible_rows[0]['within_0_375pt'] = 0
        self.write(self.source/'visible-comparison.json', visible_rows)
        with self.assertRaisesRegex(ValueError, 'mismatching'):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertFalse(self.evidence.exists())

    def test_projection_mismatch_and_missing_visible_proof_cannot_promote(self):
        _, _, rows, _ = self.prepare_unclipped_capture()
        rows[0]['table_glyph_projection'] = 'visible'
        self.write(self.source/'comparison.json', rows)
        with self.assertRaisesRegex(ValueError, 'projection'):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        rows[0]['table_glyph_projection'] = 'unclipped'
        self.write(self.source/'comparison.json', rows)
        self.write(self.source/'visible-comparison.json', [])
        with self.assertRaisesRegex(ValueError, 'every visible fixture'):
            archiver.archive(self.manifest, self.evidence, ['probe'])

    def test_visible_origins_are_reconstructed_from_original_pdf(self):
        _, visible, _, _ = self.prepare_unclipped_capture()
        visible['fixtures'][0]['glyphs'][0][2] += 1
        self.write(self.source/'word-visible-reference.json', visible)
        with self.assertRaisesRegex(ValueError, 'original Word PDF'):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertFalse(self.evidence.exists())

    def test_painted_pdf_failure_cannot_be_hidden_by_matching_snapshots(self):
        self.prepare_unclipped_capture()
        path = self.source/'painted-comparison.json'
        rows = json.loads(path.read_text(encoding='utf-8'))
        rows[0]['within_0_375pt'] = 0
        self.write(path, rows)
        with self.assertRaisesRegex(ValueError, 'mismatching'):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertFalse(self.evidence.exists())

    def test_changed_native_pdf_cannot_use_a_stale_painted_comparison(self):
        self.prepare_unclipped_capture()
        (self.source/'probe.native.pdf').write_bytes(b'changed native PDF')
        with self.assertRaisesRegex(ValueError, 'actual native PDF'):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertFalse(self.evidence.exists())

    def test_copying_word_as_native_pdf_cannot_create_a_self_comparison(self):
        self.prepare_unclipped_capture()
        pdf = (self.source/'probe.pdf').read_bytes()
        (self.source/'probe.native.pdf').write_bytes(pdf)
        path = self.source/'painted-comparison.json'
        rows = json.loads(path.read_text(encoding='utf-8'))
        rows[0]['native_pdf_sha256'] = hashlib.sha256(pdf).hexdigest()
        self.write(path, rows)
        with self.assertRaisesRegex(ValueError, 'actual native PDF'):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertFalse(self.evidence.exists())

    def test_unknown_projection_cannot_be_extracted(self):
        self.capture['table_glyph_projection'] = 'unknown'
        self.write(self.manifest, self.capture)
        with self.assertRaisesRegex(ValueError, 'Unknown'):
            extractor.extract(self.manifest)

    def test_full_version_directory_and_no_overwrite(self):
        with patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
            target = archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertEqual(target.parent.name, 'word-16.0.19127.20860')
        self.assertEqual((target/'capture.json').read_bytes(), self.manifest.read_bytes())
        original = (self.evidence/'index.json').read_bytes()
        with self.assertRaises(FileExistsError):
            archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertEqual((self.evidence/'index.json').read_bytes(), original)

    def test_relative_parent_segments_are_normalized_before_copying_long_names(self):
        # Reproduce a deep Windows checkout with ../core as the destination.
        # The canonical path fits MAX_PATH; the unresolved path does not.
        original = Path.cwd()
        working = self.root / ("worktree-" + "x" * 90)
        working.mkdir()
        try:
            os.chdir(working)
            relative = Path("..") / ("core-" + "y" * 40) / "word-oracle"
            with patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
                target = archiver.archive(self.manifest, relative, ['probe'])
            self.assertTrue(target.is_absolute())
            self.assertNotIn('..', target.parts)
            self.assertEqual((target/'inputs/probe.docx').read_bytes(), (self.source/'probe.docx').read_bytes())
            self.assertTrue((target.parent.parent/'index.json').exists())
        finally:
            os.chdir(original)

    def test_another_word_build_accumulates_without_replacing_old_evidence(self):
        with patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
            first = archiver.archive(self.manifest, self.evidence, ['probe'])
        original = (first/'capture.json').read_bytes()
        version = '16.0.20000.10000'
        self.capture['word']['product_version'] = version
        self.capture['word']['file_version'] = version
        self.write(self.manifest, self.capture)
        reference_path = self.source/'word-reference.json'
        reference = json.loads(reference_path.read_text(encoding='utf-8'))
        reference['word_product_version'] = version
        self.write(reference_path, reference)
        with patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
            second = archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertNotEqual(first, second)
        self.assertEqual(second.parent.name, 'word-'+version)
        self.assertEqual((first/'capture.json').read_bytes(), original)
        index = json.loads((self.evidence/'index.json').read_text(encoding='utf-8'))
        self.assertEqual({entry['word_product_version'] for entry in index['references']},
                         {'16.0.19127.20860', version})

    def test_original_input_survives_working_source_changes(self):
        original = (self.source/'probe.docx').read_bytes()
        with patch.object(archiver.subprocess, 'check_output', return_value='engine\n'):
            target = archiver.archive(self.manifest, self.evidence, ['probe'])
        (self.source/'probe.docx').write_bytes(b'a later edit')
        self.assertEqual((target/'inputs/probe.docx').read_bytes(), original)
        inputs = json.loads((target/'inputs.json').read_text(encoding='utf-8'))
        self.assertEqual(inputs, dict(schema=1, documents=[dict(
            name='probe', file='inputs/probe.docx', sha256=hashlib.sha256(original).hexdigest())]))

    def test_input_changed_during_copy_cannot_be_registered(self):
        copy = archiver.shutil.copyfile
        def change_during_copy(source, target):
            if Path(source) == self.source/'probe.docx':
                Path(source).write_bytes(b'changed during archive')
            return copy(source, target)
        with patch.object(archiver.shutil, 'copyfile', side_effect=change_during_copy):
            with self.assertRaisesRegex(ValueError, 'changed while archiving'):
                archiver.archive(self.manifest, self.evidence, ['probe'])
        self.assertFalse((self.evidence/'index.json').exists())

    def test_fixture_name_cannot_escape_archived_input_directory(self):
        for name in ('../escape', 'a/b', 'a\\b', 'C:escape', '..', ''):
            with self.subTest(name=name):
                self.capture['documents'][0]['name'] = name
                self.comparison[0]['fixture'] = name
                self.write(self.manifest, self.capture)
                self.write(self.source/'comparison.json', self.comparison)
                with self.assertRaisesRegex(ValueError, 'plain filename'):
                    archiver.archive(self.manifest, self.evidence, [name])

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
