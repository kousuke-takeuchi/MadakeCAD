# M5 機能仕様: 機械CAD連携(FreeCAD)

対応する柱: G4。SOLIDWORKS Electrical⇔SOLIDWORKS相当の電気・機械連携。設計の骨子は[マスタースペック §7](../../superpowers/specs/2026-08-20-madakecad-design.md)が正(Link API・madake_id・マスタ権の原則)。本書はその実装単位への分解。

## 前提(実装済みの土台)

- Link API `/api/v1`(REST+SSE)は稼働中。書き込みは全てCommandエンジン経由
- 部品DBに`model_3d`(STEP/FCStdパス)・`mounting`列を予約済み
- 対象: FreeCAD 1.1+(Python API)

## M5-1: FreeCADアドオンWB「MadakeCAD Link」骨格 — ✅ 実装 (2026-09-27)

- 配置: 本リポジトリ`freecad-addon/`(Pythonワークベンチ、`package.xml`付き)。FreeCADのAddon Manager対応は公開後(M6)。計画: [`docs/superpowers/plans/2026-09-27-m5-1-freecad-addon-skeleton.md`](../../superpowers/plans/2026-09-27-m5-1-freecad-addon-skeleton.md)
- 機能: 接続設定(ポート。FreeCADのユーザーパラメータ`Mod/MadakeCADLink/Port`に保存)、接続状態表示、プロジェクト概要(名前・revision・シートごとの要素数)、**ネットリストビュー**(ネット名・線番・ラベル・`K1:A1, TB1:3`形式のピン・配線本数)、`GET /api/v1/events`(SSE)購読による図面変更のライブ追従(表示中シートに関わるpatchとシート構成の変化で再読込、revisionの重複・逆行は無視)
- 構成: FreeCAD非依存の純Python(`client.py`=標準ライブラリだけのREST+SSEクライアント / `model.py` / `events.py` / `settings.py`)を単体テストし(`python3 -m unittest discover -s freecad-addon/tests`、CIジョブ`freecad`)、Qt依存は`panel.py`/`commands.py`に閉じ込める。テストのdocstring(英日2行)は`docs/13-specification.md`の仕様項目になる
- MadakeCAD側: 変更不要(既存APIのみ)。`model_3d`フィルタはM5-2で必要になれば追加

受け入れ基準: FreeCADのパネルから起動中MadakeCADの図面概要とネットリストが見え、図面編集がリアルタイムに反映される。**実FreeCADでの目視確認はユーザー確認事項**(本リポジトリのCIにFreeCADは無い。純Python部分は15テストで固定)。

## M5-2: 部品対応付け+電線長書き戻し — ✅ 実装 (2026-09-27)

実装内容(計画: [`docs/superpowers/plans/2026-09-27-m5-2-part-linking-wire-length.md`](../../superpowers/plans/2026-09-27-m5-2-part-linking-wire-length.md)):
- モデル: `Project.mech_links: Vec<MechLink {entity_id, fcstd_path, object_name, synced_at}>`、`Wire.length_source: manual | freecad`(既定manual)。**format_version 2→3**(旧ファイルは既定値で開ける)
- Command: `set_mech_link`(entity_idで登録・置換)/ `remove_mech_link` / `set_wire_lengths`(シート単位の一括書き戻し。長さと出所を同時に設定、逆コマンドは書き換え前の値)。PatchOp `mech_links_replaced`。Link API `/commands`・MCP `execute_commands`でそのまま使える
- アドオン: パネルに「Parts」タブ(シンボル一覧+部品DBの`model_3d`+対応付け済みオブジェクト。**Insert 3D model**=STEP/IGES/BREPは`Part.read`、FCStdは`mergeProject`で取り込み、`madake_id`プロパティを付けて`set_mech_link`。**Link selected object** / **Unlink**)と「Wires」タブ(配線一覧+長さ・出所+経路オブジェクト。**Link selected route**=Draft Wire等を配線に対応付け、**Measure routes → write back lengths**=`madake_id`付き経路オブジェクトの`Shape.Length`をmm→m(1mm単位)へ換算し確認ダイアログの後に`set_wire_lengths`(出所freecad)+未登録経路の`set_mech_link`)
- MadakeCAD側UI: 配線プロパティの長さに「FreeCAD計測」バッジ、対応付け済みシンボル/配線に「3D対応付け」行(オブジェクト名)。計測値を手で変えると出所はmanualへ戻り、ログに警告
- 同期は明示操作のみ(ボタン)。自動上書きなし


- **部品挿入**: アドオンの部品リスト(図面のシンボル+部品DBのmodel_3d)から3DモデルをFreeCADアセンブリへ挿入。FreeCADオブジェクトに`madake_id`(entity UUID)を保存
- **対応付けの永続化**: MadakeCAD側`Project.mech_links: Vec<MechLink {entity_id, fcstd_path, object_name, synced_at}>`を新設(format_version++)。Command `set_mech_link`
- **電線長書き戻し**: FreeCADで経路(Draft Wire等)の長さを計測し、`POST /api/v1/commands`の`update_entity`で`Wire.length_m`へ一括反映。`Wire`に長さの出所フラグ(`length_source: manual | freecad`)を追加し、手入力での誤上書きを警告
- 同期は明示操作のみ(自動上書きしない)

受け入れ基準: FreeCADで引いた経路長がMadakeCADの電線リスト・電圧降下検証に反映される。往復してもmadake_idで再同期できる。

## M5-3: 経路同期・盤レイアウト(将来)

- 3D経路の可視化同期、パネル図(2D盤面)⇔3D筐体配置。詳細仕様はM5-2完了後に起こす

## デザイン対象

- FreeCADアドオンのパネルUI(FreeCAD側の流儀に従うためPencil対象外。ワイヤフレームのみ)
- MadakeCAD側: 電線長の出所表示(プロパティ/電線リストでのfreecad由来マーク)

## 未決事項

- [x] `length_source`の表示方法 → 配線プロパティの長さ欄の右にinfoバッジ「FreeCAD計測」+ツールチップ(2026-09-27。`.pen`ボードへの反映は次回のPencil作業時)
- [ ] アドオンの配布形態(リポジトリ同梱→Addon Manager登録のタイミング=M6と連動)
