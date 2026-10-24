#!/usr/bin/env python3
"""Validate actual authored OOXML components using unmodified official XSDs.

Supply extracted ECMA-376 Part4 Transitional and Part1 Strict XMLSchema ZIPs.
Requires xmllint; schemas and workbook namespaces are never adapted to pass.
This is component XSD validation, not an Office/package/semantic certification.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--transitional-schema", type=Path, required=True)
    parser.add_argument("--strict-schema", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--profile", choices=["authoring", "lifecycle"], default="authoring", help="Lifecycle corpus includes intentional report/cache deletion")
    parser.add_argument("--allow-office-xml-space", action="store_true", help="Classify only Excel's documented xml:space-on-text extension separately; never rewrite workbook XML or official schemas")
    args = parser.parse_args()
    namespaces = {
        "http://schemas.openxmlformats.org/spreadsheetml/2006/main": (args.transitional_schema, "sml.xsd"),
        "http://schemas.openxmlformats.org/drawingml/2006/chart": (args.transitional_schema, "dml-chart.xsd"),
        "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing": (args.transitional_schema, "dml-spreadsheetDrawing.xsd"),
        "http://purl.oclc.org/ooxml/spreadsheetml/main": (args.strict_schema, "sml.xsd"),
        "http://purl.oclc.org/ooxml/drawingml/chart": (args.strict_schema, "dml-chart.xsd"),
        "http://purl.oclc.org/ooxml/drawingml/spreadsheetDrawing": (args.strict_schema, "dml-spreadsheetDrawing.xsd"),
    }
    report = {"profile": args.profile, "scope": "Original authored Transitional/Strict SpreadsheetML, chart and spreadsheet-drawing component XML; no namespace/schema rewriting during validation; OPC relationships/content types and Office semantics are separate", "workbooks": [], "components": [], "failed": [], "office_deviations": [], "xml_space_policy": "https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/82ad64a7-401b-4f34-9319-3600b16b2791"}
    with tempfile.TemporaryDirectory(prefix="jade-pivot-xsd-") as directory:
        part_file = Path(directory) / "component.xml"
        for path in sorted(args.corpus.glob("*.xlsx")):
            report["workbooks"].append({"file": str(path), "sha256": digest(path.read_bytes())})
            with zipfile.ZipFile(path) as package:
                validated = set()
                for part in package.namelist():
                    if not part.endswith(".xml"):
                        continue
                    data = package.read(part)
                    root = ET.fromstring(data)
                    namespace = root.tag.removeprefix("{").split("}")[0] if root.tag.startswith("{") else ""
                    if namespace not in namespaces:
                        continue
                    base, schema_name = namespaces[namespace]
                    schema = base / schema_name
                    assert schema.is_file(), schema
                    part_file.write_bytes(data)
                    command = ["xmllint", "--nonet", "--noout", "--schema", str(schema), str(part_file)]
                    result = subprocess.run(command, text=True, capture_output=True)
                    entry = {"workbook": path.name, "part": part, "namespace": namespace, "sha256": digest(data), "schema": str(schema), "schema_sha256": digest(schema.read_bytes()), "exit_code": result.returncode}
                    report["components"].append(entry)
                    validated.add(part)
                    if result.returncode:
                        errors = [line for line in result.stderr.splitlines() if "Schemas validity error" in line]
                        xml_space = "{http://www.w3.org/XML/1998/namespace}space"
                        expected = f"Element '{{{namespace}}}t', attribute '{xml_space}': The attribute '{xml_space}' is not allowed."
                        office_text = namespace in {"http://schemas.openxmlformats.org/spreadsheetml/2006/main", "http://purl.oclc.org/ooxml/spreadsheetml/main"}
                        attributes_valid = all(node.tag == f"{{{namespace}}}t" and node.attrib[xml_space] in {"default", "preserve"} for node in root.iter() if xml_space in node.attrib)
                        deviation = args.allow_office_xml_space and office_text and attributes_valid and result.returncode == 3 and errors and all(expected in line for line in errors)
                        key = "office_deviations" if deviation else "failed"
                        report[key].append(entry | {"diagnostics": result.stderr})
                required = {"xl/workbook.xml", "xl/styles.xml", "xl/worksheets/sheet1.xml", "xl/worksheets/sheet2.xml", "xl/pivotTables/report.xml", "xl/pivotCache/cache.xml", "xl/pivotCache/records.xml", "xl/charts/pivot.xml", "xl/drawings/pivot.xml"}
                if args.profile == "lifecycle":
                    required -= {"xl/pivotTables/report.xml", "xl/pivotCache/cache.xml", "xl/pivotCache/records.xml"}
                assert required <= validated, (path.name, "unvalidated/missing corpus components", sorted(required - validated))
    report["passed"] = len(report["components"]) - len(report["failed"]) - len(report["office_deviations"])
    report["schema_inventories"] = {str(base): {str(p.relative_to(base)): digest(p.read_bytes()) for p in sorted(base.rglob("*.xsd"))} for base in [args.transitional_schema, args.strict_schema]}
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f'{len(report["workbooks"])} workbooks; {report["passed"]} components passed; {len(report["office_deviations"])} documented Office xml:space deviations; {len(report["failed"])} unexpected failures')
    for failure in report["failed"][:8]:
        print(failure["workbook"], failure["part"], failure["diagnostics"])
    assert report["workbooks"] and report["components"] and not report["failed"]


if __name__ == "__main__":
    main()
