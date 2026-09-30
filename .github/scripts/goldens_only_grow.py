"""Fails if a recorded golden case changed or disappeared since a base commit.

Each case in tests/golden/wire_cases.json records an input and what the fork's serde did with
it. Those two fields are the wire-format contract: every version of this crate must keep
reading them the same way, so a new version never breaks SDKs or TEEs on an older one. New cases
may be added. Existing ones never change or go away.

Usage: goldens_only_grow.py <base commit>
"""

import json
import subprocess
import sys

GOLDEN_FILE = "tests/golden/wire_cases.json"
CONTRACT_FIELDS = ("input", "fork_output")


def cases_at(revision: str) -> dict:
    """The golden cases at `revision`, by name. Empty if the file did not exist there."""
    shown = subprocess.run(
        ["git", "show", f"{revision}:{GOLDEN_FILE}"], capture_output=True, text=True
    )
    if shown.returncode != 0:
        return {}
    return {case["name"]: case for case in json.loads(shown.stdout)}


def current_cases() -> dict:
    with open(GOLDEN_FILE, encoding="utf-8") as file:
        return {case["name"]: case for case in json.load(file)}


def broken_cases(before: dict, after: dict) -> list[str]:
    problems = []
    for name, case in before.items():
        if name not in after:
            problems.append(f"{name}: removed")
            continue
        for field in CONTRACT_FIELDS:
            if after[name][field] != case[field]:
                problems.append(f"{name}: `{field}` changed")
    return problems


def main(base: str) -> int:
    # A push that creates a branch has no "before" commit (all zeros).
    if not base.strip("0"):
        print("No base commit to compare against.")
        return 0
    before, after = cases_at(base), current_cases()
    problems = broken_cases(before, after)
    for problem in problems:
        print(f"::error file={GOLDEN_FILE}::{problem}")
    added = len(after.keys() - before.keys())
    print(f"{len(before)} recorded cases checked against {base[:12]}, {added} added.")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else ""))
