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

- [x] Step 1 (red): vitest: 保存ダイアログ(名前/カテゴリ/基準点/バリアント)のコマンド組み立て/挿入カテゴリの選択→配置プレビューstate/Tabバリアント切替/⌘C/V
- [x] Step 2 (green): デザインどおり実装(保存ダイアログ+部品挿入「マクロ」カテゴリ+ゴーストプレビュー)。i18n。実機確認(保存→2回挿入→参照記号重複なし・ERC通過→undoで後始末)+コミット

> **完了 (2026-08-22)**: vitest 43件追加(`src/canvas/macroPreview.test.ts` 15 / `src/stores/macros.test.ts` 17 / `src/tools/macroPlacement.test.ts` 11)。UI= `MacroSaveDialog.vue`(名前/カテゴリ/基準点表示/バリアントAチップ+無効の「+追加」/選択範囲プレビュー)、`SymbolPickerDialog.vue`に「部品」/「マクロ」タブ行+マクロギャラリー(カテゴリツリー+バリアント数バッジ付きタイル+プレビュー/バリアント行/無効の値セット/配置)、`MacroPreview.vue`、`canvas/macroPreview.ts`(バリアント選択・配置変換・ゴースト描画)、`stores/macros.ts`、`tools/controller.ts`に`macro`ツール(ゴースト+R回転+Tabバリアント+Esc+連続配置、⌘C/V)。⌘C/Vの無名マクロ用にRust側へ`build_macro`(書き出さずに組み立て)と`apply_macro_inline`(マクロそのものを挿入)をIPC+Link API(`POST /macros/build`・`POST /macros/apply-inline`、2テスト)で追加。値セット(プレースホルダ)とバリアント追加はフェーズ3のまま(disabledプレースホルダ+title)。

### Task 3: コイル⇔接点XRef

- [x] Step 1 (red): Rustテスト: リレーシンボル3種の定義/同一参照記号の関連付け/接点マップの導出(所在・未使用—)/接点数超過ERC(contact_config)/親無し接点・接点無しコイルの例外/SVG描画(コイル下の表・接点脇のコイル所在)
- [x] Step 2 (green): シンボル追加+`relay_xref.rs`+ERC+SVG実装。gen_spec→コミット
- [x] Step 3 (red/green): キャンバス描画+部品DB`contact_config`(スキーマv3マイグレーション+テスト)。コミット

> **完了 (2026-08-22)**: `madake-core/src/relay_xref.rs`(コア21テスト)+SVG3テスト+シンボル2テスト+部品DB2テスト、`src/canvas/relayXref.ts`(vitest 13テスト)。
> - **デバイス**: 同一参照記号のコイル+接点群=1デバイス(`relay_devices`)。接点はシート番号→ゾーン→idの読み順に並び、その連番が端子対になる
> - **端子対**: IEC 60947-1(先頭=接点の連番、末尾=a接点3/4・b接点1/2 → 1個目a=13-14 / 2個目b=21-22 / 3個目b=31-32)。コイルはA1-A2
> - **接点マップ**: コイル外形の下端+2.5mmに中央揃えの2列表(端子対|所在)。`contact_config`が読めるときは未使用接点も「—」行として続く。接点の脇(外形右端+1mm)にコイル所在「(/1.C2)」。SVGとキャンバスは`contact_map_layout`/`contactMapLayout`の同一数式
> - **ERC(verify_project)**: `erc.contact_overflow`=Error / `erc.orphan_contact`=Error / `erc.coil_without_contact`=Warning / `erc.invalid_contact_config`=Warning(接点数検証が黙って無効になるため)
> - **部品DB v3**: `contact_config`列+v2→v3マイグレーション(既存行保持)+サンプル2件に"2NO+2NC"付与。部品挿入時に`attrs.contact_config`へ写す(図面はDBのスナップショット)
> - **シンボル**: `relay_contact_nc`(b接点)を追加。`relay_coil`(A1/A2)・`relay_contact_no`は既存
> - **実機確認**: K1コイル+a接点2個を配置・結線 → 接点マップ(13-14 /1.B2・23-24 /1.B3)と接点脇の「(/1.C2)」が図面に出る → MY2N(2NO+2NC)割当て+3個目の接点でERC Error「a接点 3個 (実装 2個)」→ undoで空図面へ戻る

### Task 4: 検索+デバイスナビゲータ+Surfer

- [x] Step 1 (red): Rustテスト: プロジェクト横断検索(対象5種・大文字小文字・部分一致)/デバイスツリーの導出(参照記号→機能)。TS: 検索バーstate(フィルタ・件数・Enter巡回)/結果パネル行→reveal/ナビゲータツリー整形/Surferポップアップの所在一覧
- [x] Step 2 (green): コア検索+IPC/Link API。UI: ⌘F浮きバー+下部ドック結果パネル(デザイン準拠)。i18n。コミット
- [x] Step 3 (red/green): 左パネル「デバイス」タブ+Surferポップアップ(参照記号クリック)。実機確認+コミット

> **完了 (2026-08-22)**: `madake-core/src/search.rs`(コア16テスト)+Link API 5テスト+vitest 50件
> (`canvas/search` 13 / `canvas/deviceTree` 11 / `canvas/surfer` 7 / `stores/search` 13 / `stores/devices` 8 / `stores/surfer` 6)。
> - **検索**: 対象5種=参照記号・型番(`value`+`attrs`の値)・ネット名(ネットラベル)・線番(`Wire::net`)・テキスト。
>   部分一致・大文字小文字無視・空クエリは0件(全件を並べない)。並びはシート番号→ゾーン→entity id→種別で決定的。
>   シンボルのヒットには**そのシンボルがデバイスで果たす機能**(コイル/接点13-14…)を添えるので、結果パネルの
>   「種別」列をUIがi18nで組み立てられる。露出=IPC `search_project` / `GET /api/v1/search?q=&kinds=`
> - **デバイスツリー**: 参照記号ごとに機能を列挙(リレー=コイル+接点(端子対は接点マップと同じIEC 60947-1連番)、
>   端子台=端子群"1-8"1行、その他=本体)。露出=IPC `get_device_tree` / `GET /api/v1/devices`
> - **検索UI**: ⌘F浮きバー(幅340・focus=acad-blue 1.5枠・フィルタチップ・件数)+下部ドック結果パネル
>   (参照/種別/所在/補足の4列)。Enter=次へ・Shift+Enter=前へ(端で回り込み)・Esc=閉じる。
>   「ネット」チップはネット名と線番の両方を探す
> - **デバイスナビゲータ**: 左パネル3タブ目「デバイス」。ツリー(▾K1 リレー MY2N → コイル A1-A2 `/1.C2`…)、
>   行クリック=reveal、右クリック=図面へジャンプ/削除(`delete_entities` Command経由なのでCmd+Zで戻る)。
>   未配置機能のドラッグ配置はフェーズ3のまま(ヒント表示)
> - **Surferの起動操作(決定)**: **キャンバス上でAlt(Option)+クリック**。素のクリックは選択・作画のままなので
>   既存操作を壊さず、XRefラベルのプロパティパネルからのジャンプとも共存する。対象=参照記号のあるシンボル
>   (同じデバイスの全機能)/ネットラベル(同名ラベルの全所在)/線番のついたワイヤ(同じ線番の全ワイヤ)。
>   ↑↓で巡回・Enterで確定・Escで閉じる
> - **reveal**: 検証パネル・検索・ナビゲータ・Surferの4経路を`composables/reveal.ts`に一本化

### Task 5: 受け入れと仕上げ

- [ ] Step 1: 受け入れ: モータ回路をマクロ保存→別シートへ2回挿入で参照記号重複なし・ERC通過/K1コイル+接点2個で接点マップと逆参照が図面に出て、3個目でERC Error(contact_config=2NO時)/⌘FでK1検索→結果からreveal/デバイスタブのツリーからreveal/Surferで巡回
- [ ] Step 2: docs(03/09等該当ページEN+JA)・feature-inventory・roadmap更新。CLI/MCP露出確認。全テストgreen+後始末

## 受け入れ基準

- マクロ: 保存→2回挿入で参照記号が重複せずERCが通る。Tabでバリアント切替
- コイル/接点: 接点マップ・コイル所在が自動描画され、接点数超過と親無しがERCに出る
- ナビゲーション: ⌘F・デバイスツリー・Surferの3経路すべてでreveal(シート切替+選択+ズーム)が効く
