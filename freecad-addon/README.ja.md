# MadakeCAD Link(FreeCADアドオン)

[English (canonical)](README.md)

起動中の**MadakeCAD**とLink API(`http://127.0.0.1:9310/api/v1`)で会話するFreeCAD 1.0+のワークベンチ。
マイルストーンM5-1(骨格): 接続設定・プロジェクト概要・図面編集にライブ追従するネットリストビュー。
部品対応付けと電線長の書き戻しはM5-2(`docs/internal/specs/m5-freecad.md`参照)。

## インストール

このフォルダをFreeCADの`Mod`ディレクトリへコピーまたはシンボリックリンクし、FreeCADを再起動する:

| OS | `Mod`ディレクトリ |
|---|---|
| Linux | `~/.local/share/FreeCAD/Mod/`(または`~/.FreeCAD/Mod/`) |
| macOS | `~/Library/Application Support/FreeCAD/Mod/` |
| Windows | `%APPDATA%\FreeCAD\Mod\` |

```bash
ln -s /path/to/MadakeCAD/freecad-addon ~/.local/share/FreeCAD/Mod/MadakeCADLink
```

FreeCADのAddon Managerへの登録はOSS公開(M6)のときに行う。

## 使い方

1. MadakeCADを起動する(起動中はLink APIが生きている)。
2. FreeCADでワークベンチ**MadakeCAD Link**を選ぶ。パネルが右側にドッキングする。
3. ポート(既定9310)を設定して**Connect**。状態行にプロジェクト名・revision・要素数が出て、表に選択中シートのネット(ネット名・線番・ラベル・`K1:A1, TB1:3`形式のピン・配線本数)が並ぶ。
4. **Follow live**(既定ON)でMadakeCADのイベントストリームを購読し、表示中シートが変わると再読込する。**Refresh**で手動再読込。

## 構成

- `InitGui.py` — ワークベンチ登録(ツールバー/メニューに「MadakeCAD Link panel」「Refresh」)
- `madakecad_link/client.py` — Link APIクライアント(REST+SSE、標準ライブラリのみ)
- `madakecad_link/model.py`・`events.py`・`settings.py` — 純粋な補助関数。FreeCAD無しで単体テスト
- `madakecad_link/panel.py`・`commands.py` — Qtパネルとコマンド(FreeCAD内でのみ動く)
- `tests/` — `python3 -m unittest discover -s freecad-addon/tests`(CIでも実行。各テストは`docs/13-specification.md`の仕様項目になる)

FreeCADからの書き込みは必ず`POST /api/v1/commands`=MadakeCADのCommandエンジンを通る(アドオンは図面モデルに直接触らない)。
