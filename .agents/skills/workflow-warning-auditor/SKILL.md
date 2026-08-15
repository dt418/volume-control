---
name: workflow-warning-auditor
description: Audit and harden GitHub Actions workflow files for runtime deprecation warnings, unsafe action refs, missing permissions, branch-trigger drift, and accidental PR job skips. Use when writing or reviewing `.github/workflows/*.yml`, when CI reports Node runtime deprecation warnings, or before changing action versions/triggers.
---

# Workflow Warning Auditor

## Overview

Audit workflow YAML deterministically, classify warnings by risk, and apply
minimal version/configuration changes only after checking the action's current
official release metadata. Keep warnings separate from failures: a runner
warning may be harmless today but still needs a planned upgrade before the
runtime removal deadline.

## Workflow

1. Resolve the repository root and enumerate `.github/workflows/*.yml`. Do not
   inspect generated artifacts or copied workflow snippets as live workflows.

2. Run the bundled audit from the repository root:

   ```bash
   python .agents/skills/workflow-warning-auditor/scripts/audit_workflows.py \
     --repo . --json
   ```

3. Classify each finding:

   - `node-runtime`: action major still targets a deprecated Node runtime;
     verify the current major in the action's release notes before changing it.
   - `mutable-ref`: an action uses `@main`, `@master`, or another mutable ref;
     prefer a reviewed major or full commit SHA for security-sensitive jobs.
   - `permissions`: workflow has no explicit least-privilege permissions.
   - `trigger-branch`: a live trigger still names an obsolete default branch.
   - `pr-skip`: a platform job is intentionally skipped on pull requests;
     ensure the release/main push matrix covers it and document the tradeoff.
   - `dangerous-trigger`: `pull_request_target` needs an explicit security
     review before any change.

4. For `node-runtime`, consult the action's official repository release notes
   or metadata; never guess a major from a warning alone. Update the workflow,
   then run the audit again and inspect the diff.

5. Validate after edits:

   ```bash
   python .agents/skills/workflow-warning-auditor/scripts/audit_workflows.py \
     --repo . --strict
   git diff --check
   ```

   Also run the repository's workflow contract tests and `actionlint` when
   available. Treat external-provider checks as report-only.

## Guardrails

- Do not auto-upgrade every action: major versions can change inputs,
  permissions, artifact behavior, or runner requirements.
- Preserve intentional platform scheduling, but make skipped jobs explicit in
  the workflow and handoff records.
- Use least-privilege `permissions`; never add a broad write token to silence a
  permission failure.
- Record every workflow change in the project's required audit records before
  committing.

## Resources

### scripts/

`audit_workflows.py` parses live workflow YAML and emits stable text or JSON
findings. It exits non-zero only with `--strict` when warnings exist.

### references/

Read `action-runtime-baseline.md` when deciding whether a warning is a known
runtime migration or requires checking a newer action release first.
