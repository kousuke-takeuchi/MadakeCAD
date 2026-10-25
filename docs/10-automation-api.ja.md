# 自動化・API

[English (canonical)](10-automation-api.md)

UIでできることはすべてスクリプト化できる。3つの入口が同じCommandエンジンを共有するため、自動編集もundo履歴に乗り、UIへ即時反映される。

## 利用できる機能

### MCPサーバー(AIクライアント向け)
`127.0.0.1:9310/mcp`(Streamable HTTP、`MADAKE_MCP_PORT`で変更可)。ツール:
`open_project`・`save_project`(`.mdkproj`を開く・保存する。チャット履歴も一緒に扱う。開くと図面が置き換わり、エージェントの応答中は断る。アプリ内エージェントからは開けない)・`get_project`・`list_symbols`(同梱**と**ユーザーシンボル)・`place_symbol`(置いた部品のピンを返す。属性`hide_reference: "true"`で参照記号を印字しない。記号自体は残る)・`draw_wire`・`get_pins`(配置済み部品のピン: 番号・名前・用紙座標・向き)・`connect_pins`(ピン名どうしを直角配線でつなぐ。例 `U1.SDA` から `J1.2`。`connections` で複数をundo一発の1編集で引く)・`place_mate`(プラグとソケットの組を1回で置く。`mates` で複数。任意の`pole`は両方の部品に入り、From-To電線リストが`参照記号:1`の代わりに`参照記号:極番号`と書く。`mates`で極番号なしに同じ参照記号を並べると並び順に1, 2, 3と振る)・`connect_cable`(多芯ケーブルを片側または両側を1本線で描いて引く。`cores`=芯の名前、`a`/`b`=端ごとに`pins`か`part`と`style: multi`(ピンから分岐点まで芯ごとに配線)/`single`(1本線を部品に着ける)。`multi`の端は`pins`/`part`の代わりに`labels`(芯ごとのネットラベル名。`breakout_at`必須)も書け、芯ごとに区間と反対向きへ10mmの配線を引いてその名前のネットラベルで終える。分岐の配線は線径・線色を持たない。芯はピン順に対応する。`cables`で複数をundo一発の1編集で引く)・`execute_commands`(Command全スキーマ公開)・`get_netlist`・`run_verification`・`get_tidy_metrics`(4つの数値と、数えた問題を1件ずつ種類・エンティティ・文字・場所で並べた`details`。最大50件)・`simulate_op`・`search_parts`・`upsert_part`・`delete_part`・`import_kicad`・`import_dxf`(AutoCAD Electrical / EPLANのDXF。`wire_layers`任意)・`list_terminal_blocks`・`get_terminal_chart`・`check_terminal_block`・`list_templates`・`apply_template`・`list_macros`・`save_macro`・`apply_macro`・`duplicate_symbol`・`save_user_symbol`・`delete_user_symbol`・PLC I/O系・`get_sheet_image`(シートをPNG画像で返す。SVG出力と同じ見た目。`px_per_mm`とmmの`region`で拡大)・`export_svg`・`export_pdf`・`export_dxf`(AutoCAD 2000形式DXF)・`export_kicad`(`.kicad_sch`)・`export_report`(帳票7種のいずれかをCSVまたは図枠付きPDFへ)・`export_pdf_book`(表紙+全シート+選択帳票を1つのPDFへ)・`export_bom`・`export_wire_list`・`undo`・`redo`。
専用ツールが無い編集は`execute_commands`から実行する: `set_revisions`(改訂欄)、`set_sheet_annotation_text`(シートの注記文字高。`height_mm`は1.0〜3.5mm、nullで既定2.0mm。配線注記・線番・ネットタグの大きさが一緒に変わる)、`renumber_wires`(`mode: append | renumber`・`sheet_id`任意・`start`)と`set_wire_numbers`(線番)、ハーネス境界は`add_entity`で`harness`エンティティを追加、端子台のサドルジャンパは`update_entity`で端子台シンボルの`attrs["jumpers"] = "1-2,3-4"`を書き換える。図枠テンプレートは`set_frame_template`、盤レイアウトは`add_sheet`の`kind: "panel"`+`panel_outline`/`din_rail`/`duct`/`footprint`/`panel_legend`エンティティ、バルーン採番は`renumber_panel_items`、FreeCAD連携(3D対応付けと計測した電線長)は`set_mech_link`/`remove_mech_link`/`set_wire_lengths`。
ポートのピン: `place_symbol` の `attrs.block_pins` でピンに `contacts`(`{"no":"U3","name":"USB-A メス","contacts":["VBUS","D-","D+","GND"]}`)を付けると、1本のピンが複数の導体を運ぶ(接点は1ピンに2〜32個、前後の空白は除き、ピンの中で重ならない)。`get_pins` は `contacts` を返し、`get_netlist` はそのネットを `<名前>.<接点>` に分け、ネットとポートのピンに `contact` を付ける。`connect_cable` は `pins: ["39.U3"]` と `style: single` で片側をポートのピン1本に着ける(芯は同じ名前の接点に、名前がどれも合わず数が同じなら並び順に対応する。名前の一部だけの一致と、ポートへの `style: multi` は断る)。
コネクタの絵: `block_pins` のピンには `connector`(`usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f` のどれか。`_f` はメス、`_m` はオス)も書け、`screw`・`jst_ph_f`・`jst_xh_f` には `connector_ref` も書ける(同じ ref の続くピンは1つの本体になり、`J3 PH`・`TB1` と書く)。知らない種類・空の ref・ほかの種類の ref・2つのまとまりに使う ref は断る。コネクタのあるピンの接続点は外形から5mmなので、位置は `get_pins` で読む。`get_pins` とネットリストは種類を返さない。
Claude Codeはリポジトリの`.mcp.json`で自動接続。

### Link API(REST+SSE、外部ツール向け)
`127.0.0.1:9310/api/v1` — CLI・ブラウザでのUI検証・(将来)FreeCADアドオンが使う素のJSON REST:
- 読み: `/project`・`/symbols`(+`/symbols/user`)・`/frame-templates`・`/panel/unplaced`・`/netlist`・`/verify`・`/search`・`/devices`・`/parts`・`/wire-parts`・`/terminals`・`/terminals/chart`・`/terminals/check`・`/plc/modules`
- 書き: `/commands`(Command配列。`?batch=true`で配列全体を**1回の編集**=undo1回にする。途中で拒否されたら1件も適用しない)・`/undo`・`/redo`・`/save`・`/load`・`/import/kicad`・`/import/dxf`・`/simulate/op`・`/symbols/user/{save,delete}`・`/symbols/duplicate`・`/macros/{build,save,apply,apply-inline}`・`/templates/apply`・`/variants/{start,finish}`・`/export/{svg,pdf,dxf,kicad,pdf-book,report,bom,wire-list}`・部品CRUD
- ライブ更新: `GET /events`(SSEパッチストリーム)
- ローカルオリジンガード(外部Webオリジンからのアクセスを拒否)

### madake CLI
Link APIの薄いターミナルクライアント: `status`・`project`・`netlist`・`verify`・`sim`・`parts`・`terminals`(端子台一覧)・`export`・`save`/`open`(`.kicad_sch`・`.dxf`対応。DXFは`--wire-layer`)・`renumber`(線番採番: `--sheet`・`--mode append|renumber`・`--start`)・`exec`(JSONのCommand配列)・`undo`/`redo`。`--json`で機械可読出力。

`madake export <種別> <path>`は回路図と帳票の両方を扱う:

| 種別 | オプション |
|---|---|
| `svg`・`pdf`・`dxf`・`kicad` | `--sheet <ID>`(省略時は先頭シート)。`dxf`=ACADE/EPLAN向けAutoCAD 2000形式、`kicad`=`.kicad_sch` |
| `pdf-book` | `--reports wire-list,terminal-chart,terminal-diagram,bom,xref,panel-placement`(並べた順に付く)・`--no-cover` |
| `bom`・`wire-list`・`terminal-chart`・`terminal-diagram`・`xref-table`・`plc-io`・`panel-placement` | `--format csv\|pdf`(省略時は出力先の拡張子から判定。`.pdf`なら図枠付き図面シート)・端子台の2帳票は`--terminal <参照記号\|ID>` |

帳票は常に図面全体が対象で、絞り込めるのは`--terminal TB1`だけ(同じ参照記号が複数シートにある場合は`--sheet <ID>`を併用)。候補は`madake terminals`で調べる。

## 計画中

- M5アドオンが必要とするFreeCAD向けエンドポイント(例: 3Dモデル有り部品のフィルタ)
- OSS公開に向けたAPIの安定性・バージョニング方針(M6)
