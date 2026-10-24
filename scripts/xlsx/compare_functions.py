#!/usr/bin/env python3
"""Compare the actual Jade registry with a pinned ibisOffice name inventory."""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jade", type=Path, required=True)
    parser.add_argument("--reference", type=Path, default=Path(__file__).resolve().parents[2] / "docs/xlsx/ibis-function-baseline.json")
    args = parser.parse_args()
    reference = json.loads(args.reference.read_text())
    jade = set(json.loads(args.jade.read_text()))
    ibis = set(reference["names"])
    print(json.dumps({
        "reference_commit": reference["commit"],
        "jade_registered": len(jade),
        "ibis_registered": len(ibis),
        "shared": len(jade & ibis),
        "not_registered_in_jade": sorted(ibis - jade),
        "jade_only": sorted(jade - ibis),
    }, indent=2))


if __name__ == "__main__":
    main()
