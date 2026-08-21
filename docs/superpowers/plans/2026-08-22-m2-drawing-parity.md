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

- [x] Step 1 (red): テスト: renumber_wires(Append)が未採番ネットのみへ連番/Renumberが全振り直し/手動線番・ネットラベルは優先保持/undoで旧線番復元/線番はネット内全Wireへ同値
- [x] Step 2 (green): Command実装。コミット
- [x] Step 3 (red/green): 電線リストCSVに線番列、ネットリストのNet.name線番優先。SVGに線番テキスト(代表位置・mono)。表示クラス「線番」。コミット

実装メモ(確定): 線番は`madake-core/src/wire_no.rs`。Command は `renumber_wires { sheet_id?, mode: append|renumber, start }` と
`set_wire_numbers { sheet_id, numbers: [{wire_id, number}] }`(後者が逆コマンド兼個別編集)。採番順は**上→下、同じ高さなら左→右**
(同値はwire idで安定化)。番号は既存の線番・ネットラベルと衝突しないよう予約表でスキップし、sheet_id省略時は図面全体で一意。
ネットラベル付きネットは採番しない(`Net.name`優先順=ラベル>線番>自動名、`Net`に`label`/`wire_no`を追加)。
SVGの代表位置=ネット内で最長の線分の中点、横線は上方2.5mm・縦線は左方2.5mm、font-family=monospace 2.5mm。
表示クラス「線番」はキャンバス描画とセットのTask 4で追加する。

### Task 4: 線番UI (ダイアログ+キャンバス)

- [x] Step 1 (red): vitest: 採番ダイアログ(方式/開始番号/対象/既存の扱い)がCommandを組み立てる/線番クリックで個別編集(update系Command)
- [x] Step 2 (green): デザイン(.pen「線番自動採番ダイアログ」)どおり実装。リボン「配線」の「線番挿入」todoを置換。renderer.tsに線番描画+表示トグル。実機確認+コミット

実装メモ(確定): ダイアログ=`components/WireNumberDialog.vue`+`stores/wireNumbers.ts`(対象=現在のシート→sheet_id指定 /
プロジェクト全体→省略、既存=保持→append / 振り直す→renumber)。実行後は採番したネット数をステータスバーへ出す。
ゾーン基準ラジオは無効表示(M4)。キャンバス描画は`canvas/wireNumbers.ts`の純関数(ネットごと最長線分の中点、
横線は上・縦線は左へ2.5mm)+`renderer.ts`、色は`theme.wireNumber`。表示クラス「線番」(8クラス目)は画面のみでSVG/PDFへは非反映。
個別編集はプロパティパネルの「線番」行→`components/propertyCommands.ts`が`set_wire_numbers`を組み立てる(undo可)。

### Task 5: ハーネス境界

- [x] Step 1 (red): Rustテスト: Harness entityのadd/update/remove+undo/内包判定(全点内包のみ所属・境界上は所属・部分内包と外は非所属)/電線リストにハーネス列/SVGに破線+名前
- [x] Step 2 (green): モデル+svg.rs実装。コミット
- [x] Step 3 (red/green): UI: リボン「配線」に「ハーネス」ツール(矩形ドラッグ)、renderer.ts破線描画、プロパティパネル(名前編集・含む電線数表示)、表示クラス追加。実機確認+コミット

実装メモ(確定): コアは`madake-core/src/harness.rs`。Entityは`Harness { points, name, note }`(kind:"harness")で、
**新Commandは作らず**既存の add/update/delete/move_entity で扱う(逆コマンドも既存のまま)。所属判定は外接矩形への
全点内包(境界線上は内側・許容誤差1e-9mm)、入れ子の囲みは**最も小さい囲みを優先**。名前は参照記号と同じ規則で
`W1, W2 …`(既存W番号の最大+1を自動提案)。電線リストCSVは線番の次に「ハーネス」列を追加(旧列の並びは不変)。
SVGは破線ポリゴン(`stroke-dasharray="3 2"`・線幅0.25mm)を配線の**背面**に描き、名前は囲み左上角の外側
(右へ1mm・上へ1mm、2.5mm)。フロントは`canvas/harness.ts`(同一ルールの純関数)+`renderer.ts`の`drawHarness`
(`theme.harness`)、ツールは`controller.ts`の`tool="harness"`(矩形ドラッグ・ドラッグ中も同じ破線でプレビュー)。
名前・備考の編集は`components/propertyCommands.ts`の`harnessUpdateCommand`→`update_entity`。表示クラス
「ハーネス」(9クラス目)は画面のみでSVG/PDFへは非反映。`format_version`は据え置き(旧ファイルに新kindは現れないため互換)。

### Task 6: シート間クロスリファレンス

- [x] Step 1 (red): Rustテスト: extract_netlist_projectが同名ラベルをシート横断統合/相手先アドレス算出(「/2.B3」書式・複数列挙・自シート除外)/シート跨ぎネットの電線リスト・ERC統合
- [x] Step 2 (green): コア実装+SVGのXRefラベル描画。コミット
- [x] Step 3 (red/green): UI: renderer.ts描画+プロパティパネル「相手先」リンク(クリックでreveal)。実機確認+コミット

実装メモ(確定): コアは`madake-core/src/xref.rs`。**統合キーはネットラベル名**で、`extract_netlist_project`が
シートごとのネットリストを取り、同名ラベルを共有するネット同士をシートを跨いで1ネットに統合する
(`ProjectNet { name, label_names, sites, members }`)。**相手先アドレスは同名ラベルの他シートでの所在**
(`NetSite { sheet_id, sheet_no(1始まり), sheet_name, zone, label_id, name }` の `/シート.ゾーン`)で、
自シート内の所在は除外・同一住所は重複排除・相手がいなければ何も表示しない。この定義なら
Rust(`xref.rs`)とTS(`canvas/xref.ts`)で同じ計算を持てるので、キャンバスとSVGが必ず一致する。
ゾーンは図枠(FRAME_MARGIN=10mm)を`zone_cols`×`zone_rows`で割り、行=英字を上から・列=数字を左から
(図枠外の点は最も近いゾーンへ丸める)。**表示位置はネットラベル本文の右脇**(本文幅を文字数×2.5mm×0.6で
見積り+間隔1mm、ベースラインは本文と同じ)で、等幅・文字高2.0mm、複数は半角空白区切り。
SVG/PDFは`svg::project_sheet_to_svg` / `pdf::project_sheet_to_pdf`(シート単体の`sheet_to_svg`は
従来どおりXRefなし)。ERCは`verify::verify_project`を追加し、`erc.label_conflict`をシートを跨いだ
統合ネットで評価する(2枚に跨る競合も1件にまとまる)。Tauri IPC / MCP / Link APIのSVG・PDF出力と
全体検証をプロジェクト文脈へ切替。フロントは`canvas/xref.ts`+`renderer.ts`(`theme.xref`)、
表示クラスは**「ネットラベル」に含める**(新クラスは作らない)。プロパティパネルの「相手先」行は
acad-blue mono 600のリンクで、クリックで該当シートへ切替+相手ラベルを選択+ズーム
(検証結果パネルと同じ`controller.reveal`)。

### Task 7: 露出と仕上げ

- [ ] Step 1: MCPツール/Link API/CLIへ露出(renumber_wires・set_revisions・harness操作。既存execute_commands経由で足りるものはドキュメントのみ)
- [ ] Step 2: PDFデモ更新(改訂2行+線番+ハーネス+2シートXRef入りのexample)、docs/04・05の更新(EN+JA)、feature-inventory更新
- [ ] Step 3: gen_spec --check・cargo test・vitest・vue-tsc全green確認、受け入れ基準(spec各節)を実機で検証してスクショ記録

## 受け入れ基準(仕様より)

- 改訂2行追加→キャンバス/SVG/PDFに同一の表、表題欄Rev=最新mark、undo可
- 自動採番後、全ネット一意の線番が画面・PDF・電線リストで一致。個別編集は追い番で保持
- ハーネス矩形→内包ワイヤの電線リストにハーネス名、SVG/PDFに破線+名前
- シート1のラベルに「/2.B3」が出て、シート2側に逆参照。跨ぎネットは帳票・検証で1ネット
