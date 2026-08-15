#!/usr/bin/env bash
# Contract tests for the project-scoped Claude Code agent safe-flow hook.

set -u

hook=".claude/hooks/agent-safe-flow.sh"
failures=0

report() {
    if [ "$1" = ok ]; then
        printf 'ok   - %s\n' "$2"
    else
        printf 'FAIL - %s%s\n' "$2" "${3:+ ($3)}"
        failures=$((failures + 1))
    fi
}

run_hook() {
    local payload="$1"
    hook_output_file="$(mktemp)"
    printf '%s' "$payload" | bash "$hook" >"$hook_output_file" 2>&1
    hook_rc=$?
}

json_payload() {
    local escaped
    escaped="$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g')"
    printf '{"tool_input":{"command":"%s"}}' "$escaped"
}

expect_blocked() {
    local label="$1" command="$2" rc output
    run_hook "$(json_payload "$command")"
    rc="$hook_rc"
    output="$(cat "$hook_output_file")"
    rm -f "$hook_output_file"
    if [ "$rc" -eq 2 ] && grep -q 'BLOCKED:' <<<"$output"; then
        report ok "$label"
    else
        report FAIL "$label" "expected exit 2, got $rc: $output"
    fi
}

expect_allowed() {
    local label="$1" command="$2" rc output
    run_hook "$(json_payload "$command")"
    rc="$hook_rc"
    output="$(cat "$hook_output_file")"
    rm -f "$hook_output_file"
    if [ "$rc" -eq 0 ]; then
        report ok "$label"
    else
        report FAIL "$label" "expected exit 0, got $rc: $output"
    fi
}

expect_malformed_blocked() {
    local result rc output
    run_hook '{}'
    rc="$hook_rc"
    output="$(cat "$hook_output_file")"
    rm -f "$hook_output_file"
    if [ "$rc" -eq 2 ] && grep -q 'BLOCKED:' <<<"$output"; then
        report ok 'malformed hook input is blocked'
    else
        report FAIL 'malformed hook input is blocked' "expected exit 2, got $rc: $output"
    fi
}

if [ ! -x "$hook" ]; then
    report FAIL 'agent safe-flow hook is executable' 'missing executable bit'
else
    report ok 'agent safe-flow hook is executable'
fi

expect_blocked 'direct git push is blocked' 'git push origin feature/test'
expect_blocked 'forced push is blocked' 'git push --force-with-lease origin feature/test'
expect_blocked 'git global-option push is blocked' 'git -C repo push origin main'
expect_blocked 'short pager push is blocked' 'git -p push origin main'
expect_blocked 'hard reset is blocked' 'git reset --hard HEAD~1'
expect_blocked 'git global-option hard reset is blocked' 'git --no-pager reset --hard HEAD~1'
expect_blocked 'short no-pager hard reset is blocked' 'git -P reset --hard HEAD~1'
expect_blocked 'working-tree clean is blocked' 'git clean -fd'
expect_blocked 'separate clean force flag is blocked' 'git clean -d -f'
expect_blocked 'destructive branch delete is blocked' 'git branch -D stale-branch'
expect_blocked 'git global-option branch delete is blocked' 'git -c core.hooksPath=/tmp branch -D stale-branch'
expect_blocked 'config-env branch delete is blocked' 'git --config-env=foo=bar branch -D stale-branch'
expect_blocked 'restore path is blocked' 'git restore src/main.rs'
expect_blocked 'checkout path discard is blocked' 'git checkout -- src/main.rs'
expect_blocked 'legacy checkout of an existing file is blocked' 'git checkout CLAUDE.md'
expect_blocked 'global-option legacy checkout is blocked' 'git --no-pager checkout CLAUDE.md'
expect_blocked 'path-option legacy checkout is blocked' 'git -C . checkout CLAUDE.md'
expect_blocked 'redirected legacy checkout of an existing file is blocked' 'git -C scripts checkout check-records.sh'
expect_blocked 'no-verify commit is blocked' 'git commit --no-verify -m bypass'
expect_blocked 'short no-verify commit is blocked' 'git commit -n -m bypass'
expect_blocked 'admin merge bypass is blocked' 'gh pr merge 123 --admin'
expect_malformed_blocked

expect_allowed 'canonical ship push is allowed' 'bash scripts/ship.sh --push'
expect_allowed 'read-only git inspection is allowed' 'git status --short'
expect_allowed 'ref named push is not treated as a push' 'git fetch origin push'
expect_allowed 'show argument named push is not treated as a push' 'git show --stat push'
expect_allowed 'merged local branch deletion is allowed' 'git branch -d merged-branch'
expect_allowed 'normal branch checkout is allowed' 'git checkout feature/test'
expect_allowed 'dotted release branch checkout is allowed' 'git checkout release/1.2.3'
expect_allowed 'version tag checkout is allowed' 'git checkout v1.2.3'
expect_allowed 'normal PR merge remains branch-protection gated' 'gh pr merge 123 --merge --delete-branch'

if if command -v jq >/dev/null 2>&1; then
    jq empty .claude/settings.json >/dev/null 2>&1
elif command -v node >/dev/null 2>&1; then
    node -e "JSON.parse(require('fs').readFileSync('.claude/settings.json', 'utf8'))"
else
    python_bin=""
    for candidate in python3 python python.exe; do
        if command -v "$candidate" >/dev/null 2>&1; then
            python_bin="$candidate"
            break
        fi
    done
    [ -n "$python_bin" ] && "$python_bin" -c "import json; json.load(open('.claude/settings.json', encoding='utf-8'))"
fi; then
    report ok 'Claude settings JSON parses'
else
    report FAIL 'Claude settings JSON parses'
fi

settings_hook_ok=1
if command -v jq >/dev/null 2>&1; then
    jq -e '[.hooks.PreToolUse[]? | select(.matcher == "Bash") | .hooks[]? | select(.type == "command" and (.command | contains("agent-safe-flow.sh")))] | length == 1' .claude/settings.json >/dev/null 2>&1 || settings_hook_ok=0
elif command -v node >/dev/null 2>&1; then
    node -e "const s=JSON.parse(require('fs').readFileSync('.claude/settings.json','utf8')); const ok=(s.hooks?.PreToolUse??[]).some(e=>e.matcher==='Bash' && (e.hooks??[]).some(h=>h.type==='command' && h.command.includes('agent-safe-flow.sh'))); if(!ok) process.exit(1)" || settings_hook_ok=0
else
    python_bin=""
    for candidate in python3 python python.exe; do
        if command -v "$candidate" >/dev/null 2>&1; then
            python_bin="$candidate"
            break
        fi
    done
    if [ -n "$python_bin" ]; then
        "$python_bin" -c "import json,sys; s=json.load(open('.claude/settings.json', encoding='utf-8')); ok=any(e.get('matcher') == 'Bash' and any(h.get('type') == 'command' and 'agent-safe-flow.sh' in h.get('command','') for h in e.get('hooks',[])) for e in s.get('hooks',{}).get('PreToolUse',[])); sys.exit(0 if ok else 1)" || settings_hook_ok=0
    else
        settings_hook_ok=0
    fi
fi
if [ "$settings_hook_ok" -eq 1 ]; then
    report ok 'Claude settings wires the PreToolUse safe-flow hook'
else
    report FAIL 'Claude settings wires the PreToolUse safe-flow hook'
fi

if [ "$failures" -eq 0 ]; then
    echo 'All agent safe-flow checks passed.'
    exit 0
fi

echo "$failures agent safe-flow check(s) failed." >&2
exit 1
