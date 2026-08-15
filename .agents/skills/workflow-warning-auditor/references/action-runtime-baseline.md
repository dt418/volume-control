# GitHub Actions runtime baseline

Action runtime warnings refer to the Node runtime embedded in the action,
not the Node version installed by `actions/setup-node`. Confirm the current
major from the action's official release notes before editing a workflow.

Known migration baseline used by the bundled audit:

- `actions/setup-node@v5+` — Node 24 line.
- `actions/setup-python@v6+` — Node 24 line.

The audit deliberately does not guess baselines for every action. When an
action emits a runtime warning but is not in this table, inspect its official
metadata/release notes and add a narrowly scoped rule only when the evidence is
stable.
