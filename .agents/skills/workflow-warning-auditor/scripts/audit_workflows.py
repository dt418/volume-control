#!/usr/bin/env python3
"""Audit GitHub Actions workflow warnings without mutating the repository."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

import yaml


NODE_RUNTIME_BASELINES = {
    "actions/setup-node": (5, "Node 24"),
    "actions/setup-python": (6, "Node 24"),
}
MUTABLE_REFS = {"main", "master", "HEAD", "latest"}


def _loader_without_yaml11_booleans() -> type[yaml.SafeLoader]:
    class Loader(yaml.SafeLoader):
        pass

    for key, resolvers in list(Loader.yaml_implicit_resolvers.items()):
        Loader.yaml_implicit_resolvers[key] = [
            resolver
            for resolver in resolvers
            if resolver[0] != "tag:yaml.org,2002:bool"
        ]
    return Loader


Loader = _loader_without_yaml11_booleans()


def _line_for(text: str, needle: str) -> int | None:
    for number, line in enumerate(text.splitlines(), 1):
        if needle in line:
            return number
    return None


def _parse_action(value: str) -> tuple[str, str] | None:
    if not isinstance(value, str) or "@" not in value:
        return None
    action, ref = value.rsplit("@", 1)
    if "/" not in action or not ref:
        return None
    return action, ref


def _walk_uses(node: Any, path: tuple[str, ...] = ()) -> list[tuple[str, tuple[str, ...]]]:
    found: list[tuple[str, tuple[str, ...]]] = []
    if isinstance(node, dict):
        for key, value in node.items():
            next_path = (*path, str(key))
            if key == "uses" and isinstance(value, str):
                found.append((value, next_path))
            found.extend(_walk_uses(value, next_path))
    elif isinstance(node, list):
        for index, value in enumerate(node):
            found.extend(_walk_uses(value, (*path, str(index))))
    return found


def _finding(path: Path, text: str, code: str, message: str, needle: str) -> dict[str, Any]:
    return {
        "file": str(path),
        "line": _line_for(text, needle),
        "code": code,
        "message": message,
    }


def audit_file(path: Path) -> list[dict[str, Any]]:
    text = path.read_text(encoding="utf-8")
    try:
        workflow = yaml.load(text, Loader=Loader) or {}
    except yaml.YAMLError as error:
        return [_finding(path, text, "yaml", f"invalid workflow YAML: {error}", "")]

    findings: list[dict[str, Any]] = []
    for value, _ in _walk_uses(workflow):
        parsed = _parse_action(value)
        if parsed is None:
            continue
        action, ref = parsed
        baseline = NODE_RUNTIME_BASELINES.get(action)
        if baseline:
            match = re.fullmatch(r"v?(\d+)", ref)
            if match and int(match.group(1)) < baseline[0]:
                findings.append(
                    _finding(
                        path,
                        text,
                        "node-runtime",
                        f"{value} targets a deprecated runtime; review {action}@v{baseline[0]}+ for {baseline[1]}.",
                        value,
                    )
                )
        if ref in MUTABLE_REFS:
            findings.append(
                _finding(
                    path,
                    text,
                    "mutable-ref",
                    f"{value} uses a mutable ref; prefer a reviewed major or commit SHA.",
                    value,
                )
            )

    on = workflow.get("on", {}) if isinstance(workflow, dict) else {}
    for event_name, event_config in (on.items() if isinstance(on, dict) else []):
        if event_name not in {"push", "pull_request", "workflow_dispatch"}:
            continue
        if not isinstance(event_config, dict):
            continue
        branches = event_config.get("branches", [])
        if isinstance(branches, str):
            branches = [branches]
        if "master" in branches:
            findings.append(
                _finding(
                    path,
                    text,
                    "trigger-branch",
                    "workflow trigger still names master; confirm the repository default branch is intentional.",
                    "master",
                )
            )

    if isinstance(workflow, dict) and "permissions" not in workflow:
        findings.append(
            _finding(
                path,
                text,
                "permissions",
                "workflow has no explicit top-level permissions; declare least privilege.",
                "name:",
            )
        )
    if "pull_request_target" in text:
        findings.append(
            _finding(
                path,
                text,
                "dangerous-trigger",
                "pull_request_target requires an explicit untrusted-code security review.",
                "pull_request_target",
            )
        )
    return findings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--json", action="store_true", dest="as_json")
    parser.add_argument("--strict", action="store_true")
    args = parser.parse_args()
    workflow_dir = args.repo / ".github" / "workflows"
    paths = sorted((*workflow_dir.glob("*.yml"), *workflow_dir.glob("*.yaml")))
    findings = [finding for path in paths for finding in audit_file(path)]
    if args.as_json:
        print(json.dumps(findings, indent=2, sort_keys=True))
    elif findings:
        for finding in findings:
            location = f"{finding['file']}:{finding['line'] or '?'}"
            print(f"WARNING {location} [{finding['code']}] {finding['message']}")
    else:
        print(f"No workflow warnings found in {len(paths)} file(s).")
    return 1 if args.strict and findings else 0


if __name__ == "__main__":
    sys.exit(main())
