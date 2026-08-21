# MadakeCAD M2 (参考図面の完全再現) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 改訂欄・線番・ハーネス境界・シート間クロスリファレンスを実装し、参考図面と同等品質の図面をMadakeCADだけで出図できるようにする。仕様=`docs/internal/specs/m2-drawing-parity.md`、記法=`.pen`「M2デザイン - 図面記法(規格準拠)」、UI=「M2デザイン - ハーネス/XRef操作UI」+改訂欄/線番ダイアログ(デザイン確定済み)。

**共通ルール:** 全編集はCommand経由(undo必須)。全テストに対訳仕様文(tests-as-spec)を付け、タスク完了ごとに`python3 scripts/gen_spec.py`で仕様書を再生成してコミットに含める。新規UI文字列はi18nカタログ(en/ja)経由。

## 設計決定

- **改訂欄**: モデルは既存`Sheet::revisions`+Command `set_revisions`(実装済み・テスト済み)を使用。残りは描画(svg.rs / renderer.ts、ISO 7200様式: 表題欄直上・下から積む・列=記号/日付/内容/承認)とダイアログ。行数上限=表題欄上の空きに収まる分(既定6行)、溢れたら古い行から非表示(全履歴はデータに保持)
- **線番**: ネット単位属性。`Wire::net`を線番保存先として正式化し、新Command `renumber_wires { sheet_id: Option<Id>, mode: Append|Renumber, start: u32 }`(逆コマンド=旧線番マップ復元)。代表位置=ネット内最長ワイヤ線分の中点、ワイヤ上方2.5mm・mono。表示クラス「線番」(8クラス目)
- **ハーネス**: 新Entity `Harness { points(矩形), name, note }`。所属=幾何内包(ワイヤ全点が囲み内)。破線はIEC 61082-1様式(SVGはstroke-dasharray、canvasはsetLineDash)。表示クラス「ハーネス」(9クラス目)
- **XRef**: `extract_netlist_project(project)`で同名ラベルをシート横断統合。ラベル脇に「/シート.ゾーン」を自動描画(複数は列挙)。プロパティパネルの相手先クリックでreveal(シート切替+選択+ズーム)
- **帳票連動**: 電線リストに線番・ハーネス列を追加(From-To化はM4)
- **露出**: 全機能をMCPツール/Link API/CLIにも露出(エージェントが線番採番・改訂追加できること)

### Task 1: 改訂欄の描画 (svg.rs + renderer.ts)

- [x] Step 1 (red): Rustテスト: SVG出力に改訂表(2行・下から積む・列見出し)が含まれる/0行なら描かない/表題欄Revが最新markになる/7行目以降は古い行から省略
- [x] Step 2 (green): svg.rsに改訂欄描画を実装(ISO 7200様式・図枠テンプレの寸法定数)、表題欄Rev連動。コミット
- [x] Step 3 (red/green): renderer.ts(canvas)に同一ルールで描画+vitestでレイアウト計算(行の並び・省略・列分割)をテスト。gen_spec再生成、コミット

寸法(確定): 表題欄と同じ右端・幅120mm、行高8mm(表題欄と同一)、列幅=記号14 / 日付28 / 内容56 / 承認22mm、最下段が列見出し、その上に古い改訂から下→上。最大6行で溢れは古い行から省略(データは保持)。0行なら何も描かない。

### Task 2: 改訂欄編集ダイアログ (Vue)

- [x] Step 1 (red): vitest: ダイアログstoreが行追加/編集/削除でset_revisions Commandを送る/保存でrevision増加/キャンセルで送らない
- [x] Step 2 (green): デザイン(.pen「改訂欄編集ダイアログ」)どおり実装(CAD調・白テーブル・+改訂を追加・記号自動採番A→B→C)。i18nキー追加。実機確認+コミット

起動ボタン(確定): IAの「プロジェクト」タブは未実装プレースホルダのため、「回路図」タブ>「回路図を編集」グループへ小ボタン「改訂欄」(FileClockアイコン)を追加した。プロジェクトタブ実装時にそちらへ移す。

### Task 3: 線番コア (madake-core)

- [ ] Step 1 (red): テスト: renumber_wires(Append)が未採番ネットのみへ連番/Renumberが全振り直し/手動線番・ネットラベルは優先保持/undoで旧線番復元/線番はネット内全Wireへ同値
- [ ] Step 2 (green): Command実装。コミット
- [ ] Step 3 (red/green): 電線リストCSVに線番列、ネットリストのNet.name線番優先。SVGに線番テキスト(代表位置・mono)。表示クラス「線番」。コミット

### Task 4: 線番UI (ダイアログ+キャンバス)

- [ ] Step 1 (red): vitest: 採番ダイアログ(方式/開始番号/対象/既存の扱い)がCommandを組み立てる/線番クリックで個別編集(update系Command)
- [ ] Step 2 (green): デザイン(.pen「線番自動採番ダイアログ」)どおり実装。リボン「配線」の「線番挿入」todoを置換。renderer.tsに線番描画+表示トグル。実機確認+コミット

### Task 5: ハーネス境界

- [ ] Step 1 (red): Rustテスト: Harness entityのadd/update/remove+undo/内包判定(全点内包のみ所属・境界上・部分内包は非所属)/電線リストにハーネス列/SVGに破線+名前
- [ ] Step 2 (green): モデル+svg.rs実装。コミット
- [ ] Step 3 (red/green): UI: リボン「配線」に「ハーネス」ツール(矩形ドラッグ)、renderer.ts破線描画、プロパティパネル(名前編集・含む電線数表示)、表示クラス追加。実機確認+コミット

### Task 6: シート間クロスリファレンス

- [ ] Step 1 (red): Rustテスト: extract_netlist_projectが同名ラベルをシート横断統合/相手先アドレス算出(「/2.B3」書式・複数列挙・自シート除外)/シート跨ぎネットの電線リスト・ERC統合
- [ ] Step 2 (green): コア実装+SVGのXRefラベル描画。コミット
- [ ] Step 3 (red/green): UI: renderer.ts描画+プロパティパネル「相手先」リンク(クリックでreveal)。実機確認+コミット

### Task 7: 露出と仕上げ

- [ ] Step 1: MCPツール/Link API/CLIへ露出(renumber_wires・set_revisions・harness操作。既存execute_commands経由で足りるものはドキュメントのみ)
- [ ] Step 2: PDFデモ更新(改訂2行+線番+ハーネス+2シートXRef入りのexample)、docs/04・05の更新(EN+JA)、feature-inventory更新
- [ ] Step 3: gen_spec --check・cargo test・vitest・vue-tsc全green確認、受け入れ基準(spec各節)を実機で検証してスクショ記録

## 受け入れ基準(仕様より)

- 改訂2行追加→キャンバス/SVG/PDFに同一の表、表題欄Rev=最新mark、undo可
- 自動採番後、全ネット一意の線番が画面・PDF・電線リストで一致。個別編集は追い番で保持
- ハーネス矩形→内包ワイヤの電線リストにハーネス名、SVG/PDFに破線+名前
- シート1のラベルに「/2.B3」が出て、シート2側に逆参照。跨ぎネットは帳票・検証で1ネット
