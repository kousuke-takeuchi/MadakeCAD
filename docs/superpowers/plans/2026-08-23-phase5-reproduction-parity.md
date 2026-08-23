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
- **G11 電線品番の解決** (台帳の正しい番号は**G9**。G11は筐体囲み): `Project.wire_parts`(色+sq→品番)を電線リスト/From-Toの品番列で実際に引く。未登録は空のまま。管理UI=プロジェクト設定の簡易表(フェーズ5では設定コマンド+CSV読み込みまで。専用UIはフェーズ6)

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

- [x] Step 1 (red): Rust: 注記テキスト生成(書式・色名変換・欠落属性の省略)/代表位置(線番と反対側)/表示クラス10/SVG出力/タグ形状の五角形ジオメトリ(左右)/従来ラベルとの互換
- [x] Step 2 (green): SVG+キャンバス実装(共有数式)。表示タブ・プロパティ(タグ向き)配線。実機確認+コミット

  実装メモ: `madake-core/src/wire_note.rs`(`{sq}sq {色} {長さ}m`の生成+代表位置。色は英語名→日本語の
  変換表で、未知の色名はそのまま)、`madake-core/src/net_tag.rs`(`NetLabel.attrs["tag"]`=`left`/`right`の
  五角形グリフ。枠幅は`tidy::text_width`×1.1+余白1mm)。SVGは`render_wire_notes`とネットラベル分岐で描画し、
  **配線注記は表示クラスに関わらず常に出力**する(参考図面の主情報)。タグ付きラベルのXRefは旗の右端の外へ逃がす。
  キャンバス側は同じ数式の`canvas/wireNotes.ts`/`canvas/netTag.ts`、色トークンは`theme.wireNote`。
  表示クラスは11個になり、リボンのラベルをi18n化(`view.*`)。プロパティのネットラベルに「タグ形状」
  (なし/左/右→`update_entity`)。`NetLabel`に`attrs`(serde default)を追加。既存ファイルはそのまま読める。
  **未了**: `MadakeCAD.pen`へのデザイン反映(Pen.appのMCPが`.pen`スキーマを返さず`execute`を書けないため。
  代わりに`docs/internal/design-system.md` §2.1/§6へ配線注記・タグ形状の規定を追記済み)。

### Task 4: G4 テキスト装飾+第3スロット

- [x] Step 1 (red): TextEntity拡張(format_version・旧ファイル互換・既定不変)/anchor・color・boldの描画(SVG+キャンバス)/シンボル機能ラベルスロット
- [x] Step 2 (green): 実装+プロパティパネル(色はトークン選択)。gen_spec→コミット

  実装メモ: `madake-core/src/text_style.rs`(色トークン`TEXT_COLORS`=default/accent/warning/success/muted
  → 印刷色。`svg_anchor`。機能ラベルの属性`function_label`と位置決め`function_label_placement`)。
  `TextEntity`に`anchor`/`color`/`bold`を追加し**format_version 5**へ。旧ファイルは左揃え・既定色・
  太字なしで開き、**装飾なしの注記のSVG出力は1バイトも変わらない**(ゴールデンテストで固定)。
  色は**トークン名のみ**で、生の色コードは`Engine::execute`が`InvalidCommand`で拒否(Link APIは400)。
  シンボルの第3テキスト欄=機能ラベルは`symbol::SLOT_FUNCTION`スロットに従い、**スロットが無ければ
  外形の下3.5mmへ自動**(同梱シンボル定義は変更不要)。キャンバス側は`canvas/textStyle.ts`
  (画面色は`theme.textColors`)、プロパティは注記の基準位置/色/太字とシンボルの機能ラベル行。
  ついでに: **注記がクリックで選べなかった**(`hitTest`にtextの枝が無かった)ので、書かれた文字の
  範囲(`noteBounds`。基準位置で左右どちらへ伸びるかが変わる)で掴めるようにした。
  既知の制限: `TextEntity.rotation`は引き続き未描画(G4の回転は次フェーズ)。リレーコイルの
  接点マップと機能ラベルは同じ「外形の下」に出るため、両方付けると重なる。

### Task 5: G17 サイレント失敗+G11 電線品番

- [x] Step 1 (red): dangling_label検出(頂点上/ピン上/浮き)/不正add_entityのCommandエラー化(既存の寛容挙動に依存するテストの洗い出しと更新)/Link APIの400化/wire_parts引き当て(電線リスト・From-To品番列)/wire_parts設定Command+CSV
- [x] Step 2 (green): 実装。gen_spec→コミット

  実装メモ: **G17-a** `netlist::label_is_attached`(配線の線分上 or シンボルピン)を新設し、
  ネットリストの結線判定とERCの`erc.dangling_label`(Warning)が**同じ関数**を使う。
  ついでにネットリストが**ラベル⇔ピンも結ぶ**ようになった(ピンに直接置いたラベルが
  同名ネットへ繋がる。従来は配線の上にしか載らなかった)。
  **G17-b** `Engine::execute`の`validate_entity`が`symbol_id`を`resolve_symbol`で解決できないと
  `InvalidCommand`。極数可変シンボルの範囲外は`symbol::dynamic_family`で「極数が範囲外
  (`connector_{1〜50}p`)」と理由を出し分ける。**寛容挙動に依存していた既存経路は無し**
  (テンプレート・回路マクロ・PLC生成・端子接続図はすべて正当なidを使っており、
  cargo test --workspace は追加テストだけが増えた)。
  **G17-c** Link API `POST /commands`は失敗時400 + `{error, failed_command_index, applied}`。
  MCP `execute_commands`のエラーも「commands[i] が拒否されました: 理由 (ここまでのn件は適用済み)」。
  **G11** `madake-core/src/wire_parts.rs`: `wire_part_number(project, wire)`=**ワイヤに書かれた
  品番が最優先**、空なら線色+線径(誤差1e-9)で`Project.wire_parts`を引く(先に書いた行が勝つ・
  無ければ空欄)。線色は前後空白無視・英字大小同一視に加え、**英日の色名を同一視**
  (`wire_note::color_ja`で正規化。`light_blue`の図面に「水色」の表がそのまま効く)。電線リストの品番列が追従。設定は既存の
  `set_wire_parts`コマンドで、CSV(線色,線径sq,電線品番[,備考]・見出し任意)は
  `import_wire_parts_csv`→同コマンド1回=undo一発。露出は Link API
  `GET /project/wire-parts`・`POST /project/wire-parts/csv` と Tauri IPC `import_wire_parts_csv`。
  フロントは`canvas/wireParts.ts`(同じ規則)でプロパティに「品番 (自動)」行と入力欄の
  プレースホルダを出す。端子接続図の電線仕様欄はシート単位の帳票なので手書き品番のみ(据え置き)。

### Task 6: パリティ再検証と仕上げ

- [x] Step 1: 再現スクリプト(docs/references/repro/)をG1〜G4適用形へ更新して再実行し、要素判定表の❌必須4件が✅になることを確認(成果物はrepro/へ、台帳のステータス更新はコミット)
- [x] Step 2: docs(03/05該当、EN+JA)・feature-inventory・reproduction-gaps.mdのG1〜G4/G9/G17を「解消済み」へ。全テストgreen+後始末

  結果: 再現スクリプトを新機能適用形に書き直し(コントローラ・アンプ・全コネクタを`block`、
  配線注記は自動、ネットラベルは方向付きタグ、注記は色トークン/太字/基準位置、
  用途名は機能ラベル、電線品番はCSV投入)、Link API経由でシートを組み直した。
  **258コマンド / 52ネット / 111配線。手置きの注記はゼロ**(前回は約210個)。
  **必須4件(G1〜G4)がすべて✅**。ERC+電気検証の指摘は11件→**3件**、
  **`erc.dangling_label`はゼロ**。電線リストの電線品番欄は**111/111行**が
  3行のCSVから埋まった(図面は`blue`・表は`青`でも一致)。不正コマンド4種
  (未知のsymbol_id・極数超過・生の色コード・ブロックのピン番号重複)はすべて400+`applied:0`。
  17ページのブックPDFも出力。**なお計画本文が「G11 電線品番」と書いているのは台帳ではG9**
  (台帳のG11は筐体囲み)。台帳側に注記済み。

  第2回で新たに見つかり、台帳へG19〜G21として追加したもの:
  - **G19** タグ形状の本体が`left`/`right`とも基準点の+x側へ伸びるため、機器から左へ出る
    配線では自分の導線と配線注記に重なる(`right`は先端を基準点にすべき)
  - **G20** 5mmピン間隔では配線注記(2.0mm固定)が隣の行のタグに重なる。注記・ラベルの
    文字高さをシート設定にしたい
  - **G21** `relay_xref::relay_role()`が`relay_contact_co`を接点と判定しないため、
    c接点だけのリレーが`erc.coil_without_contact`と誤警告される

## 受け入れ基準

- [x] コントローラ相当の名前付きピンブロックが配置・配線・netlist・BOM・SVGで機能する
- [x] 全配線にsq・色・長さの注記が図面(画面+PDF)に出て、表示クラスで消せる
- [x] ネットラベルが方向付きタグで描け、従来図面は不変
- [x] 浮いたネットラベルがERCで警告される。不正なCommandは黙って成功しない
- [x] 電線リストの品番列がwire_partsから埋まる
- [x] 再現検証の必須ギャップ4件が✅(台帳更新)

## 完了

**2026-08-23 フェーズ5完了。** 再現検証で「必須」と判定されたG1〜G4を実装し、
参考図面と同等の情報が載る状態(パリティ)に到達した。基盤のG17(黙って通る失敗)と
G9(電線品番の死にデータ)も解消。残りは`docs/internal/specs/reproduction-gaps.md`の
品質向上・軽微の各項目で、次に効くのは小さく直せるG19・G20・G21。
