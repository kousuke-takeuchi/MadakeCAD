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

## 計画側の反省 (次のフェーズ計画に効かせる)

- 受け入れ基準に結果を書き込むと、その基準は反証できなくなる。結果は実施記録 (Step 行) に書く
- 「大きさの決まった導出ブロック」を作る機能は、置き場所の検査 (`erc.*` か配置規則) まで含めて計画する。
  凡例ブロックが図枠に重なる/どこにも入らない問題は、それが無かったために受け入れ実行で初めて出た
