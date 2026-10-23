#!/usr/bin/env python3
"""Validate MAP JSON Schemas and conformance vectors.

Checks:
- every schemas/*.schema.json is valid Draft 2020-12 and declares $schema/$id/title
- every vector file that names a schema accepts its valid documents
  (valid[] / flow / transcript lines) and rejects its invalid[] documents
- invalid cases carry a name and an error code
- every JSON document under schemas/ and vectors/ parses

Usage: python scripts/validate_artifacts.py
Requires: pip install jsonschema
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT / "schemas"
VECTORS = ROOT / "vectors"
DRAFT = "https://json-schema.org/draft/2020-12/schema"

failures: list[str] = []


def fail(message: str) -> None:
    failures.append(message)


def load_json(path: Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"{path.relative_to(ROOT)}: invalid JSON: {error}")
        return None


def document_of(case: dict):
    for key in ("document", "envelope"):
        if key in case:
            return case[key]
    return None


def check_schemas() -> dict[Path, Draft202012Validator]:
    validators: dict[Path, Draft202012Validator] = {}
    for path in sorted(SCHEMAS.glob("*.schema.json")):
        relative = path.relative_to(ROOT)
        schema = load_json(path)
        if schema is None:
            continue
        for key in ("$schema", "$id", "title"):
            if key not in schema:
                fail(f"{relative}: missing {key}")
        if schema.get("$schema") != DRAFT:
            fail(f"{relative}: $schema must be {DRAFT}")
        if not str(schema.get("$id", "")).endswith(f"/schemas/{path.name}"):
            fail(f"{relative}: $id does not match the file name")
        try:
            Draft202012Validator.check_schema(schema)
        except Exception as error:  # jsonschema.SchemaError
            fail(f"{relative}: invalid schema: {error.message}")
            continue
        validators[path.resolve()] = Draft202012Validator(schema)
    return validators


def check_vector_file(
    path: Path, validators: dict[Path, Draft202012Validator]
) -> tuple[int, int]:
    relative = path.relative_to(ROOT)
    data = load_json(path)
    if data is None:
        return 0, 0
    schema_ref = data.get("schema")
    if not schema_ref:
        return 0, 0
    # Vector files reference schemas relative to vectors/ (the repository
    # convention), not relative to the vector file itself.
    schema_path = (VECTORS / schema_ref).resolve()
    if SCHEMAS not in schema_path.parents:
        fail(f"{relative}: schema {schema_ref} is outside schemas/")
        return 0, 0
    validator = validators.get(schema_path)
    if validator is None:
        fail(f"{relative}: schema {schema_ref} not found or not loadable")
        return 0, 0

    accepted = 0
    rejected = 0

    def check_valid(label: str, document) -> None:
        nonlocal accepted
        accepted += 1
        errors = sorted(validator.iter_errors(document), key=lambda error: list(error.path))
        if errors:
            error = errors[0]
            fail(
                f"{relative}: {label} rejected: {error.message} "
                f"(at {'/'.join(map(str, error.path)) or '<root>'})"
            )

    for case in data.get("valid", []):
        name = case.get("name")
        if not name:
            fail(f"{relative}: valid case without a name")
            continue
        document = document_of(case)
        if document is None:
            fail(f"{relative}: valid case '{name}' has no document/envelope")
            continue
        check_valid(f"valid case '{name}'", document)

    for case in data.get("invalid", []):
        name = case.get("name")
        if not name:
            fail(f"{relative}: invalid case without a name")
            continue
        if not case.get("code"):
            fail(f"{relative}: invalid case '{name}' has no error code")
        document = document_of(case)
        if document is None:
            fail(f"{relative}: invalid case '{name}' has no document/envelope")
            continue
        rejected += 1
        if validator.is_valid(document) and not case.get("semantic"):
            fail(f"{relative}: invalid case '{name}' was accepted by the schema")

    for case in data.get("flow", []):
        name = case.get("name", "<unnamed flow>")
        for key in ("request", "decision"):
            document = case.get(key)
            if isinstance(document, dict):
                check_valid(f"flow '{name}' {key}", document)

    for index, item in enumerate(data.get("transcript", [])):
        line = item.get("line")
        if isinstance(line, dict):
            check_valid(f"transcript line {index}", line)

    return accepted, rejected


def main() -> int:
    validators = check_schemas()
    files = len(list(SCHEMAS.rglob("*.json"))) + len(list(VECTORS.rglob("*.json")))
    accepted = 0
    rejected = 0
    for path in sorted(VECTORS.rglob("*.json")):
        valid, invalid = check_vector_file(path, validators)
        accepted += valid
        rejected += invalid

    if failures:
        print("artifact validation failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(
        f"ok: {len(validators)} schemas, {files} JSON files, "
        f"{accepted} accepted and {rejected} rejected vector cases"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
