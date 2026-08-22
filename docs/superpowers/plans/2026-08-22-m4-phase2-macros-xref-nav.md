# MadakeCAD M4フェーズ2 (回路マクロ+コイル/接点XRef+検索/ナビゲータ) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M4優先順の続き3項目: ①回路マクロ(§2、保存/挿入/バリアント)、②コイル⇔接点クロスリファレンス(§4)、③プロジェクト内検索+デバイスナビゲータ+Surfer(§10)。仕様=`docs/internal/specs/m4-industrial-core.md`、デザイン=`.pen`「M4デザイン - 回路マクロ」「M4デザイン - 端子台チャート/帳票様式」(コイル接点XRef記法)「M4デザイン - 検索/デバイスナビゲータ/Surfer」(確定済み)。

**共通ルール:** 全編集はCommand経由。tests-as-spec+gen_spec再生成。i18n(en/ja)。ドキュメントEN+JA同期。

## 設計決定(スコープの割り切り含む)

- **マクロ形式**: M3テンプレートと同形式(`{id, name, description, commands}`)+`variants: [{key:"A", name, commands}]`+`base_point: Point`。保存先`~/MadakeCAD/macros/`。**値セット(プレースホルダ)はフェーズ3へ**(仕様§2の残り)
- **マクロ保存**: 選択範囲のエンティティをCommand列へ逆変換(add_entity列+相対座標化)。基準点=選択範囲の左下ピン(デザイン確定済みの既定)
- **マクロ挿入**: テンプレートの`execute_batch`+UUID振り直し機構を再利用。挿入時に参照記号を自動再採番(既存最大+1)・線番クリア。配置プレビュー+R回転+**Tabでバリアント切替**。部品挿入ダイアログに「マクロ」カテゴリ(デザイン確定済み)。⌘C/Vも同機構(コピー=無名マクロ)
- **コイル⇔接点デバイスモデル**: 新シンボル`relay_coil`/`relay_contact_no`/`relay_contact_nc`を同梱ライブラリへ追加(JIS C 0617系)。**同一参照記号(K1)のコイル+接点群=1デバイス**として関連付け(参照記号ベース、専用エンティティ不要)。部品DBに`contact_config`(例 "2NO+2NC")を追加し使用数超過をERC Error
- **接点マップ描画**: コイルの下に接点所在表(端子対+/シート.ゾーン、未使用は—)、接点脇にコイル所在(デザイン確定済み記法)。SVG+キャンバス同一ルール(M2 XRefの機構を再利用)
- **例外レポート**: 親の無い接点・接点の無いコイルをERCへ(デザイン済み「XRef例外」)
- **検索(⌘F)**: 対象=参照記号・型番・ネット名・線番・テキスト。浮き検索バー+下部ドック結果パネル(デザイン確定済み)。revealはM2の機構を再利用
- **デバイスナビゲータ**: 左パネル3タブ目「デバイス」。参照記号ツリー(デバイス→機能: コイル/接点/端子)。所在クリックでreveal。**未配置機能のドラッグ配置はフェーズ3**(予約接点のモデルが要るため)。右クリックメニューはreveal/削除まで
- **Surfer(参照サーフィン)**: 参照記号・XRefラベルクリック→所在一覧ポップアップ(デザイン確定済み)→クリックで巡回ジャンプ

### Task 1: 回路マクロ(コア)

- [x] Step 1 (red): Rustテスト: 選択範囲→マクロ保存(相対座標化・基準点)/挿入で再採番・線番クリア/バリアント列挙とTab切替相当の選択適用/undo一発/往復(保存→挿入)で図面等価/壊れたマクロJSONのエラー
- [x] Step 2 (green): `macros.rs`実装(テンプレ機構再利用)+IPC/Link API(保存・列挙・挿入)。gen_spec→コミット

> **完了 (2026-08-22)**: `madake-core/src/macros.rs`(コア16テスト)+Link API 3エンドポイント(`GET /macros`・`POST /macros/save`・`POST /macros/apply`、3テスト)+MCPツール(`list_macros`/`save_macro`/`apply_macro`)+Tauri IPC(`list_macros`/`save_macro`/`apply_macro`/`open_macros_folder`)。テンプレート機構との共通化=UUIDプレースホルダ置換(`templates::substitute`)・`*.json`走査(`templates::json_files`)・issue型(`TemplateIssue`)・回転規則(`netlist::rotate_local`)。値セット(プレースホルダ)はフェーズ3のまま。

### Task 2: 回路マクロ(UI)

- [ ] Step 1 (red): vitest: 保存ダイアログ(名前/カテゴリ/基準点/バリアント)のコマンド組み立て/挿入カテゴリの選択→配置プレビューstate/Tabバリアント切替/⌘C/V
- [ ] Step 2 (green): デザインどおり実装(保存ダイアログ+部品挿入「マクロ」カテゴリ+ゴーストプレビュー)。i18n。実機確認(保存→2回挿入→参照記号重複なし・ERC通過→undoで後始末)+コミット

### Task 3: コイル⇔接点XRef

- [ ] Step 1 (red): Rustテスト: リレーシンボル3種の定義/同一参照記号の関連付け/接点マップの導出(所在・未使用—)/接点数超過ERC(contact_config)/親無し接点・接点無しコイルの例外/SVG描画(コイル下の表・接点脇のコイル所在)
- [ ] Step 2 (green): シンボル追加+`relay_xref.rs`+ERC+SVG実装。gen_spec→コミット
- [ ] Step 3 (red/green): キャンバス描画+部品DB`contact_config`(スキーマv3マイグレーション+テスト)。コミット

### Task 4: 検索+デバイスナビゲータ+Surfer

- [ ] Step 1 (red): Rustテスト: プロジェクト横断検索(対象5種・大文字小文字・部分一致)/デバイスツリーの導出(参照記号→機能)。TS: 検索バーstate(フィルタ・件数・Enter巡回)/結果パネル行→reveal/ナビゲータツリー整形/Surferポップアップの所在一覧
- [ ] Step 2 (green): コア検索+IPC/Link API。UI: ⌘F浮きバー+下部ドック結果パネル(デザイン準拠)。i18n。コミット
- [ ] Step 3 (red/green): 左パネル「デバイス」タブ+Surferポップアップ(参照記号クリック)。実機確認+コミット

### Task 5: 受け入れと仕上げ

- [ ] Step 1: 受け入れ: モータ回路をマクロ保存→別シートへ2回挿入で参照記号重複なし・ERC通過/K1コイル+接点2個で接点マップと逆参照が図面に出て、3個目でERC Error(contact_config=2NO時)/⌘FでK1検索→結果からreveal/デバイスタブのツリーからreveal/Surferで巡回
- [ ] Step 2: docs(03/09等該当ページEN+JA)・feature-inventory・roadmap更新。CLI/MCP露出確認。全テストgreen+後始末

## 受け入れ基準

- マクロ: 保存→2回挿入で参照記号が重複せずERCが通る。Tabでバリアント切替
- コイル/接点: 接点マップ・コイル所在が自動描画され、接点数超過と親無しがERCに出る
- ナビゲーション: ⌘F・デバイスツリー・Surferの3経路すべてでreveal(シート切替+選択+ズーム)が効く
