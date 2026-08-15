# Rust Inner Check Loop Design

Date: 2026-08-15
Status: Approved

## Goal

Reduce accumulated Rust compiler errors during AI-assisted development by requiring a fast compile-feedback loop after each small logical Rust change, without making the existing commit, CI, or release gates heavier.

## Scope

- Update agent-facing instructions for Codex/Claude so Rust edits use an inner `cargo check` loop.
- Add a small helper script only if it fits the repository's existing script conventions and reduces command drift.
- Keep the existing format, Clippy, test, record-keeping, CI, and shipping gates authoritative and unchanged in purpose.
- Do not refactor application code as part of this change.

## Development Loop

After completing one small logical Rust change, the agent runs:

```sh
cargo check --workspace --no-default-features
```

If the command fails, feature work stops. The agent reads the complete compiler diagnostics, identifies the root cause, and makes the smallest targeted correction before continuing. It must not stack speculative fixes or continue implementing unrelated parts of the feature while the workspace does not type-check.

Once `cargo check` passes, the agent may continue to the next logical change.

## Compiler Error Policy

Errors that commonly indicate API or type-contract mistakes, including `E0308`, `E0277`, `E0599`, and `E0061`, trigger explicit signature verification before a fix is chosen.

The agent should distinguish between:

1. a local type/data-flow error in project code; and
2. an incorrect assumption about an external crate API.

For external APIs, the fix must be based on the dependency version actually resolved by the project rather than remembered or generic examples.

## Dependency/API Verification

Extra care applies to the project's boundary-heavy dependencies and APIs, including:

- Tauri
- `windows-sys`
- `winit`
- `tray-icon`
- WASAPI and COM interfaces
- other external crates whose signatures are involved in the compiler error

Before changing code to satisfy one of these APIs, the agent verifies the relevant version and function/type signature using project dependency metadata and an authoritative local or upstream source appropriate to the development environment.

`Cargo.lock` remains the reproducibility source for resolved dependency versions and should not be broadly updated as a side effect of fixing a compiler error.

## Existing Quality Gates

The inner check does not replace the existing completion gates. Before commit/PR/shipping, the repository's existing authoritative checks still apply, including formatting, diff hygiene, Clippy with warnings denied, workspace tests, record-keeping guards, and the shipping workflow.

The intent is layered feedback:

- `cargo check`: fast feedback while coding;
- Clippy/tests: completion validation;
- CI/release gates: integration and platform validation.

## Agent Instruction Placement

The rule should live in the shared agent guidance so all coding agents see the same contract. Tool-specific guidance may reference the shared rule instead of maintaining divergent copies. Existing hard guardrails remain the source of truth for commit and shipping requirements.

The wording should make `cargo check` mandatory after a logical Rust change but should not require it after documentation-only, frontend-only, or other changes that do not affect Rust compilation.

## Helper Script

A helper script is optional. Add one only if it matches existing repository conventions and provides a stable single command for the inner check. It must remain intentionally lightweight and must not silently expand into the full test/release gate.

If added, the script should fail fast and transparently run the equivalent of:

```sh
cargo check --workspace --no-default-features
```

## Error Handling

When the inner check fails:

1. stop additional feature implementation;
2. inspect the complete compiler diagnostic and source location;
3. trace the mismatched value/signature to its origin;
4. verify external API contracts when applicable;
5. make one minimal correction;
6. rerun the inner check;
7. continue only after it passes.

Repeated speculative fixes are explicitly discouraged. If multiple attempted fixes expose unrelated failures, the agent should reassess the underlying assumption or architecture rather than layering more changes.

## Testing

Implementation verification should demonstrate that:

- agent documentation contains the new inner-loop requirement;
- any helper script runs the intended `cargo check` command and propagates failures;
- existing format/lint/test/records/ship rules remain intact;
- documentation-only workflows are not burdened with Rust checks;
- no application behavior changes are introduced.

## Success Criteria

The change is successful when an AI coding agent working on Rust is instructed to detect type/API errors after each small logical change, compiler failures block further feature work until resolved, external API fixes are version-aware, and the repository's existing final quality gates remain unchanged.