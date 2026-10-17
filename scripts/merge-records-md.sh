#!/usr/bin/env bash
# Git merge driver for the markdown record logs (claude-progress.md,
# session-handoff.md).
#
# Every branch prepends or appends its own "## Session N" block, and the two
# sides usually touch the same file at the top, so a textual merge conflicts
# even though the edit is purely additive. This driver merges by heading:
# blocks are keyed by their heading line, ours keep their order, a heading
# present on both sides keeps one heading plus the union of its lines, and
# headings only present on theirs are appended.
#
# Git invokes it as: merge-records-md.sh %O %A %B
#   %O = merge base, %A = ours (the result is written here), %B = theirs.
# Exit 0 = resolved; any other status leaves the conflict for the operator.
set -eu

if [ "$#" -ne 3 ]; then
  echo "usage: merge-records-md.sh BASE OURS THEIRS" >&2
  exit 2
fi

python3 - "$1" "$2" "$3" <<'PY'
import re
import sys

base_path, ours_path, theirs_path = sys.argv[1:4]
HEADING = re.compile(r"^#{1,2} ")


def load(path):
    with open(path, encoding="utf-8") as handle:
        return handle.read()


def split_blocks(text):
    """Split into (preamble_lines, [(heading, lines), ...]) preserving order."""
    preamble = []
    blocks = []
    current = None
    for line in text.splitlines():
        if HEADING.match(line):
            current = (line.rstrip(), [])
            blocks.append(current)
        elif current is None:
            preamble.append(line)
        else:
            current[1].append(line)
    return preamble, blocks


def union_lines(ours, theirs):
    merged = list(ours)
    seen = set(merged)
    for line in theirs:
        if line not in seen:
            merged.append(line)
            seen.add(line)
    return merged


def main():
    ours_text = load(ours_path)
    theirs_text = load(theirs_path)
    if "<<<<<<<" in ours_text or "<<<<<<<" in theirs_text:
        print("merge-records-md: input already contains conflict markers", file=sys.stderr)
        sys.exit(1)

    ours_pre, ours_blocks = split_blocks(ours_text)
    theirs_pre, theirs_blocks = split_blocks(theirs_text)

    theirs_by_heading = {}
    for heading, lines in theirs_blocks:
        theirs_by_heading.setdefault(heading, []).append(lines)

    out = union_lines(ours_pre, theirs_pre)
    seen_headings = set()

    def emit(lines):
        while out and out[-1] == "":
            out.pop()
        if out:
            out.append("")
        out.extend(lines)

    for heading, lines in ours_blocks:
        pending = theirs_by_heading.pop(heading, [])
        merged_lines = union_lines(lines, [line for block in pending for line in block])
        seen_headings.add(heading)
        emit([heading, *merged_lines])
    for heading, lines in theirs_blocks:
        if heading in seen_headings:
            continue
        if heading not in theirs_by_heading:
            continue
        theirs_by_heading.pop(heading)
        emit([heading, *lines])

    text = "\n".join(out).rstrip("\n") + "\n"
    with open(ours_path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
    print(f"merge-records-md: merged {len(ours_blocks)} + {len(theirs_blocks)} blocks", file=sys.stderr)


main()
PY
