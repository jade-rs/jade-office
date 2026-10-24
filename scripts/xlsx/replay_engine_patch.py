#!/usr/bin/env python3
"""Replay the canonical engine patch against the hash-pinned 0.8.3 crate."""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile


ARCHIVE_SHA256 = "ce1dd83cada7da742782076daccda92e33162e9f4396a958ae3e7c5bb64e6a01"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--update", action="store_true", help="Regenerate jade.patch before replay")
    args = parser.parse_args()
    vendor = Path(__file__).resolve().parents[2] / "jade-core/third_party/ironcalc-base"
    digest = hashlib.sha256(args.archive.read_bytes()).hexdigest()
    if digest != ARCHIVE_SHA256:
        raise SystemExit("The archive does not match the pinned official release")
    with tarfile.open(args.archive) as archive:
        original = {
            item.name.split("/", 1)[1]: archive.extractfile(item).read()
            for item in archive if item.isfile()
        }
    inventory = json.loads((vendor / "upstream-sha256.json").read_text())
    for name, expected in inventory.items():
        if hashlib.sha256(original[name]).hexdigest() != expected:
            raise SystemExit(f"Upstream provenance differs: {name}")
    paths = {p for p in original if p.startswith("src/") or p in {"Cargo.toml", "build.rs"}}
    paths.update(p.relative_to(vendor).as_posix() for p in (vendor / "src").rglob("*") if p.is_file())
    patch_path = vendor / "jade.patch"
    changed = []
    if args.update:
        patch = []
        for name in sorted(paths):
            before = original.get(name, b"")
            after = (vendor / name).read_bytes() if (vendor / name).exists() else b""
            if before == after:
                continue
            if any(body and not body.endswith(b"\n") for body in [before, after]):
                raise SystemExit(f"Changed text must end with a newline: {name}")
            patch.extend(difflib.unified_diff(
                before.decode().splitlines(keepends=True), after.decode().splitlines(keepends=True),
                fromfile=f"a/{name}" if name in original else "/dev/null",
                tofile=f"b/{name}" if after else "/dev/null",
            ))
            changed.append(name)
        patch_path.write_text("".join(patch))
    with tempfile.TemporaryDirectory(prefix="jade-engine-patch-") as directory:
        root = Path(directory)
        for name, body in original.items():
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(body)
        result = subprocess.run(
            ["patch", "-p1", "--batch"], cwd=root, input=patch_path.read_bytes(),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False,
        )
        if result.returncode:
            raise SystemExit(result.stdout.decode(errors="replace"))
        for name in paths:
            actual = vendor / name
            replayed = root / name
            if actual.exists() != replayed.exists() or (actual.exists() and actual.read_bytes() != replayed.read_bytes()):
                raise SystemExit(f"Patch replay differs: {name}")
        current_sources = {p.relative_to(vendor).as_posix() for p in (vendor / "src").rglob("*") if p.is_file()}
        replayed_sources = {p.relative_to(root).as_posix() for p in (root / "src").rglob("*") if p.is_file()}
        if current_sources != replayed_sources:
            raise SystemExit("Patch replay has extra or missing source files")
    print(json.dumps({
        "result": "passed", "source_and_build_files": len(paths),
        "updated": args.update, "changed_files": len(changed) if args.update else None,
        "archive_sha256": digest, "jade_patch_sha256": hashlib.sha256(patch_path.read_bytes()).hexdigest(),
        "upstream_hash_inventory_checked": len(inventory),
    }, indent=2))


if __name__ == "__main__":
    main()
