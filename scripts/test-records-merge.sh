#!/usr/bin/env bash
# Tests for the record-file merge drivers (scripts/merge-records-json.sh and
# scripts/merge-records-md.sh).
#
# The drivers exist so that additive record edits on parallel branches merge
# without conflict markers. The suite drives the drivers directly on crafted
# inputs and then proves the whole thing end to end: a throwaway repository
# with .gitattributes plus the registered drivers must complete `git merge`
# with exit 0 and both sides' records intact.
#
# Usage: bash scripts/test-records-merge.sh
set -uo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
json_driver="$root/scripts/merge-records-json.sh"
md_driver="$root/scripts/merge-records-md.sh"
failures=0

work="$(mktemp -d "${TMPDIR:-/tmp}/records-merge.XXXXXX")"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

report() { # status description
  if [ "$1" = ok ]; then
    echo "ok   - $2"
  else
    echo "FAIL - $2"
    failures=$((failures + 1))
  fi
}

# --- JSON driver ----------------------------------------------------------

cat > "$work/base.json" <<'JSON'
{
  "last_updated": "2026-10-17T10:00",
  "features": [
    {
      "id": "vol-100",
      "title": "shared feature",
      "status": "in_progress",
      "priority": 100,
      "evidence": ["base evidence"],
      "notes": "base note"
    }
  ]
}
JSON

cat > "$work/ours.json" <<'JSON'
{
  "last_updated": "2026-10-17T11:00",
  "features": [
    {
      "id": "vol-100",
      "title": "shared feature",
      "status": "passing",
      "priority": 100,
      "evidence": ["base evidence", "ours evidence"],
      "notes": "ours note"
    },
    {
      "id": "vol-101",
      "title": "ours only",
      "status": "in_progress",
      "priority": 101,
      "evidence": ["ours 101"],
      "notes": ""
    }
  ]
}
JSON

cat > "$work/theirs.json" <<'JSON'
{
  "last_updated": "2026-10-17T12:00",
  "features": [
    {
      "id": "vol-100",
      "title": "shared feature",
      "status": "in_progress",
      "priority": 100,
      "evidence": ["base evidence", "theirs evidence"],
      "notes": "theirs note"
    },
    {
      "id": "vol-102",
      "title": "theirs only",
      "status": "passing",
      "priority": 102,
      "evidence": ["theirs 102"],
      "notes": ""
    }
  ]
}
JSON

cp "$work/ours.json" "$work/merged.json"
bash "$json_driver" "$work/base.json" "$work/merged.json" "$work/theirs.json" 2>"$work/json.log"

if python3 - "$work/merged.json" <<'PY'
import json, sys
data = json.load(open(sys.argv[1], encoding="utf-8"))
features = {feature["id"]: feature for feature in data["features"]}
assert [feature["id"] for feature in data["features"]] == ["vol-100", "vol-101", "vol-102"], data["features"]
assert features["vol-100"]["evidence"] == ["base evidence", "ours evidence", "theirs evidence"], features["vol-100"]
assert features["vol-100"]["status"] == "passing", features["vol-100"]
assert features["vol-100"]["notes"] == "ours note", features["vol-100"]
assert data["last_updated"] == "2026-10-17T12:00", data["last_updated"]
assert features["vol-101"]["title"] == "ours only" and features["vol-102"]["title"] == "theirs only"
PY
then
  report ok 'json driver unions evidence, keeps both new features and the newer timestamp'
else
  report FAIL 'json driver unions evidence, keeps both new features and the newer timestamp'
fi

if [[ ! -s "$work/json.log" ]] || ! grep -q "merged 3 features" "$work/json.log"; then
  report FAIL 'json driver reports the merge summary on stderr'
else
  report ok 'json driver reports the merge summary on stderr'
fi

# Running the driver on its own output must be byte-stable, so repeated merges
# never churn the record file.
cp "$work/merged.json" "$work/stable.json"
before="$(sha256sum "$work/stable.json" | cut -d' ' -f1)"
bash "$json_driver" "$work/base.json" "$work/stable.json" "$work/merged.json" >/dev/null 2>&1
after="$(sha256sum "$work/stable.json" | cut -d' ' -f1)"
if [ "$before" = "$after" ]; then
  report ok 'json driver output is byte-stable when re-merged'
else
  report FAIL 'json driver output is byte-stable when re-merged'
fi

# Malformed input must fail so git keeps the conflict instead of writing junk.
printf 'not json\n' > "$work/broken.json"
cp "$work/ours.json" "$work/kept.json"
if bash "$json_driver" "$work/base.json" "$work/kept.json" "$work/broken.json" >/dev/null 2>&1; then
  report FAIL 'json driver refuses malformed input'
else
  report ok 'json driver refuses malformed input'
fi

# --- markdown driver ------------------------------------------------------

cat > "$work/log-ours.md" <<'MD'
# Progress Log

## Session 200 - ours

- ours bullet one
- shared bullet
MD

cat > "$work/log-theirs.md" <<'MD'
# Progress Log

## Session 201 - theirs

- theirs bullet one

## Session 200 - ours

- ours bullet one
- shared bullet
- theirs bullet two
MD

cp "$work/log-ours.md" "$work/log-merged.md"
bash "$md_driver" "$work/log-ours.md" "$work/log-merged.md" "$work/log-theirs.md" 2>"$work/md.log"

if python3 - "$work/log-merged.md" <<'PY'
import sys
text = open(sys.argv[1], encoding="utf-8").read()
assert "<<<<<<<" not in text and ">>>>>>>" not in text, text
assert text.index("## Session 200 - ours") < text.index("## Session 201 - theirs"), text
assert text.count("## Session 200 - ours") == 1, text
assert text.count("## Session 201 - theirs") == 1, text
assert text.count("ours bullet one") == 1, text
assert "theirs bullet two" in text, text
PY
then
  report ok 'markdown driver keeps both session blocks and unions shared ones'
else
  report FAIL 'markdown driver keeps both session blocks and unions shared ones'
fi

# Refuse to merge already-conflicted input: the operator must resolve it first.
printf '<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> other\n' > "$work/log-conflicted.md"
cp "$work/log-ours.md" "$work/log-kept.md"
if bash "$md_driver" "$work/log-ours.md" "$work/log-kept.md" "$work/log-conflicted.md" >/dev/null 2>&1; then
  report FAIL 'markdown driver refuses input that already has conflict markers'
else
  report ok 'markdown driver refuses input that already has conflict markers'
fi

# --- end-to-end through git ----------------------------------------------

e2e="$work/e2e"
mkdir -p "$e2e/scripts"
cp "$json_driver" "$md_driver" "$root/scripts/install-merge-drivers.sh" "$e2e/scripts/"
(
  cd "$e2e" || exit 1
  git init --quiet -b main
  git config user.email "records@example.invalid"
  git config user.name "records test"
  cp "$root/.gitattributes" .gitattributes
  cat > claude-progress.md <<'MD'
# Progress Log

## Session 1

- base bullet
MD
  cat > feature_list.json <<'JSON'
{
  "last_updated": "2026-10-17T10:00",
  "features": [
    {
      "id": "vol-100",
      "status": "in_progress",
      "evidence": ["base evidence"],
      "notes": "base note"
    }
  ]
}
JSON
  git add -A
  git commit --quiet -m base
  git checkout --quiet -b topic

  cat > claude-progress.md <<'MD'
# Progress Log

## Session 2 - topic

- topic bullet
MD
  python3 - <<'PY'
import json
data = json.load(open("feature_list.json", encoding="utf-8"))
data["last_updated"] = "2026-10-17T11:00"
data["features"][0]["evidence"].append("topic evidence")
json.dump(data, open("feature_list.json", "w", encoding="utf-8"), indent=2)
PY
  git add -A
  git commit --quiet -m "topic records"

  git checkout --quiet main
  cat > claude-progress.md <<'MD'
# Progress Log

## Session 3 - main

- main bullet
MD
  python3 - <<'PY'
import json
data = json.load(open("feature_list.json", encoding="utf-8"))
data["last_updated"] = "2026-10-17T12:00"
data["features"][0]["evidence"].append("main evidence")
data["features"][0]["status"] = "passing"
json.dump(data, open("feature_list.json", "w", encoding="utf-8"), indent=2)
PY
  git add -A
  git commit --quiet -m "main records"

  git checkout --quiet topic
  # Register the drivers the way a clone does (config plus .git/info/attributes)
  # instead of passing -c flags, so the documented flow is what gets tested.
  bash scripts/install-merge-drivers.sh > /dev/null
  git merge --no-edit main > "$work/e2e.log" 2>&1
  echo "merge-exit=$?" >> "$work/e2e.log"
)

if grep -q "merge-exit=0" "$work/e2e.log"; then
  report ok 'git merge with the drivers completes without conflicts'
else
  report FAIL 'git merge with the drivers completes without conflicts'
  sed -n '1,20p' "$work/e2e.log"
fi

if python3 - "$e2e" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
progress = (root / "claude-progress.md").read_text(encoding="utf-8")
assert "<<<<<<<" not in progress, progress
for needle in ("## Session 2 - topic", "- topic bullet", "## Session 3 - main", "- main bullet"):
    assert needle in progress, (needle, progress)
data = json.loads((root / "feature_list.json").read_text(encoding="utf-8"))
evidence = data["features"][0]["evidence"]
assert "topic evidence" in evidence and "main evidence" in evidence and "base evidence" in evidence, evidence
assert data["features"][0]["status"] == "passing", data
assert data["last_updated"] == "2026-10-17T12:00", data
PY
then
  report ok 'the merged checkout carries both sides of both record files'
else
  report FAIL 'the merged checkout carries both sides of both record files'
fi

# A branch that predates .gitattributes still merges conflict-free because the
# installer records the attributes in .git/info/attributes too.
if (cd "$e2e" && rm -f .gitattributes && [ "$(git check-attr merge -- claude-progress.md feature_list.json | grep -c 'merge: records')" -eq 2 ]); then
  report ok 'installer attributes cover branches that predate .gitattributes'
else
  report FAIL 'installer attributes cover branches that predate .gitattributes'
fi

if [ "$failures" -eq 0 ]; then
  echo 'All records merge-driver checks passed.'
  exit 0
fi
echo "$failures records merge-driver check(s) failed."
exit 1
