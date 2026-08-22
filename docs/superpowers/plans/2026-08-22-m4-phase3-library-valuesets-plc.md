# MadakeCAD M4フェーズ3 (シンボルライブラリ拡充+マクロ値セット+PLC I/O) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** M4優先順の残りから3項目: ①同梱シンボルライブラリ拡充(JIS C 0617主要記号を50種規模へ、仕様§7の一部) ②マクロ値セット(プレースホルダ、仕様§2の残り) ③PLC I/O(仕様§3)。シンボルエディタGUI・盤レイアウト・図枠テンプレートの実装はフェーズ4。

**共通ルール:** tests-as-spec+gen_spec。i18n。Command絶対原則。ドキュメントEN+JA同期。デザインは確定済み(.pen「M4デザイン - PLC I/O」等)。

## 設計決定

- **ライブラリ拡充**: JIS C 0617/IEC 60617の主要記号を追加(目標: コンタクタ主接点・サーマルリレー・MCB/MCCB・遮断器・変圧器・ブザー/ベル・表示灯色種・切替スイッチ・リミットスイッチ・近接/光電センサ・ソレノイド・モータ種・電源種・接地種・ヒューズ種等で計50種規模)。全て2.5mmグリッドピン+方向付き接続点+属性スロット。**形状に自信の無い記号は追加しない**(一般に確立した形のみ。参考にした記号名を仕様文へ)。部品挿入ダイアログのカテゴリ整理も同時に
- **値セット**: マクロ形式へ `placeholders: [{key, label{en,ja}, targets:[{entity_ref, field(value|attrs.X), …}]}]` と `value_sets: [{id, label, values:{key: value}}]` を追加。挿入時に値セット選択→対象フィールドへ一括適用(execute_batch内)。UI=マクロ挿入プレビューの値セットドロップダウン(デザイン確定済み)+保存ダイアログでのプレースホルダ指定(v1=既存attrs/valueから選ぶ簡易UI)
- **PLC I/O**: デザイン確定済み(割付表エディタ+生成設定+モジュールライブラリ+ラダーページ)どおり:
  - 部品DB v4: `plc_module`(点数・種別DI/DO・アドレス体系プレフィックス)。サンプル3メーカ
  - 動的シンボル `plc_di_{n}p`/`plc_do_{n}p`
  - I/O割付表(プロジェクト保存: `Project.plc_assignments`、format_version注意)+CSV入出力
  - 図面生成: 生成設定(ラダー形式・ラング間隔・配置方針3種)→execute_batchでI/O図面シートを生成(1ターンundo)
  - 双方向同期: 図面の結線→割付表の接続先・線番列(読み取り導出)。I/Oレポート(帳票機構re利用)

### Task 1: シンボルライブラリ拡充

- [x] Step 1 (red): Rustテスト: 新規各記号のピン(グリッド上・方向・番号)/属性スロット/カテゴリ分類/既存図面の後方互換(既存シンボルidの定義不変)
- [x] Step 2 (green): 記号追加(まとまりごとに小コミット)+部品挿入ダイアログのカテゴリ整理+i18n名。SVGサンプル出力で目視確認→gen_spec→コミット

> **完了(2026-08-22)。** 同梱47種+動的2系統。追加32種は下表のとおり。副産物として
> (a) 弧の角度が回転・ミラーに追従していなかった描画バグを修正(`rotate_arc_angles` /
> `rotateArcAngles`。従来は弧を使う同梱記号が無く露見していなかった)、
> (b) 多極機器の導通を極ごとに分離(`conducting_pin_groups`を`verify`/`spice`で使用)。
> 目視確認は`cargo run -p madake-core --example symbol_sheet -- <path.svg>`(全記号+
> 回転90/180/270の一覧シート)と、起動中アプリの部品挿入ダイアログで実施。

#### 追加リスト(2026-08-22確定。既存15種+新規32種=47種、加えて動的2系統`connector_{n}p`/`terminal_block_{n}p`)

`SymbolDef`を拡張: ピンに**接続方向**(`dir`: up/down/left/right)、シンボルに**属性スロット**(`text_slots`: TAG/PART/DESC/RATING の位置)と**検索キーワード**(`keywords`: en+ja)。カテゴリは`CATEGORY_ORDER`(power → protection → switch → relay → semiconductor → passive → output → instrument → connector)で並べ、同カテゴリは連続させる。

| カテゴリ | 追加ID(参照接頭辞) | 形の根拠 |
|---|---|---|
| power | `ac_source`(G) / `transformer`(T) / `rectifier_bridge`(D) / `earth_protective`(PE) / `frame_ground`(FG) | 交流電源=円+正弦波、変圧器=2巻線+鉄心2線、整流器=菱形ブリッジ内にダイオード、保護接地=接地記号を円で囲む(IEC 60417-5019)、フレーム接地=横棒+3本の斜線(IEC 60417-5020) |
| protection | `breaker_1p` / `breaker_2p` / `breaker_3p`(CB) / `disconnector_1p` / `disconnector_3p`(DS) | 遮断器=固定接点に×印、断路器=固定接点に直交する短棒。多極は極を5mmピッチで並べ破線の連動線 |
| switch | `pushbutton_nc`(PB) / `emergency_stop`(PB) / `switch_spdt`(SW) / `switch_3pos`(SW) / `limit_switch_no` / `limit_switch_nc`(LS) | b接点は既存`relay_contact_nc`と同じ house style(斜め可動接点+横切り線)。非常停止=きのこ形頭部(半円)、リミットスイッチ=操作ロッド端の塗り四角、切替=c接点(共通+2固定接点)、3位置=中立で開 |
| relay | `relay_contact_co`(K) / `contactor_3p`(K) | c接点の端子番号11/12/14(IEC 60947-5-1)。電磁接触器の主接点=各極の固定接点に半円(コンタクタ機能)+端子1/2・3/4・5/6 |
| semiconductor | `zener_diode`(D) | 陰極バーの両端を折り曲げたZ形 |
| passive | `capacitor_polarized`(C) / `inductor`(L) / `resistor_variable`(VR) / `varistor`(RV) | 有極性コンデンサ=直線極板+塗り極板+「+」、インダクタ=半円4連、可変抵抗=矩形を斜めに貫く矢印、バリスタ=矩形を斜線が貫き「U」を添える(電圧依存抵抗) |
| output | `motor_3ph` / `motor_1ph` / `motor_dc`(M) / `bell`(BL) | 円内に「M」+「3~」「1~」「⎓」。ベル=半円ドーム+底辺 |
| instrument(新設) | `voltmeter`(VM) / `ammeter`(AM) / `current_transformer`(CT) | 円内に「V」「A」。変流器=一次導体が円を貫き二次2線 |
| connector | `connector_plug` / `connector_socket`(J) | 差込接続器の雄=くさび、雌=受け側の半円(IEC 60617 03-03-01/02) |

**見送り**(一般に確立した形を確信できないため。仕様§7のシンボルエディタで各社様式に合わせて作る想定): サーマルリレー(熱動継電器)、近接センサ・光電センサ、圧力/フロート/温度スイッチ、セレクタスイッチ(回転操作子)、限時接点(タイマ)、ソレノイド・電磁弁、ヒータ、ブザー、避雷器(SPD)、ヒューズ断路器。

多極機器の追加に伴い、導通判定(`verify`/`spice`)を**極ごと**(1-2 / 3-4 / 5-6)に橋渡しするよう修正する(相間が短絡扱いになるのを防ぐ)。

### Task 2: マクロ値セット

- [x] Step 1 (red): Rustテスト: placeholders/value_setsの形式・検証/挿入時の一括適用(value・attrs)/値セット無し・不正参照の扱い/undo一発維持。TS: 挿入プレビューの値セット選択→apply引数/保存ダイアログのプレースホルダ指定
- [x] Step 2 (green): 実装(macros.rs拡張+UI)。実機確認(値セット付きマクロ→挿入で定格一括設定)→コミット

> **完了(2026-08-22)。** 形式: `placeholders: [{key, label, label_ja, targets: [{entity, field}]}]` +
> `value_sets: [{id, label, label_ja, values: {key: 値}}]`(どちらも`#[serde(default)]`で省略可 =
> 旧マクロJSONはそのまま読める)。`field`は`"value"`か`"attrs.<名前>"`。挿入は
> `insert_macro(…, value_set)`で、行き先の解決を**全部済ませてから**書き込み、
> `execute_batch`は1回のまま(undo一発)。不正な値セット・行き先は図面を変えずにエラー。
> UI=保存ダイアログのプレースホルダ表/値セット表、挿入ダイアログの値セットドロップダウン
> (値セットを持つマクロにだけ出る)。⌘C/Vの無名マクロは値セット無し。

### Task 3: PLC I/O(コア)

- [x] Step 1 (red): Rustテスト: 部品DB v4移行/動的シンボルplc_di_{n}p/割付表モデル(format_version)+CSV入出力/接続先・線番の導出/生成設定→ラダーページ生成(配置方針3種・ページ分割)/undo一発/I/Oレポート
- [x] Step 2 (green): 実装。gen_spec→コミット

> **完了(2026-08-22)。** 実装 = `madake-core/src/plc.rs`(モジュール定義・アドレス採番・
> CSV入出力・接続先/線番の導出・ラダーページ生成)+`model.rs`(`Project.plc_assignments`、
> **format_version 2**。旧ファイルは空の割付表で開き`io::migrate`で現行版へ)+
> `command.rs`(`set_plc_assignments`。逆コマンド=旧リスト)+`symbol.rs`(動的
> `plc_di_{n}p`/`plc_do_{n}p`、1〜64点・点ピッチ5mm・カテゴリ`plc`)+`parts.rs`
> (**スキーマv4**の`plc_module`列+サンプル3種)+`report_sheet.rs`(帳票`plc-io`)。
>
> 設計判断:
> - **アドレス体系**: 三菱=8進(X0..X7,X10)/Siemens=バイト.ビット(%I0.0)/AB=ワード/ビット(I:0/0)。
>   開始点をずらせるので2枚目のモジュールは続き番号から振れる
> - **CSVは2種類**: 割付表の往復用(アドレス・信号名・コメントの3列。取り込みは
>   Command経由でモジュール単位に置換)と、読み取り専用のI/Oレポート(接続先・線番を含む6列)
> - **ラダー生成**: 新シートは`Command::RestoreSheet`(組み立て済みシートの挿入)1本+
>   必要なら`set_plc_assignments`を`execute_batch`で1履歴 = **undo一発**。左の縦バス
>   (上端にネットラベル`P24`)から1点=1ラング、点ピッチとラング間隔が違っても線が
>   重ならないよう点ごとに違う列で縦に振り分ける。生成したページはERC指摘ゼロ
> - **v1の範囲**: ラダー形式=縦バス+横ラング、配置方針=モジュールごとに新ラダーのみ。
>   横バス形式・同居2方針・ページ分割は設定enumだけ用意して「未実装」エラー
> - 露出 = Tauri IPC 6本 / Link API(`GET /plc/modules`・`GET|PUT /plc/assignments`・
>   `POST /plc/assignments/import`・`POST /plc/generate`)/ MCPツール3種 / CLIの`plc-io`帳票
>
> 残り(Task 4以降): 生成設定のプロジェクト保存(再生成での再利用)、割付表エディタUI。

### Task 4: PLC I/O(UI)

- [x] Step 1 (red): TS: 割付表エディタ(編集→Command/CSV読込/生成設定の組み立て)
- [x] Step 2 (green): デザインどおり実装(割付表エディタ+生成設定ダイアログ、リボン配線)。実機確認(16点DI割付→図面生成→結線変更が表へ反映→undo)→コミット

> **完了(2026-08-22)。** `src/stores/plcIo.ts`(下書き+生成設定)+
> `PlcIoDialog.vue`(割付表エディタ)+`PlcGenerateDialog.vue`(生成設定)。
> 仕様テスト30本(`src/stores/plcIo.test.ts`)。
>
> 設計判断:
> - **下書き方式**: グリッドはモジュールの点数ぶんの行を常に出し、未割付の行には
>   自動採番の候補アドレスを**placeholderとして**見せる(表そのものは変えないので
>   開いた直後はdirtyにならない)。「自動採番で埋める」で実際に埋め、「保存」で
>   `set_plc_assignments`1回。**末尾の空行は保存しない**(途中の空行は
>   n行目=点n番の対応がずれるため残す)
> - **アドレス採番はTS側にも実装**(`autoAddress`/`parseAddressIndex`)。Rustの
>   `PlcModuleSpec::address_at`と同じ規則で、入力中のプレビューと開始アドレスの
>   読み戻しに使う(ジャンパ指定と同じく「Rustが正・TSは同じ規則を持つ」方式)
> - **モジュール定義の引き当て**: 図面の`value`(型番)で部品DBのPLCモジュールを探し、
>   無ければ種別どおりの既定(三菱・DI=X / DO=Y)へフォールバック
> - **外部変更**: `document.revision`監視。編集中(dirty)は下書きを残したまま
>   接続先・線番の列だけ取り直し、編集していなければ表ごと読み直す
> - **CSV**: 読み込みは`<input type="file">`→`import_plc_assignments_csv`
>   (TauriでもブラウザでもFileReaderで同じ経路)。書き出しは帳票`plc-io`のCSV
>   (リボン「PLC I/Oレポート」)を使う
> - **リボン**: IAの「読み込み/書き出し」タブが未実装のため、実装済みの「レポート」
>   タブへ**PLC I/Oグループ**を追加(大=PLC I/O割付表 / 小=I/O図面を生成・PLC I/Oレポート)。
>   「PLC I/Oレポート」は帳票ダイアログ(種別`plc-io`)に接続した
> - **デザインからの追加**: 割付表エディタのフッタに「保存」(下書きの確定手段が
>   デザインに無かった)。生成設定の「ラダー幅」はv1の生成が使わないので出さない。
>   どちらも`docs/internal/design-system.md`に記載済み
> - 副産物: **TS側の動的シンボルに`plc_di_{n}p`/`plc_do_{n}p`を追加**
>   (`src/canvas/dynamicSymbol.ts`)。Task 3でRust側にしか無く、PLCモジュールを
>   図面に置けなかった(「既定シンボルが無い」で配置が失敗していた)
>
> 実機確認(`npm run tauri dev` + http://localhost:1420): FX5-16EX相当を配置 →
> 自動採番 → 信号名2件+コメント入力 → 保存(16行) → 生成設定(間隔10mm)→
> 「PLC PLC1」ページ生成(1点=1ラング・X0行に「X0 非常停止入力」)→ 外部から
> 結線+ネットラベル追加で接続先「ESTOP」・線番「15」が表へ反映 → CSV読み込みで
> 3行に置換 → 帳票`plc-io`のCSV書き出し(6列)→ undo6回で空図面へ戻ることを確認。

### Task 5: 仕上げ

- [ ] 受け入れ: 新記号での作図・BOM・SVGが既存同様に動く/値セット付きモータマクロで定格一括設定/CSVから16点DI→図面生成→レポート一致。docs(03/07他EN+JA)・feature-inventory・roadmap更新。全テストgreen+後始末

## 受け入れ基準

- 新記号50種規模が配置・配線・BOM・SVG/PDFで既存記号と同等に機能
- 値セット選択で関連フィールドが一括設定され、挿入はundo一発のまま
- CSV→割付表→I/O図面生成→結線変更→割付表/レポート反映の往復が成立
