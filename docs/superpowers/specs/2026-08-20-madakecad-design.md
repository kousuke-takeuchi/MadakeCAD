# MadakeCAD 全体設計(スペック)

日付: 2026-08-20
状態: フェーズ0実装済み。フェーズ1はUIデザイン(Pencil)確定後に着手。

## 1. 目的と背景

***REMOVED***向けロボットの電気配線図(現状KiCad 8で作図)を、産業基準に準拠した図面として作成できる電気CADを開発する。目標品質は***REMOVED***社の機器構成図・基板図面(ユーザー提供PDF: SAMPLE0001-***REMOVED***等):

- JIS図枠: 表題欄(図番・品名・尺度・日付・設計/製図/検図/承認)、改訂欄、ゾーン番号(縦横アドレス)
- 電線管理: 線色、線径(0.3〜3.5sq)、長さ、線色+sqごとの電線品番マスタ
- 端子台・コネクタの型番+ピン番号単位の結線、ハーネス境界(破線囲み)
- 部品表(BOM)・電線リストの出力

将来目標: Amazon等で買える市販部品を含む部品DB、配線検証(電圧降下・線径適合・ヒューズ協調)、SPICEシミュレーション、AIによる自動作図。

## 2. 技術選定(確定済み)

| 項目 | 選定 | 理由 |
|---|---|---|
| シェル | Tauri 2 | 軽量・Rustコアと同居 |
| UI | Vue 3 + TypeScript + Pinia | ユーザー選好 |
| 描画 | Canvas2D自作レンダラ | 印刷品質・スナップ制御・依存最小 |
| コア | Rust (madake-core) | UI非依存、ヘッドレス実行可能 |
| AI連携 | 内蔵MCPサーバー (rmcp 3.x, Streamable HTTP) | Claude Code/Desktopから直接編集 |
| 部品DB | SQLite (rusqlite) ※フェーズ2 | ローカル完結 |
| シミュレーション | 配線検証(自作)→ngspice(フェーズ3) | 配線図用途を優先 |
| UIデザイン | Pencil (pen.dev) で design-first | ユーザー方針 |

## 3. アーキテクチャ

### 3.1 Command中心設計(実装済み)

全編集はシリアライズ可能な`Command`(serde tagged enum)として`Engine::execute()`で実行される。エンジンは適用時に逆コマンドを生成してundo/redo履歴を持ち、変更差分を`Patch`(`PatchOp`列+単調増加revision)としてbroadcastする。Tauriはpatchを`doc:patch`イベントでwebviewへ転送し、フロントのPiniaストアはpatchだけを信頼してミラーを更新する(フロントでモデルを直接変更しない)。

### 3.2 モジュール構成

```
src-tauri/crates/madake-core   ドキュメントモデル、Command、シンボル、IO、(将来)ネットリスト/検証
src-tauri/crates/madake-mcp    MCPサーバー。SharedDoc { engine, patches } がUI/MCP共通の編集入口
src-tauri/src                  Tauri IPC(execute_command/undo/redo/save/load等)、MCP起動、patch転送
src/                           Vue UI(フェーズ1で本実装。現在は仮デバッグ画面)
design/                        Pencil (.pen) デザインファイル
```

### 3.3 データモデル(実装済み、フェーズ1で拡張)

- `Project`: format_version, name, sheets[], wire_parts[](電線品番マスタ)
- `Sheet`: JIS図枠(A4〜A0、縦横)、zone分割、表題欄、改訂欄、entities(BTreeMap<Uuid, Entity>)
- `Entity`(tagged enum): Symbol / Wire / Junction / NetLabel / Text
  - `SymbolInstance`: symbol_id, at(mm), rotation(0/90/180/270), mirror, reference("K1"), value(型番), attrs
  - `Wire`: points[](直交ポリライン), color, sq, length_m, part_no, net
- `SymbolDef`: JSONベクタプリミティブ(Line/Circle/Arc/Rect/Text)+ピン定義。JIS C 0617系15種を同梱。端子台/コネクタのピン数可変シンボルはフェーズ1
- 保存形式 `.mdkproj`: 整形JSON。KiCadインポート(S式→本モデル)はフェーズ2

### 3.4 ネットリスト(フェーズ1)

座標一致(2.5mmグリッド上の端点・ピン)とJunctionから接続グラフを構築し、NetLabelで論理結合。BOM(参照記号+型番集計)と電線リスト(品番・色・sq・長さ)はネットリストから導出する。

### 3.5 検証エンジン(フェーズ2)

- ERC: 未接続ピン、参照記号重複、短絡(異電位ネットの直結)
- 電気検証: 電源からの到達性、電圧降下(線長×断面積×電流)、線径-電流容量適合、ヒューズ協調
- 検証結果は`Diagnostic { severity, message, entity_ids, sheet_id }`のリストとしてMCP/UI双方に返す

### 3.6 シミュレーション(フェーズ3)

部品DBにSPICEモデルを紐付け、ngspice(libngspice FFIまたはサブプロセス)でDC動作点/過渡解析。ネットリスト抽出器がSPICEネットリストも出力する。

## 4. UI設計(Pencilでデザイン後に確定)

AutoCADをベースにした画面構成の想定(Pencilデザインで詳細を決める):

- 中央: 図面キャンバス(無限パン/ズーム、mm座標、グリッド、用紙枠)
- 上部: メニュー+ツールバー(選択/配線/シンボル/テキスト/計測)
- 左: シンボルパレット(カテゴリ別)、シートツリー
- 右: プロパティパネル(選択物の属性: 線色/sq/品番/参照記号/型番)
- 下部: コマンドライン(AutoCAD風キー入力: L=配線, E=消去等)+座標/スナップ状態バー
- シートタブ(TB1前部ボックス/TB2底部ボックス…のような複数シート)

対話ツールはステートマシンとして実装し、ドラッグ中はローカルプレビュー、確定時にCommand発行。

## 5. MCP API(実装済み、フェーズ1で拡張)

実装済みツール: `get_project` / `list_symbols` / `place_symbol` / `draw_wire` / `execute_commands`(全Commandスキーマ公開) / `undo` / `redo`
フェーズ1追加予定: `get_netlist` / `run_checks` / `export_bom` / `export_pdf`

## 6. フェーズ計画

- フェーズ0(完了): Commandエンジン+モデル+シンボル+IO、MCPサーバー、Tauri足場、スモークテスト
- フェーズ1: Canvas2DエディタUI(Pencilデザイン準拠)、ネットリスト、BOM/電線リストCSV、PDF/SVG出力、端子台動的シンボル、AutoCAD風コマンドライン
- フェーズ2: 検証エンジン、部品DB(SQLite+購入先URL)、KiCadインポート
- フェーズ3: ngspiceシミュレーション、アプリ内AIチャット(検討)

## 7. 非目標(現時点)

- PCBレイアウト・ガーバー出力(電子基板CADではない)
- クラウド同期・同時編集(ローカルファースト)
- Windows/Linuxビルド(まずmacOS。Tauriなので移植は容易)
