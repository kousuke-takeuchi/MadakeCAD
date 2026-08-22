# MadakeCAD フェーズ5 (再現検証の必須ギャップ解消: G1〜G4+基盤修正) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 参考図面再現検証(`docs/internal/specs/reproduction-gaps.md`)で「必須」と判定されたG1〜G4を実装し、置き換え対象図面との同等性(パリティ)を達成する。あわせて検証で見つかった基盤問題(G17サイレント失敗・G11電線品番の死にデータ)を解消する。

**共通ルール:** Command絶対原則・tests-as-spec+gen_spec・i18n(en/ja)・ドキュメントEN+JA同期・.mdkproj変更はformat_version。**機密**: テスト・例・ドキュメントに参考図面固有の識別子を使わない(汎用名で)。

## 設計決定

- **G1 多ピンブロックシンボル(名前付きピン)**: 新エンティティではなく**動的シンボル`block`**を追加: `SymbolInstance`のattrsに`block_pins: {left: [{no,name}], right: [...], top?, bottom?}`と`block_size{w,h}`を持たせ、定義はattrsから動的生成(terminal_block前例の発展)。ピンは2.5mmグリッド・5mmピッチ、ピン名をピン脇に描画、シンボル名/型番スロットは既存。配置UI=部品挿入に「ブロック(汎用)」+ピン表エディタ(簡易: 左右のピン名を行編集)。エージェント/APIからはattrs指定で一発作成
- **G2 配線注記(sq・色・長さ)の図面描画**: ワイヤ属性から`0.75sq 白 0.4m`形式のテキストを配線中点付近へ自動描画(線番と同じ代表位置機構・重なり回避で線番の反対側)。表示クラス「配線注記」(10クラス目・画面/印刷とも既定ON、SVGにも出力=参考図面の主情報のため)。書式はテンプレート化(`{sq}sq {color} {length}m`、色は日本語名変換表)
- **G3 ネットラベルのタグ形状**: ネットラベルを方向付きタグ(五角形グリフ、左右向き)で描画するオプション。`NetLabel.attrs["tag"]="left"|"right"`(未指定=従来の文字のみ)。SVG/キャンバス同一ルール
- **G4 テキスト装飾**: `TextEntity`へ`anchor(start|middle|end)`・`color(注記色トークン名)`・`bold`を追加(format_version・既定は現状互換)。シンボルに第3テキスト(機能ラベル)スロット
- **G17 サイレント失敗の解消**: (a)ネットラベルが配線頂点・ピンのどれにも載っていない→ERC Warning `erc.dangling_label` (b)未知シンボルid・極数範囲外のadd_entity→CommandエラーでHTTP 400(200で無視しない) (c)Commandエラーの一覧をLink APIレスポンスへ
- **G11 電線品番の解決**: `Project.wire_parts`(色+sq→品番)を電線リスト/From-Toの品番列で実際に引く。未登録は空のまま。管理UI=プロジェクト設定の簡易表(フェーズ5では設定コマンド+CSV読み込みまで。専用UIはフェーズ6)

### Task 1: G1 ブロックシンボル(コア)

- [x] Step 1 (red): attrsからの動的定義生成(左右ピン名・グリッド・サイズ自動/指定)/netlist・ERC・BOM・SVG(ピン名描画)/回転・ミラー/バリデーション(重複ピン番号・空名)
- [x] Step 2 (green): 実装+MCP/Link API経由の作成をテスト。gen_spec→コミット

  実装メモ: `madake-core/src/block.rs`(attrs `block_pins`/`block_size` → SymbolDef生成・検証)、
  `symbol::DefIndex`/`instance_symbol_def`(インスタンス単位の定義解決)を netlist/svg/reports/verify/tidy が使用。
  不正なブロックは`Engine::execute`のAddEntity/UpdateEntityで`InvalidCommand`(G17先行適用)。
  MCP `place_symbol`に`attrs`を追加。キャンバス側は`canvas/dynamicSymbol.ts`の`blockSymbol`/`resolveInstanceDef`。

### Task 2: G1 ブロックシンボル(UI)

- [x] Step 1 (red): TS: 部品挿入「ブロック」カテゴリ+ピン表エディタ(行追加/削除/並べ替え)→attrs組み立て/配置ゴースト
- [x] Step 2 (green): 実装(i18n)。実機確認(コントローラ相当を作成→配線→BOM)+コミット

  実装メモ: `canvas/blockEditor.ts`(行の表⇔attrs変換・指摘・配線済みピン判定)、`stores/blockEditor.ts`
  (挿入/編集で共有する下書き。編集の確定は`update_entity`1回)、`components/BlockPinsEditor.vue`+
  `BlockPinColumn.vue`(部品挿入ダイアログのパネルと`BlockEditorDialog.vue`が同じ1枚を使う)。
  プロパティにピン構成の行+「ピン表を編集…」。配置ゴーストは`EditorController.ghostInstance()`が
  属性つきインスタンスを返し`resolveInstance`で描く。**選択判定はシンボル外形の囲み矩形ベースへ変更**
  (`symbolBounds`+1mm。定義を引けない壊れたブロックだけ従来の固定半径9mmで拾う)。
  デザインはMadakeCAD.penへ3フレーム追加(「フェーズ5 - 部品挿入 ブロック(ピン表エディタ)」ほか)。
  既知の挙動: ピンを足すと辺のピンが再センタリングされるため既設配線が外れることがある(要再配線)。

### Task 3: G2 配線注記+G3 タグ形状

- [ ] Step 1 (red): Rust: 注記テキスト生成(書式・色名変換・欠落属性の省略)/代表位置(線番と反対側)/表示クラス10/SVG出力/タグ形状の五角形ジオメトリ(左右)/従来ラベルとの互換
- [ ] Step 2 (green): SVG+キャンバス実装(共有数式)。表示タブ・プロパティ(タグ向き)配線。実機確認+コミット

### Task 4: G4 テキスト装飾+第3スロット

- [ ] Step 1 (red): TextEntity拡張(format_version・旧ファイル互換・既定不変)/anchor・color・boldの描画(SVG+キャンバス)/シンボル機能ラベルスロット
- [ ] Step 2 (green): 実装+プロパティパネル(色はトークン選択)。gen_spec→コミット

### Task 5: G17 サイレント失敗+G11 電線品番

- [ ] Step 1 (red): dangling_label検出(頂点上/ピン上/浮き)/不正add_entityのCommandエラー化(既存の寛容挙動に依存するテストの洗い出しと更新)/Link APIの400化/wire_parts引き当て(電線リスト・From-To品番列)/wire_parts設定Command+CSV
- [ ] Step 2 (green): 実装。gen_spec→コミット

### Task 6: パリティ再検証と仕上げ

- [ ] Step 1: 再現スクリプト(docs/references/repro/)をG1〜G4適用形へ更新して再実行し、要素判定表の❌必須4件が✅になることを確認(成果物はrepro/へ、台帳のステータス更新はコミット)
- [ ] Step 2: docs(03/05該当、EN+JA)・feature-inventory・reproduction-gaps.mdのG1〜G4/G11/G17を「解消済み」へ。全テストgreen+後始末

## 受け入れ基準

- コントローラ相当の名前付きピンブロックが配置・配線・netlist・BOM・SVGで機能する
- 全配線にsq・色・長さの注記が図面(画面+PDF)に出て、表示クラスで消せる
- ネットラベルが方向付きタグで描け、従来図面は不変
- 浮いたネットラベルがERCで警告される。不正なCommandは黙って成功しない
- 電線リストの品番列がwire_partsから埋まる
- 再現検証の必須ギャップ4件が✅(台帳更新)
