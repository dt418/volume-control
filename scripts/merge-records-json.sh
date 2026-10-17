#!/usr/bin/env bash
# Git merge driver for feature_list.json.
#
# The record file is edited by every task on every branch, so a plain textual
# merge conflicts on the same evidence arrays, the same feature ids, and the
# shared last_updated scalar. This driver merges it semantically: features are
# keyed by id, string lists (evidence, verification) are unioned, and scalar
# changes are resolved against the merge base so the side that actually
# changed a field wins.
#
# Git invokes it as: merge-records-json.sh %O %A %B
#   %O = merge base, %A = ours (the result is written here), %B = theirs.
# Exit 0 = resolved; any other status leaves the conflict for the operator.
set -eu

if [ "$#" -ne 3 ]; then
  echo "usage: merge-records-json.sh BASE OURS THEIRS" >&2
  exit 2
fi

python3 - "$1" "$2" "$3" <<'PY'
import json
import sys

base_path, ours_path, theirs_path = sys.argv[1:4]


def load(path):
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)


def merge_scalar(ours, theirs, base, label):
    """Resolve one scalar: the side that changed from the base wins."""
    if ours == theirs:
        return ours
    ours_changed = ours != base
    theirs_changed = theirs != base
    if ours_changed and not theirs_changed:
        return ours
    if theirs_changed and not ours_changed:
        return theirs
    if ours_changed and theirs_changed:
        print(f"merge-records-json: {label}: both sides changed, kept ours", file=sys.stderr)
    return ours


def merge_list(ours, theirs):
    """Union two lists, keeping ours first and dropping duplicates."""
    merged = list(ours)
    for item in theirs:
        if item not in merged:
            merged.append(item)
    return merged


STATUS_RANK = {"not_started": 0, "blocked": 1, "in_progress": 1, "passing": 2}


def merge_status(ours, theirs, base):
    if ours == theirs:
        return ours
    ours_changed = ours != base
    theirs_changed = theirs != base
    if ours_changed and not theirs_changed:
        return ours
    if theirs_changed and not ours_changed:
        return theirs
    ours_rank = STATUS_RANK.get(ours, -1)
    theirs_rank = STATUS_RANK.get(theirs, -1)
    if theirs_rank > ours_rank:
        return theirs
    if ours_rank > theirs_rank:
        return ours
    print(f"merge-records-json: status: {ours} vs {theirs}, kept ours", file=sys.stderr)
    return ours


def merge_notes(ours, theirs, base):
    if ours == theirs:
        return ours
    if not ours:
        return theirs
    if not theirs:
        return ours
    ours_changed = ours != base
    theirs_changed = theirs != base
    if theirs_changed and not ours_changed:
        return theirs
    if ours_changed and not theirs_changed:
        return ours
    print("merge-records-json: notes: both sides changed, kept ours", file=sys.stderr)
    return ours


def merge_feature(ours, theirs, base):
    merged = {}
    for key in list(ours) + [k for k in theirs if k not in ours]:
        if key not in theirs:
            merged[key] = ours[key]
            continue
        if key not in ours:
            merged[key] = theirs[key]
            continue
        ours_value, theirs_value = ours[key], theirs[key]
        base_value = base.get(key) if isinstance(base, dict) else None
        if key == "last_updated":
            # The stamp records when the file last changed, so the newest value
            # is the true one regardless of which side wrote it.
            merged[key] = max(str(ours_value), str(theirs_value))
        elif key == "status":
            merged[key] = merge_status(ours_value, theirs_value, base_value)
        elif key == "notes":
            merged[key] = merge_notes(ours_value, theirs_value, base_value)
        elif isinstance(ours_value, list) and isinstance(theirs_value, list):
            merged[key] = merge_list(ours_value, theirs_value)
        elif isinstance(ours_value, dict) and isinstance(theirs_value, dict):
            merged[key] = merge_mapping(ours_value, theirs_value, base_value)
        else:
            merged[key] = merge_scalar(ours_value, theirs_value, base_value, f"{ours.get('id', '?')}.{key}")
    return merged


def merge_mapping(ours, theirs, base):
    if not isinstance(base, dict):
        base = {}
    merged = {}
    for key in list(ours) + [k for k in theirs if k not in ours]:
        if key not in theirs:
            merged[key] = ours[key]
        elif key not in ours:
            merged[key] = theirs[key]
        elif isinstance(ours[key], list) and isinstance(theirs[key], list):
            merged[key] = merge_list(ours[key], theirs[key])
        elif key == "last_updated":
            merged[key] = max(str(ours[key]), str(theirs[key]))
        elif isinstance(ours[key], dict) and isinstance(theirs[key], dict):
            merged[key] = merge_mapping(ours[key], theirs[key], base.get(key))
        else:
            merged[key] = merge_scalar(ours[key], theirs[key], base.get(key), key)
    return merged


def merge_features(ours, theirs, base):
    base_by_id = {}
    for feature in base:
        base_by_id[feature.get("id")] = feature
    theirs_by_id = {}
    for feature in theirs:
        theirs_by_id.setdefault(feature.get("id"), feature)
    merged = []
    for feature in ours:
        feature_id = feature.get("id")
        if feature_id in theirs_by_id:
            merged.append(merge_feature(feature, theirs_by_id[feature_id], base_by_id.get(feature_id, {})))
        else:
            merged.append(feature)
    ours_ids = {feature.get("id") for feature in ours}
    for feature in theirs:
        if feature.get("id") not in ours_ids:
            merged.append(feature)
    return merged


def main():
    ours = load(ours_path)
    theirs = load(theirs_path)
    try:
        base = load(base_path)
    except (FileNotFoundError, ValueError):
        base = {}

    merged = merge_mapping(ours, theirs, base)
    ours_features = ours.get("features", [])
    theirs_features = theirs.get("features", [])
    if isinstance(ours_features, list) and isinstance(theirs_features, list):
        base_features = base.get("features", []) if isinstance(base, dict) else []
        merged["features"] = merge_features(ours_features, theirs_features, base_features)

    with open(ours_path, "w", encoding="utf-8", newline="\n") as handle:
        json.dump(merged, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    print(
        f"merge-records-json: merged {len(merged.get('features', []))} features "
        f"(ours {len(ours_features)}, theirs {len(theirs_features)})",
        file=sys.stderr,
    )


main()
PY
