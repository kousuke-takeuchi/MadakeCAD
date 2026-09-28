# MadakeCAD M3フェーズ4 (整えバリアント+並列エージェントの比較案UX) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M3仕様§3の残り「バリアント数(2〜4案の比較)」と§4の残り「比較案のUX」を、1つの仕組みで実装する。整えポップアップで**バリアント数**を選ぶと、同じ整え指示が**並列の会話**で2〜4案走り、案ごとの結果を**比べて1案だけ採用**(または全部破棄)できる。採用・破棄はいずれもundo一発。

**共通ルール:** 全編集はCommand経由(モデル直接変更禁止)。tests-as-spec+gen_spec。i18n(en/ja)。ドキュメントEN+JA同期。ロジックはmadake-core。

**Spec:** `docs/internal/specs/m3-ai-first.md` §3・§4(未決事項「比較案のUX」をここで決定)

## 設計決定(2026-09-27)

- **比較の場 = シート複製**(未決だった2案「シート複製で並べる / 提案パッチのプレビュー」のうち前者)。理由: 並列会話(フェーズ2実装済み)がそのまま使え、案は普通のシートとして図面タブで見比べられる(新しい描画UIが要らない)。パッチプレビューは1案ずつ順番に走らせる必要があり、並列の利点を捨てる
  - 開始: 元シートを`count`枚複製(名前「<元シート名> · 案A/B/C/D」、エンティティidは振り直し、**元id→複製idの対応表**を保持)。複製は`RestoreSheet`(組み立て済みシートを1コマンドで入れる既存の口)を`execute_batch`で入れる=開始は**undo一発**
  - 実行: 複製ごとに**新しい会話**を作り、同じ整えプロンプト(`tidyPrompt`。範囲=その複製のsheet_id、選択は対応表で複製側idへ写す)を送る。会話色の編集オーバーレイで、どの案がどこを編集中か見える(フェーズ2の並列会話そのまま)
  - 比較: 下部ドックパネル「整え案の比較」に案ごとの行(ラベル / 実行中・完了 / `tidy_metrics`の交差・重なり・グリッド外と合計、元シートとの差 / 「表示」で図面タブ切替 / 「採用」)。既存のドックパネル様式(検証結果・検索結果と同じ)を使い、**新規コンポーネントは作らない**
  - 採用: 選んだ複製の内容を**元シートへ写し戻す**(対応表で元idを保ったまま`UpdateEntity`、複製で消えたものは`DeleteEntities`、複製で増えたものは`AddEntity`)+全複製を`RemoveSheet`。これを`execute_batch`1回=**undo一発**。元idを保つのはFreeCAD連携(`mech_links`はentity UUIDがキー)と選択・検索の安定のため
  - 破棄: 全複製を`RemoveSheet`する`execute_batch`1回=undo一発
- **複製は普通のシート**(専用のエンティティも描画も足さない)。ただし**どのシートの複製かの印**だけは持つ(`Sheet.variant_of`、2026-09-28のレビュー対応で追加): 印が無いとシート間クロスリファレンス (G13) が元シートの全シンボルを「複数シートに跨る参照記号」と見なし、元シートにも各案にも存在しない住所注記 (`/2.B3`) が生える。印は複製が無いプロジェクトでは書き出されないので`format_version`は上げない。検証 (`verify_project`) には今も複製分が並ぶが、整えプロンプトは「エラーが増えていないか」の相対比較なので判断は狂わない
- 対応表(id_map)の保持場所はフロントのvariantsストア(比較はセッション内で完結する)。Link API/Tauriには開始・終了の2口を足し、対応表はクライアントが往復させる(サーバー側に状態を持たない)
- MCPツールには出さない(エージェント自身が案を並べる用途は無い)。Link APIには出す(CLI・外部からの自動比較が可能)
- Pencilボード「AIチャット - ポップアップ集」の**バリアント数チップ**は既存デザインどおり(disabled→有効化のみ)。比較パネルは既存ドックパネル様式の再利用。**`.pen`ボードへの反映は次回のPencil作業時**(フェーズ3のプロバイダUIと同じ扱い)

### Task 1: コア `variants.rs`(複製と写し戻しのCommand組み立て)

- [x] Step 1 (red): Rustテスト: 複製(名前・用紙・表題欄・**改訂欄**・シート種別`kind`・注記文字高`annotation_text_mm`・ゾーン数・全エンティティが同じ形でidだけ新しい / 引き出し線 (`leader_to`) が複製側へ写る / 対応表が全idを持つ / 複数枚で対応表が独立) / `start_commands`(RestoreSheetが末尾から順に並ぶ) / `adopt_commands`(動かした要素→元idのUpdateEntity、消えた要素→DeleteEntities、増えた要素→新idのAddEntity、変わらない要素→コマンド無し、引き出し線は元シート側のidへ写し戻す、最後に全複製のRemoveSheet) / `discard_commands` / `validate_run`(実在しないシート・別の比較の案・元シートの手編集を断る) / Engineで開始→編集→採用の往復がundo1回ずつで戻る
- [x] Step 2 (green): `madake-core/src/variants.rs`実装(`Entity::set_id`をmodelへ追加)。`lib.rs`へ`pub mod variants`。gen_spec

### Task 2: Link API + Tauriコマンド

- [x] Step 1 (red): `link_export.rs`(または新規`variants_api.rs`): `POST /api/v1/variants/start {sheet_id, count}`→`{patch, run:{original_sheet_id, variants:[{sheet_id,label,id_map}]}}` / `POST /api/v1/variants/finish {original_sheet_id, variants, chosen_sheet_id|null}`→patch(採用/破棄) / `GET /api/v1/tidy-metrics?sheet_id=`は既存。count範囲外(1未満・5以上)は400
- [x] Step 2 (green): `SharedDoc::execute_batch_as`追加、ハンドラ実装、Tauri `start_variants` / `finish_variants` / `get_tidy_metrics`。gen_spec

### Task 3: フロント(variantsストア・整えポップアップ・比較パネル)

- [x] Step 1 (red): vitest: `stores/variants.ts`: start=複製patch適用→案ごとに新会話で`tidyPrompt`送信(範囲=複製sheet_id、選択は対応表で写す)→パネル表示 / 会話の終了で`allDone` / `refreshMetrics`で元+各案の指標(案ごとに自分のシートを測る) / adopt=finishへchosenを渡しpatch適用・元シートをアクティブに・runを消す / discard=finishへnull / 実行中の案があるうちは採用不可 / 複製シートが消えた実行は理由つきで片付ける / `canvas/agentOverlay.ts`: patchのオーバーレイは表示中シートのopだけを拾い、案シートでは会話色を保つ
  - **計画変更 (2026-09-28)**: `tidy.ts`に`tidyVariantPrompt`は作らない。既存の`tidyPrompt(mode, sheet, selection)`が範囲 (sheet_id) と選択を引数で受けるので、複製のシートと写した選択を渡すだけで足りる (`stores/variants.ts`)。`tidy.ts`側の変更は「進め方の各手順にもsheet_idを埋める」だけ(省略すると`get_tidy_metrics`/`run_verification`が先頭シート=元シートを見てしまい、全案が同じ数値を報告する)
- [x] Step 2 (green): `ipc.ts`(startVariants/finishVariants/getTidyMetrics 両実装)、`stores/variants.ts`、`ChatTidyMenu.vue`(バリアント数チップを有効化。1=反復しない)、`VariantsPanel.vue`(ドックパネル)、`EditorLayout.vue`へ配置、i18n `chat.tidy.variant*`・`variants.*`。ブラウザ検証(モックAPI)

### Task 4: ドキュメント

- [x] `docs/internal/specs/m3-ai-first.md`(§3・§4をフェーズ4完了へ、未決事項に決定を記録)、`docs/12-roadmap(.ja).md`(M3完了)、`docs/09-ai-assistant(.ja).md`(整えバリアントの節、計画中から削除)、`docs/internal/feature-inventory.md`、`docs/internal/design-system.md`(比較パネル=ドックパネル様式の行)、`scripts/gen_spec.py`の見出し、`docs/13`再生成。全テストgreen

## 実施記録 (2026-09-27)

- Task 1: `madake-core/src/variants.rs`(`duplicate_sheet` / `start_commands` / `adopt_commands` / `discard_commands`、`Entity::set_id`追加)。テスト5件(複製・開始・採用の写し戻し・無変更採用・Engine往復のundo粒度)
- Task 2: `SharedDoc::execute_batch_as` / `start_variants` / `finish_variants`、Link API `/api/v1/variants/start`・`/finish`、Tauri `start_variants` / `finish_variants` / `get_tidy_metrics`。Link APIテスト(開始→案を編集→採用→undo→破棄、count範囲外は400)
- Task 3: `ipc.ts`(3口)、`stores/variants.ts`(+テスト11件)、`ChatTidyMenu.vue`(バリアント数チップ有効化・「比較パネルを開く」)、`VariantsPanel.vue`(ドックパネル)、i18n。モックLink API+ブラウザで「配置整理×2案」→案A/Bのタブ・比較パネル・実行中は採用不可を確認
- Task 4: 本ファイル、m3仕様(§3・§4・未決事項)、roadmap(M3完了)、09-ai-assistant(EN+JA)、feature-inventory、design-system、gen_spec見出し、仕様書再生成
- Tauri本体クレートはコンテナにGTKが無く`cargo check`不可(3コマンドは既存パターンに沿った目視確認のみ)。`.pen`ボードへの反映は次回のPencil作業時

## レビュー対応 (2026-09-28、`.superpowers/sdd/integration/review-3a4804f-variants.md`)

- **C1 引き出し線の写し替え**: `Entity::refs` / `Entity::remap_refs`をmodelへ追加し、`duplicate_sheet`は元id→複製id、`adopt_commands`は複製id→元idで`leader_to`を写す。採用後の元シートに残らない対象への参照は落とし、増えた要素は「参照される側が先」の順に`AddEntity`する
- **I1・I2 編集オーバーレイ**: `canvas/agentOverlay.ts`の`patchOverlayTargets`が表示中シートのopだけを拾う。色は案の複製なら会話色(シートで会話が分かる)、そうでなければ従来どおり
- **I3 sheet_id**: 整えプロンプトの「進め方」各手順に`sheet_id`を埋め、`get_tidy_metrics`のツール説明も明示を求める
- **I4 クロスリファレンス**: `Sheet.variant_of`を印に、`xref::device_sites`(RustとTSの両方)が複製を数えない。案シートは元シートを「自分の所在」として扱う
- **I5 終了の検証**: `variants::validate_run`(元シート・案シートの実在、その元シートの案であること、対応表の向き、採用先が案に含まれること)
- **I6 宙に浮いた実行**: `variants.clearIfStale()`が複製シートの消滅を検出して比較を片付ける(開始のundo・別経路の削除)
- **I7 元シートの手編集**: 開始時に元シートの指紋 (`sheet_digest`) を`VariantRun`へ入れ、採用の直前に照合して合わなければ断る(破棄は通す)
- **M1・M2**: 本ファイルのT3 S1・T1 S1の文言を実装に合わせ、複製テストに`revisions`・`kind`・`annotation_text_mm`・ゾーン数のassertを追加
- レビューI6 (start/finishが組み立てと適用の間でロックを放す) も同じ機会に直した(同一ロック内で組み立てて適用)

## 受け入れ基準

- 整えポップアップで「配置整理 × 3」を選ぶと、案A/B/Cのシートが増えて3会話が同時に走り、比較パネルに案ごとの指標が並ぶ
- 「採用」で元シートが選んだ案の配置になり、複製3枚が消える。Cmd+Z 1回でその前(複製3枚+元シート未変更)へ戻る。「すべて破棄」も同様にundo1回
- 採用後も元シートのエンティティidは変わらない(選択・FreeCAD対応付けが壊れない)
- 反復しない(1案)は従来どおり1ターン
