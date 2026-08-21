# インポート/エクスポート

[English (canonical)](08-import-export.md)

## 利用できる機能

### KiCadインポート(`.kicad_sch`)
- KiCad 8/9の回路図を読み込み(自前S式パーサ、外部依存なし)
- 用紙サイズ/向き・表題欄・配線・ジャンクション・ラベル(local/global/hierarchical)・テキスト・対応表にあるシンボル(Device:R/C/D/LED/Fuse/Lamp/Battery、スイッチ、リレー、モータ)を変換。`Conn_01xNN`/`Screw_Terminal_01xNN`は極数を読んで可変ピンシンボル化。電源シンボル(`power:GND`・`power:+24V`等)は電気的意味を保ってネットラベル化
- 未対応シンボルはスキップし**インポートレポート**(件数・スキップlib_id・警告)で報告
- 入口: アプリの「開く」、`madake open file.kicad_sch`、MCP `import_kicad`、REST `POST /api/v1/import/kicad`
- 既知の制限: ライブラリ間でピン形状が異なるため取り込み後に接続の手直しが必要な箇所がある(まさにその箇所をERCが指摘する設計)

### プロジェクト形式(`.mdkproj`)
- 整形JSON(git差分可読)、`format_version`+マイグレーション。チャット履歴は`<名前>.chat.json`を併存

### 出力
- SVG・PDF・部品表CSV・電線リストCSV([規格・出力](04-standards-output.ja.md)参照)

## 計画中

- KiCadエクスポート(`kicad-cli sch erc`による独立クロスチェックが可能になる)(M6時期)
- DXF/DWGエクスポート(AutoCADエコシステム連携。バックログ、ギャップ分析参照)
- KiCadインポートの階層シート・バス対応(バックログ)
