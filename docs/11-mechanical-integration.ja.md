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

## 計画中(M5)

- **M5-2 部品対応付け+電線長書き戻し**: 部品リストから3DモデルをFreeCADアセンブリへ挿入、FreeCADオブジェクトに`madake_id`・プロジェクトに`mech_links`を保存。FreeCADで計測した経路長をワイヤの`length_m`へ書き戻し(出所フラグで手入力の暗黙上書きを防止)— 電圧降下検証・電線リストへ反映
- **M5-3 経路可視化・盤レイアウト**: 3D経路同期、2D盤面レイアウト⇔3D筐体の対応

設計の記録: マスタースペック§7 / 実装分解: [specs/m5-freecad.md](internal/specs/m5-freecad.md)
