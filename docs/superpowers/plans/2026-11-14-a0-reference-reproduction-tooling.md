# A0 Reference Reproduction: Tooling and Region 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the ground-truth extractor, the overlay comparison and the runner for the A0 reference reproduction, then reproduce region 1 (frame, title block, revision block) and record every gap it exposes.

**Architecture:** Three small Python tools in the git-ignored folder `docs/references/repro/a0/`: `truth.py` turns the reference PDF into line segments in A0 millimetres, `compare.py` overlays the exported sheet on the reference, and `sheet.py` is the sheet file that the existing drawing helper `~/.claude/skills/drawing-madakecad-wiring/mdk_draw.py` sends to a development build on port 9320. Region 1 is a user frame template (JSON in `~/MadakeCAD/templates/frames/`). Nothing in this plan changes MadakeCAD source code; each gap found gets its own later plan.

**Tech Stack:** Python 3 (standard library, Pillow 11, numpy 2), `pdftocairo` (poppler), MadakeCAD Link API (`/api/v1`), `mdk_draw.py`.

**Spec:** `docs/superpowers/specs/2026-11-14-a0-reference-reproduction-design.md`

## Global Constraints

- The reference PDF and everything derived from it stay in `docs/references/` (git-ignored). The path of the reference is passed in the environment variable `A0_REF_PDF`; it is never written into a committed file.
- Committed files describe the reference in general terms only. Run `python3 scripts/audit_confidential.py` before every commit; it must print `no confidential terms found`.
- Customer-specific content (logo, company name, part numbers) goes into user data (`~/MadakeCAD/templates`, the project file) or `docs/references/`, never into the repository.
- The development build listens on port 9320 (`MADAKE_MCP_PORT=9320`). The installed app on 9310 is not used.
- Coordinates: paper millimetres, origin top left, Y down, snapped to the 2.5 mm grid.
- Text that cannot be read with confidence is listed for the user, never guessed.
- Every part is a symbol or block with pins; ratings printed on the reference are attributes as well as text; values the reference does not give stay empty.
- The reference PDF stays in its own folder, never in the folder that holds the scripts. One-off inline Python that reads derived data runs with `python3 -I`; the tools themselves import their neighbours, so they run without `-I` from their own folder.
- No `Co-Authored-By` or other AI attribution trailer in commit messages.

## Review Focus

1. A stroked path in the reference with a cubic curve (`C`): the extractor must keep its end points instead of dropping the path or crashing.
2. A region box that touches the page edge: cropping and rasterising must clamp to the page instead of failing.
3. The exported sheet PDF having a different page size or rotation than A0 landscape: `compare.py` must report the size mismatch in plain words instead of producing a shifted overlay.
4. The development build not running: the runner must stop with one line that says how to start it.
5. A segment lying exactly on a region boundary: it belongs to the region (inclusive bounds), so that no wire is lost between two regions.

## Facts about the reference file

The reference is a single-page vector PDF: stroked paths in a small number of widths and colours, all under one page transform, and no extractable text. The exact counts, widths, colours and the page transform are recorded in the git-ignored `docs/references/repro/a0/NOTES.md`; `truth.py` holds the constants it needs.

## File Structure

| File | Responsibility |
|---|---|
| `docs/references/repro/a0/truth.py` | Reference PDF → segments in A0 mm; crop by region; CLI |
| `docs/references/repro/a0/test_truth.py` | Unit tests of `truth.py` on synthetic SVG |
| `docs/references/repro/a0/compare.py` | Rasterise reference and reproduction, overlay, coverage numbers; CLI |
| `docs/references/repro/a0/test_compare.py` | Unit tests of the overlay arithmetic |
| `docs/references/repro/a0/regions.py` | Region boxes in A0 mm (one dict) |
| `docs/references/repro/a0/sheet.py` | Sheet file for `mdk_draw.py`; runs the region files in order |
| `docs/references/repro/a0/r1_frame.py` | Region 1: title block fields, revisions |
| `docs/references/repro/a0/run.sh` | Check the build is up, draw, export PDF, compare |
| `~/MadakeCAD/templates/frames/a0-reference.json` | User frame template for region 1 |
| `docs/internal/specs/reproduction-gaps.md` + `.ja.md` | Gap ledger (committed, general terms) |

---

### Task 1: Ground-truth extractor

**Files:**
- Create: `docs/references/repro/a0/truth.py`
- Test: `docs/references/repro/a0/test_truth.py`

**Interfaces:**
- Produces: `K: float`; `snap(v: float, g: float = 2.5) -> float`; `parse_svg(text: str) -> list[dict]` where each dict is `{"pts": [[x_mm, y_mm], ...], "color": str, "w": "thin"|"mid"|"heavy", "curve": bool}`; `crop(segs, box) -> list[dict]` with `box = (x0, y0, x1, y1)` in mm, inclusive; CLI `truth.py --out DIR` reading `A0_REF_PDF`.

- [ ] **Step 1: Write the failing tests**

```python
# docs/references/repro/a0/test_truth.py
import unittest
import truth

HEAD = '<svg xmlns="http://www.w3.org/2000/svg" width="1191" height="842">'
T = 'transform="matrix(0, 0.24, 0.24, 0, 0, 0)"'


def path(d, color="rgb(0%, 0%, 0%)", w="4.1"):
    return (f'<path fill="none" stroke-width="{w}" stroke="{color}" d="{d}" {T}/>')


class TruthTest(unittest.TestCase):
    def test_transform_swaps_axes_and_scales_to_a0_mm(self):
        """A path point (y=1000, x=500 in path units) lands at x=240*K, y=120*K mm.
        パスの点は軸を入れ替えて0.24倍し、A0のmmへ換算する。"""
        segs = truth.parse_svg(HEAD + path("M 500 1000 L 500 2000 ") + "</svg>")
        self.assertEqual(len(segs), 1)
        (x0, y0), (x1, y1) = segs[0]["pts"]
        self.assertAlmostEqual(x0, 240 * truth.K, places=3)
        self.assertAlmostEqual(y0, 120 * truth.K, places=3)
        self.assertAlmostEqual(x1, 480 * truth.K, places=3)

    def test_colour_and_width_are_named(self):
        """Stroke colour and width become short names.
        線の色と太さは短い名前になる。"""
        segs = truth.parse_svg(HEAD + path("M 0 0 L 10 0 ", "rgb(100%, 50%, 0%)", "1.5") + "</svg>")
        self.assertEqual(segs[0]["color"], "orange")
        self.assertEqual(segs[0]["w"], "thin")

    def test_curve_keeps_end_points(self):
        """A cubic curve keeps its end point and is flagged, it is not dropped.
        三次曲線は終点を残して印を付ける。捨てない。"""
        segs = truth.parse_svg(HEAD + path("M 0 0 C 1 1 2 2 30 40 ") + "</svg>")
        self.assertTrue(segs[0]["curve"])
        self.assertEqual(len(segs[0]["pts"]), 2)

    def test_filled_paths_and_clip_paths_are_ignored(self):
        """Paths without a stroke are not segments.
        線を持たないパスは線分にしない。"""
        svg = HEAD + '<path fill="rgb(0%,0%,0%)" d="M 0 0 L 1 1 Z"/>' + "</svg>"
        self.assertEqual(truth.parse_svg(svg), [])

    def test_crop_is_inclusive_on_the_boundary(self):
        """A segment lying exactly on the box edge belongs to the box.
        箱の辺にちょうど乗る線分はその箱に入る。"""
        seg = {"pts": [[10.0, 0.0], [10.0, 5.0]], "color": "black", "w": "mid", "curve": False}
        self.assertEqual(truth.crop([seg], (10.0, 0.0, 20.0, 5.0)), [seg])
        self.assertEqual(truth.crop([seg], (10.1, 0.0, 20.0, 5.0)), [])

    def test_snap_rounds_to_the_grid(self):
        """Positions round to the nearest 2.5 mm.
        位置は最寄りの2.5mmへ丸める。"""
        self.assertEqual(truth.snap(6.2), 5.0)
        self.assertEqual(truth.snap(6.3), 7.5)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd docs/references/repro/a0 && python3 -m unittest test_truth -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'truth'`

- [ ] **Step 3: Write the implementation**

```python
#!/usr/bin/env python3
# docs/references/repro/a0/truth.py
"""参考図面PDF → A0のmmで表した線分 (正解データ)。

参考図面のパスは環境変数 A0_REF_PDF で渡す (コミットするファイルへ書かない)。
"""
import argparse
import json
import os
import re
import subprocess
import sys

K = 1189.0 / 1191.0          # SVGの1ptあたりのA0のmm
UNIT = 0.24                  # パス単位 → SVGのpt
COLORS = {
    "rgb(0%, 0%, 0%)": "black", "rgb(100%, 0%, 0%)": "red", "rgb(0%, 100%, 0%)": "green",
    "rgb(0%, 100%, 100%)": "cyan", "rgb(100%, 0%, 100%)": "magenta",
    "rgb(100%, 50%, 0%)": "orange", "rgb(100%, 100%, 0%)": "yellow",
    "rgb(0%, 0%, 100%)": "blue", "rgb(50%, 0%, 0%)": "darkred",
}
PATH = re.compile(r"<path ([^>]*)>")
ATTR = re.compile(r'([a-z-]+)="([^"]*)"')
TOKEN = re.compile(r"[MLCZ]|-?\d+(?:\.\d+)?")


def snap(v, g=2.5):
    return round(v / g) * g


def _width(w):
    w = float(w)
    return "thin" if w < 3 else "mid" if w < 6 else "heavy"


def _polylines(d):
    """パスのdを折れ線の並びへ。Cは終点だけを残す。戻り値は (点の並び, 曲線を含むか) の並び。"""
    out, pts, curve = [], [], False
    tok = TOKEN.findall(d)
    i = 0
    while i < len(tok):
        t = tok[i]
        if t == "M":
            if len(pts) > 1:
                out.append((pts, curve))
            pts, curve = [(float(tok[i + 1]), float(tok[i + 2]))], False
            i += 3
        elif t == "L":
            pts.append((float(tok[i + 1]), float(tok[i + 2])))
            i += 3
        elif t == "C":
            pts.append((float(tok[i + 5]), float(tok[i + 6])))
            curve = True
            i += 7
        elif t == "Z":
            if pts:
                pts.append(pts[0])
            i += 1
        else:
            i += 1
    if len(pts) > 1:
        out.append((pts, curve))
    return out


def parse_svg(text):
    segs = []
    for m in PATH.finditer(text):
        a = dict(ATTR.findall(m.group(1)))
        stroke = a.get("stroke", "none")
        if stroke == "none" or "d" not in a or "transform" not in a:
            continue
        for pts, curve in _polylines(a["d"]):
            segs.append({
                # matrix(0, 0.24, 0.24, 0): x_svg = 0.24*y, y_svg = 0.24*x
                "pts": [[round(y * UNIT * K, 3), round(x * UNIT * K, 3)] for x, y in pts],
                "color": COLORS.get(stroke, stroke),
                "w": _width(a.get("stroke-width", "0")),
                "curve": curve,
            })
    return segs


def crop(segs, box):
    x0, y0, x1, y1 = box
    return [s for s in segs
            if all(x0 <= x <= x1 and y0 <= y <= y1 for x, y in s["pts"])]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    pdf = os.environ.get("A0_REF_PDF")
    if not pdf or not os.path.exists(pdf):
        sys.exit("A0_REF_PDF に参考図面PDFのパスを入れてください")
    os.makedirs(args.out, exist_ok=True)
    svg = os.path.join(args.out, "ref.svg")
    subprocess.run(["pdftocairo", "-svg", pdf, svg], check=True)
    segs = parse_svg(open(svg).read())
    json.dump(segs, open(os.path.join(args.out, "all.json"), "w"))
    by = {}
    for s in segs:
        by[(s["color"], s["w"])] = by.get((s["color"], s["w"]), 0) + 1
    print(f"segments {len(segs)}")
    for k in sorted(by):
        print(f"  {k[0]:8} {k[1]:5} {by[k]}")
    import regions
    for name, box in regions.REGIONS.items():
        part = crop(segs, box)
        json.dump(part, open(os.path.join(args.out, f"{name}.json"), "w"))
        print(f"  region {name}: {len(part)}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd docs/references/repro/a0 && python3 -m unittest test_truth -v`
Expected: 6 tests, `OK`

- [ ] **Step 5: No commit**

The folder is git-ignored. Confirm with `git status --short` that nothing under `docs/references/` is listed.

---

### Task 2: Region boxes and the real extraction

**Files:**
- Create: `docs/references/repro/a0/regions.py`
- Create (generated): `docs/references/repro/a0/truth/all.json`, `truth/<region>.json`, `truth/ref.svg`

**Interfaces:**
- Consumes: `truth.parse_svg`, `truth.crop`, `truth.K`
- Produces: `regions.REGIONS: dict[str, tuple[float, float, float, float]]` (A0 mm, inclusive), keys `r1_frame` and `r2_…` to `r7_…` (the six drawing regions; their names are in the git-ignored file)

- [ ] **Step 1: Write `regions.py` with first-guess boxes**

The boxes are read from the 1650 px wide page image (1 px = 1191/1650 pt, then times `K` for mm). The frame region is the whole page; its truth file is filtered later by Task 5.

```python
# docs/references/repro/a0/regions.py
"""領域の箱 (A0のmm、両端を含む)。実測して決める。"""
REGIONS = {
    "r1_frame": (0.0, 0.0, 1189.0, 841.0),
    # r2〜r7: 図面の内容に合わせた6つの箱。名前と座標は、この (git管理外の) ファイルにだけ書く
}
```

- [ ] **Step 2: Run the extraction**

Run: `cd docs/references/repro/a0 && A0_REF_PDF="<path of the reference, see docs/references/README.md>" python3 truth.py --out truth`
Expected: first line `segments N` (tens of thousands), then one line per colour and width, then seven `region` lines, each above 0.

- [ ] **Step 3: Check the mapping on the frame and correct the boxes**

Run:

```bash
cd docs/references/repro/a0 && python3 -I - <<'EOF'
import json
segs = json.load(open("truth/all.json"))
heavy = [s for s in segs if s["w"] in ("mid", "heavy") and s["color"] == "black"]
xs = [x for s in heavy for x, _ in s["pts"]]; ys = [y for s in heavy for _, y in s["pts"]]
print("ink box mm", round(min(xs), 1), round(min(ys), 1), round(max(xs), 1), round(max(ys), 1))
longest = sorted(heavy, key=lambda s: -(abs(s["pts"][0][0]-s["pts"][-1][0]) + abs(s["pts"][0][1]-s["pts"][-1][1])))[:6]
for s in longest: print(s["w"], s["pts"][0], s["pts"][-1], len(s["pts"]))
EOF
```

Expected: the longest black paths are the frame border; its corners give the frame margin on each side. Write the measured frame rectangle as a comment at the top of `regions.py` and tighten each region box so that it contains its parts with a 5 mm margin. Rerun Step 2; every region still has more than 0 segments.

---

### Task 3: Overlay comparison

**Files:**
- Create: `docs/references/repro/a0/compare.py`
- Test: `docs/references/repro/a0/test_compare.py`

**Interfaces:**
- Consumes: `truth.K`, `regions.REGIONS`
- Produces: `ink(img: PIL.Image) -> numpy.ndarray[bool]`; `overlay(ref: ndarray, rep: ndarray, tol: int = 3) -> (PIL.Image, dict)` where the dict is `{"ref_covered": float, "rep_covered": float}` (fractions 0..1; 1.0 when the side has no ink); `clamp_box(box, w=1189.0, h=841.0) -> tuple`; CLI `compare.py REPRO.pdf --region NAME --out DIR [--dpi 300]`.

- [ ] **Step 1: Write the failing tests**

```python
# docs/references/repro/a0/test_compare.py
import unittest
import numpy as np
import compare


class CompareTest(unittest.TestCase):
    def test_identical_ink_is_fully_covered(self):
        """Two equal images cover each other completely.
        同じ画像どうしは互いを完全に覆う。"""
        a = np.zeros((20, 20), bool); a[10, 2:18] = True
        _, m = compare.overlay(a, a.copy())
        self.assertEqual(m, {"ref_covered": 1.0, "rep_covered": 1.0})

    def test_shift_within_tolerance_still_counts(self):
        """A line moved by 2 px is covered with the default tolerance of 3 px.
        2pxずれた線は、既定の許容3pxで覆われた扱いになる。"""
        a = np.zeros((20, 20), bool); a[10, 2:18] = True
        b = np.zeros((20, 20), bool); b[12, 2:18] = True
        _, m = compare.overlay(a, b)
        self.assertEqual(m["ref_covered"], 1.0)

    def test_missing_line_lowers_ref_coverage_and_is_red(self):
        """Ink only in the reference lowers ref_covered and is painted red.
        参考図面にしか無い線はref_coveredを下げ、赤で塗る。"""
        a = np.zeros((20, 20), bool); a[5, 2:18] = True; a[15, 2:18] = True
        b = np.zeros((20, 20), bool); b[5, 2:18] = True
        img, m = compare.overlay(a, b)
        self.assertAlmostEqual(m["ref_covered"], 0.5)
        self.assertEqual(img.getpixel((10, 15)), (255, 0, 0))
        self.assertEqual(img.getpixel((10, 5)), (0, 0, 0))

    def test_extra_line_is_blue(self):
        """Ink only in the reproduction is painted blue.
        再現にしか無い線は青で塗る。"""
        a = np.zeros((20, 20), bool)
        b = np.zeros((20, 20), bool); b[5, 2:18] = True
        img, m = compare.overlay(a, b)
        self.assertEqual(img.getpixel((10, 5)), (0, 0, 255))
        self.assertEqual(m["ref_covered"], 1.0)
        self.assertEqual(m["rep_covered"], 0.0)

    def test_box_is_clamped_to_the_page(self):
        """A box that leaves the page is cut at the page edge.
        用紙からはみ出す箱は用紙の端で切る。"""
        self.assertEqual(compare.clamp_box((-5.0, 10.0, 1300.0, 900.0)), (0.0, 10.0, 1189.0, 841.0))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd docs/references/repro/a0 && python3 -m unittest test_compare -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'compare'`

- [ ] **Step 3: Write the implementation**

```python
#!/usr/bin/env python3
# docs/references/repro/a0/compare.py
"""再現シートのPDFを参考図面に重ね、領域ごとの差分画像と被覆率を出す。

赤 = 参考図面にだけある線、青 = 再現にだけある線、黒 = 両方にある線。
"""
import argparse
import json
import os
import subprocess
import sys

import numpy as np
from PIL import Image, ImageFilter

from truth import K

A0_W, A0_H = 1189.0, 841.0
A0_PT = (A0_W / 25.4 * 72, A0_H / 25.4 * 72)   # 約 3370 x 2384


def clamp_box(box, w=A0_W, h=A0_H):
    x0, y0, x1, y1 = box
    return (max(0.0, x0), max(0.0, y0), min(w, x1), min(h, y1))


def ink(img):
    """白でない画素をインクとみなす (色付きの線も含む)。"""
    a = np.asarray(img.convert("RGB")).astype(int)
    return a.min(axis=2) < 200


def _dilate(a, tol):
    if tol <= 0:
        return a
    im = Image.fromarray((a * 255).astype("uint8")).filter(ImageFilter.MaxFilter(2 * tol + 1))
    return np.asarray(im) > 0


def overlay(ref, rep, tol=3):
    both = ref & rep
    out = np.full(ref.shape + (3,), 255, "uint8")
    out[ref & ~rep] = (255, 0, 0)
    out[rep & ~ref] = (0, 0, 255)
    out[both] = (0, 0, 0)
    ref_n, rep_n = int(ref.sum()), int(rep.sum())
    m = {
        "ref_covered": 1.0 if ref_n == 0 else float((ref & _dilate(rep, tol)).sum()) / ref_n,
        "rep_covered": 1.0 if rep_n == 0 else float((rep & _dilate(ref, tol)).sum()) / rep_n,
    }
    return Image.fromarray(out), m


def _page_pt(pdf):
    out = subprocess.run(["pdfinfo", pdf], capture_output=True, text=True, check=True).stdout
    line = next(l for l in out.splitlines() if l.startswith("Page size"))
    w, h = float(line.split()[2]), float(line.split()[4])
    rot = next((int(l.split()[2]) for l in out.splitlines() if l.startswith("Page rot")), 0)
    return (h, w) if rot in (90, 270) else (w, h)


def _raster(pdf, dpi, box_px, out):
    x, y, w, h = box_px
    subprocess.run(["pdftocairo", "-png", "-singlefile", "-r", str(dpi), "-x", str(x), "-y", str(y),
                    "-W", str(w), "-H", str(h), pdf, out[:-4]], check=True)
    return Image.open(out)


def main():
    import regions
    ap = argparse.ArgumentParser()
    ap.add_argument("repro")
    ap.add_argument("--region", required=True, choices=list(regions.REGIONS))
    ap.add_argument("--out", required=True)
    ap.add_argument("--dpi", type=int, default=300)
    args = ap.parse_args()
    ref_pdf = os.environ.get("A0_REF_PDF")
    if not ref_pdf or not os.path.exists(ref_pdf):
        sys.exit("A0_REF_PDF に参考図面PDFのパスを入れてください")
    pw, ph = _page_pt(args.repro)
    if abs(pw - A0_PT[0]) > 3 or abs(ph - A0_PT[1]) > 3:
        sys.exit(f"再現PDFの用紙が {pw:.0f}x{ph:.0f} pt です。A0横 ({A0_PT[0]:.0f}x{A0_PT[1]:.0f} pt) で書き出してください")
    os.makedirs(args.out, exist_ok=True)
    x0, y0, x1, y1 = clamp_box(regions.REGIONS[args.region])
    # 参考図面: mm → SVG pt (÷K) → px
    f = args.dpi / 72 / K
    box = (round(x0 * f), round(y0 * f), round((x1 - x0) * f), round((y1 - y0) * f))
    ref = _raster(ref_pdf, args.dpi, box, os.path.join(args.out, f"{args.region}-ref.png"))
    # 再現: 同じ画素/mmになる解像度で描く
    rdpi = 25.4 * args.dpi / (72 * K)
    g = rdpi / 25.4
    rbox = (round(x0 * g), round(y0 * g), box[2], box[3])
    rep = _raster(args.repro, rdpi, rbox, os.path.join(args.out, f"{args.region}-rep.png"))
    img, m = overlay(ink(ref), ink(rep.resize(ref.size)))
    img.save(os.path.join(args.out, f"{args.region}-diff.png"))
    json.dump(m, open(os.path.join(args.out, f"{args.region}-metrics.json"), "w"))
    print(f"{args.region}: ref_covered {m['ref_covered']:.3f}  rep_covered {m['rep_covered']:.3f}")
    print(os.path.join(args.out, f"{args.region}-diff.png"))


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd docs/references/repro/a0 && python3 -m unittest test_compare test_truth -v`
Expected: 11 tests, `OK`

- [ ] **Step 5: Self-check against the reference itself**

A reproduction does not exist yet, so compare the reference with an A0-sized copy of itself, written to `docs/references/repro/a0/out/self.pdf`.

Run:

```bash
cd docs/references/repro/a0 && mkdir -p out && \
pdftocairo -pdf -paperw 3370 -paperh 2384 -expand "$A0_REF_PDF" out/self.pdf && \
python3 compare.py out/self.pdf --region <one of r2…r7> --out out
```

Expected: `ref_covered` and `rep_covered` both at or above 0.98. If the tool stops with the page size message, run `pdfinfo out/self.pdf`, find which of page size and rotation `_page_pt` misreads, fix `_page_pt`, add a unit test for that case, and rerun. Open the `-diff.png` it prints with Read: the picture is black with thin red or blue fringes only.

---

### Task 4: Runner against the development build

**Files:**
- Create: `docs/references/repro/a0/sheet.py`
- Create: `docs/references/repro/a0/r1_frame.py`
- Create: `docs/references/repro/a0/run.sh`

**Interfaces:**
- Consumes: `mdk_draw.py` globals (`sheet`, `title_block`, `add`, `block`, `sym`, `wire`, `text`, `req`), `compare.py` CLI
- Produces: `run.sh [region ...]` that draws the sheet named `A0`, exports `out/sheet.pdf` and prints one comparison line per region; `sheet.py` execs every `r*_*.py` in name order

- [ ] **Step 1: Start the development build**

Run (background): `cd ~/Documents/MadakeCAD && export PATH="$HOME/.cargo/bin:$PATH" && MADAKE_MCP_PORT=9320 npm run tauri dev`
If vite port 1420 is busy: `cd src-tauri && MADAKE_MCP_PORT=9320 cargo run -p madake-cad`.
Then: `cd src-tauri && cargo run -q -p madake-cli -- --port 9320 status`
Expected: a line `接続         : http://127.0.0.1:9320/api/v1`.

- [ ] **Step 2: Write the sheet file, the empty region file and the runner**

```python
# docs/references/repro/a0/sheet.py  (mdk_draw.py が exec する)
import glob as _glob
import os as _os

R = _os.path.expanduser("~/Documents/MadakeCAD/docs/references/repro/a0")
sheet("A0")
for _f in sorted(_glob.glob(_os.path.join(R, "r[0-9]_*.py"))):
    exec(open(_f).read())
```

```python
# docs/references/repro/a0/r1_frame.py  (領域1: 図枠・表題欄・改訂欄)
# 表題欄の値は Task 5 で参考図面から読んで入れる。
title_block()
```

```bash
#!/bin/bash
# docs/references/repro/a0/run.sh  使い方: A0_REF_PDF=... ./run.sh [領域名 ...]
set -euo pipefail
cd "$(dirname "$0")"
API=http://127.0.0.1:9320/api/v1
if ! python3 -c "import urllib.request,sys; urllib.request.urlopen('$API/project', timeout=3)" 2>/dev/null; then
  echo "開発ビルドが動いていません。MADAKE_MCP_PORT=9320 npm run tauri dev で起動してください" >&2
  exit 1
fi
mkdir -p out
python3 ~/.claude/skills/drawing-madakecad-wiring/mdk_draw.py sheet.py --api "$API" --size A0 \
  --out out --save "$PWD/out/a0.mdkproj"
python3 - <<EOF
import json, urllib.request
p = json.load(urllib.request.urlopen("$API/project")); p = p.get("project", p)
sid = next(s["id"] for s in p["sheets"] if s["name"] == "A0")
body = json.dumps({"path": "$PWD/out/sheet.pdf", "sheet_id": sid}).encode()
urllib.request.urlopen(urllib.request.Request("$API/export/pdf", data=body,
    headers={"Content-Type": "application/json"}))
EOF
for r in "${@:-r1_frame}"; do
  python3 compare.py out/sheet.pdf --region "$r" --out out
done
```

Then `chmod +x docs/references/repro/a0/run.sh`.

- [ ] **Step 3: Run it**

Run: `cd docs/references/repro/a0 && A0_REF_PDF="<path>" ./run.sh r1_frame`
Expected: `sheets 1  entities 0  unconnected/dangling 0`, then `r1_frame: ref_covered 0.0xx  rep_covered ...` (low coverage: only the default frame exists). If `/export/pdf` rejects `sheet_id` or writes several pages, read `post_export_pdf` in `src-tauri/crates/madake-mcp/src/link_api.rs`, use the body it expects, and record the difference as a note in `docs/references/repro/a0/NOTES.md`.

- [ ] **Step 4: Check the failure message**

Stop the development build, run `./run.sh`, expect exit code 1 and the line `開発ビルドが動いていません。…`. Start the build again.

---

### Task 5: Region 1 — frame, title block, revision block

**Files:**
- Create: `~/MadakeCAD/templates/frames/a0-reference.json`
- Modify: `docs/references/repro/a0/r1_frame.py`
- Create: `docs/references/repro/a0/NOTES.md`
- Create: `docs/references/repro/a0/unreadable.md`

**Interfaces:**
- Consumes: `truth/r1_frame.json`, `FrameTemplate` JSON schema (`src-tauri/crates/madake-core/src/frame_template.rs`: `id`, `name`, `name_ja`, `paper{w,h}`, `frame_margin`, `zones{cols,rows,col_labels,row_labels}`, `title_block{origin,w,h,row_h,grid_lines[{x1,y1,x2,y2}],cells[{x,y,w,h,text|bind,size,weight,align,omit_if_empty}]}`, `revision_block{rows_max,col_w}`, `logo{x,y,w,h,path}`), Commands `set_frame_template {frame_template}`, `set_title_block`, `set_project_meta`
- Produces: the sheet `A0` drawn with the frame template `a0-reference`; the list of region 1 gaps in `NOTES.md`

- [ ] **Step 1: Measure the frame, the title block and the revision block**

Run:

```bash
cd docs/references/repro/a0 && python3 -I - <<'EOF'
import json
segs = json.load(open("truth/all.json"))
# 表題欄のある隅を箱で囲み、その中の黒い線の縦横の位置を一覧にする
box = TITLE_BLOCK_BOX  # NOTES.md に記録した箱 (用紙mm)
hs, vs, diag = {}, {}, []
for s in segs:
    if s["color"] != "black" or s["w"] == "thin": continue
    for (xa, ya), (xb, yb) in zip(s["pts"], s["pts"][1:]):
        if not (box[0] <= xa <= box[2] and box[1] <= ya <= box[3]): continue
        if abs(ya - yb) < 0.2: hs.setdefault(round(ya, 1), []).append((round(min(xa, xb), 1), round(max(xa, xb), 1)))
        elif abs(xa - xb) < 0.2: vs.setdefault(round(xa, 1), []).append((round(min(ya, yb), 1), round(max(ya, yb), 1)))
        else: diag.append(((round(xa, 1), round(ya, 1)), (round(xb, 1), round(yb, 1))))
print("horizontal y: x-ranges"); [print(" ", y, sorted(set(v))) for y, v in sorted(hs.items())]
print("vertical x: y-ranges");   [print(" ", x, sorted(set(v))) for x, v in sorted(vs.items())]
print("diagonal", diag)
EOF
```

Expected: a table of horizontal and vertical rule positions and the diagonal lines of the empty cells. Record in `NOTES.md`: frame rectangle, frame margin on each side, title block width and height, each row and column position relative to the title block's top left corner, revision row height and the six column widths, centre marks and corner marks of the frame.

- [ ] **Step 2: Read the text of region 1**

Render 600 dpi crops of the title block, the revision block and every note or table printed outside the frame (`pdftocairo -png -r 600 -x … -y … -W … -H … "$A0_REF_PDF" out/t-<name>`), open each with Read, and write every string with its position (A0 mm) and height into `NOTES.md`. Strings that are not certain go into `unreadable.md` as `position | best reading | crop file`, and the user is asked to resolve them before Step 6.

- [ ] **Step 3: Write the frame template**

Copy `src-tauri/resources/frame_templates/jis-a3.json` to `~/MadakeCAD/templates/frames/a0-reference.json` and replace the values with the measured ones: `id` `a0-reference`, `paper` 1189 x 841, `frame_margin` (the measured value; if the four sides differ, use the smallest and record the difference as a gap), `title_block.w/h/row_h`, one `grid_lines` entry per measured rule (diagonals included), one `cells` entry per string (fixed labels as `text`, values as `bind` to `{title_block.*}` / `{project.name}` / `{project.no}`), `revision_block.rows_max` and `col_w`, `logo` with the measured logo box and no `path`.

Validate: `python3 -c "import json;json.load(open('$HOME/MadakeCAD/templates/frames/a0-reference.json'))"` prints nothing, and `GET /api/v1/frame-templates` on port 9320 lists `a0-reference` (use Python `urllib`, print only the ids).

- [ ] **Step 4: Set the title block values and select the template**

Replace `r1_frame.py`:

```python
# docs/references/repro/a0/r1_frame.py  (領域1: 図枠・表題欄・改訂欄)
# 値は NOTES.md の読み取り結果 (参考図面の表題欄) を入れる。
title_block(**json.load(open(_os.path.join(R, "title_block.json"))))
req("/commands", [{"type": "set_frame_template", "frame_template": "a0-reference"}])
```

and write `docs/references/repro/a0/title_block.json` with the keys `company`, `title`, `drawing_no`, `scale`, `date`, `designed`, `drawn`, `checked`, `approved`, `rev`, each holding the string read in Step 2 (empty string where the reference cell is empty). Revisions: find the revision command with `grep -n "Revision" src-tauri/crates/madake-core/src/command.rs`, and send one command per revision row read in Step 2 from `r1_frame.py` through `req("/commands", [...])`.

- [ ] **Step 5: Draw and compare**

Run: `cd docs/references/repro/a0 && A0_REF_PDF="<path>" ./run.sh r1_frame`
Then render the title block corner of `out/sheet.pdf` and of the reference at the same size and open both with Read, and open `out/r1_frame-diff.png`.

- [ ] **Step 6: List every difference as a gap candidate**

For each red (missing) or blue (extra) area in the diff and each difference seen in the crops, write one line in `NOTES.md`: what the reference has, what MadakeCAD drew, and which of these it is: (a) fixable in the template JSON, (b) a missing MadakeCAD feature, (c) out of scope by the spec (typeface). Fix every (a) and rerun Step 5 until only (b) and (c) remain. Before classifying something as (b), check the other six regions for the same element and note where else it occurs.

Elements to check explicitly, because the template schema read on 2026-11-14 has no field for them: a revision table with other columns than the built-in four; cells crossed by a diagonal line; a frame with corner and centre marks and no zone labels; different margins per side; a mark drawn inside a shape; rotated text outside the frame; a table outside the frame; rule and border line weights; a logo image.

---

### Task 6: Record the gaps and hand over

**Files:**
- Modify: `docs/internal/specs/reproduction-gaps.md`
- Modify: `docs/internal/specs/reproduction-gaps.ja.md`

**Interfaces:**
- Consumes: the (b) lines of `NOTES.md`
- Produces: ledger entries G24 onward; the list of gap plans to write next

- [ ] **Step 1: Read the ledger's entry format**

Run: `sed -n '187,235p' docs/internal/specs/reproduction-gaps.md` and read one full entry (G1). New entries use the same headings and the same severity words.

- [ ] **Step 2: Add a section for this reproduction and one entry per gap**

Add a heading `## Fifth run — A0 reference, region 1 (2026-11-14)` before `## Gap list` with a short table (element / status), and one `### G<n> — <title> · <severity>` entry per (b) line after G23, each with: what the reference needs, what happens today (with the file and function that decides it), the proposed change in general terms. Write the same content in the `.ja.md` file. No customer name, part number or drawing number.

- [ ] **Step 3: Audit and commit**

```bash
python3 scripts/audit_confidential.py   # must print: no confidential terms found
git add docs/internal/specs/reproduction-gaps.md docs/internal/specs/reproduction-gaps.ja.md
git commit -m "docs: A0参考図面の領域1で見つけたギャップを台帳へ追加"
```

- [ ] **Step 4: Report**

Show the user: the region 1 overlay numbers, the diff image path, the list of new gaps with severity, and `unreadable.md` if it is not empty. Each gap then gets its own design and plan (brainstorming, then writing-plans) before region 2 starts, unless the user decides to batch several gaps into one plan.
