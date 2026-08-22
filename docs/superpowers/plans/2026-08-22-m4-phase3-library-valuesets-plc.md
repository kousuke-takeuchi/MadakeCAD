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

- [ ] Step 1 (red): Rustテスト: placeholders/value_setsの形式・検証/挿入時の一括適用(value・attrs)/値セット無し・不正参照の扱い/undo一発維持。TS: 挿入プレビューの値セット選択→apply引数/保存ダイアログのプレースホルダ指定
- [ ] Step 2 (green): 実装(macros.rs拡張+UI)。実機確認(値セット付きマクロ→挿入で定格一括設定)→コミット

### Task 3: PLC I/O(コア)

- [ ] Step 1 (red): Rustテスト: 部品DB v4移行/動的シンボルplc_di_{n}p/割付表モデル(format_version)+CSV入出力/接続先・線番の導出/生成設定→ラダーページ生成(配置方針3種・ページ分割)/undo一発/I/Oレポート
- [ ] Step 2 (green): 実装。gen_spec→コミット

### Task 4: PLC I/O(UI)

- [ ] Step 1 (red): TS: 割付表エディタ(編集→Command/CSV読込/生成設定の組み立て)
- [ ] Step 2 (green): デザインどおり実装(割付表エディタ+生成設定ダイアログ、リボン配線)。実機確認(16点DI割付→図面生成→結線変更が表へ反映→undo)→コミット

### Task 5: 仕上げ

- [ ] 受け入れ: 新記号での作図・BOM・SVGが既存同様に動く/値セット付きモータマクロで定格一括設定/CSVから16点DI→図面生成→レポート一致。docs(03/07他EN+JA)・feature-inventory・roadmap更新。全テストgreen+後始末

## 受け入れ基準

- 新記号50種規模が配置・配線・BOM・SVG/PDFで既存記号と同等に機能
- 値セット選択で関連フィールドが一括設定され、挿入はundo一発のまま
- CSV→割付表→I/O図面生成→結線変更→割付表/レポート反映の往復が成立
