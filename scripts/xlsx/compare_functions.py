#!/usr/bin/env python3
"""Compare the actual Jade registry with a pinned Excel name inventory."""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jade", type=Path, required=True)
    parser.add_argument("--reference", type=Path, default=Path(__file__).resolve().parents[2] / "docs/xlsx/function-baseline.json")
    args = parser.parse_args()
    reference = json.loads(args.reference.read_text())
    jade = set(json.loads(args.jade.read_text()))
    baseline = set(reference["names"])
    print(json.dumps({
        "reference_commit": reference["commit"],
        "jade_registered": len(jade),
        "baseline_registered": len(baseline),
        "shared": len(jade & baseline),
        "not_registered_in_jade": sorted(baseline - jade),
        "jade_only": sorted(jade - baseline),
    }, indent=2))


if __name__ == "__main__":
    main()
