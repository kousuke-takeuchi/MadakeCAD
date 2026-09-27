# 機械CAD連携

[English (canonical)](11-mechanical-integration.md)

SOLIDWORKS Electrical ⇔ SOLIDWORKS の関係に相当する電気・機械の往復連携を、FreeCAD 1.1+の上に構築する。電気データ(参照記号・型番・ピン接続)はMadakeCADがマスタ、ジオメトリ(3D配置・経路・実測長)はFreeCADがマスタ。同期は常に明示操作(暗黙の上書きなし)。

## 利用できる機能(土台)

- **Link API**(`/api/v1`、REST+SSE): 連携面はすでに稼働中。外部ツールはプロジェクト/ネットリストを読み、変更を購読し、Commandエンジン経由で書き込める
- **部品DBのフック**: `model_3d`(STEP/FCStdパス)・`mounting`列を予約済み
- 両世界を結ぶキーとしてのエンティティUUID

## 利用できる機能: ワークベンチ「MadakeCAD Link」(M5-1)

- FreeCAD 1.0+のアドオン([`freecad-addon/`](../freecad-addon/README.ja.md)): フォルダをFreeCADの`Mod`ディレクトリへコピーまたはシンボリックリンクし、ワークベンチ**MadakeCAD Link**を選び、ポート(既定9310)を設定して**Connect**
- パネルにはプロジェクト名・revision・シートごとの要素数と、選択中シートのネットリスト表(ネット名・線番・ラベル・`K1:A1, TB1:3`形式のピン・配線本数)が出る
- **Follow live**でMadakeCADのイベントストリームを購読し、表示中シートやシート構成が変わると再読込するので、MadakeCAD側(手作業でもAIでも)の編集がその場でFreeCADに現れる
- Link APIクライアントは標準ライブラリだけのPythonで、パネルのモデルとライブ追従の判断と合わせてFreeCAD無しで単体テストされる(`python3 -m unittest discover -s freecad-addon/tests`。テストは[仕様書](13-specification.ja.md)の一部)。書き込みは常に`POST /api/v1/commands`=Commandエンジン経由

## 利用できる機能: 部品対応付け+電線長書き戻し(M5-2)

- **Partsタブ**: プロジェクトの全シンボルと型番、部品DBの3Dモデルパス(`model_3d`)、対応付け済みのFreeCADオブジェクト。**Insert 3D model**はSTEP/IGES/BREPを読み込み(FCStdは取り込み)、オブジェクトに`madake_id`プロパティ(エンティティUUID)を付けてMadakeCADへ対応付けを登録する(`set_mech_link`、`Project.mech_links`に保存)。自分で置いたモデルは**Link selected object** / **Unlink**で対応付ける
- **Wiresタブ**: 全配線とネット/線番・現在の長さと出所・対応付け済みの経路オブジェクト。**Link selected route**でDraft Wire(形状を持つ任意のオブジェクト)を配線に対応付け、**Measure routes → write back lengths**で対応付け済み経路をすべて計測(`Shape.Length`、mm→m、1mm単位)し、確認のうえ`set_wire_lengths`で書き戻す(MadakeCAD側ではシートごとにundo1回。電線リストと電圧降下検証に反映)
- **暗黙の上書きなし**: 書き戻した長さは出所`freecad`を持つ。MadakeCADの配線プロパティでは長さの隣に「FreeCAD計測」バッジが出て、手で変えると出所は手入力に戻り警告がログに出る。同期は常にボタン操作
- ファイル形式: `.mdkproj`形式3で`mech_links`と`length_source`が増える。旧ファイルは対応付け空・長さ手入力扱いでそのまま開ける

## 利用できる機能: 経路同期・配置同期(M5-3)

- **Create route stubs**(Wiresタブ): 両端の部品が対応付け済みの配線ごとに、部品の配置どうしを結ぶ直線をFreeCADに作る(配線の`madake_id`付き、ネット名で命名)。頂点を足すかDraft Wireへ置き換えて(タグは保つ)実際の経路にし、「Measure routes → write back lengths」で長さを戻す。再実行しても経路オブジェクトがある配線は飛ばす
- **ネットのハイライト**: Netlistタブでネットを選ぶと、対応付け済みの部品と経路が3Dビューで選択される
- **Sync placements**(Partsタブ): 対応付け済みオブジェクトの配置(基点mm・Z軸回転deg)をMadakeCADへ書き戻す(`mech_links[].placement`、ファイル形式4)。MadakeCADのプロパティ「3D対応付け」行に表示され、将来の2D盤レイアウトシート(M4 §8)がフットプリントの初期配置として使う。配置のマスタはFreeCADで、このボタンでだけ同期する

## 計画中(M5)

- **2D盤レイアウトシート**(M4 §8): 同期した配置を使うMadakeCAD側の盤図面。先にUIデザインが必要

設計の記録: マスタースペック§7 / 実装分解: [specs/m5-freecad.md](internal/specs/m5-freecad.md)
