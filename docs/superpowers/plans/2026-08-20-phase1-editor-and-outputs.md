# MadakeCAD フェーズ1 (エディタUI+ネットリスト+出力) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** MadakeCADを「JIS図枠つき配線図を実際に描いて、BOM/電線リスト/SVGを出力できるCAD」にする。

**Architecture:** 全編集はmadake-coreのCommand APIを通す(CLAUDE.md参照)。ロジック(ネットリスト・帳票・SVG)はmadake-coreに置きRustでTDD。UIはPiniaストアがpatchイベントをミラーし、Canvas2Dレンダラが描画する。UIの見た目(レイアウト/配色/コンポーネント)は`design/`のPencilデザイン確定後に実装する(Task 7以降)。

**Tech Stack:** Rust (madake-core, rmcp 3.x, Tauri 2), Vue 3 + TypeScript + Pinia + Vitest, Canvas2D

**Spec:** `docs/superpowers/specs/2026-08-20-madakecad-design.md`

## Global Constraints

- 全編集はCommand経由。モデル直接変更禁止(CLAUDE.md「アーキテクチャの絶対原則」)
- 座標はmm、左上原点、Y下向き。グリッド/ピンピッチ2.5mm、回転は0/90/180/270のみ
- 接続判定の座標一致許容誤差: 0.01mm
- Rustツールチェーンは`~/.cargo/bin`(PATHになければ`export PATH="$HOME/.cargo/bin:$PATH"`)
- テスト実行: `cd src-tauri && cargo test -p madake-core`、フロントは`npx vitest run`
- コミットメッセージは日本語可、末尾に`Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`
- UI見た目の実装(Task 7-8)は`design/madakecad-ui.pen`のデザイン確定が前提。未確定なら着手せずユーザーに確認

---

### Task 1: ピン絶対座標の解決 (madake-core)

ネットリストの前提となる「配置済みシンボルの各ピンの用紙上絶対座標」を計算する。

**Files:**
- Modify: `src-tauri/crates/madake-core/src/symbol.rs`(回転関数を追加)
- Create: `src-tauri/crates/madake-core/src/netlist.rs`(pin_positions関数から開始)
- Modify: `src-tauri/crates/madake-core/src/lib.rs`(`pub mod netlist;`追加)

**Interfaces:**
- Consumes: `SymbolInstance { at, rotation, mirror, symbol_id }`, `SymbolDef::pins`
- Produces: `pub fn pin_positions(inst: &SymbolInstance, def: &SymbolDef) -> Vec<(String, Point)>`(ピン番号→絶対座標。後続タスクがネット判定に使う)

- [ ] **Step 1: 失敗するテストを書く** (`netlist.rs`内`#[cfg(test)]`)

```rust
#[test]
fn pin_positions_apply_rotation_and_translation() {
    let def = builtin_symbols().into_iter().find(|s| s.id == "resistor").unwrap();
    // resistorのピン: ("1", (-7.5, 0)), ("2", (7.5, 0))
    let inst = SymbolInstance {
        id: Uuid::new_v4(),
        symbol_id: "resistor".into(),
        at: Point::new(100.0, 50.0),
        rotation: 90,
        mirror: false,
        reference: "R1".into(),
        value: String::new(),
        attrs: Default::default(),
    };
    let pins = pin_positions(&inst, &def);
    // 90度回転(時計回り、Y下向き座標系): (x,y) -> (-y, x)
    assert!((pins[0].1.x - 100.0).abs() < 1e-9);
    assert!((pins[0].1.y - (50.0 - 7.5)).abs() < 1e-9);
    assert!((pins[1].1.x - 100.0).abs() < 1e-9);
    assert!((pins[1].1.y - (50.0 + 7.5)).abs() < 1e-9);
}
```

- [ ] **Step 2: テストが失敗することを確認**

Run: `cd src-tauri && cargo test -p madake-core pin_positions`
Expected: FAIL(pin_positionsが未定義でコンパイルエラー)

- [ ] **Step 3: 最小実装**

```rust
use crate::geometry::Point;
use crate::model::SymbolInstance;
use crate::symbol::SymbolDef;

/// ローカル座標を回転(0/90/180/270、時計回り、Y下向き座標系)+ミラー+平行移動する。
pub fn transform_local(p: Point, inst: &SymbolInstance) -> Point {
    let (x, y) = if inst.mirror { (-p.x, p.y) } else { (p.x, p.y) };
    let (rx, ry) = match inst.rotation % 360 {
        90 => (-y, x),
        180 => (-x, -y),
        270 => (y, -x),
        _ => (x, y),
    };
    Point::new(inst.at.x + rx, inst.at.y + ry)
}

pub fn pin_positions(inst: &SymbolInstance, def: &SymbolDef) -> Vec<(String, Point)> {
    def.pins
        .iter()
        .map(|pin| (pin.number.clone(), transform_local(pin.at, inst)))
        .collect()
}
```

- [ ] **Step 4: テストが通ることを確認**

Run: `cd src-tauri && cargo test -p madake-core`
Expected: 全テストPASS

- [ ] **Step 5: コミット**

```bash
git add src-tauri/crates/madake-core/src/
git commit -m "feat(core): シンボルピンの絶対座標解決を追加"
```

---

### Task 2: ネットリスト抽出 (madake-core)

配線・ジャンクション・ピン・ネットラベルから接続グラフ(ネットリスト)を導出する。

**Files:**
- Modify: `src-tauri/crates/madake-core/src/netlist.rs`

**Interfaces:**
- Consumes: Task 1の`pin_positions`、`Sheet::entities`
- Produces:
  - `pub struct NetPin { pub reference: String, pub entity_id: EntityId, pub pin: String }`
  - `pub struct Net { pub name: String, pub pins: Vec<NetPin>, pub wire_ids: Vec<EntityId> }`
  - `pub fn extract_netlist(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net>`

接続ルール(仕様):
- Wireの全頂点とピン座標が0.01mm以内で一致すれば同一ノード
- Wire同士は「端点同士の一致」または「一致点上にJunctionがある」場合のみ接続(交差だけでは非接続)
- NetLabelはその座標に一致するWire頂点のネットに名前を与える。同名ラベルのネットは統合
- 無名ネットは"N001"から連番(シート内でソートし決定的に)

- [ ] **Step 1: 失敗するテストを書く**

```rust
fn wire(points: &[(f64, f64)]) -> Entity {
    Entity::Wire(Wire {
        id: Uuid::new_v4(),
        points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        color: "black".into(), sq: 0.3, length_m: None, part_no: None, net: None,
    })
}

fn symbol(sym: &str, reference: &str, x: f64, y: f64) -> Entity {
    Entity::Symbol(SymbolInstance {
        id: Uuid::new_v4(), symbol_id: sym.into(), at: Point::new(x, y),
        rotation: 0, mirror: false, reference: reference.into(),
        value: String::new(), attrs: Default::default(),
    })
}

#[test]
fn wires_connect_symbol_pins_into_one_net() {
    let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
    // R1のピン2(x=107.5)とR2のピン1(x=142.5)を配線で接続
    for e in [symbol("resistor", "R1", 100.0, 50.0),
              symbol("resistor", "R2", 150.0, 50.0),
              wire(&[(107.5, 50.0), (142.5, 50.0)])] {
        sheet.entities.insert(e.id(), e);
    }
    let nets = extract_netlist(&sheet, &builtin_symbols());
    let net = nets.iter().find(|n| n.pins.len() == 2).expect("connected net");
    let mut refs: Vec<_> = net.pins.iter().map(|p| format!("{}:{}", p.reference, p.pin)).collect();
    refs.sort();
    assert_eq!(refs, vec!["R1:2", "R2:1"]);
}

#[test]
fn crossing_without_junction_stays_separate_and_label_names_net() {
    let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
    // 十字交差(端点は不一致) + 横線にラベル
    let w1 = wire(&[(0.0, 10.0), (20.0, 10.0)]);
    let w2 = wire(&[(10.0, 0.0), (10.0, 20.0)]);
    let label = Entity::NetLabel(NetLabel {
        id: Uuid::new_v4(), at: Point::new(0.0, 10.0), name: "24-P1".into(), rotation: 0,
    });
    for e in [w1, w2, label] { sheet.entities.insert(e.id(), e); }
    let nets = extract_netlist(&sheet, &builtin_symbols());
    assert_eq!(nets.len(), 2, "交差のみでは接続しない");
    assert!(nets.iter().any(|n| n.name == "24-P1"));
}
```

- [ ] **Step 2: テストが失敗することを確認**

Run: `cd src-tauri && cargo test -p madake-core netlist`
Expected: FAIL(extract_netlist未定義)

- [ ] **Step 3: 実装**

Union-Find(petgraph不要、自前の小さいdisjoint set)で実装:

```rust
struct DisjointSet { parent: Vec<usize> }
impl DisjointSet {
    fn new(n: usize) -> Self { Self { parent: (0..n).collect() } }
    fn find(&mut self, i: usize) -> usize {
        if self.parent[i] != i { let r = self.find(self.parent[i]); self.parent[i] = r; }
        self.parent[i]
    }
    fn union(&mut self, a: usize, b: usize) { let (ra, rb) = (self.find(a), self.find(b)); if ra != rb { self.parent[ra] = rb; } }
}

const EPS: f64 = 0.01;

fn near(a: &Point, b: &Point) -> bool { a.distance_to(b) < EPS }
```

ノード列挙: 各Wireの「端点」(接続可能点)+各ピン+各Junction+各NetLabelを1ノードとし、
(1) Wire内の全頂点は同一ネット(wire自身が1ノード)、
(2) Wire端点⇔他Wire端点の一致でunion、
(3) Junction座標⇔Wireの任意頂点一致でそのWireとJunctionをunion(Junction経由でT字/十字接続)、
(4) ピン座標⇔Wire任意頂点(端点だけでなく途中点も可: 実配線ではピンに端点を置く運用だが許容)でunion、
(5) NetLabel座標⇔Wire頂点でunion、同名ラベル同士もunion。
最後にグループごとにNetを構築、名前はラベル優先、無名はソート後"N%03d"連番。

- [ ] **Step 4: テストが通ることを確認**

Run: `cd src-tauri && cargo test -p madake-core`
Expected: 全テストPASS

- [ ] **Step 5: コミット**

```bash
git add src-tauri/crates/madake-core/src/netlist.rs
git commit -m "feat(core): ネットリスト抽出(Union-Find、Junction/ラベル対応)"
```

---

### Task 3: BOM・電線リスト生成 (madake-core)

**Files:**
- Create: `src-tauri/crates/madake-core/src/reports.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs`(`pub mod reports;`)

**Interfaces:**
- Consumes: `Project`, `Sheet::entities`
- Produces:
  - `pub fn bom_csv(project: &Project) -> String`(列: 参照記号,型番/値,シンボル,数量 — 参照記号+valueで集計、全シート横断)
  - `pub fn wire_list_csv(project: &Project) -> String`(列: シート,品番,色,sq,長さm — Wireごと1行、part_no無しは空欄)

CSVはUTF-8・カンマ区切り・ダブルクォートエスケープ(フィールドに`,`か`"`か改行があれば`"..."`で囲み`"`は`""`)。ヘッダ行は日本語。

- [ ] **Step 1: 失敗するテストを書く**

```rust
#[test]
fn bom_groups_by_value_and_counts() {
    let mut project = Project::new("t");
    let sid = project.sheets[0].id;
    let sheet = project.sheet_mut(sid).unwrap();
    for (r, v) in [("R1", "10k"), ("R2", "10k"), ("K1", "JZX-22F")] {
        let e = /* Task2のsymbolヘルパ相当。valueを設定 */
            Entity::Symbol(SymbolInstance { id: Uuid::new_v4(), symbol_id: "resistor".into(),
                at: Point::new(0.0, 0.0), rotation: 0, mirror: false,
                reference: r.into(), value: v.into(), attrs: Default::default() });
        sheet.entities.insert(e.id(), e);
    }
    let csv = bom_csv(&project);
    let lines: Vec<_> = csv.lines().collect();
    assert_eq!(lines[0], "参照記号,型番/値,シンボル,数量");
    assert!(lines.iter().any(|l| l.starts_with("\"R1, R2\",10k,resistor,2") || l.starts_with("R1, R2") ));
    assert!(lines.iter().any(|l| l.contains("JZX-22F") && l.ends_with(",1")));
}

#[test]
fn wire_list_contains_attributes() {
    let mut project = Project::new("t");
    let sid = project.sheets[0].id;
    let sheet = project.sheet_mut(sid).unwrap();
    let e = Entity::Wire(Wire { id: Uuid::new_v4(),
        points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
        color: "red".into(), sq: 0.75, length_m: Some(0.4),
        part_no: Some("SAMPLE0001".into()), net: None });
    sheet.entities.insert(e.id(), e);
    let csv = wire_list_csv(&project);
    assert!(csv.lines().nth(1).unwrap().contains("SAMPLE0001,red,0.75,0.4"));
}
```

- [ ] **Step 2: 失敗確認** Run: `cargo test -p madake-core reports` → FAIL
- [ ] **Step 3: 実装**(BTreeMapで(value, symbol_id)集計→参照記号を", "連結。csv_escape関数を共通化)
- [ ] **Step 4: 成功確認** Run: `cargo test -p madake-core` → PASS
- [ ] **Step 5: コミット** `git commit -m "feat(core): BOM/電線リストCSV生成"`

---

### Task 4: SVGエクスポート (madake-core)

印刷品質のSVG出力。図枠(枠線・ゾーン番号・表題欄・改訂欄)+全エンティティを描く。PDFはフェーズ1後半にSVG経由(webviewの印刷 or resvg)で対応するため、まずSVGを正とする。

**Files:**
- Create: `src-tauri/crates/madake-core/src/svg.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs`(`pub mod svg;`)

**Interfaces:**
- Consumes: `Sheet`, `builtin_symbols()`, Task 1の`transform_local`
- Produces: `pub fn sheet_to_svg(sheet: &Sheet, symbols: &[SymbolDef]) -> String`

仕様:
- ルート: `<svg xmlns="http://www.w3.org/2000/svg" width="{W}mm" height="{H}mm" viewBox="0 0 {W} {H}">`(W,H=`sheet.paper_mm()`)
- 図枠: 外周から10mm内側に太線枠(stroke-width 0.5)、ゾーン番号(横=数字1..zone_cols、縦=英字A..)を枠外周に配置、右下に表題欄(幅180mm×高さ40mm程度の罫線表: 図番/品名/尺度/日付/設計/製図/検図/承認/Rev)
- Wire: `<polyline>`、色は`color_hex()`(red→#c00000等の対応表、未知色はそのまま文字列出力)、stroke-width 0.35
- Symbol: プリミティブをtransform_localで変換して`<path>`/`<circle>`/`<rect>`/`<text>`出力、参照記号とvalueをシンボル上部に文字高2.5mmで併記
- Junction: 塗り潰し円 r=0.6
- NetLabel/Text: `<text>`(font-family="sans-serif", font-sizeはmm単位そのまま)

- [ ] **Step 1: 失敗するテストを書く**

```rust
#[test]
fn svg_contains_frame_wire_and_symbol() {
    let mut sheet = Sheet::new("TB1", PaperSize::A3, Orientation::Landscape);
    sheet.title_block.drawing_no = "MDK-001".into();
    let w = Entity::Wire(Wire { id: Uuid::new_v4(),
        points: vec![Point::new(50.0, 50.0), Point::new(100.0, 50.0)],
        color: "red".into(), sq: 0.3, length_m: None, part_no: None, net: None });
    let s = Entity::Symbol(SymbolInstance { id: Uuid::new_v4(), symbol_id: "fuse".into(),
        at: Point::new(120.0, 50.0), rotation: 0, mirror: false,
        reference: "F1".into(), value: "5A".into(), attrs: Default::default() });
    for e in [w, s] { sheet.entities.insert(e.id(), e); }
    let svg = sheet_to_svg(&sheet, &builtin_symbols());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("viewBox=\"0 0 420 297\""));
    assert!(svg.contains("MDK-001"));           // 表題欄
    assert!(svg.contains("polyline"));           // 配線
    assert!(svg.contains("F1"));                 // 参照記号
    assert!(svg.matches("<text").count() >= 3);  // ゾーン番号+表題欄+参照記号
}
```

- [ ] **Step 2: 失敗確認** → FAIL(sheet_to_svg未定義)
- [ ] **Step 3: 実装**(String連結で十分。XMLエスケープ関数`xml_escape`を用意し全テキストに適用)
- [ ] **Step 4: 成功確認** `cargo test -p madake-core` → PASS。さらに目視確認: `cargo test`後、手動で1枚書き出してブラウザで開く
- [ ] **Step 5: コミット** `git commit -m "feat(core): JIS図枠つきSVGエクスポート"`

---

### Task 5: エクスポートのTauri IPC・MCPツール公開

**Files:**
- Modify: `src-tauri/src/lib.rs`(IPC: `export_svg(sheet_id, path)`, `export_bom(path)`, `export_wire_list(path)`, `get_netlist(sheet_id)`)
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs`(MCPツール: `get_netlist`, `export_bom`, `export_svg`)

**Interfaces:**
- Consumes: Task 2 `extract_netlist` / Task 3 `bom_csv`,`wire_list_csv` / Task 4 `sheet_to_svg`
- Produces(IPC): `export_svg(sheet_id: Uuid, path: String) -> Result<(), String>`ほか、いずれもファイル書き出し
- Produces(MCP): `get_netlist(sheet_id?) -> String(JSON)`, `export_bom(path) -> ok`, `export_svg(sheet_id?, path) -> ok`

- [ ] **Step 1: MCPツールを追加**(既存`#[tool_router]`ブロックに追記。sheet_id省略時は先頭シート=`resolve_sheet`を再利用)
- [ ] **Step 2: Tauri IPCハンドラを追加し`generate_handler!`に登録**
- [ ] **Step 3: ビルド確認** Run: `cd src-tauri && cargo build` → Finished
- [ ] **Step 4: スモークテスト**(アプリ起動→curlでMCP `export_svg`→ファイル生成確認→アプリ終了。手順は前回スモークテストと同様: initialize→initialized→tools/call)
- [ ] **Step 5: コミット** `git commit -m "feat: ネットリスト/BOM/SVGをIPCとMCPに公開"`

---

### Task 6: フロントエンド基盤 — IPCラッパとPiniaドキュメントストア

デザイン非依存の土台。patchイベントだけを信頼してミラーを保つ。

**Files:**
- Create: `src/ipc.ts`
- Create: `src/stores/document.ts`
- Create: `src/stores/document.test.ts`
- Modify: `package.json`(`"test": "vitest run"`スクリプト追加)

**Interfaces:**
- Consumes: Tauri IPC(`get_project`/`execute_command`/`undo`/`redo`)、`doc:patch`イベント
- Produces(store): `useDocumentStore()`: state `{ project, revision, canUndo, canRedo }`、action `applyPatch(patch: Patch)`, `bootstrap()`, `execute(cmd: Command)`
- 型定義: `src/ipc.ts`に`Patch`/`PatchOp`/`Command`/`Entity`等のTS型をmadake-coreのserde表現(tagged union: `{type: ...}` / `{kind: ...}` / `{op: ...}`)と一致させて手書きする

- [ ] **Step 1: 失敗するテストを書く**(`document.test.ts`、Tauri APIはモック)

```typescript
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { useDocumentStore } from "./document";

describe("document store", () => {
  beforeEach(() => setActivePinia(createPinia()));

  it("applies entity_upserted and entity_removed patches", () => {
    const store = useDocumentStore();
    store.project = { format_version: 1, name: "t", wire_parts: [], sheets: [
      { id: "s1", name: "Sheet1", size: "A3", orientation: "Landscape",
        zone_cols: 4, zone_rows: 6, title_block: {}, revisions: [], entities: {} } as any,
    ]};
    store.applyPatch({ revision: 1, ops: [
      { op: "entity_upserted", sheet_id: "s1",
        entity: { kind: "junction", id: "e1", at: { x: 1, y: 2 } } as any },
    ]});
    expect(store.revision).toBe(1);
    expect(store.project!.sheets[0].entities["e1"]).toBeTruthy();
    store.applyPatch({ revision: 2, ops: [{ op: "entity_removed", sheet_id: "s1", id: "e1" }] });
    expect(store.project!.sheets[0].entities["e1"]).toBeUndefined();
  });

  it("replaces whole project on project_replaced", () => {
    const store = useDocumentStore();
    store.applyPatch({ revision: 5, ops: [
      { op: "project_replaced", project: { format_version: 1, name: "new", wire_parts: [], sheets: [] } as any },
    ]});
    expect(store.project!.name).toBe("new");
    expect(store.revision).toBe(5);
  });
});
```

- [ ] **Step 2: 失敗確認** Run: `npx vitest run` → FAIL
- [ ] **Step 3: 実装**(store: PatchOpごとのswitch。`sheet_added`はindexにinsert、`sheet_meta_updated`はentities以外を差し替え。ipc.ts: `invoke`ラッパと`listen("doc:patch")`購読を`bootstrap()`で設定)
- [ ] **Step 4: 成功確認** `npx vitest run` + `npx vue-tsc --noEmit` → PASS
- [ ] **Step 5: コミット** `git commit -m "feat(ui): IPCラッパとPiniaドキュメントストア(patchミラー)"`

---

### Task 7: Canvas2Dレンダラコア(ビューポート+グリッド+図枠+エンティティ描画)

見た目のスタイリングはPencilデザインに従うが、レンダラの座標変換・描画そのものはデザイン非依存なので先行実装する。**キャンバス上の配色(背景色・グリッド色・選択色)は`design/`のデザイントークン確定後に`src/canvas/theme.ts`で差し替えられる構造にする。**

**Files:**
- Create: `src/canvas/viewport.ts`(ワールドmm⇔スクリーンpx変換)
- Create: `src/canvas/viewport.test.ts`
- Create: `src/canvas/theme.ts`(色トークン。仮値でよい)
- Create: `src/canvas/renderer.ts`(描画本体。Canvas 2D contextに対しSheet全体を描画)

**Interfaces:**
- Consumes: Task 6のstore型(`Sheet`, `Entity`)、`list_symbols` IPC
- Produces:
  - `class Viewport { scale: number; originX: number; originY: number; toScreen(p:{x,y}):{x,y}; toWorld(p:{x,y}):{x,y}; zoomAt(screenPt, factor): void; pan(dxPx, dyPx): void; snap(p:{x,y}, pitch?:number):{x,y} }`
  - `function renderSheet(ctx: CanvasRenderingContext2D, sheet: Sheet, symbols: SymbolDef[], vp: Viewport, opts: { selection: Set<string> }): void`

- [ ] **Step 1: viewportの失敗するテストを書く**

```typescript
import { describe, it, expect } from "vitest";
import { Viewport } from "./viewport";

describe("Viewport", () => {
  it("roundtrips world<->screen and zooms around anchor", () => {
    const vp = new Viewport();
    vp.scale = 4; vp.originX = 10; vp.originY = 20; // 1mm = 4px
    const s = vp.toScreen({ x: 100, y: 50 });
    const w = vp.toWorld(s);
    expect(w.x).toBeCloseTo(100); expect(w.y).toBeCloseTo(50);
    const anchor = { x: 300, y: 200 };
    const before = vp.toWorld(anchor);
    vp.zoomAt(anchor, 1.25);
    const after = vp.toWorld(anchor);
    expect(after.x).toBeCloseTo(before.x); expect(after.y).toBeCloseTo(before.y);
  });

  it("snaps to 2.5mm grid", () => {
    const vp = new Viewport();
    expect(vp.snap({ x: 101.2, y: 48.9 })).toEqual({ x: 100, y: 50 });
  });
});
```

- [ ] **Step 2: 失敗確認** `npx vitest run` → FAIL
- [ ] **Step 3: viewport実装+renderer実装**(renderer: 用紙白地→グリッド点(2.5mm、ズーム閾値で間引き)→図枠/ゾーン/表題欄(svg.rsと同じ寸法ロジックをTSに移植)→Wire(色マップはtheme.ts)→Symbol(プリミティブ描画+参照記号)→Junction→選択ハイライト。rendererは純関数でDOM非依存、テストはviewportのみ)
- [ ] **Step 4: 成功確認** `npx vitest run` + `npx vue-tsc --noEmit` → PASS
- [ ] **Step 5: コミット** `git commit -m "feat(ui): Canvas2Dレンダラコア(ビューポート/グリッド/図枠)"`

---

### Task 8: エディタUI組み立て(★Pencilデザイン確定が前提)

`design/madakecad-ui.pen`のデザインに従い画面を組む。**着手前にデザインが承認済みか必ず確認。未承認ならこのタスクで止めてユーザーに確認する。**

**Files:**
- Create: `src/components/EditorLayout.vue`(デザインどおりの全体レイアウト)
- Create: `src/components/CanvasView.vue`(canvas要素+pointerイベント→ツール状態機械)
- Create: `src/components/Toolbar.vue` / `SymbolPalette.vue` / `PropertiesPanel.vue` / `SheetTabs.vue` / `CommandBar.vue`
- Create: `src/tools/`(select / place-symbol / draw-wire / pan の状態機械。ドラッグ中ローカルプレビュー→確定時`store.execute(Command)`)
- Modify: `src/App.vue`(デバッグ画面をEditorLayoutに差し替え)

**Interfaces:**
- Consumes: Task 6 store、Task 7 renderer/viewport
- Produces: 動くエディタ。操作仕様: 左ドラッグ=選択矩形/移動、中ドラッグorSpace+ドラッグ=パン、ホイール=ズーム、W=配線ツール、Esc=選択ツール、Delete=削除、Cmd+Z/Cmd+Shift+Z=undo/redo、配線クリックで頂点確定・ダブルクリックで終了、ピン/グリッドスナップ表示

- [ ] **Step 1: デザイン承認確認**(design/にpenファイルとユーザー承認があるか。なければ停止)
- [ ] **Step 2: レイアウトとコンポーネントをデザイントークンどおり実装**
- [ ] **Step 3: ツール状態機械実装**(各ツール: `onPointerDown/Move/Up(world: {x,y})`+`render(ctx, vp)`のインターフェース)
- [ ] **Step 4: 手動スモーク** `npm run tauri dev`: シンボル配置→配線→undo/redo→保存/読込→MCP経由編集のリアルタイム反映
- [ ] **Step 5: 型チェック+テスト** `npx vue-tsc --noEmit && npx vitest run` → PASS
- [ ] **Step 6: コミット** `git commit -m "feat(ui): エディタUI(Pencilデザイン準拠)"`

---

### Task 9: 保存/読込/エクスポートのUIメニュー統合(★デザイン準拠)

**Files:**
- Modify: `src/components/Toolbar.vue`ほか(ファイルメニュー: 新規/開く/保存/名前を付けて保存/エクスポート)
- Consumes: `@tauri-apps/plugin-dialog`の`open()`/`save()`、Task 5のIPC

- [ ] **Step 1: ダイアログ→IPC呼び出しを実装**(拡張子フィルタ: `.mdkproj`、SVG/CSV)
- [ ] **Step 2: 手動スモーク**(保存→再起動→開く→同一図面)
- [ ] **Step 3: コミット** `git commit -m "feat(ui): ファイルメニューとエクスポート統合"`

---

## Self-Review結果

- スペック§3.4(ネットリスト)→Task 1-2、§5(MCP拡張のget_netlist/export)→Task 5、フェーズ1のBOM/電線リスト→Task 3、SVG→Task 4、エディタUI→Task 6-9でカバー
- PDF出力はスペック上フェーズ1だが、本プランではSVGを正としPDF変換は別プラン(SVG確定後)に分離した
- 端子台の動的シンボル・AutoCAD風コマンドライン・レイヤはフェーズ1後半として次プランに送る(YAGNI: エディタが動いてから)
- 型整合: `extract_netlist(&Sheet, &[SymbolDef])`をTask 2で定義しTask 5が同シグネチャで使用。`transform_local`はTask 1定義→Task 4使用
