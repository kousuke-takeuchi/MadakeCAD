# KiCad / EPLAN / AutoCAD Electrical との連携(インポート・エクスポート)

[English (canonical)](interop.md)

作成: 2026-09-27。対応する柱: G3(ベテラン品質・ACADE/EPLANベンチマーク)、G5(OSS: ロックインしない)。
ステータス: **実装済み**(KiCad入出力、DXF入出力)。末尾の「未決事項」は未実装。

## 1. 目的

KiCadから移行するユーザーや、AutoCAD Electrical(ACADE)・EPLANを使う顧客と図面をやり取りするユーザーが、描き直さずに図面を持ち込み・持ち出せるようにする。本仕様は、各エコシステムで現実的なファイル形式と、MadakeCADのモデルとの対応を確定する。

## 2. 形式の境界(できること・できないこと)

| エコシステム | ネイティブ形式 | 公開・文書化 | MadakeCADが使う形式 |
|---|---|---|---|
| KiCad | `.kicad_sch`(S式テキスト) | あり(公開形式、版管理あり) | `.kicad_sch`を直接**読み書き** |
| AutoCAD Electrical | `.dwg`図面+`.wdp`プロジェクト一覧+MDB/SQLiteカタログ | なし(`.dwg`は非公開バイナリ) | **DXF**(ASCII、AutoCAD 2000形式=AC1015)。ACADEはDXFをネイティブに読み書きできる(`DXFIN`/`DXFOUT`、または「DXFで保存」) |
| EPLAN Electric P8 | `.elk` / `.zw1`(非公開データベース) | なし | **DXF**。EPLANはページ単位でDXF/DWGを入出力できる(「ページ > エクスポート > DXF/DWG」) |

帰結:

- `.dwg`・`.elk`・`.zw1`をMadakeCAD側で直接読み書きすることはせず、計画もしない。非公開かつリリースごとに変わる形式のため。`.dwg`→DXFの変換はAutoCAD・EPLAN・無償のODA File Converterで行ってから読み込む。
- DXFが運ぶのは**図形とブロック属性**であり、電気モデルではない。読み込みは配線レイヤの線分から配線を、ブロック挿入からシンボルを再構成し、対応の無いものはKiCadインポートと同じくスキップとして報告する。

## 3. ユーザーストーリー

1. KiCadユーザーとして、MadakeCADのシートを`.kicad_sch`に書き出し、ライブラリを入れずにKiCadで開き、`kicad-cli sch erc`で独立したクロスチェックをしたい。
2. ACADE図面を受け取る設計者として、顧客にDXFをもらい、MadakeCADで開いて配線・線番・注記を取り込み、対応付けられなかったブロックの一覧を見て手で置き直したい。
3. EPLAN/ACADEのレビュアーに図面を渡す設計者として、シートをDXFに書き出し、レビュアーが見慣れたレイヤ(`WIRES`・`WIRENO`・`TAGS`…)で開けるようにしたい。
4. 自動化ユーザーとして、上記をCLI・REST Link API・MCPサーバーから行いたい。

## 4. 機能仕様

### 4.1 KiCadエクスポート(`madake_core::kicad::export_kicad_sch`)

- シート1枚につき1つの`.kicad_sch`(KiCad 9形式、version `20250114`): 用紙サイズ/向き・表題欄(題名・日付・有効な改訂記号・会社)。
- シートで使うシンボルは`lib_symbols`へ`MadakeCAD:<symbol_id>`として1回だけ埋め込む。図形(折れ線/円/弧/矩形/文字。KiCadのシンボルライブラリ座標に合わせてY反転)とピン(長さ0、接続点=ピン位置)を持つので、外部ライブラリ無しでKiCadが開ける。
- シンボルインスタンスは`Reference`/`Value`プロパティ、その他属性は非表示プロパティ、回転と`(mirror y)`をインポーターと同じ規則で書くので、書き出し→読み戻しで配置が変わらない。
- 配線: セグメントごとに2点の`wire`。ジャンクション・ネットラベル・注記は1:1。
- 線番: KiCadに線番は無いので、**ネットの最長セグメント上のラベル**にする(ラベルの無いネットのみ)。KiCadでも再インポート後もネット名として残る。
- ハーネス境界: 破線の閉じた`polyline`+名前の`text`(KiCadに相当物が無い)。
- インポーター側の変更: `MadakeCAD:`で始まる`lib_id`は(静的・動的いずれも)symbol_idへ直接解決するので、自前の書き出しはlib_id対応表を介さずに往復する。

### 4.2 DXFエクスポート(`madake_core::dxf::sheet_to_dxf`)

ASCII DXF、`$ACADVER`=AC1015、`$INSUNITS`=4(mm)、非ASCII文字は`\U+XXXX`(コードページに依存しない表記)。Y軸は反転(`y_dxf = 用紙高さ − y`)、用紙枠を`FRAME`レイヤに書いて外形を用紙に一致させる。

| MadakeCAD | DXF | レイヤ |
|---|---|---|
| 配線(セグメントごと) | `LINE`(ACADEは配線レイヤのLINEを配線として認識する) | `WIRES` |
| 線番 | 最長セグメントの上(横線)/左(縦線)の`TEXT` | `WIRENO` |
| シンボル | ブロック`MDK_<symbol_id>`の`INSERT`(回転=グループ50、ミラー=X倍率−1)+属性`TAG1`(参照記号)・`CAT`(型番/値)・`DESC1`・`RATING1`・その他属性(大文字化)・`TERMnn`(ピン番号、非表示) | `SYMS`(ブロック)・`TAGS`・`DESC` |
| シンボルブロックの図形 | ローカル座標(Y反転)の`LINE`/`LWPOLYLINE`/`CIRCLE`/`ARC`/`SOLID`/`TEXT`と、上記属性の`ATTDEF` | `SYMS` |
| ジャンクション | ブロック`WDDOT`(ACADEの結線ドットと同名)の`INSERT` | `WIRES` |
| ネットラベル | `TEXT` | `LABELS` |
| 注記 | 行ごとの`TEXT` | `MISC` |
| ハーネス | 線種`DASHED`の閉じた`LWPOLYLINE`+名前の`TEXT` | `HARNESS` |

ブロック名はACADEのファミリコード(`HCR1`・`VCR1`…)やEPLANのシンボル番号を**模倣しない**。それらの対応表は製品固有で非公開のため。属性タグ(`TAG1`・`CAT`・`DESC1`・`RATING1`・`TERMnn`)はACADEの属性の慣習に合わせているので、属性を読むACADEのツール(タグ・カタログの帳票)からは値が見える。

### 4.3 DXFインポート(`madake_core::dxf::import_dxf`)

- `ENTITIES`セクションを持つASCII DXF(R12以降)を受け付ける。`$INSUNITS`=1(インチ)は×25.4、それ以外はmmとして読む。
- 用紙: 全エンティティの外形が収まる最小のA判(A4…A0。縦長なら縦置き)。図面は外形の左上を用紙原点に置く。MadakeCADが書き出したファイルは`FRAME`枠が外形を決めるので座標がそのまま往復する。
- 配線: **配線レイヤ**上の`LINE`/`LWPOLYLINE`/`POLYLINE`。配線レイヤは (a) `wire_layers`で指定されたもの、無ければ (b) 名前に`WIRE`を含み`WIRENO`/`WIRE_NO`/`WIRENUM`を含まないレイヤ(ACADEの`WIRES`・`_MULTI_WIRE_n`に一致)。該当レイヤに線分が1本も無ければ**全ての線分**を配線として読み、見つかったレイヤ名を警告に出す(`wire_layers`を指定して読み直せる)。
- ジャンクション: `WDDOT`の`INSERT`、または配線レイヤ上の半径1mm以下の`CIRCLE`。
- シンボル: `MDK_<symbol_id>`(静的・動的)の`INSERT`。属性`TAG1`/`TAG`→参照記号、`CAT`→値、`DESC1`/`RATING1`→DESC/RATINGスロット、その他タグ→attrs、`TERMnn`は無視。回転は90°の倍数のみ(それ以外は0°+警告)、X倍率が負ならミラー。他のブロック名はスキップし`NAME (TAG1) xN`として一覧する。
- 文字: レイヤ名に`WIRENO`→5mm以内の最寄り配線の線番(無ければ注記+警告)、`LABELS`→ネットラベル、`HARNESS`→左上角が5mm以内の囲みの名前、それ以外→注記(高さはグループ40)。`MTEXT`の段落区切り(`\P`)と書式コード、`\U+XXXX`をデコードする。
- `FRAME`レイヤと配線レイヤ上の閉じた多角形は無視。未対応の種類はスキップ一覧に数える。
- 結果: 1シートのプロジェクトと、KiCadインポーターと同じ`ImportReport`(件数・スキップ・警告)。

### 4.4 露出

| 経路 | DXF読み込み | DXF書き出し | KiCad書き出し |
|---|---|---|---|
| アプリ | 開くダイアログ(`.dxf`フィルタ)とリボン**読み込み/書き出し**タブ(KiCad / DXF読み込み、DXF / KiCad / SVG / PDF書き出し、帳票CSV)。読み込みは未保存の編集を先に確認し、KiCad読み込みと同じく保存先無しの状態になる | 同タブ(表示中のシート) | 同タブ |
| REST | `POST /api/v1/import/dxf` `{path, wire_layers?}` | `POST /api/v1/export/dxf` `{sheet_id?, path}` | `POST /api/v1/export/kicad` `{sheet_id?, path}` |
| MCP | `import_dxf` | `export_dxf` | `export_kicad` |
| CLI | `madake open x.dxf [--wire-layer WIRES,_MULTI_WIRE_1]` | `madake export dxf out.dxf [--sheet ID]` | `madake export kicad out.kicad_sch [--sheet ID]` |

読み込みはすべて`Engine::replace_project`を通る(undo履歴クリア、エージェントターン中断、チャット履歴リセット)。KiCad読み込み・ファイルを開くと同じ経路。

## 5. デザイン対象

- リボン「読み込み/書き出し」タブ: 読み込み(開く / KiCad回路図 / DXF)、書き出し(DXF大ボタン / KiCad / SVG / PDF)、帳票(部品表CSV / 電線リストCSV)。既存のリボン部品を再利用し、デザインシステムへの新規部品追加は無し。
- インポート結果の提示: 現状はコマンドラインのログ(件数・スキップ一覧・警告)。スキップしたブロックの一覧と「手で置く」チェックリストを出すダイアログはデザインフェーズの対象。

## 6. 受け入れ基準(すべてテストで固定。`docs/13-specification.ja.md`の「KiCadインポート/エクスポート」「DXF連携」「REST Link API」「引数定義とディスパッチ」「ファイルメニュー」参照)

- シートの書き出し→読み戻しで、用紙・シンボル(種類・位置・回転・ミラー・参照記号・型番・属性)・配線・ジャンクション・線番・ラベル・注記・ハーネス名が保たれる(`.kicad_sch`・DXFの両方)。
- ACADE流のDXF(`WIRES`/`_MULTI_WIRE_*`の配線、`WIRENO`の文字、`TAG1`付きの未知ブロック`HCR1`)から配線と線番が読め、未知ブロックが報告される。
- `wire_layers`の明示で配線になる線分が絞られる。配線レイヤの無いファイルは全線分を配線として読み警告する。
- インチのファイルは換算され、用紙は収まる最小のA判になる。
- 非ASCII文字は`\U+XXXX`で書き出し→読み戻しを生き残る。
- REST・MCP・CLIが既存のKiCad読み込み / SVG書き出しと同じJSON形で3操作を公開する。

## 7. 未決事項(未実装。着手前に判断)

1. **ACADE / EPLANのブロック名対応表** — ACADEのファミリコード(例: `HCR1`→リレーコイル)やEPLANのシンボル番号をMadakeCADのsymbol_idへ対応付ければ、スキップされる挿入が実シンボルになる。実案件のサンプルDXF(社外秘の参考図面)で作って検証する必要がある。`~/MadakeCAD/interop/`配下の編集可能なJSON表として提案。
2. **`.dwg`の直接対応** — 外部コンバータ(ODA File Converter・AutoCAD・EPLAN)経由のみ。「ODAがあれば呼ぶ」補助は可能だが配布・ライセンスの問題(ODAのコンバータは無償だが再配布不可)。
3. **EPLAN PXF / プロジェクトXML** — EPLAN独自の交換形式は非公開。計画しない。
4. **複数シートのKiCad書き出し** — 現状はシートごとに1ファイル。階層ルートシートから各シートを参照する形は可能だが、KiCadのインスタンスパスのため往復が壊れやすい。具体的な要望が出るまで保留。
5. **DXFでの線色・線径** — 未伝達(レイヤ色以外にACADEの慣習が無い)。必要ならXDATAで書く。
