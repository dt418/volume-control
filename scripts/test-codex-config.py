#!/usr/bin/env python3
"""Validate the repository's reproducible Codex coordinator contract."""

from __future__ import annotations

import sys
import tomllib
from pathlib import Path


EXPECTED_MAX_THREADS = 4
EXPECTED_MAX_DEPTH = 2
PIPELINE_ROLES = ("coordinator", "planner", "implementer", "release_reviewer")


def fail(message: str) -> int:
    print(f"Codex config contract failed: {message}", file=sys.stderr)
    return 1


def repository_root(arguments: list[str]) -> Path:
    if len(arguments) > 2:
        raise ValueError("usage: python scripts/test-codex-config.py [repo-root]")
    if len(arguments) == 2:
        return Path(arguments[1]).resolve()
    return Path(__file__).resolve().parent.parent


def validate(root: Path) -> list[str]:
    config_path = root / ".codex" / "config.toml"
    try:
        with config_path.open("rb") as config_file:
            config = tomllib.load(config_file)
    except FileNotFoundError:
        return [f"missing configuration: {config_path}"]
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"unable to parse {config_path}: {error}"]

    errors: list[str] = []
    agents = config.get("agents")
    if not isinstance(agents, dict):
        return ["missing [agents] table"]

    max_threads = agents.get("max_threads")
    if max_threads != EXPECTED_MAX_THREADS:
        errors.append(f"agents.max_threads must be {EXPECTED_MAX_THREADS}, got {max_threads!r}")

    max_depth = agents.get("max_depth")
    if max_depth != EXPECTED_MAX_DEPTH:
        errors.append(f"agents.max_depth must be {EXPECTED_MAX_DEPTH}, got {max_depth!r}")

    agents_directory = (root / ".codex" / "agents").resolve()
    profile_roles = [
        role
        for role, value in agents.items()
        if isinstance(value, dict) and "config_file" in value
    ]
    missing_roles = [role for role in PIPELINE_ROLES if role not in profile_roles]
    if missing_roles:
        errors.append(f"missing configured pipeline role(s): {', '.join(missing_roles)}")

    for role in profile_roles:
        profile_path_value = agents[role].get("config_file")
        if not isinstance(profile_path_value, str) or not profile_path_value:
            errors.append(f"agents.{role}.config_file must be a non-empty relative path")
            continue

        profile_path = Path(profile_path_value)
        if profile_path.is_absolute():
            errors.append(f"agents.{role}.config_file must be relative: {profile_path_value}")
            continue

        resolved_profile = (root / ".codex" / profile_path).resolve()
        try:
            resolved_profile.relative_to(agents_directory)
        except ValueError:
            errors.append(
                f"agents.{role}.config_file must resolve inside .codex/agents/: "
                f"{profile_path_value}"
            )
            continue
        if not resolved_profile.is_file():
            errors.append(f"missing profile for agents.{role}: {profile_path_value}")

    orchestrator_path = root / ".codex" / "ORCHESTRATOR.md"
    try:
        orchestrator = orchestrator_path.read_text(encoding="utf-8")
    except FileNotFoundError:
        errors.append(f"missing orchestration documentation: {orchestrator_path}")
    except OSError as error:
        errors.append(f"unable to read {orchestrator_path}: {error}")
    else:
        normalized_orchestrator = orchestrator.casefold()
        required_handoff_text = (
            "## handoff contract",
            "coordinator records task ownership",
            "final handoff state",
        )
        missing_handoff_text = [
            text
            for text in required_handoff_text
            if text not in normalized_orchestrator
        ]
        if missing_handoff_text:
            errors.append(
                ".codex/ORCHESTRATOR.md must include the Handoff contract section "
                "and coordinator ownership/handoff text; missing: "
                + ", ".join(repr(text) for text in missing_handoff_text)
            )

    return errors


def main(arguments: list[str]) -> int:
    try:
        root = repository_root(arguments)
    except ValueError as error:
        return fail(str(error))

    errors = validate(root)
    if errors:
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print(f"Codex config contract passed: {root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
