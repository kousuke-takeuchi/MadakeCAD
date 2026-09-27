# インポート/エクスポート

[English (canonical)](08-import-export.md)

MadakeCADは3つのエコシステムと図面をやり取りできる。KiCadは公開のテキスト形式なので直接読み書きする。AutoCAD Electrical(ACADE)とEPLANはプロジェクトを非公開形式(`.dwg`、`.elk`/`.zw1`)で持つため、両製品がネイティブに入出力できる中間形式の**DXF**で受け渡す。設計の記録と対応表は内部の[連携仕様](internal/specs/interop.ja.md)にある。

## 利用できる機能

### KiCadインポート(`.kicad_sch`)
- KiCad 8/9の回路図を読み込み(自前S式パーサ、外部依存なし)
- 用紙サイズ/向き・表題欄・配線・ジャンクション・ラベル(local/global/hierarchical)・テキスト・対応表にあるシンボル(Device:R/C/D/LED/Fuse/Lamp/Battery、スイッチ、リレー、モータ)を変換。`Conn_01xNN`/`Screw_Terminal_01xNN`は極数を読んで可変ピンシンボル化。電源シンボル(`power:GND`・`power:+24V`等)は電気的意味を保ってネットラベル化。エクスポーターが書いた`MadakeCAD:<id>`は同じシンボルに戻る
- 未対応シンボルはスキップし**インポートレポート**(件数・スキップlib_id・警告)で報告
- 入口: アプリの「開く」またはリボン**読み込み/書き出し > KiCad回路図**、`madake open file.kicad_sch`、MCP `import_kicad`、REST `POST /api/v1/import/kicad`
- 既知の制限: ライブラリ間でピン形状が異なるため取り込み後に接続の手直しが必要な箇所がある(まさにその箇所をERCが指摘する設計)

### KiCadエクスポート(`.kicad_sch`)
- シートごとに1ファイル(KiCad 9形式)。シートで使うシンボルは図形とピンごとファイルへ埋め込む(`lib_symbols`、名前は`MadakeCAD:<symbol_id>`)ので、KiCad側にMadakeCADのライブラリを入れなくても開ける
- 配線(セグメントごとにKiCadのwire)・ジャンクション・ネットラベル・テキスト・表題欄は1:1。線番はそのネットのラベルになる(KiCadに線番は無い)。ハーネス境界は破線の多角形+名前
- 往復: 書き出して読み戻してもシンボルの配置・回転・ミラー・参照記号・値が保たれる
- `kicad-cli sch erc`による独立クロスチェックや、KiCadユーザーへの図面受け渡しに使う
- 入口: リボン**読み込み/書き出し > KiCad回路図**(書き出しグループ)、`madake export kicad out.kicad_sch [--sheet ID]`、MCP `export_kicad`、REST `POST /api/v1/export/kicad`

### DXFエクスポート(AutoCAD Electrical / EPLAN向け)
- AutoCAD 2000形式(AC1015)のASCII DXF、mm単位。日本語などの非ASCII文字は`\U+XXXX`で書く(AutoCAD・EPLANが復号する)
- レイヤ構成はACADEの慣習に合わせる: 配線=`WIRES`レイヤの`LINE`、線番=`WIRENO`、参照記号=`TAGS`、型番/説明=`DESC`、ネットラベル=`LABELS`、注記=`MISC`、ハーネス境界=`HARNESS`の破線多角形、用紙枠=`FRAME`
- シンボルはブロック`MDK_<symbol_id>`の挿入で、ACADE流の属性`TAG1`(参照記号)・`CAT`(型番/値)・`DESC1`・`RATING1`・`TERMnn`(ピン番号)を持つ。ジャンクションは`WDDOT`ブロック
- 入口: リボン**読み込み/書き出し > DXF**、`madake export dxf out.dxf [--sheet ID]`、MCP `export_dxf`、REST `POST /api/v1/export/dxf`

### DXFインポート(AutoCAD Electrical / EPLANから)
- ASCII DXF(R12以降)を読む。インチの図面はmmへ換算、用紙は図面が収まる最小のA判
- 配線レイヤ上の線分が配線になる。配線レイヤは名前で判定(`WIRE`を含む。ACADEの`WIRES`・`_MULTI_WIRE_1`など)するか明示する(`--wire-layer`、`wire_layers`)。配線レイヤが無ければ全ての線分を配線として読み、その旨を報告する
- `WIRENO`の文字は最寄りの配線の線番に、`MDK_<symbol_id>`ブロック(MadakeCAD自身の書き出し)は属性付きのシンボルに、`WDDOT`の挿入と配線レイヤ上の小さい円はジャンクションに、その他の文字は注記になる
- ACADE・EPLAN独自のシンボルブロックは公開された対応表が無いため、インポートレポートにスキップ(`HCR1 (CR1) x2`)として一覧し、手で置き直す
- 入口: アプリの「開く」またはリボン**読み込み/書き出し > DXF (ACADE/EPLAN)**、`madake open drawing.dxf [--wire-layer WIRES,_MULTI_WIRE_1]`、MCP `import_dxf`、REST `POST /api/v1/import/dxf`

### 他ツールからDXFを出す手順
- **AutoCAD Electrical**: `DXFOUT` / 名前を付けて保存 → AutoCAD 2000 DXF。配線レイヤ名は既定のままにするとインポーターが認識する
- **EPLAN Electric P8**: ページ → エクスポート → DXF/DWG(ページごとに1ファイル、DXFを選ぶ)
- **`.dwg`ファイル**: AutoCAD・EPLAN・無償のODA File Converterで先に変換する(MadakeCADは`.dwg`を読まない)

### プロジェクト形式(`.mdkproj`)
- 整形JSON(git差分可読)、`format_version`+マイグレーション。チャット履歴は`<名前>.chat.json`を併存

### 出力
- SVG・PDF・部品表CSV・電線リストCSV([規格・出力](04-standards-output.ja.md)参照)

## 計画中・未決

- ACADEのファミリブロックとEPLANのシンボル番号の対応表(挿入を実シンボルにする。サンプル図面が必要。連携仕様§7)
- `.dwg`/EPLANプロジェクトのネイティブ対応: 計画しない(非公開形式)。DXFを使う
- KiCadインポートの階層シート・バス対応(バックログ)
