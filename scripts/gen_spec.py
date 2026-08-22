#!/usr/bin/env python3
"""Generate the Detailed Specification documents from test code.

Every test doubles as a specification clause:
  - Rust:   two `///` doc lines above `#[test]` / `#[tokio::test]`
            (line 1 = English, line 2 = Japanese)
  - Vitest: `it("English sentence")` with a `// ja: 日本語文` comment
            on the line immediately above

Outputs (DO NOT EDIT BY HAND):
  docs/13-specification.md      (English)
  docs/13-specification.ja.md   (Japanese)

Usage:
  python3 scripts/gen_spec.py           # regenerate
  python3 scripts/gen_spec.py --check   # exit 1 if committed docs are stale (CI)
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (glob, section key). Order defines document order.
RUST_SOURCES = [
    ("src-tauri/crates/madake-core/src/*.rs", "core"),
    ("src-tauri/crates/madake-mcp/tests/*.rs", "api"),
    ("src-tauri/crates/madake-agent/tests/*.rs", "agent"),
    ("src-tauri/crates/madake-agent/src/*.rs", "agent"),
    ("src-tauri/crates/madake-cli/src/*.rs", "cli"),
]
TS_SOURCES = [("src/**/*.test.ts", "frontend")]

SECTIONS = {
    "core": ("Core domain (madake-core)", "コアドメイン (madake-core)"),
    "api": ("Automation APIs (MCP / REST)", "自動化API (MCP / REST)"),
    "agent": ("AI assistant (madake-agent)", "AIアシスタント (madake-agent)"),
    "cli": ("madake CLI", "madake CLI"),
    "frontend": ("Frontend (editor UI)", "フロントエンド (エディタUI)"),
}

# Module (file stem) -> readable heading (EN, JA)
MODULES = {
    "geometry": ("Geometry & coordinates", "座標・ジオメトリ"),
    "model": ("Document model", "ドキュメントモデル"),
    "command": ("Command engine (undo/redo)", "Commandエンジン (undo/redo)"),
    "io": ("Project file I/O (.mdkproj)", "プロジェクトファイルI/O (.mdkproj)"),
    "symbol": ("Symbol library", "シンボルライブラリ"),
    "netlist": ("Netlist extraction", "ネットリスト抽出"),
    "wire_no": ("Wire numbering", "線番採番"),
    "harness": ("Harness boundaries", "ハーネス境界"),
    "svg": ("SVG output (JIS frame)", "SVG出力 (JIS図枠)"),
    "pdf": ("PDF output", "PDF出力"),
    "reports": ("Reports (BOM / wire list)", "帳票 (部品表 / 電線リスト)"),
    "report_sheet": ("Reports as drawing sheets", "帳票の図面シート化"),
    "terminal_chart": ("Terminal block charts", "端子台チャート"),
    "terminal_diagram": ("Terminal connection diagrams", "端子接続図"),
    "verify": ("Verification (ERC & electrical)", "検証 (ERC・電気検証)"),
    "spice": ("SPICE netlist generation", "SPICEネットリスト生成"),
    "ngspice": ("ngspice runner", "ngspiceランナー"),
    "sim": ("DC simulation", "DCシミュレーション"),
    "parts": ("Parts database", "部品データベース"),
    "kicad": ("KiCad import", "KiCadインポート"),
    "link_export": ("REST Link API", "REST Link API"),
    "agent_api": ("Agent REST endpoints", "エージェントRESTエンドポイント"),
    "origin": ("Edit origin (user / agent / mcp)", "編集origin (ユーザー/エージェント/MCP)"),
    "backend": ("Claude CLI backend", "Claude CLIバックエンド"),
    "parser": ("stream-json parser", "stream-jsonパーサ"),
    "conversation": ("Conversations & history", "会話・履歴"),
    "manager": ("Agent manager (turns)", "エージェントマネージャ (ターン)"),
    "settings": ("AI settings", "AI設定"),
    "cli": ("Argument parsing & dispatch", "引数解釈・ディスパッチ"),
    "client": ("Link API client", "Link APIクライアント"),
    "format": ("Human-readable output", "人間向け整形出力"),
}

RUST_ATTR = re.compile(r"^\s*#\[(tokio::)?test\]")
RUST_FN = re.compile(r"^\s*(async\s+)?fn\s+([a-zA-Z0-9_]+)")
DOC = re.compile(r"^\s*///\s?(.*)$")
TS_IT = re.compile(r"^\s*it\(\s*[\"'](.+?)[\"']\s*,")
TS_DESCRIBE = re.compile(r"^\s*describe\(\s*[\"'](.+?)[\"']\s*,")
TS_JA = re.compile(r"^\s*//\s*ja:\s?(.*)$")

missing: list[str] = []


def parse_rust(path: Path):
    """Yield (test_name, en, ja) for each test in a Rust file."""
    lines = path.read_text(encoding="utf-8").splitlines()
    docs: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        m = DOC.match(line)
        if m:
            docs.append(m.group(1))
            i += 1
            continue
        if RUST_ATTR.match(line):
            # attribute may be followed by more attributes then fn
            j = i + 1
            while j < len(lines) and not RUST_FN.match(lines[j]):
                j += 1
            if j < len(lines):
                name = RUST_FN.match(lines[j]).group(2)
                en = docs[0] if len(docs) >= 1 else None
                ja = docs[1] if len(docs) >= 2 else None
                if not en or not ja:
                    missing.append(f"{path.relative_to(ROOT)}::{name}")
                yield name, en or name.replace("_", " "), ja or (en or name)
            docs = []
            i = j + 1
            continue
        if line.strip() and not line.strip().startswith("//"):
            docs = []
        i += 1


def parse_ts(path: Path):
    """Yield (group, test_name(en), en, ja) for each vitest `it`."""
    lines = path.read_text(encoding="utf-8").splitlines()
    group = path.stem.replace(".test", "")
    prev_ja: str | None = None
    for idx, line in enumerate(lines):
        d = TS_DESCRIBE.match(line)
        if d:
            group = d.group(1)
            continue
        j = TS_JA.match(line)
        if j:
            prev_ja = j.group(1)
            continue
        m = TS_IT.match(line)
        if m:
            en = m.group(1)
            ja = prev_ja
            if not ja:
                missing.append(f"{path.relative_to(ROOT)}::{en}")
            yield group, en, ja or en
            prev_ja = None
            continue
        if line.strip():
            prev_ja = None


def collect():
    """sections[key] -> {module_heading_key: [(en, ja, id)]}"""
    sections: dict[str, dict[str, list[tuple[str, str, str]]]] = {}
    for glob, key in RUST_SOURCES:
        for path in sorted(ROOT.glob(glob)):
            module = path.stem
            for name, en, ja in parse_rust(path):
                sections.setdefault(key, {}).setdefault(module, []).append((en, ja, name))
    for glob, key in TS_SOURCES:
        for path in sorted(ROOT.glob(glob)):
            for group, en, ja in parse_ts(path):
                sections.setdefault(key, {}).setdefault(path.stem.replace(".test", ""), []).append((en, ja, group))
    return sections


HEADER_EN = """# Detailed Specification

**日本語: [13-specification.ja.md](13-specification.ja.md)** | ← [Roadmap](12-roadmap.md)

> **Generated from the test suite — do not edit by hand.**
> Every clause below is enforced by an automated test; the test id is shown in gray.
> Regenerate with `python3 scripts/gen_spec.py` after changing tests.

This document is the living, always-verified specification of MadakeCAD:
if a behavior is listed here, a test proves it on every run of the suite.

"""

HEADER_JA = """# 詳細仕様設計書

**English (canonical): [13-specification.md](13-specification.md)** | ← [ロードマップ](12-roadmap.ja.md)

> **テストスイートから自動生成 — 手で編集しないこと。**
> 以下の各項目は自動テストで常に検証されている(灰色はテストID)。
> テスト変更後は `python3 scripts/gen_spec.py` で再生成する。

本書はMadakeCADの「生きた仕様書」である:
ここに載っている挙動は、テスト実行のたびに証明される。

"""


def render(sections, lang: str) -> str:
    out = [HEADER_EN if lang == "en" else HEADER_JA]
    total = sum(len(v) for mods in sections.values() for v in mods.values())
    if lang == "en":
        out.append(f"**{total} specification clauses** across {len(sections)} areas.\n")
    else:
        out.append(f"全{len(sections)}領域・**{total}仕様項目**。\n")
    for key, (title_en, title_ja) in SECTIONS.items():
        if key not in sections:
            continue
        out.append(f"\n## {title_en if lang == 'en' else title_ja}\n")
        for module, items in sections[key].items():
            head = MODULES.get(module, (module, module))
            out.append(f"\n### {head[0] if lang == 'en' else head[1]}\n")
            for en, ja, tid in items:
                text = en if lang == "en" else ja
                out.append(f"- {text} <sub>`{tid}`</sub>")
        out.append("")
    return "\n".join(out) + "\n"


def main():
    sections = collect()
    en = render(sections, "en")
    ja = render(sections, "ja")
    targets = {
        ROOT / "docs/13-specification.md": en,
        ROOT / "docs/13-specification.ja.md": ja,
    }
    if "--check" in sys.argv:
        stale = [str(p.relative_to(ROOT)) for p, content in targets.items()
                 if not p.exists() or p.read_text(encoding="utf-8") != content]
        if stale:
            print("STALE spec docs (run: python3 scripts/gen_spec.py):", ", ".join(stale))
            sys.exit(1)
        if missing:
            print(f"WARNING: {len(missing)} tests lack bilingual spec comments:")
            for m in missing[:20]:
                print("  -", m)
            sys.exit(1)
        print("spec docs up to date; all tests have bilingual spec comments")
        return
    for p, content in targets.items():
        p.write_text(content, encoding="utf-8")
        print("wrote", p.relative_to(ROOT))
    if missing:
        print(f"\nWARNING: {len(missing)} tests lack bilingual spec comments:")
        for m in missing:
            print("  -", m)


if __name__ == "__main__":
    main()
