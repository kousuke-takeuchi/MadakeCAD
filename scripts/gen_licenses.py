#!/usr/bin/env python3
"""Generate docs/internal/third-party-licenses.md from the Rust and npm dependency graphs.

Rust:  `cargo tree --workspace -e normal` over src-tauri (runtime/normal deps only).
npm:   `npm ls --all --json` plus the `license` field of each installed package.

Usage:  python3 scripts/gen_licenses.py [--check]

`--check` exits non-zero if the generated content differs from the committed file.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "internal" / "third-party-licenses.md"

# Strong copyleft: would force MadakeCAD's own source under the same terms. Must be zero.
STRONG_COPYLEFT = re.compile(r"\b(?:AGPL|LGPL|GPL|SSPL|EUPL|CPAL|OSL)\b", re.I)
# File-level ("weak") copyleft: obligations attach to the dependency's own files only.
WEAK_COPYLEFT = re.compile(r"\b(?:MPL|CDDL|EPL|Ms-PL)\b", re.I)
# Non-permissive / needs-a-look markers.
NON_PERMISSIVE = re.compile(r"\b(?:CC-BY-NC|CC-BY-SA|proprietary|UNLICENSED|custom)\b", re.I)


def normalize(lic: str) -> str:
    """Strip cargo-tree's `(*)` dedupe marker and unify `A/B` into `A OR B`."""
    lic = lic.replace("(*)", "").strip()
    if "/" in lic and " OR " not in lic and "(" not in lic:
        lic = " OR ".join(part.strip() for part in lic.split("/"))
    return lic or "(none declared)"


def run(args: list[str], cwd: Path) -> str:
    p = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    if p.returncode != 0 and not p.stdout:
        sys.exit(f"command failed: {' '.join(args)}\n{p.stderr}")
    return p.stdout


def rust_deps() -> list[tuple[str, str, str]]:
    """-> [(name, version, license)] for normal (non-dev, non-build) dependencies."""
    raw = run(
        ["cargo", "tree", "--workspace", "-e", "normal",
         "--prefix", "none", "--format", "{p}|{l}"],
        ROOT / "src-tauri",
    )
    workspace = {"madake-core", "madake-mcp", "madake-cli", "madake"}
    seen: dict[tuple[str, str], str] = {}
    for line in raw.splitlines():
        line = line.strip()
        if not line or "|" not in line:
            continue
        pkg, lic = line.rsplit("|", 1)
        pkg = pkg.replace(" (proc-macro)", "").strip()
        m = re.match(r"^(\S+)\s+v(\S+)", pkg)
        if not m:
            continue
        name, version = m.group(1), m.group(2)
        if name in workspace:
            continue
        seen[(name, version)] = normalize(lic or "")
    return sorted((n, v, l) for (n, v), l in seen.items())


def npm_deps() -> list[tuple[str, str, str, str]]:
    """-> [(name, version, license, scope)] where scope is 'prod' or 'dev'."""
    def tree(extra: list[str]) -> dict:
        raw = run(["npm", "ls", "--all", "--json"] + extra, ROOT)
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return {}

    def walk(node: dict, acc: dict) -> None:
        for name, info in (node.get("dependencies") or {}).items():
            ver = info.get("version") or ""
            if (name, ver) not in acc:
                acc[(name, ver)] = info.get("path") or ""
            walk(info, acc)

    prod: dict = {}
    walk(tree(["--omit=dev"]), prod)
    allp: dict = {}
    walk(tree([]), allp)

    def license_of(name: str, path: str) -> str:
        for cand in ([Path(path)] if path else []) + [ROOT / "node_modules" / name]:
            pj = cand / "package.json"
            if pj.is_file():
                try:
                    data = json.loads(pj.read_text(encoding="utf-8"))
                except (json.JSONDecodeError, UnicodeDecodeError):
                    continue
                lic = data.get("license")
                if isinstance(lic, dict):
                    return lic.get("type", "(none declared)")
                if isinstance(lic, list):
                    return " OR ".join(
                        x.get("type", "?") if isinstance(x, dict) else str(x) for x in lic
                    )
                if lic:
                    return str(lic)
                lics = data.get("licenses")
                if isinstance(lics, list) and lics:
                    return " OR ".join(
                        x.get("type", "?") if isinstance(x, dict) else str(x) for x in lics
                    )
        return "(none declared)"

    rows = []
    for (name, ver), path in sorted(allp.items()):
        installed = (Path(path) if path else ROOT / "node_modules" / name) / "package.json"
        if not ver or not installed.is_file():
            # Optional dependency for another platform (e.g. per-arch esbuild/rollup
            # binaries): listed in the tree but never installed or shipped here.
            continue
        rows.append((name, ver, normalize(license_of(name, path)),
                     "prod" if (name, ver) in prod else "dev"))
    return rows


def summarize(licenses: list[str]) -> list[tuple[str, int]]:
    return sorted(Counter(licenses).items(), key=lambda kv: (-kv[1], kv[0]))


def render() -> str:
    rust = rust_deps()
    npm = npm_deps()

    def bucket(lic: str) -> str | None:
        if STRONG_COPYLEFT.search(lic):
            return "strong copyleft"
        if NON_PERMISSIVE.search(lic):
            return "non-permissive"
        if WEAK_COPYLEFT.search(lic):
            return "file-level copyleft"
        return None

    rust_flagged = [("Rust", n, v, l, bucket(l)) for n, v, l in rust if bucket(l)]
    npm_flagged = [(f"npm ({s})", n, v, l, bucket(l)) for n, v, l, s in npm if bucket(l)]
    flagged = rust_flagged + npm_flagged
    strong = [r for r in flagged if r[4] in ("strong copyleft", "non-permissive")]

    out: list[str] = []
    w = out.append
    w("# Third-Party Licenses")
    w("")
    w("> Generated by `python3 scripts/gen_licenses.py`. Do not edit by hand.")
    w("")
    w("MadakeCAD itself is licensed **MIT OR Apache-2.0** (see `LICENSE-MIT` / `LICENSE-APACHE`).")
    w("This file inventories the licenses of its dependencies.")
    w("")
    w("## Summary")
    w("")
    w(f"- Rust crates (normal dependencies of the workspace, dev/build deps excluded): **{len(rust)}**")
    w(f"- npm packages (installed tree; per-platform optional binaries that are not "
      f"installed here are omitted): **{len(npm)}** "
      f"({sum(1 for r in npm if r[3] == 'prod')} production, "
      f"{sum(1 for r in npm if r[3] == 'dev')} dev-only)")
    w(f"- Strong copyleft (GPL/AGPL/LGPL/SSPL) or non-permissive licenses: **{len(strong)}**"
      + ("  ← must be zero before release" if strong else " — clear"))
    w(f"- File-level copyleft (MPL/CDDL/EPL): **{len(flagged) - len(strong)}** "
      "— compatible with an MIT OR Apache-2.0 release; obligations attach only to those "
      "dependencies' own files, which are consumed unmodified")
    w("")
    w("### Rust license distribution")
    w("")
    w("| License | Crates |")
    w("|---|---|")
    for lic, n in summarize([l for _, _, l in rust]):
        w(f"| `{lic}` | {n} |")
    w("")
    w("### npm license distribution")
    w("")
    w("| License | Packages |")
    w("|---|---|")
    for lic, n in summarize([l for _, _, l, _ in npm]):
        w(f"| `{lic}` | {n} |")
    w("")
    w("### Flagged (copyleft or non-permissive)")
    w("")
    if not flagged:
        w("None. No copyleft or proprietary license appears in either graph.")
    else:
        w("| Ecosystem | Package | Version | License | Class |")
        w("|---|---|---|---|---|")
        for eco, n, v, l, cls in flagged:
            w(f"| {eco} | `{n}` | {v} | `{l}` | {cls} |")
        w("")
        if strong:
            w("**Action required:** the entries above classed *strong copyleft* or "
              "*non-permissive* are incompatible with releasing MadakeCAD under "
              "MIT OR Apache-2.0. Replace or remove them before publishing.")
        else:
            w("No strong copyleft. The remaining entries are file-level copyleft "
              "(MPL-2.0 class): they may be redistributed inside an MIT OR Apache-2.0 "
              "product as long as their own source stays available and any modifications "
              "to *their* files are published. MadakeCAD consumes them unmodified via "
              "crates.io, so no additional obligation arises.")
    w("")
    w("## Out-of-process tools (no license propagation)")
    w("")
    w("These are invoked as separate processes and are never linked into or redistributed with MadakeCAD:")
    w("")
    w("| Tool | License | Role |")
    w("|---|---|---|")
    w("| ngspice | BSD-3-Clause (core; some models differ) | SPICE simulation backend, spawned as a child process |")
    w("| Claude CLI / other AI providers | proprietary, user-installed | AI assistant backend, spawned or reached over HTTP |")
    w("| FreeCAD (planned, M5) | LGPL-2.0-or-later | mechanical integration over the Link API (HTTP), no linking |")
    w("")
    w("## Bundled assets")
    w("")
    w("| Asset | Origin | License |")
    w("|---|---|---|")
    w("| `src-tauri/icons/*`, `public/favicon.svg`, `docs/images/lockup-*.png` | original MadakeCAD logo, designed in `MadakeCAD.pen` and exported | MIT OR Apache-2.0 (this project) |")
    w("| `docs/images/sample-drawing.svg` | generated by this project's own renderer from synthetic sample data | MIT OR Apache-2.0 (this project) |")
    w("| UI icon set (`lucide-vue-next`) | Lucide | ISC — attribution retained via `node_modules`/this file; no icon files are vendored into the repository |")
    w("| Fonts | **none bundled**. The UI names font families in CSS only; PDF export loads fonts from the host system (`fontdb::load_system_fonts`) | n/a — no font binaries are redistributed |")
    w("")
    w("## Full inventory")
    w("")
    w("### Rust")
    w("")
    w("| Crate | Version | License |")
    w("|---|---|---|")
    for n, v, l in rust:
        w(f"| `{n}` | {v} | `{l}` |")
    w("")
    w("### npm")
    w("")
    w("| Package | Version | Scope | License |")
    w("|---|---|---|---|")
    for n, v, l, s in npm:
        w(f"| `{n}` | {v} | {s} | `{l}` |")
    w("")
    return "\n".join(out)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    content = render()
    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != content:
            print(f"{OUT.relative_to(ROOT)} is out of date; run: python3 scripts/gen_licenses.py",
                  file=sys.stderr)
            return 1
        print(f"{OUT.relative_to(ROOT)} is up to date")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(content, encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
