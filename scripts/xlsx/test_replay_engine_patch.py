"""Regression checks for patch portability and end-of-file preservation."""
import subprocess
import tempfile
import unittest
from pathlib import Path

from replay_engine_patch import canonical_text, text_diff


class PatchReplayTests(unittest.TestCase):
    def test_replay_preserves_missing_final_newlines(self):
        for before, after in [
            (b"old", b"new"),
            (b"old\n", b"new"),
            (b"old", b"new\n"),
            (b"", b"new"),
            (b"old", b""),
        ]:
            with self.subTest(before=before, after=after), tempfile.TemporaryDirectory() as directory:
                target = Path(directory) / "sample.txt"
                if before:
                    target.write_bytes(before)
                patch = "".join(text_diff(before, after, "sample.txt", bool(before)))
                result = subprocess.run(
                    ["patch", "-p1", "--batch"], cwd=directory,
                    input=patch.encode("utf-8"), capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(target.read_bytes() if target.exists() else b"", after)

    def test_crlf_checkouts_generate_the_same_patch(self):
        before = b"old\nkept\n"
        after = "new\n日本語\n".encode("utf-8")
        expected = "".join(text_diff(before, after, "sample.txt", True))
        actual = "".join(text_diff(
            canonical_text(before.replace(b"\n", b"\r\n")),
            canonical_text(after.replace(b"\n", b"\r\n")), "sample.txt", True,
        ))
        self.assertEqual(actual, expected)
        self.assertNotEqual(canonical_text(b"text"), canonical_text(b"text\n"))

    def test_binary_bytes_are_not_normalized(self):
        body = b"\x89PNG\r\n\xff"
        self.assertEqual(canonical_text(body), body)


if __name__ == "__main__":
    unittest.main()
