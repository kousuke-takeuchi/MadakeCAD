# Phase 6 follow-ups — deferred items carried out of tasks 2-6

フェーズ6 (`2026-08-23-phase6-quality-gaps.md`) の各タスクで、**意図的に見送った**項目の一覧。
各タスクの最終レビュー (`.superpowers/sdd/2026-08-23-phase6-quality-gaps/final-review-task*.md`) と
コントローラのルーリングから拾ったもので、**フェーズ6の完了を妨げないと判断済み**の残作業だけが載る。
1項目1行、分かっているものは `ファイル:行` を添えた。着手するときはここから1つ選び、
いつもどおり spec → plan → TDD の順で進める (この文書は棚卸しであって計画ではない)。

台帳のギャップ番号 (G…) は `docs/internal/specs/reproduction-gaps.md` を参照。

## Task 2 — 多段端子台・ジャンパバー (G5/G6)

- 端子接続図で、ある段のジャンパが他の段の行を横切って描かれる
- 端子接続図の千鳥配置 (行ごとに1箱) が未対応
- 端子エディタの grid で Lv 列が非表示のまま
- キャンバスのベースライン `h/2.8` と SVG の `h/2` の差 (文字の縦位置が画面と出力で僅かに違う)
- `docs/internal/feature-inventory.md:64` — 端子台チャートCSVへ Lv 列を足した互換性の注記が無い
- 台帳の G6 状態文が「列間の配線レーン」を謳ったまま (実際には段間ジャンパがそのレーンを使う)
- 「丸なし」の説明句が `drawSymbol text placement` のテストID の下に載っている (置き場所が違う)
- 最寄りマーク帰属のテストが、端子番号の入れ替わりを検出しない
- 多段端子台の `10a` とバーの間隔が 0.000 mm (レビューで許容済み。将来の見直し用に記録)
- 段間バーが内側のピン端に重なる (同上、許容済み)
- `jumper_bars` の可視性 (表示クラスの扱いが他のシンボル付随描画と揃っていない)
- ストアが端子一覧を持たないまま再正規化する経路がある
- `_1l` 別名 id (1段の多段端子台が既定の端子台と別idで作れてしまう)
- 接点イメージが IEC の汎用端子番号で、メーカのピン番号体系ではない (G7の残りと同根)

## Task 3 — リレー実機ブロック・デバイス単位シート間参照 (G7/G13/G21/G22)

- 実機ブロックの当たり判定の矩形が、その上を横切る配線のクリックを吸ってしまう
- 表示クラス「参照記号」を隠すと、中身の無い背の高い外形だけが残る
- `Math.sign` と `copysign` が x = 0 で食い違う
- `annotated_instances` が2回計算される
- 90°回した深いブロックに飲み込まれたピンが、外形の内側にスタブを描く
- 接点イメージが IEC 汎用端子番号で、メーカのピン番号体系でない (G7の明示的な残り)
- `final-review-task3.md` の残り9件のminor

## Task 4 — ピングループ括弧・絵記号・引き出し線 (G8/G12/G20残)

- M5 キャンバスのベースライン (保留と判断済み)
- M6 括弧の色の継承
- M10 `text_bounds` の回転未対応
- `symbol_bounds` の拡張により、ピングループを持つインスタンスの xref・引き出し線の取り付き位置が動く
- `symbol_bounds` が呼ばれるたびに `pin_groups` 属性を再パースする
- チャート順の計算が重複している
- N1 のゲートが属性を見ているのに、レンダラは実機ブロック所属を見ている (エラー時のみ差が出る) + 3件の nit

## Task 5 — 軽微バッチ (G14/G15/G16/G18)

- JSON の明示的な `null` (`name`/`company`/`title`/`drawing_no`) が「変更しない」の意味になることが文書化されていない
- `erc.outside_paper` を遡って出すかどうか (設計判断として保留)
- プロジェクト情報ダイアログが1枚のシートの表題欄だけを読む
- `declared_mate` の端ごとの走査
- G14 動的コネクタ系統への性別 (オス/メス) 欄が未着手
- `final-review-task5.md` の残りminor

## Task 6 — 電線品番凡例ブロック・BOM電線集約 (G9残) + 受け入れ

- **G23** — 整えメトリクスが配線注記と描画時に決まる外形を見ていない。`tidy::text_boxes` へ配線注記を、
  `tidy::symbol_boxes` へ実機ブロック・接点マップ・電線品番凡例の外形を入れ、その上で G22 と同じ
  「空いている側へ逃がす」扱いを与える。凡例ブロックには自動配置か、図枠に重なったときの診断を足す
  (台帳 `docs/internal/specs/reproduction-gaps.md` の G23)
- **線色の並び順の twin parity** — キャンバスは `localeCompare`、Rust は `String::cmp` (コードポイント順)。
  日本語の色名で行の順序が食い違いうるのに、どちらにも突き合わせるテストが無い
  (`src/canvas/wireLegend.ts:77` / `src-tauri/crates/madake-core/src/wire_legend.rs:79`)
- 塗り順の twin 差 — キャンバスは凡例を線番・配線注記の**前**に、SVG は**後**に描く
  (`src/canvas/renderer.ts:1366` / `src-tauri/crates/madake-core/src/svg.rs:1341`)
- セル内余白の twin 差 — キャンバス 1 mm、SVG `CELL_PAD = 2.0` mm (盤の凡例表から引き継いだ既存差)
  (`src/canvas/renderer.ts:1196` / `src-tauri/crates/madake-core/src/svg.rs:1060`)
- `RIBBON_SMALL_ROWS = 4` がリボンの高さ (`RibbonBar.vue` の `.ribbon-body { height: 96px }`) と
  結びついていない。全グループを `smallColumns` に通す (今は配線グループだけ)
  (`src/components/ribbonLayout.ts:9`)
- BOM の電線集約行が数量欄へ `"3.75 m"` と単位付きで書く (単位列が無いための逸脱)。台帳かdocs/04へ1行
  (`src-tauri/crates/madake-core/src/reports.rs:97`)
- 使用長の無い配線しか無いプロジェクトでも `(未割当) 0.00 m` の行が出る
- `legend_rect_with` が凡例1つごとに全行を導出し直す (`render_wire_legends` も行を2度作る)
  (`src-tauri/crates/madake-core/src/wire_legend.rs:95`, `svg.rs:1028`)
- `evidence/task6-acceptance.txt` のテスト件数が受け入れ回のまま (修正後の件数は報告書が正)

## 統合レビュー (2026-09-28) — origin/master 取り込み分の積み残し

origin 8コミット (`80ab43f`/`fd2d7d0`/`3a4804f`/`e6dbef0`/`06352a8`/`f916c8b`/`aa5a8c7`/`1b671dc`) の
統合レビュー・修正ウェーブ1〜4・再レビュー1〜3 (`.superpowers/sdd/integration/`) を通して、
「残作業」「未着手」「次の波へ」「新規Minor」と明示された項目のうち、現行ツリーで確認しても
まだ直っていないものだけを載せる。ウェーブが「対応済み」と判定した項目は載せていない。

### 機密・国際化・CI

- 機密用語の履歴ヒット 29 件 — 公開時は M6 仕様 §2 の「新規エクスポート」で対応、ユーザー判断
- zh/es/fr/de カタログの翻訳 (現状 EN プレースホルダ)
- CI の新 bundle ジョブは未実行 (push 時に検証)
- `src/i18n/i18n.test.ts` に「非EN値がEN値と異なる」ことを固定するテストが無い (翻訳率がテストで担保されない。i18n仕様に「続き」として記載のみ)
- ローカル計画 Task3 Step1 が求めた「Vueテンプレート/スクリプト内の日本語直書き検出」のlintテストが未着手 (origin計画が黙って置き換えた分)
- `.github/workflows/release.yml:47` の `fonts-noto-cjk` はビルド機のみに入り、`.deb` を入れた利用者の環境には入らない (docs/02は開発者向けの記載のみ。M6レビューM2)
- `release.yml:27` の `ubuntu-22.04` ランナーラベルの生存確認がオフラインでは未実施 (M6レビューM1)
- `.mcp.json` / `.claude/settings.json` / `.claude/launch.json` が追跡対象のまま同梱可否の判断が未確定 (`docs/internal/release-runbook.md` §6へ課題化のみ。M6レビューM5)

### FreeCAD / M5連携

- `madake-mcp/src/link_api.rs:143` の「個々のコマンドは全か無か」はドキュメントのみで、`set_wire_lengths` 以外のアーム追加時に無言で破られうる (テストが1本 (`set_wire_lengths`) しか守っていない)
- `?batch=true` はLink APIのみで、MCP `execute_commands` (`madake-mcp/src/lib.rs:749-772`) は1コマンド=1undoのまま (エージェント経由の同じ編集だけN回undoが必要な非対称。batch失敗時の応答キー `failed_command_index: null` / `batch` もdocs/10未記載)
- `freecad-addon/madakecad_link/panel.py:557-558` `_stop_follow` の `wait(3000)` はほぼタイムアウトする (再接続のたびGUIが最大3秒固まり、旧SSEソケットが最大60秒残る)
- `panel.py:509-522` `sync_placements` の傾き警告が対応付けの無いオブジェクトまで数える (実際に保存していない件数を誤って報告する)
- `panel.py:437,478-482` スタブ登録の `_send` が失敗すると、その配線は永久に未登録のまま残る (FreeCAD側のタグは付くので再実行でも拾い直せない)
- `src/components/mechLink.ts:8-14` の「Pythonの `%g` と同じ」は常用域のみの近似 (1e6超・1e-5未満で表記が分かれる。実害は小)
- `freecad-addon/madakecad_link/routes.py:101-114` `placement_commands` がオブジェクト名だけで照合し `fcstd_path` を見ない (別 `.FCStd` にある同名オブジェクトへ配置が誤って書き込まれうる。review1と同型の穴)
- `routes.py:27-30` `RouteStub.name` がネット内で衝突する (`Route_<net>` 固定。FreeCADが改名しName/Labelが食い違う)
- `routes.py:34-40` スタブの鎖の並びがFreeCADオブジェクト名の辞書順 (最近傍順にする余地があり、盤上で経路が交差しうる)
- `src-tauri/crates/madake-core/src/reports.rs:352-363` `wire_list_rows` に出所 (`length_source`) の列が無い (電線リストで測定値と手入力値を区別できない。プロパティパネルのバッジのみ)
- `panel.py:107` ネットリスト行選択→3D選択の副作用を無効化するスイッチが無い
- `src-tauri/crates/madake-core/src/command.rs:833-839` `SetMechLink` が更新のたびに `mech_links` の並びを末尾へ動かす (undoで値は戻るが順序は戻らず `.mdkproj` diffが荒れる)
- `panel.py:315,321` `insert_model` の `Part.read`/`mergeProject` にエラーハンドリングが無い (不正な `model_3d` パスがFreeCADコンソールへ例外を投げる)
- `panel.py:251-254` `load_links` が部品DBエラーを握りつぶし「no model_3d」と誤表示する
- `panel.py:29` Wiresタブが同一ネットの2本の配線を区別できない (Net列が未採番だと空になる)
- `freecad-addon/package.xml` の `maintainer email` がプレースホルダのまま、`<license file="../LICENSE-MIT">` のパスがアドオンディレクトリを抜ける (M6 Addon Manager提出前に要修正)
- `freecad-addon/tests/_fake_server.py` が実APIと契約テストで結ばれていない (形状変更がCIでなく実行時に発覚する)
- `panel.py:498-500` パネルを閉じてもfollowチェックは入ったまま、再度開くとfollow設定が黙って停止のまま (`commands.py:19-24`)
- `sync_placements` が経路リンクも配置同期の対象にする (意味の無い `(0,0,0)/0°` が保存されundoエントリも増える)
- `panel.py:369-373` 書き戻し確認ダイアログの文面が手入力件数を示さない (影響件数が全件表示のまま)

### KiCad/DXF連携 (interop)

- `src-tauri/crates/madake-core/src/dxf.rs:1261-1263` `wire_no` を持つネットで `longest_segment` が `None` だと無言で `continue` する (`ExportReport.skipped` に載らず、書いた配線番号もカウントされない)
- 外部ファイルの `FRAME` レイヤの内容 (`dxf.rs:1856-1858` のテキスト・`:1775` の線) が `skipped` に入らず無言で捨てられる
- `dxf.rs:1710-1714` 空文字列の `wire_layers` をフィルタしていない (`--wire-layer ""` がフォールバックを無効化し配線ゼロで警告)
- ヘッダ由来の用紙サイズを採用したとき、範囲外エンティティがオフセットのまま紙外 (負座標) へ着地し警告が出ない
- `dxf.rs:1577` `let _ = &blocks;` — `BLOCKS` セクションを解析して捨てている (未知ブロック→`Entity::Shape` 化というC3のインポート側対称が未着手)
- 塗りつぶし円が `SOLID` 正方形近似のまま (`dxf.rs:377-393`。JISの接続ドットが正方形に見える。Shapeの塗りも未エクスポート)
- `instance_attributes` (`dxf.rs:475-505`) が追加インスタンス属性に対応する `ATTDEF` を書かない (予約タグへ大文字化衝突する属性キーが参照を上書きする)
- 90°刻みでない `TEXT` 回転を無言で0へ丸める (`dxf.rs:1337-1342`。`INSERT` は同条件で警告するのに不一致)
- スナップしない配線番号1件ごとに警告が無制限に出る、O(texts×wires×segments)のスナップに空間索引が無い (`dxf.rs:1489-1528`)
- MCP `import_dxf` が `replace_project` を直接呼び、REST/Tauriの `agent::import_dxf_with_chat` と経路が違う (`madake-mcp/src/lib.rs:1291-1303`。spec§4.4の「全インポートがエージェントターンを取り消す」主張と食い違う)
- import = `replace_project` (undo不可・`AddEntity` と合成不可) の是非が未決のまま (`madake-core/src/command.rs:271-282`)
- KiCad書き出しで配線番号ラベルに `(uuid …)` が無く `(sheet_instances (path "/" (page "1")))` も出力されない、インスタンス属性の `(property <key> …)` が `Reference`/`Value` と衝突しうる (`kicad.rs:742-747,796-804`)
- 空/切り詰められたDXF、`MTEXT` のgroup-3継続チャンク分割、実ACADE/EPLANファイルでのレイヤ・属性規約の検証がテストで埋まっていない
- DXFの改訂欄は書き出されない (docs/08 + interop specに制限として明記のみ)

### 整えバリアント (M3フェーズ4)

- `src-tauri/crates/madake-core/src/model.rs:432-436` `Entity::refs` が `_ => Vec::new()` で非網羅 (`remap_refs` と違い、参照を持つエンティティ種が増えてもコンパイルエラーで気付けない。C1再発ガードの穴)
- `src/components/CanvasView.vue:109` `ToolUseStarted` 由来のオーバーレイにシート絞り込みが無い (案表示中も元と同じ座標に幽霊ハイライトが出る経路が残る)
- `src-tauri/crates/madake-core/src/xref.rs:246` / `src/canvas/xref.ts:192` `device_sites` のシート番号が `enumerate` の生添字のまま (比較中に新しいシートを追加すると既存シートの番号が複製枚数ぶんずれる)
- `src/stores/variants.ts:163-174` `clearIfStale` は案シート消失だけを見る (元シートが消えた実行は自動で片付かない。watchがパネル側にあるためパネルを閉じたままの Cmd+Z を取りこぼす)
- `verify_project`/`relay_xref` (`madake-core/src/verify.rs`, `relay_xref.rs`) が比較中の複製シート分の指摘を含む (検証パネルに元の指摘が1+N重に並ぶ。仕様書の未決事項どおり)
- `VariantRun` の永続化・ウィンドウ再読み込み後の復旧が未実装
- 開始ガードが送信中は無効 (`src/stores/variants.ts:116-125`)、`ChatTidyMenu.vue`の「反復しない」チップが比較中も押せる、下部ドックが4枚積み重なる — いずれも未着手
- 複製が `mech_links` を引き継がない点が仕様書の未決事項1文のみ (FreeCADリンク付きシンボルは案シートで未リンク表示になる)
- `command.rs:689-711` `DeleteEntities` が `mech_links` を掃除しない (案がリンク付きシンボルを消した案を採用すると宙ぶらりんな `MechLink` が残る。バリアントで踏みやすくなった一般挙動)

### ファイルメニュー・ネイティブダイアログ

- `src/composables/fileActions.test.ts:136-141,207-221` 1テストで2挙動を確認しているテストが2本残る (「1テスト=1挙動」原則に反する)
- `src/composables/closeGuard.ts` / `EditorLayout.vue` の `anyModalOpen()`・`onCloseRequested` 配線自体は `.vue` のため単体テストが無く、実機 (`npm run tauri dev`) での検証待ち
- `browserCloseGuard` はTauri外で `beforeunload` のカスタム文面を出せない (ブラウザ仕様。Tauri側のみ `file.discardConfirm` 文面が出る)

### デザイン (.pen) 整合

- `src/components/RibbonBar.vue:607-622` `adminGroups` に「設定」グループ (AI設定/言語) が無い (`.pen`「帯 管理」`G8kMzh` ボードには存在。design-system.md C1)
- 同ボードの「プロジェクト▾」グループ (ゾーン分割設定/プロジェクト設定/改訂欄) も管理タブ実装に無い
- `src/components/FileTabs.vue:116,125,130` / `src/components/ProjectPanel.vue:145` / `src/components/settings/SettingsDialog.vue:979` に生色コードが5箇所残る (8コミットとは無関係の既存債務。design-system.md C5、未トークン化のまま)

### 端子番号の自動採番 (M4フェーズ5 Task 2)

- **決定事項として残す**: ネットリスト・From-To は電気的なピン識別 (シンボル定義のピン番号) を使い、
  採番後の表示番号はチャート・接続図・図面に出る (`netlist.rs:54` が `pin.number` を見る。
  採番はラベル付けの操作で、結線を変えないという判断)。結果として、同じ端子をネットリストは `1`、
  チャート・端子接続図・図面は `11` と呼ぶ。**後のフェーズで「From-Toに表示番号も併記する」
  オプションを検討する**か、併記しないことを docs/05 に明記したままにするかを決める
  (docs/05-wire-management{,.ja}.md には現状の規則を1文で書いてある)
- 採番ポップオーバーの「対象: 選択行」は無効のまま (`RenumberTerminals` に行単位のスコープが無い)。
  デザイン `G9SNm` はラジオ2つを描いているので、選択行だけの採番を入れるか、デザインを
  「全端子」だけに絞るかを決める
- 端子番号の上書きが**古いラベル集合と新しいラベル集合で重なる**採番 (例 `1..8` → `2..9`) では、
  読めないジャンパ・ピングループの項目に入った未知の端子番号が新ラベルと衝突しうるため、
  採番→⌘Zがその項目だけバイト一致で戻らない場合がある (実用上の採番は番号帯をずらすので、
  現状は規則の単純さを優先している)

## 計画側の反省 (次のフェーズ計画に効かせる)

- 受け入れ基準に結果を書き込むと、その基準は反証できなくなる。結果は実施記録 (Step 行) に書く
- 「大きさの決まった導出ブロック」を作る機能は、置き場所の検査 (`erc.*` か配置規則) まで含めて計画する。
  凡例ブロックが図枠に重なる/どこにも入らない問題は、それが無かったために受け入れ実行で初めて出た
