#!/usr/bin/env python3
"""Puts the head checkout's perf harness into the base checkout.

    scripts/perf/graft_harness.py <head checkout> <base checkout>

Both sides of a comparison must run the same scenarios, so the base commit is
measured with the head's `crates/jade-office-perf` rather than its own (which
may differ, or not exist yet). The crate is its own workspace, so copying it
is all it takes. If the harness does not compile against the base, the
workflow reports the head alone.
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

CRATE = "crates/jade-office-perf"


def main() -> int:
    head, base = (Path(arg) for arg in sys.argv[1:3])
    target = base / CRATE
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(head / CRATE, target, ignore=shutil.ignore_patterns("target"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
