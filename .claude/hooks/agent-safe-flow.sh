#!/usr/bin/env bash
# Claude Code PreToolUse guard for the repository's safe delivery flow.
#
# The hook protects the high-risk edges that must stay behind the tested
# scripts/ship.sh + pull-request + required Release gate path. It is a local
# agent guard, not a replacement for GitHub branch protection or CI.

set -u

input="$(cat)"
tool_command=""

# Prefer jq, but keep the hook usable on a clean Windows Git Bash/macOS/Linux
# install where jq may be absent. Node ships with the frontend toolchain and is
# the first fallback; Python is the final fallback.
if command -v jq >/dev/null 2>&1; then
    tool_command="$(printf '%s' "$input" | jq -er '.tool_input.command // empty' 2>/dev/null || true)"
elif command -v node >/dev/null 2>&1; then
    tool_command="$(printf '%s' "$input" | node -e '
      const fs = require("node:fs");
      try {
        const value = JSON.parse(fs.readFileSync(0, "utf8"));
        const command = value?.tool_input?.command;
        if (typeof command !== "string" || command.length === 0) process.exit(1);
        process.stdout.write(command);
      } catch (_) { process.exit(1); }
    ' 2>/dev/null || true)"
else
    python_bin=""
    for candidate in python3 python python.exe; do
        if command -v "$candidate" >/dev/null 2>&1; then
            python_bin="$candidate"
            break
        fi
    done
    if [ -n "$python_bin" ]; then
        tool_command="$(printf '%s' "$input" | "$python_bin" -c 'import json,sys; value=json.load(sys.stdin); command=value.get("tool_input",{}).get("command"); assert isinstance(command,str) and command; print(command,end="")' 2>/dev/null || true)"
    fi
fi

if [ -z "$tool_command" ]; then
    echo "BLOCKED: the agent safe-flow hook received malformed tool input." >&2
    exit 2
fi

# Git accepts several global options before the subcommand. Match those
# explicitly so `git -C repo push` is blocked without treating a ref named
# `push` in `git fetch origin push` or `git show --stat push` as a push.
git_global_opts='([[:space:]]+((-[pP]|--[a-z][a-z-]*(=[^;&|[:space:]]+)?|--html-path|--man-path|--info-path)|(-C|-c|--git-dir|--work-tree|--namespace|--exec-path|--super-prefix)(=[^;&|[:space:]]+|[[:space:]]+[^;&|[:space:]]+)))*'
git_prefix="(^|[;&|[:space:]])git([.]exe)?${git_global_opts}[[:space:]]+"

# Keep the matcher conservative: shell operators and whitespace are accepted
# before a command name so a destructive command cannot hide in a compound
# Bash invocation. Commands run inside scripts/ship.sh are not re-evaluated by
# Claude's PreToolUse hook, which lets the canonical ship flow push normally.
if printf '%s' "$tool_command" | grep -Eq "${git_prefix}push([[:space:]]|$)"; then
    echo "BLOCKED: direct git push is not part of the safe flow; run scripts/ship.sh --push after its full gate passes." >&2
    exit 2
fi

if printf '%s' "$tool_command" | grep -Eq "${git_prefix}reset[^;&|]*--hard([[:space:]]|$)|${git_prefix}clean[^;&|]*(-[^;&|[:space:]]*f[^;&|[:space:]]*|--force)([[:space:]]|$)|${git_prefix}branch[^;&|]*[[:space:]]-D([[:space:]]|$)|${git_prefix}restore([[:space:]]|$)|${git_prefix}checkout[^;&|]*([[:space:]]--([[:space:]]|$)|[[:space:]]-f([[:space:]]|$)|[[:space:]]\.($|[[:space:]]))"; then
    echo "BLOCKED: destructive git operation detected; preserve the worktree and use a targeted, reviewable change." >&2
    exit 2
fi

# The legacy `git checkout <file>` form is destructive only when the target is
# an existing worktree path. Check that concrete case without overblocking
# dotted branch/tag names such as `release/1.2.3`.
if printf '%s' "$tool_command" | grep -Eq "${git_prefix}checkout[[:space:]]+[^;&|[:space:]]+([[:space:]]|$)"; then
    checkout_target="$(printf '%s' "$tool_command" | sed -nE 's/^[^;&|]*[[:space:]]checkout[[:space:]]+([^;&|[:space:]]+).*$/\1/p')"
    checkout_base="."
    checkout_c_re='(^|[;&|[:space:]])git([.]exe)?[^;&|]*-C[=[:space:]]*([^;&|[:space:]]+)[^;&|]*checkout[[:space:]]+'
    if [[ "$tool_command" =~ $checkout_c_re ]]; then
        checkout_base="${BASH_REMATCH[3]}"
    fi
    checkout_path="$checkout_target"
    if [ -n "$checkout_base" ] && [ "$checkout_base" != "." ] && [[ "$checkout_target" != /* ]]; then
        checkout_path="$checkout_base/$checkout_target"
    fi
    if [ -n "$checkout_target" ] && [ -e "$checkout_path" ]; then
        echo "BLOCKED: destructive git operation detected; preserve the worktree and use a targeted, reviewable change." >&2
        exit 2
    fi
fi

if printf '%s' "$tool_command" | grep -Eq "${git_prefix}commit[^;&|]*([[:space:]]--no-verify|[[:space:]]--no-gpg-sign|[[:space:]]-n)([[:space:]]|$)|(^|[;&|[:space:]])gh([.]exe)?[^;&|]*[[:space:]]pr[[:space:]]+merge[^;&|]*[[:space:]]--admin([[:space:]]|$)"; then
    echo "BLOCKED: the command bypasses a required safety/review gate." >&2
    exit 2
fi

exit 0
