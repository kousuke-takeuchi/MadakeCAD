# MadakeCAD M4 フェーズ5 (端子台エディタの積み残し + 端子台まわりの帳票) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M4仕様 §1 が「フェーズ2に残した項目」として挙げた端子台エディタの8項目
(並べ替え・多段端子(レベル)・アクセサリ・部品割当・内外スワップ・図面へジャンプ・端子番号の自動採番・
予備端子の挿入) を実装し、`terminals.phase2.*` のdisabledプレースホルダを実機能へ置き換える。
併せて端子台に隣接する帳票の積み残し (端子接続図の段ジャンパ・千鳥配置、BOMの付属品行) を閉じ、
§1 と `docs/12-roadmap.md` の状態文を実態に合わせる。

**仕様:** `docs/internal/specs/m4-industrial-core.md` §1 (+ §5 の端子台に関わる分)。
ベンチマークは仕様が既に挙げている2件をそのまま使い、**新しい挙動を発明しない**:
[EPLAN: Edit terminal strip dialog](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2024/Content/htm/stripmanagementgui_d_klemmenleistebearbeiten.htm)
(列構成・並べ替え・多段・サドルジャンパ・アクセサリ) /
[ACADE: Terminal Strip Editor](https://help.autodesk.com/view/ACAD_E/2023/ENU/?guid=GUID-D2DDB196-DC73-44C4-AE68-51337272F89A)、
接続図の様式は [EPLAN 端子図フォーム](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/formeditorgui_k_klemmenplan.htm)。

**共通ルール:** Command絶対原則 (編集は必ず`Engine::execute`を通す) ・tests-as-spec + `python3 scripts/gen_spec.py`
(`docs/13-specification(.ja).md`は手編集禁止)・i18n 6カタログ (`src/locales/{en,ja,zh,es,fr,de}.json`。
en/jaは訳す、zh/es/fr/deは既存慣行どおりEN文字列を入れてキー同値を保つ)・公開ドキュメントはEN+JA同期・
`.mdkproj`のフィールドを増やすなら`format_version`+マイグレーション・機密: 汎用名のみ。
実機確認は`npm run tauri dev` → http://localhost:1420 をブラウザツールで開く (別インスタンスが9310を
占有していれば`MADAKE_MCP_PORT`で別ポート)。

**フェーズ6で既に入っているので作り直さないこと:** 多段端子台シンボル`terminal_block_{n}p_{levels}l`
(段=1〜3・段記号ピン番号`1a`/`1b`・チャートのLv列・接続図の段の列) と段対応ジャンパ+図面上のショートバー
(G5/G6)、実機ブロック表示、ピングループ括弧、引き出し線、電線品番凡例、`mates_with`、
`set_sheet_paper`、`set_project_meta`、From-Toのネットラベル辿り。

---

## 設計決定

### 全体: モデル拡張は`attrs`で足りる → `format_version`は13のまま

仕様§1が言う「モデル拡張が必要」は、フェーズ6が`jumpers`/`pin_groups`/`device_block`/`mates_with`で
使った**`SymbolInstance.attrs`(String→Stringの開いたマップ)** で全項目まかなえる。`attrs`は既存の
`update_entity`でundo可能に書き換わり、`.mdkproj`のスキーマは変わらないので`format_version`は
**13から上げない**。新しい型付きフィールドを足したくなったらそれは設計の誤りのサインなので、
上げる前に「`attrs`で表せないか」を必ず検討し、上げるなら`model.rs`の`FORMAT_VERSION`を14にして
`io.rs`にマイグレーションと「13の頃のファイル」テストを1本足す (既存の13本と同じ形)。

| 積み残し | モデル | Command | UI面 | .penボード |
|---|---|---|---|---|
| 並べ替え | `attrs["row_order"]` | `update_entity` | ツールバーのドロップダウン | 既存「M4デザイン - 端子台チャート/帳票様式」 |
| 端子番号の自動採番 | `attrs["terminal_numbers"]` | **`renumber_terminals`/`set_terminal_numbers`(新)** | ツールバー→採番ポップオーバー | 同ボードに追加 |
| 予備端子の挿入 | `symbol_id`の極数 + `terminal_numbers` | `update_entity` | ツールバー→挿入ポップオーバー | 同ボードに追加 |
| 多段端子の生成/解除 | `symbol_id`の`_{levels}l` | `update_entity` | ツールバー→段数ステッパー | 同ボードに追加 |
| 内外スワップ | `attrs["swapped"]` | `update_entity` | ツールバー (選択行に作用) | 同ボード (列見出しの入替表示) |
| 図面へジャンプ | なし (派生) | なし | ツールバー + 行のダブルクリック | 不要 (既存reveal動線) |
| 部品割当 | `SymbolInstance.value` + 部品DB`terminal_kind`列 | `update_entity` | ツールバー→部品DB検索ポップオーバー | 同ボードに追加 |
| アクセサリ | `attrs["accessories"]` | `update_entity` | ツールバー→付属品表 | 同ボードに追加 |

**新規ボードは作らない。** 既存ボード「M4デザイン - 端子台チャート/帳票様式」にはEPLAN準拠グリッド
(外部/内部・ジャンパ・多段Lv・予備・ツールバー・チェック) が既にあるので、**足りないのはツールバーから
開く5つのポップオーバー**(採番・予備挿入・段数・部品割当・付属品) と、Lv列/スワップ後の列見出しの状態
バリエーションだけ。CLAUDE.md「デザイン作業の手順」どおり、ポップオーバーの部品は先に
「デザインシステム - 共通コンポーネント」ボードを見て既存の`DS*`コンポーネントを流用し、足りない
ものだけ先にデザインシステムボードへ追加して`docs/internal/design-system.md`にも同時に書く。
実装前にPen.app (pencil MCP) でボードを更新すること — 見た目に関わる実装をデザイン確定前に進めない。

### 属性の書式 (既存の`jumpers`="1-2,3-4" / `pin_groups`="1-4:電源,5-8:信号" と同じ流儀)

- `attrs["row_order"]` = `number` | `alnum` | `page` | `cable` | `jumper` の1語。未設定・未知語は`number`
  (仕様§1.1の「数値順/英数順/ページ順(回路図の出現順)/外部ケーブル順/ジャンパ順」)。多段端子は
  **極→段の順で必ず隣り合う**ので、どの並びでもグループは自然に保たれる (EPLANの「多段端子は
  グループを保って移動」を、行の並べ替えではなくキーの組で満たす)
- `attrs["terminal_numbers"]` = ピン順のラベルをカンマ区切り (例 `"11,12,13,14,15,16,17,18"`)。
  未設定 = シンボル定義の既定 (`1`,`2`,… / 多段は `1a`,`1b`,…)。**個数がピン数と合わない・重複がある・
  空要素がある指定は図面を変えずにエラー**(部分適用しない。値セットの拒否と同じ流儀)
- `attrs["swapped"]` = 内外を入れ替えた端子番号のカンマ列 (例 `"3,4"`)。未設定=入替なし。
  端子単位にするのはEPLANが端子単位で入替えるため。知らない端子番号はその項目だけ無視し
  ERC診断`erc.terminal_swap_unknown`(Warning) を出す (`pin_groups`の未知ピンと同じ扱い)
- `attrs["accessories"]` = `端子番号:種別:型番` のカンマ列 (例 `"1:end_bracket:TB-EB,end:end_bracket:TB-EB"`)。
  「その端子の**直前**に入る付属品」で、`end`は末尾。種別は仕様§1.1どおり
  `end_bracket`(エンドブラケット) / `partition`(仕切り) / `end_plate`(エンドプレート) の3種のみ

### 部品割当

型番は既存の`SymbolInstance.value` (BOMが既に見ている欄) が正。端子ブロックの**種別**
(多段・貫通・ヒューズ端子等) は部品DBの新列`terminal_kind`で持ち、`parts.rs`の
`SCHEMA_VERSION`を**6→7**へ上げてマイグレーションを書く (`.mdkproj`の`format_version`とは別物)。
配置時に`attrs`へ写す既存の流儀 (`contact_config`/`footprint_w`) に倣い、割当時に
`attrs["terminal_kind"]`へ写してチャートの「部品型番」列と端子台チェックが読む。

### 範囲外 (このフェーズでは触らない)

仕様の他の🔶ブロックが「残り」と書いている項目は、いずれも端子台エディタと独立なので別計画にする。
Task 6でその旨を仕様の状態文に明記する。

- §2 回路マクロ: **保存ダイアログからのバリアント追加** (UIはdisabledプレースホルダのまま)
- §3 PLC I/O: **ラダーの横バス様式**、配置方針の**②同居・③同居+分割**、**生成設定のプロジェクト保存**
  (現状セッション限り)
- §4 XRef: **表現形式(表/グリッド)の図枠テンプレート側選択** (§9と同時にやる約束)
- §5 帳票: **ケーブル一覧** (Cableエンティティのモデルが未決。仕様の未決事項に残っている)、
  **BOM拡張** (部品DBのメーカ/価格/購入先の列追加)
- §10 検索/ナビゲーション: **未配置機能(予約接点)のドラッグ配置** (予約接点のモデルが必要)
- 付属品の幅を盤レイアウトの`panel.rail_fill`/間隔チェックへ数え入れること (Task 5でdocsに1行残す)

### 見落としやすい入力 (各タスクのテストで必ず押さえる)

1. 端子番号の上書き後にジャンパ指定が古い番号を指している (`jumpers`は物理ピンではなくラベルで
   書かれている → 採番と同じCommandでジャンパ指定も書き換わること)
2. 極数を増やしたとき既存のピン座標が動く/動かない (動くなら既存結線が外れる。Task 4のred stepで固定)
3. 多段(3段)の端子台で`swapped`と`row_order`を同時に指定したときの列見出しと行順
4. `terminal_numbers`の要素数がピン数と合わない・重複する・空文字が混ざる指定 (図面を変えずに拒否)
5. 付属品の型番が部品DBに無い/空のとき (BOMに空行を作らず、端子台チェックがWarningを出す)

---

### Task 1: 端子台エディタの導出基盤 (Lv列・並べ替え・図面へジャンプ)

**Files:** `src-tauri/crates/madake-core/src/terminal_chart.rs` (`RowOrder`・`row_order_of`・
`sort_terminal_rows`・`TerminalChart.row_order`) / `src/stores/terminals.ts` (`TerminalEditorRow.level`・
`setRowOrder`・`gotoTerminal`) / `src/components/TerminalEditorDialog.vue` (Lv列・並べ替えドロップダウン・
図面へジャンプ) / `src/ipc.ts` (`TerminalRow.level`・`TerminalChart.row_order`) / 6カタログ /
`docs/05-wire-management(.ja).md`

- [ ] **Step 1 (red):** `terminal_chart.rs`に5種の並びのテスト (数値順=既定 / 英数順 / ページ順=回路図の
      出現順 / 外部ケーブル順 / ジャンパ順)、未設定・未知語が`number`に落ちること、3段端子台で
      極→段の順が崩れないこと、`TerminalChart.row_order`がCSV・図面シート・接続図の生成で同じ並びを
      返すこと (チャートCSVの行順=エディタの行順)。`src/stores/terminals.test.ts`に
      `rows`が`level`を運ぶこと・`setRowOrder`が`update_entity`1回になること・`gotoTerminal`が
      `useReveal`へ`{sheetId, entityId}`を渡すこと
- [ ] **Step 2 (green):** 実装。ダイアログにLv列 (`terminals.col.level`。単段は空欄) と並べ替え
      ドロップダウン、「図面へジャンプ」ボタン+行のダブルクリックを足し、`terminals.phase2.sort`/
      `terminals.phase2.goto`のdisabledプレースホルダを外す (6カタログから`phase2.sort`/`phase2.goto`を
      削除し実キーへ。キー同値テストが落ちないよう全カタログを同時に直す)。
      ストアが端子一覧を持たないまま`reloadChart`だけ走る経路 (フェーズ6積み残し) を`reloadBlocks`
      経由に統一する。`cargo test`+`npx vitest run`+`npx vue-tsc --noEmit` → 実機確認 (TB1で
      5種の並べ替えとジャンプ、⌘Zで並びが戻る) → `python3 scripts/gen_spec.py` → コミット
- [ ] **Step 3 (docs):** `docs/05-wire-management.md`と`.ja.md`の端子台エディタ節に並べ替え・Lv列・
      ジャンプを追記 (並べ替えは図面の状態なので⌘Zで戻ることを明記)。`docs/internal/feature-inventory.md:140`
      の「Lv列表示…は未実装」を実態へ、`:68`の端子台チャートCSVにLv列が入った互換性の注記
      (フェーズ6積み残し) を1行足す → コミット

### Task 2: 端子番号の自動採番 (`renumber_terminals` / `set_terminal_numbers`)

**Files:** `src-tauri/crates/madake-core/src/command.rs` (2 variant) /
`src-tauri/crates/madake-core/src/terminal_chart.rs` (`terminal_numbers_of_instance`) /
`src-tauri/crates/madake-core/src/svg.rs` + `src/canvas/dynamicSymbol.ts` (ラベル上書きの双子) /
`src/stores/terminals.ts` / `src/components/TerminalEditorDialog.vue` / 6カタログ

- [ ] **Step 1 (red):** `command.rs`に`RenumberTerminals { sheet_id, entity_id, start, step, prefix }`の
      テスト (逆コマンド=`SetTerminalNumbers { sheet_id, entity_id, numbers: Option<Vec<String>> }`で
      ⌘Z一発。`None`=既定へ戻す)。既存の`RenumberWires`/`SetWireNumbers`・`RenumberPanelItems`/
      `SetPanelItemNumbers`と**同じ形**にすること。押さえる挙動: 採番と同時に`attrs["jumpers"]`の
      端子番号も新番号へ書き換わる / 多段は段記号を保って極だけ採番する (`11a`,`11b`,…) /
      個数不一致・重複・空要素の`numbers`は図面を変えずにエラー / 存在しないentity idはエラー。
      `terminal_chart.rs`にチャート・接続図・端子台チェックが上書き番号で出ること、`svg.rs`と
      `src/canvas/dynamicSymbol.ts`が同じラベルを描くこと (双子の突き合わせテストを両側に1本ずつ)
- [ ] **Step 2 (green):** 実装 + ツールバー「採番」→開始番号・刻み・接頭辞のポップオーバー
      (`terminals.phase2.autoNumber`を実キーへ)。Tauri IPC/Link API/MCPは`execute_commands`で
      そのまま通るので新しい口は作らない。`cargo test`+`vitest`+`vue-tsc` → 実機確認 (8極を11から
      採番 → ジャンパ`11-12`が追従 → ⌘Zで`1`..`8`へ戻る) → `gen_spec.py` → コミット
- [ ] **Step 3 (docs):** `docs/05-wire-management(.ja).md`に採番を追記。フェーズ6積み残しの
      「最寄りマーク帰属のテストが端子番号の入れ替わりを検出しない」を、採番後のショートバー帰属を
      固定するテストで閉じる (`src/canvas/terminalJumpers.test.ts` + Rust側の対になるテスト) → コミット

### Task 3: 多段端子の生成・解除

**Files:** `src-tauri/crates/madake-core/src/terminal_chart.rs` (`multilevel_id`) /
`src-tauri/crates/madake-core/src/symbol.rs` (`_1l`の正規化) / `src/stores/terminals.ts` /
`src/components/TerminalEditorDialog.vue` / 6カタログ / `docs/internal/feature-inventory.md`

- [ ] **Step 1 (red):** `terminal_chart::multilevel_id(symbol_id, levels)`のテスト:
      `terminal_block_8p` + 2段 → `terminal_block_8p_2l` / 解除 (1段) → `terminal_block_8p`に戻る
      (`_1l`の別名idは作らない = フェーズ6積み残し。`symbol.rs`側は`terminal_block_8p_1l`を
      **拒否**するか単段idへ正規化するかを1本のテストで固定する) / 4段以上と0段は拒否 /
      段を増やしても既存のジャンパ指定・結線が壊れない (段記号の付かない`1-2`は`1a-2a`へ移行) /
      段を減らすとき消える段に結線が残っていれば図面を変えずにエラー。
      `jumper_bars`の可視性が他のシンボル付随描画と同じ表示クラスで扱われること (フェーズ6積み残し)
- [ ] **Step 2 (green):** 実装 + ツールバー「多段」→段数ステッパー (`terminals.phase2.multiLevel`を
      実キーへ)。`cargo test`+`vitest`+`vue-tsc` → 実機確認 (8極を2段化 → 接続図が列に分かれる →
      ⌘Zで単段へ) → `gen_spec.py` → `feature-inventory.md:140`の「既存端子台への段の後付けは未実装」を
      実態へ → コミット

### Task 4: 予備端子の挿入 + 端子接続図の仕上げ (段ジャンパのレーン・千鳥配置)

**Files:** `src-tauri/crates/madake-core/src/terminal_chart.rs` (`insert_spare_terminals`) /
`src-tauri/crates/madake-core/src/terminal_diagram.rs` / `src/stores/terminals.ts` /
`src/components/TerminalEditorDialog.vue` / 6カタログ / `docs/internal/feature-inventory.md:69`

- [ ] **Step 1 (red):** まず**既存挙動を固定するred**: `terminal_block_8p` → `terminal_block_9p`にした
      とき1〜8番ピンの座標が動かないこと (動くなら既存結線が外れるので、挿入の設計をその場で
      「末尾に極を足して番号だけ繰り下げる」へ確定する)。続いて`insert_spare_terminals(inst, after, count)`が
      返す`(symbol_id, terminal_numbers)`のテスト: 指定端子の後ろに予備が入った番号列になる /
      予備行は`TerminalRow.spare=true`で両側未結線 / 末尾挿入 / 0個・負数の拒否。
      `terminal_diagram.rs`: ある段のジャンパが他の段の行を横切らない (段ごとの配線レーン) /
      千鳥配置 (行ごとに1箱) で外部左・内部右が崩れないこと
- [ ] **Step 2 (green):** 実装 + ツールバー「予備挿入」→位置と個数のポップオーバー
      (`terminals.phase2.spareTerminal`を実キーへ)。挿入は`update_entity`1回 (symbol_id + 
      terminal_numbers) にまとめてundo一発にする。`cargo test`+`vitest`+`vue-tsc` → 実機確認
      (3番の後ろに予備1 → チャートに予備行 → 接続図PDFで段ジャンパが他段を横切らない) →
      `gen_spec.py` → `feature-inventory.md:69`の接続図の説明に千鳥配置と段レーンを追記 → コミット

### Task 5: 部品割当 + アクセサリ (部品DBスキーマv7・BOM行)

**Files:** `src-tauri/crates/madake-core/src/parts.rs` (`terminal_kind`列・`SCHEMA_VERSION` 6→7) /
`src-tauri/crates/madake-core/src/terminal_chart.rs` (`Accessory`・`accessories_of`) /
`src-tauri/crates/madake-core/src/reports.rs` (BOMの付属品行) / `src/stores/terminals.ts` /
`src/components/TerminalEditorDialog.vue` / 6カタログ / `docs/04-standards-output(.ja).md`

- [ ] **Step 1 (red):** `parts.rs`: `terminal_kind`列の追加とv6→v7マイグレーション (既存DBが開ける・
      サンプルに端子台種別が入る)。`terminal_chart.rs`: `accessories_of`が`端子番号:種別:型番`を解析し
      3種以外・不明な端子番号・書式エラーはその項目だけ飛ばして端子台チェックのWarningになること /
      チャートに付属品行が端子行の間の正しい位置に入ること (`end`は末尾)。`reports.rs`: BOMに付属品が
      型番ごとの数量行として載ること / 型番が空・部品DBに無いときBOMへ空行を作らないこと。
      併せてフェーズ6積み残しの2件: BOMの電線集約行が数量欄へ単位付き`"3.75 m"`と書く逸脱 (docs/04か
      台帳へ1行) と、使用長の無い配線しか無いプロジェクトで`(未割当) 0.00 m`の行が出ること (行を出さない)
- [ ] **Step 2 (green):** 実装 + ツールバー「部品割当」(部品DB検索ポップオーバー → `value`と
      `attrs["terminal_kind"]`を`update_entity`1回で書く) と「付属品」(位置・種別・型番の表)
      (`terminals.phase2.accessory`を実キーへ、`terminals.assignPart`を新設)。`cargo test`+`vitest`+
      `vue-tsc` → 実機確認 (TB1へ端子台型番を割当 → 両端にエンドブラケット → BOMに型番と数量2が載る) →
      `gen_spec.py` → コミット
- [ ] **Step 3 (docs):** `docs/04-standards-output.md`と`.ja.md`のBOM節に付属品行と電線集約の数量表記を
      追記し、**付属品の幅は盤レイアウトの`panel.rail_fill`/間隔チェックに数えない**ことを制限として
      1行明記 (`docs/internal/specs/reproduction-gaps.md`にも同じ1行) → コミット

### Task 6: 内外スワップ + 仕様ステータス・ロードマップの突き合わせ

**Files:** `src-tauri/crates/madake-core/src/terminal_chart.rs` (`swapped_terminals`) /
`src-tauri/crates/madake-core/src/terminal_diagram.rs` / `src-tauri/crates/madake-core/src/verify.rs` /
`src/components/TerminalEditorDialog.vue` / 6カタログ /
`docs/internal/specs/m4-industrial-core.md` / `docs/12-roadmap.md` + `.ja.md` /
`docs/internal/feature-inventory.md` / `docs/internal/specs/reproduction-gaps.md`

- [ ] **Step 1 (red):** `swapped_terminals`が`attrs["swapped"]`を解析し、チャートの内部側/外部側の列と
      接続図の左右が指定した端子だけ入れ替わること / 未知の端子番号はその項目だけ無視して
      `erc.terminal_swap_unknown`(Warning) が出ること / 3段端子台で`swapped`と`row_order`を同時指定した
      ときの列見出しと行順 / 空指定は入替なしと同じ
- [ ] **Step 2 (green):** 実装 + ツールバー「内外スワップ」(選択行に作用。`terminals.phase2.swap`を
      実キーへ)。ここで`terminals.phase2`と`terminals.phase2Hint`が6カタログすべてから消えることを
      確認する。`cargo test`+`vitest`+`vue-tsc` → 実機確認 (3・4番だけスワップ → 接続図で左右が入れ替わる →
      ⌘Zで戻る) → `gen_spec.py` → コミット
- [ ] **Step 3 (突き合わせ):** 状態文を実態へ揃える。**受け入れの結果はこのStep行に書き、
      受け入れ基準には書かない**(フェーズ6の反省)。
      - `docs/internal/specs/m4-industrial-core.md`: §1の🔶ステータスを「フェーズ5完了(2026-09-28)」へ書き換え、
        8項目それぞれの実装形 (attrsキー・Command名) と**フェーズ6で先に入った多段シンボル/ジャンパバー**を
        区別して記す。§0ベンチマーク表12行目の「並べ替え/多段/アクセサリが残り」を消す。
        §5の🔶は**ケーブル一覧とBOM拡張だけを残り**として残す (端子台チャート/接続図の分は閉じる)。
        §2・§3・§4・§10の🔶は残したまま、各行の「残り」が別計画であることを1語添える
      - `docs/12-roadmap.md`と`.ja.md`: M4 phase 1 の "Sorting, multi-level terminals and accessories need a
        model extension and come later" を、フェーズ5で入ったことに書き換え、末尾の
        "Deferred out of M4" 一覧から端子台の項目を外す (残る項目=reserved contacts / macro variants /
        PLCのラダー様式と配置方針 / cable summary / BOM enrichment / 盤内配線図・銘板・寸法線 /
        graphical plot-frame editor)
      - `docs/internal/feature-inventory.md`: 端子台エディタ行(`:140`)を✅へ、G6の状態文で
        「列間の配線レーン」を謳っている箇所 (`reproduction-gaps.md`) を実態 (段間ジャンパがそのレーンを使う) へ
      - `docs/superpowers/plans/2026-09-28-phase6-followups.md`: このフェーズで閉じた項目に
        「→ 2026-09-28 フェーズ5で解消」と付ける (消さない。棚卸しの履歴を残す)
      - `python3 scripts/gen_spec.py --check`相当が通ることを確認 → コミット

## 受け入れ基準

- 仕様§1受け入れ基準どおり、TB1 (8極・ジャンパ2箇所・予備1) のエディタ操作 (並べ替え・採番・ジャンパ) が
  undo可能なCommandとして動き、チャート表と端子接続図の両方が図面の結線・線番と一致する
- `terminals.phase2.*` と `terminals.phase2Hint` が6カタログすべてから消え、disabledのツールバー項目が残らない
- 8項目すべてが`Engine::execute`経由で、1操作=⌘Z一発で戻る (予備挿入・多段化のようにsymbol_idと
  番号を同時に変える操作も1コマンド)
- `.mdkproj`の`format_version`は13のまま (上げた場合はマイグレーションと旧形式テストが付いている)
- 部品DBは`SCHEMA_VERSION` 7で、v6のDBがそのまま開ける
- チャートCSV・図面シート・端子接続図・BOMが、エディタで見えている並び・番号・付属品と食い違わない
- `cargo test` / `npx vitest run` / `npx vue-tsc --noEmit` / `python3 -m unittest discover -s freecad-addon/tests`
  が通り、`docs/13-specification(.ja).md`がテストから再生成済み
- 仕様§1・§0表・`docs/12-roadmap(.ja).md`・`feature-inventory.md`・`reproduction-gaps.md`の状態文が
  実装と一致し、M4の残りが「ケーブル一覧・BOM拡張・マクロのバリアント追加UI・PLCの横バス/同居配置・
  予約接点配置・盤内配線図ほか・図枠エディタ」だけに絞られている
- 各タスクで見送った項目は`docs/superpowers/plans/2026-09-28-phase6-followups.md`へ追記されている
