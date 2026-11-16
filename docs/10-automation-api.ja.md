# 自動化・API

[English (canonical)](10-automation-api.md)

UIでできることはすべてスクリプト化できる。3つの入口が同じCommandエンジンを共有するため、自動編集もundo履歴に乗り、UIへ即時反映される。

## 利用できる機能

### MCPサーバー(AIクライアント向け)
`127.0.0.1:9310/mcp`(Streamable HTTP、`MADAKE_MCP_PORT`で変更可)。ツール:
`open_project`・`save_project`(`.mdkproj`を開く・保存する。チャット履歴も一緒に扱う。開くと図面が置き換わり、エージェントの応答中は断る。アプリ内エージェントからは開けない)・`get_project`・`list_symbols`(同梱**と**ユーザーシンボル)・`place_symbol`(置いた部品のピンを返す。属性`hide_reference: "true"`で参照記号を印字しない。記号自体は残る)・`draw_wire`・`get_pins`(配置済み部品のピン: 番号・名前・用紙座標・向き)・`connect_pins`(ピン名どうしを直角配線でつなぐ。例 `U1.SDA` から `J1.2`。`connections` で複数をundo一発の1編集で引く)・`place_mate`(プラグとソケットの組を1回で置く。`mates` で複数。任意の`pole`は両方の部品に入り、From-To電線リストが`参照記号:1`の代わりに`参照記号:極番号`と書く。`mates`で極番号なしに同じ参照記号を並べると並び順に1, 2, 3と振る)・`connect_cable`(多芯ケーブルを片側または両側を1本線で描いて引く。`cores`=芯の名前、`a`/`b`=端ごとに`pins`か`part`と`style: multi`(ピンから分岐点まで芯ごとに配線)/`single`(1本線を部品に着ける)。`multi`の端は`pins`/`part`の代わりに`labels`(芯ごとのネットラベル名。`breakout_at`必須)も書け、芯ごとに区間と反対向きへ10mmの配線を引いてその名前のネットラベルで終える。分岐の配線は線径・線色を持たない。芯はピン順に対応する。`cables`で複数をundo一発の1編集で引く)・`execute_commands`(Command全スキーマ公開)・`get_netlist`・`run_verification`・`get_tidy_metrics`(4つの数値と、数えた問題を1件ずつ種類・エンティティ・文字・場所で並べた`details`。最大50件)・`simulate_op`・`search_parts`・`upsert_part`・`delete_part`・`import_kicad`・`import_dxf`(AutoCAD Electrical / EPLANのDXF。`wire_layers`任意)・`list_terminal_blocks`・`get_terminal_chart`・`check_terminal_block`・`list_templates`・`apply_template`・`list_macros`・`save_macro`・`apply_macro`・`duplicate_symbol`・`save_user_symbol`・`delete_user_symbol`・PLC I/O系・`get_sheet_image`(シートをPNG画像で返す。SVG出力と同じ見た目。`px_per_mm`とmmの`region`で拡大)・`export_svg`・`export_pdf`・`export_dxf`(AutoCAD 2000形式DXF)・`export_kicad`(`.kicad_sch`)・`export_report`(帳票7種のいずれかをCSVまたは図枠付きPDFへ)・`export_pdf_book`(表紙+全シート+選択帳票を1つのPDFへ)・`export_bom`・`export_wire_list`・`undo`・`redo`。
専用ツールが無い編集は`execute_commands`から実行する: `set_revisions`(改訂欄)、`set_sheet_annotation_text`(シートの注記文字高。`height_mm`は1.0〜3.5mm、nullで既定2.0mm。配線注記・線番・ネットタグの大きさが一緒に変わる)、`renumber_wires`(`mode: append | renumber`・`sheet_id`任意・`start`)と`set_wire_numbers`(線番)、ハーネス境界は`add_entity`で`harness`エンティティを追加、端子台のサドルジャンパは`update_entity`で端子台シンボルの`attrs["jumpers"] = "1-2,3-4"`を書き換える。図枠テンプレートは`set_frame_template`、図面の書体は`set_text_font`(`{"text_font": {"id": "kst32b", "latin": "half"}}`で全ての文字を同梱のストロークフォントで描く。`latin: "full"`で英数字を全角の字に、`null`でシステムの書体へ戻す。知らないidは拒否)、盤レイアウトは`add_sheet`の`kind: "panel"`+`panel_outline`/`din_rail`/`duct`/`footprint`/`panel_legend`エンティティ、バルーン採番は`renumber_panel_items`、FreeCAD連携(3D対応付けと計測した電線長)は`set_mech_link`/`remove_mech_link`/`set_wire_lengths`。
ポートのピン: `place_symbol` の `attrs.block_pins` でピンに `contacts`(`{"no":"U3","name":"USB-A メス","contacts":["VBUS","D-","D+","GND"]}`)を付けると、1本のピンが複数の導体を運ぶ(接点は1ピンに2〜32個、前後の空白は除き、ピンの中で重ならない)。`get_pins` は `contacts` を返し、`get_netlist` はそのネットを `<名前>.<接点>` に分け、ネットとポートのピンに `contact` を付ける。`connect_cable` は `pins: ["39.U3"]` と `style: single` で片側をポートのピン1本に着ける(芯は同じ名前の接点に、名前がどれも合わず数が同じなら並び順に対応する。名前の一部だけの一致と、ポートへの `style: multi` は断る)。
貫通ピン: `block_pins` のブロックでは、同じピン番号を向かい合う辺に1回ずつ置ける(左と右、または上と下。同じ極の両側に配線がある形)。両側のピン名は前後の空白を除いて同じにし、どちらにも `contacts`・`connector` は付けられない。同じ辺に2回・隣り合う辺・3回以上は `duplicate block pin number` で断る。2つの接続点は1つのノードで、ネットのピン一覧には1回だけ載る。`get_pins` は両方を返し、`U1.1` だけの指定は曖昧なので `U1.1#1` か `U1.1#2` で選ぶ。#1・#2 はブロックのピン表の定義順(左・右・上・下の順。回転・反転の前)で、画面で見た辺ではない。反転したブロックでは `U1.1#1` が画面の右に描かれる。`block_numbers: "inside"` のとき、2本が同じ線上に並ぶ貫通ピンの番号は、その線と交わる辺(左右の組なら上下、上下の組なら左右)にピンが無いときだけ、箱の中央に1つ描く(あるときは各ピンが自分の番号を描く)。ファイル形式23。
コネクタの絵: `block_pins` のピンには `connector`(`usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f` のどれか。`_f` はメス、`_m` はオス)も書け、`screw`・`jst_ph_f`・`jst_xh_f` には `connector_ref` も書ける(同じ ref の続くピンは1つの本体になり、`J3 PH`・`TB1` と書く)。知らない種類・空の ref・ほかの種類の ref・2つのまとまりに使う ref は断る。コネクタのあるピンの接続点は外形から5mmなので、位置は `get_pins` で読む。`get_pins` とネットリストは種類を返さない。
細かい間隔: `place_symbol` の `attrs.block_pitch: "2.5"`(既定は `"5"`)でブロックのピン間隔を2.5mm、大きさの刻みを2.5mm、最小を5×5mmにできる。ピンn本の辺は(n+1)×2.5mm以上が要る。ピンと外形の辺が2.5mmの格子に乗るよう、外形とピンの列は必要に応じて1.25mmずらすので、位置は `get_pins` で読む。`attrs.block_numbers: "inside"`(既定は `"outside"`)はピン番号を外形の内側に描き、ピン名は描かないので、信号名は配線のネットラベルで出す。ほかの値は断り、2.5mmのブロックにも番号を内側に描くブロックにもコネクタの絵(`connector`)を使えない。

図面の見た目: `execute_commands` の `set_sheet_wire_style`(`sheet_id`、`weight_mm` は0.18〜1.0mm、nullで既定の0.35mm。`hide_notes` が true なら配線注記 = 線径・色名・長さを描かない。線の色はそのまま)。このコマンドの値は必ず一緒に設定され、省いた値は既定へ戻る(たとえば `hide_notes` を省いた呼び出しは注記を再び描く)。分岐点の丸は太い配線に合わせて大きくなり(半径は太さの0.9倍、0.6mmを下回らない)、ケーブルの芯線の分かれ目の線も同じ太さになる。`place_symbol` の `attrs.block_caption: "inside"`(既定は `"above"`)で、参照記号と型番をブロックの外形の上ではなく中央に書く(他のものは避けないので、中央の行が空いている箱で使う。中央の行にあるピン名や内側の番号とは重なる)。注記(`text` エンティティ)の `rotation` が 90・180・270 のとき、基準点を中心に時計回りへ回して描く。選択範囲と引き出し線も、回した文字に合う。

線と文字の描き方: `set_sheet_wire_style` は `symbol_weight_mm`(シンボル・ブロックの外形の線の太さ、0.18〜1.0mm、nullで既定の0.3mm)・`vivid_colors`(true で配線を純色で描く。色名は変わらない)・`hide_numbers`(true で線番 = 配線に書くネット名・番号を描かない。名前はデータに残る)も受け取る。5つの値は必ず一緒に設定する。`wire` エンティティは `dashed: true` と自分の `weight_mm`(0.18〜2.0mm)を持てる。`shape` の線・折れ線は `arrow_end: true` を持てる。`set_text_font` は `pen_mm`(0.1〜1.0mm)を受け取り、文字を大きさによらずその太さで描く。`spacing`(0.3〜3)は図面の全ての文字の字間の倍率、`stretch`(0.3〜3)は字の横の伸縮率(字形と送りの両方。細長い字で書く図面向け。図枠の文字には効かない)。`text` エンティティも自分の `stretch` を持てる(書体の値に掛かる)。`text` エンティティは自分の `spacing` を持て、書体の字間に掛かる(図枠テンプレートのセルの字間も同じく掛かる)。`place_symbol` の `attrs.block_leads: "none"`(`block_numbers: "inside"` のときだけ)でリード線を描かず、ピンをブロックの外形の辺の上に置く(配線が箱に直接つながる)。どのシンボルも `attrs.caption_layout`(JSON `{"ref": {"x","y","h","anchor","hide"}, "value": {...}}`)を持て、参照記号と型番を行ごとに置ける(配置点からのずれ mm、`y` はベースライン、`anchor` は start/middle/end、`hide` が true ならその行を描かない)。省いた行はこれまでの場所のまま。
Claude Codeはリポジトリの`.mcp.json`で自動接続。

### Link API(REST+SSE、外部ツール向け)
`127.0.0.1:9310/api/v1` — CLI・ブラウザでのUI検証・(将来)FreeCADアドオンが使う素のJSON REST:
- 読み: `/project`・`/symbols`(+`/symbols/user`)・`/frame-templates`・`/stroke-fonts/{id}`(同梱ストロークフォントの字形。例`kst32b`。知らないidは404)・`/panel/unplaced`・`/netlist`・`/verify`・`/search`・`/devices`・`/parts`・`/wire-parts`・`/terminals`・`/terminals/chart`・`/terminals/check`・`/plc/modules`
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
