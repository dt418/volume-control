#!/usr/bin/env python3
"""Validate the repository's ECC Tools manifest, reference sets, and skill mirrors."""

from __future__ import annotations

import json
import sys
from pathlib import Path


EXPECTED_REFERENCE_SET_ITEMS = {
    "deep-analyzer-corpus",
    "rag-evaluator",
    "pr-salvage",
    "discussion-triage",
    "harness-compatibility",
    "security-evidence",
    "ci-failure-mode",
}


def fail(message: str) -> int:
    print(f"ECC config validation failed: {message}", file=sys.stderr)
    return 1


def repository_root(arguments: list[str]) -> Path:
    if len(arguments) > 2:
        raise ValueError("usage: python scripts/test-ecc-config.py [repo-root]")
    if len(arguments) == 2:
        return Path(arguments[1]).resolve()
    return Path(__file__).resolve().parent.parent


def validate(root: Path) -> list[str]:
    errors: list[str] = []

    # 1. Validate .claude/ecc-tools.json
    manifest_path = root / ".claude" / "ecc-tools.json"
    if not manifest_path.is_file():
        return [f"missing manifest: {manifest_path}"]

    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        return [f"unable to parse {manifest_path}: {exc}"]

    # Check referenceSetReadiness
    readiness = manifest.get("referenceSetReadiness")
    if not isinstance(readiness, dict):
        errors.append("manifest missing 'referenceSetReadiness' object")
    else:
        score = readiness.get("score")
        if score != 1.0 and score != 1:
            errors.append(f"referenceSetReadiness score must be 1.0, got {score}")

        items = readiness.get("items", [])
        if len(items) != 7:
            errors.append(f"referenceSetReadiness must contain 7 items, got {len(items)}")

        found_ids = set()
        for item in items:
            item_id = item.get("id")
            found_ids.add(item_id)
            if item.get("status") != "ready":
                errors.append(f"item {item_id} status must be 'ready', got {item.get('status')!r}")
            evidence = item.get("evidence", [])
            if not evidence:
                errors.append(f"item {item_id} must have non-empty evidence array")
            for ev_path_str in evidence:
                ev_path = root / ev_path_str
                if not ev_path.exists():
                    errors.append(f"item {item_id} references missing evidence path: {ev_path_str}")

        missing_items = EXPECTED_REFERENCE_SET_ITEMS - found_ids
        if missing_items:
            errors.append(f"missing expected reference set items: {missing_items}")

    # Check managedFiles exist
    managed_files = manifest.get("managedFiles", [])
    if not managed_files:
        errors.append("managedFiles must not be empty")
    for file_str in managed_files:
        path = root / file_str
        if not path.is_file():
            errors.append(f"managed file does not exist: {file_str}")

    # 2. Validate .claude/identity.json
    identity_path = root / ".claude" / "identity.json"
    if not identity_path.is_file():
        errors.append(f"missing identity file: {identity_path}")
    else:
        try:
            identity = json.loads(identity_path.read_text(encoding="utf-8"))
            if "rust" not in identity.get("domains", []):
                errors.append("identity.json must include 'rust' domain")
        except Exception as exc:
            errors.append(f"unable to parse {identity_path}: {exc}")

    # 3. Validate skill mirrors
    claude_skill = root / ".claude" / "skills" / "volume-control" / "SKILL.md"
    agents_skill = root / ".agents" / "skills" / "volume-control" / "SKILL.md"
    if not claude_skill.is_file():
        errors.append(f"missing Claude skill: {claude_skill}")
    if not agents_skill.is_file():
        errors.append(f"missing Agents skill: {agents_skill}")
    if claude_skill.is_file() and agents_skill.is_file():
        if claude_skill.read_bytes() != agents_skill.read_bytes():
            errors.append("volume-control SKILL.md mirrors in .claude and .agents are not byte-identical")

    # 4. Validate reference-sets JSON files parse cleanly
    ref_sets_dir = root / "docs" / "reference-sets"
    for item_id in EXPECTED_REFERENCE_SET_ITEMS:
        filename = f"{item_id}.json" if item_id != "ci-failure-mode" else "ci-failure-modes.json"
        ref_file = ref_sets_dir / filename
        if not ref_file.is_file():
            errors.append(f"missing reference set file: {ref_file}")
        else:
            try:
                json.loads(ref_file.read_text(encoding="utf-8"))
            except Exception as exc:
                errors.append(f"invalid JSON in reference set {ref_file}: {exc}")

    return errors


def main(arguments: list[str]) -> int:
    try:
        root = repository_root(arguments)
    except ValueError as error:
        return fail(str(error))

    errors = validate(root)
    if errors:
        for err in errors:
            print(f"- {err}", file=sys.stderr)
        return fail(f"{len(errors)} error(s) found")

    print(f"ECC config validation passed: {root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
