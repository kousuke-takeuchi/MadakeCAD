#!/usr/bin/env python3
"""Scan the working tree and the whole git history for confidential terms (M6 spec §2).

The term list itself is confidential, so it lives outside git:
  docs/references/confidential-terms.txt   (one term per line; `#` comments; case-insensitive)

Usage:
  python3 scripts/audit_confidential.py            # working tree + history
  python3 scripts/audit_confidential.py --tree     # working tree only
  python3 scripts/audit_confidential.py --terms path/to/terms.txt

Exit status 1 when any term is found. Run this before publishing the repository.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_TERMS = ROOT / "docs" / "references" / "confidential-terms.txt"


def load_terms(path: Path) -> list[str]:
    if not path.exists():
        sys.exit(f"term list not found: {path} (create it locally; it is git-ignored)")
    terms = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            terms.append(line)
    if not terms:
        sys.exit("term list is empty")
    return terms


def git_grep(term: str, history: bool) -> list[str]:
    hits: list[str] = []
    tree = subprocess.run(["git", "grep", "-I", "-i", "-n", "--", term], cwd=ROOT, capture_output=True, text=True)
    hits += [f"tree: {line}" for line in tree.stdout.splitlines()]
    if history:
        log = subprocess.run(
            ["git", "log", "--all", "-i", f"-S{term}", "--format=%h %s"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        hits += [f"history: {line}" for line in log.stdout.splitlines()]
    return hits


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--terms", type=Path, default=DEFAULT_TERMS)
    parser.add_argument("--tree", action="store_true", help="working tree only (skip history)")
    args = parser.parse_args()
    found = 0
    for term in load_terms(args.terms):
        hits = git_grep(term, history=not args.tree)
        if hits:
            found += len(hits)
            print(f"== {term!r}: {len(hits)} hit(s)")
            for hit in hits[:20]:
                print("   ", hit)
    if found:
        print(f"\n{found} hit(s). Remove them (history hits need a fresh export, not a rewrite — see M6 spec §2).")
        return 1
    print("no confidential terms found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
