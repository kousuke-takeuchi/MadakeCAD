# 自動化・API

[English (canonical)](10-automation-api.md)

UIでできることはすべてスクリプト化できる。3つの入口が同じCommandエンジンを共有するため、自動編集もundo履歴に乗り、UIへ即時反映される。

## 利用できる機能

### MCPサーバー(AIクライアント向け)
`127.0.0.1:9310/mcp`(Streamable HTTP、`MADAKE_MCP_PORT`で変更可)。ツール:
`open_project`・`save_project`(`.mdkproj`を開く・保存する。チャット履歴も一緒に扱う。開くと図面が置き換わり、エージェントの応答中は断る。アプリ内エージェントからは開けない)・`get_project`・`list_symbols`(同梱**と**ユーザーシンボル。同梱の補助記号は `connector_contact`・`power_rail`・`transistor_npn`・`transistor_pnp`・`photocoupler`・`opamp`・`tvs_diode`。`power_rail` は**値**がネット名になり、同じ値の印・同名のネットラベル・固定のネット名の印とシートを跨いでも1つのネットになる。値が空だと `erc.empty_net_name` の警告。`connector_contact` は配線の途中に置く素通しで、左右のピンは1つの節点、参照記号がコネクタ名・値が極の番号 (極の番号はピンの鍵でもある: 値が `5` の接点は、ネットリスト・`get_pins`・`connect_pins`・電線リストで `J7:5`、左のピンは `J7.5#1`・右は `J7.5#2`。値が空なら鍵は `1` のまま。同じコネクタの同じ極がプロジェクトのネットリストで別々のネットにあると(ラベルでシートを跨いでつながったネットは1つと数える)`erc.contact_pole_repeated` のエラーで、プロジェクトの検証が1件だけ出す。同じネットなら許す)。部品表は同じ参照記号を1つのコネクタと数え、SPICEでは0Ωの橋。半導体の記号はシミュレーションの模型を持たない。シンボルの定義は `net_from_value`・`through` も持て、ユーザーシンボルのファイルにも書ける)・`place_symbol`(置いた部品のピンを返す。属性`hide_reference: "true"`で参照記号を印字しない。記号自体は残る)・`draw_wire`・`get_pins`(配置済み部品のピン: 番号・名前・用紙座標・向き)・`connect_pins`(ピン名どうしを直角配線でつなぐ。例 `U1.SDA` から `J1.2`。`connections` で複数をundo一発の1編集で引く)・`place_mate`(プラグとソケットの組を1回で置く。`mates` で複数。任意の`pole`は両方の部品に入り、From-To電線リストが`参照記号:1`の代わりに`参照記号:極番号`と書く。`mates`で極番号なしに同じ参照記号を並べると並び順に1, 2, 3と振る)・`connect_cable`(多芯ケーブルを片側または両側を1本線で描いて引く。`cores`=芯の名前、`a`/`b`=端ごとに`pins`か`part`と`style: multi`(ピンから分岐点まで芯ごとに配線)/`single`(1本線を部品に着ける)。`multi`の端は`pins`/`part`の代わりに`labels`(芯ごとのネットラベル名。`breakout_at`必須)も書け、芯ごとに区間と反対向きへ10mmの配線を引いてその名前のネットラベルで終える。分岐の配線は線径・線色を持たない。芯はピン順に対応する。`cables`で複数をundo一発の1編集で引く)・`execute_commands`(Command全スキーマ公開)・`get_netlist`・`run_verification`・`get_tidy_metrics`(4つの数値と、数えた問題を1件ずつ種類・エンティティ・文字・場所で並べた`details`。最大50件)・`arrange_sheet`(シートの部品を列と行にそろえて並べ直す。`channel_mm`(既定8mm)の通り道をあけ、`max_gap_mm`(既定20mm)より広い空きは詰め、並び順を保ち、端が動いたかたまりの配線を引き直す(端が動かないかたまりの配線は元のまま)。1つの部品だけにつながる接地・電源の印とラベル、ピンの上に直接置いたラベルや印は、その部品と一緒に動く。`sheet_id`・任意の`options`(`channel_mm`・`max_gap_mm`・`align_mm`・`route`。範囲外はエラーで断り何も変えない)。経路が見つからないかたまりは、まず元の配線の形のまま、動いた列・行に合わせて伸び縮みさせ(端はピンに合わせる。かたまりのピン・ラベル・枝分かれ・ジャンクションが全て線の上に残り、他の線・ピン・ラベルに触れず、他の部品の外形を横切らないときだけ)、`kept_shape`に返す。それができないときは、そのかたまりのラベル名・同じネットの別のかたまりのラベル名・ネット名シンボルの名前・`ARR_<n>`のいずれかを名前にしたネットラベルでつなぎ、`labelled`に返す(接地・電源の印は消さない)。ケーブル・盤面配置・寸法・ポートの接点・他のエンティティやFreeCADが指す配線・長さの出所がまちまちな配線・名前の違うラベル(ネット名シンボルを含む)を持つかたまり・図枠に収まらないか表題欄(改訂欄を含む)に掛かる結果は断り、ネットが理由ならネット名を挙げる。1回の編集=undo一発。返り値: `moved`・`columns`・`rows`・`routed`・`labelled`(`[{net, label}]`)・`kept_shape`(ネット名)・`before`/`after`(`get_tidy_metrics`と同じ数値)・`wire_length_mm`・`size_mm`(前後))・`reroute_wires`(シートの配線を、接続を変えずに自動で引き直す。シンボル・文字・他のネットを避けた直角の配線で、曲がりと交差を少なくする。`sheet_id`・`nets`(ネット名。省略で全ネット)・任意の`options`(`grid_mm`は0.5〜10mm、費用・すき間・余白は0以上の有限の数。それ以外はエラーで断り何も変えない)。引き直しは配線で物理的につながったかたまりごとに行う。ケーブルの芯が使うかたまり・線色/線径/品番/線番や長さの出所(`length_source`)がまちまちなかたまり・他のエンティティ(ケーブルの芯・注記の引き出し線・寸法)やFreeCADとの対応付けが指すかたまり・ピンの引き出し線が他のネットに触れてしまうかたまり・経路が見つからないかたまりは元のまま残し、`unchanged`に理由を付けて返す。配線の属性は引き継ぎ、元の`length_m`の合計は新しい配線のうち一番長い1本だけが持つ。名前の無いネットの自動の名前(N001など)は、新しい配線のidが変わるため引き直しの後に入れ替わることがある。1回の編集なのでundo一発で元の配線がid付きで戻る。返り値は`rerouted`・`unchanged`(`[{net, reason}]`)・整えの数値の`before`/`after`・配線長の`wire_length_mm`の前後)・`draft_function_sheet`と`check_function_sheets`(元の図のファイルから機能シートを描く・下書きしたシートを元の図と照らす。下の「機能シートの下書き」を参照)・`simulate_op`・`search_parts`・`upsert_part`・`delete_part`・`import_kicad`・`import_dxf`(AutoCAD Electrical / EPLANのDXF。`wire_layers`任意)・`list_terminal_blocks`・`get_terminal_chart`・`check_terminal_block`・`list_templates`・`apply_template`・`list_macros`・`save_macro`・`apply_macro`・`duplicate_symbol`・`save_user_symbol`・`delete_user_symbol`・PLC I/O系・`get_sheet_image`(シートをPNG画像で返す。SVG出力と同じ見た目。`px_per_mm`とmmの`region`で拡大)・`export_svg`・`export_pdf`・`export_dxf`(AutoCAD 2000形式DXF)・`export_kicad`(`.kicad_sch`)・`export_report`(帳票7種のいずれかをCSVまたは図枠付きPDFへ)・`export_pdf_book`(表紙+全シート+選択帳票を1つのPDFへ)・`export_bom`・`export_wire_list`・`undo`・`redo`。
専用ツールが無い編集は`execute_commands`から実行する: `set_revisions`(改訂欄)、`set_sheet_annotation_text`(シートの注記文字高。`height_mm`は1.0〜3.5mm、nullで既定2.0mm。配線注記・線番・ネットタグの大きさが一緒に変わる)、`renumber_wires`(`mode: append | renumber`・`sheet_id`任意・`start`)と`set_wire_numbers`(線番)、ハーネス境界は`add_entity`で`harness`エンティティを追加、端子台のサドルジャンパは`update_entity`で端子台シンボルの`attrs["jumpers"] = "1-2,3-4"`を書き換える。図枠テンプレートは`set_frame_template`、図面の書体は`set_text_font`(`{"text_font": {"id": "kst32b", "latin": "half"}}`で全ての文字を同梱のストロークフォントで描く。`latin: "full"`で英数字を全角の字に、`null`でシステムの書体へ戻す。知らないidは拒否)、盤レイアウトは`add_sheet`の`kind: "panel"`+`panel_outline`/`din_rail`/`duct`/`footprint`/`panel_legend`エンティティ、バルーン採番は`renumber_panel_items`、FreeCAD連携(3D対応付けと計測した電線長)は`set_mech_link`/`remove_mech_link`/`set_wire_lengths`。
ポートのピン: `place_symbol` の `attrs.block_pins` でピンに `contacts`(`{"no":"U3","name":"USB-A メス","contacts":["VBUS","D-","D+","GND"]}`)を付けると、1本のピンが複数の導体を運ぶ(接点は1ピンに2〜32個、前後の空白は除き、ピンの中で重ならない)。`get_pins` は `contacts` を返し、`get_netlist` はそのネットを `<名前>.<接点>` に分け、ネットとポートのピンに `contact` を付ける。`connect_cable` は `pins: ["39.U3"]` と `style: single` で片側をポートのピン1本に着ける(芯は同じ名前の接点に、名前がどれも合わず数が同じなら並び順に対応する。名前の一部だけの一致と、ポートへの `style: multi` は断る)。
貫通ピン: `block_pins` のブロックでは、同じピン番号を向かい合う辺に1回ずつ置ける(左と右、または上と下。同じ極の両側に配線がある形)。両側のピン名は前後の空白を除いて同じにし、どちらにも `contacts`・`connector` は付けられない。同じ辺に2回・隣り合う辺・3回以上は `duplicate block pin number` で断る。2つの接続点は1つのノードで、ネットのピン一覧には1回だけ載る。`get_pins` は両方を返し、`U1.1` だけの指定は曖昧なので `U1.1#1` か `U1.1#2` で選ぶ。#1・#2 はブロックのピン表の定義順(左・右・上・下の順。回転・反転の前)で、画面で見た辺ではない。反転したブロックでは `U1.1#1` が画面の右に描かれる。`block_numbers: "inside"` のとき、2本が同じ線上に並ぶ貫通ピンの番号は、その線と交わる辺(左右の組なら上下、上下の組なら左右)にピンが無いときだけ、箱の中央に1つ描く(あるときは各ピンが自分の番号を描く)。ファイル形式23。
コネクタの絵: `block_pins` のピンには `connector`(`usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f` のどれか。`_f` はメス、`_m` はオス)も書け、`screw`・`jst_ph_f`・`jst_xh_f` には `connector_ref` も書ける(同じ ref の続くピンは1つの本体になり、`J3 PH`・`TB1` と書く)。知らない種類・空の ref・ほかの種類の ref・2つのまとまりに使う ref は断る。コネクタのあるピンの接続点は外形から5mmなので、位置は `get_pins` で読む。`get_pins` とネットリストは種類を返さない。
細かい間隔: `place_symbol` の `attrs.block_pitch: "2.5"`(既定は `"5"`)でブロックのピン間隔を2.5mm、大きさの刻みを2.5mm、最小を5×5mmにできる。ピンn本の辺は(n+1)×2.5mm以上が要る。ピンと外形の辺が2.5mmの格子に乗るよう、外形とピンの列は必要に応じて1.25mmずらすので、位置は `get_pins` で読む。`attrs.block_numbers: "inside"`(既定は `"outside"`)はピン番号を外形の内側に描き、ピン名は描かないので、信号名は配線のネットラベルで出す。ほかの値は断り、2.5mmのブロックにも番号を内側に描くブロックにもコネクタの絵(`connector`)を使えない。
番号なしのピン・すき間の行・見出し・文字の高さ: `block_pins` のピンは、`name` がブロックの中で一意なら `no` を省く(空にする)ことができる。箱の外には何も書かず、名前がどこでもピンの鍵になる(`get_pins` は `number` に名前を返し、`connect_pins` は `U1.EN` と書け、ネットリストと KiCad・DXF の書き出しもそれを使う)。番号を内側に描くブロックでは、番号の位置に名前を書く。番号がなく名前が同じ2本のピン、名前が別のピンの番号と同じピンは断り、番号なしのピンは貫通ピンにできない。辺の配列には `{"gap": n}`(n=1〜20。その辺のピンの位置を n 行空ける)と `{"caption": "文字"}`(1行を使い、箱の内側のその行に、ピン名と同じ大きさの太字で書く。左辺は左寄せ、右辺は右寄せ。上辺・下辺ではその列に書く)も並べられる。どちらもピンではなく接続点も無く、`get_pins` には出ない。1つのブロックの行はピン・見出し・すき間を合わせて400までである。`attrs.block_pin_text`(1.5〜5.0mm、既定2)でピン名・見出し・ピン番号の文字の高さを変えられ、名前が向かい側にぶつからないよう箱の最小幅も広がる。`attrs.block_number_align` を `"edge"` にすると、左右の辺の外に描く番号の終わりを外形の辺から0.5mm外に置き、文字は箱から離れる向きへリードの上を伸びるので、箱やピン名にかからない。無い(他の値の)ときは今までどおりリードの中央に置き、絵は変わらない。ピンの位置は変わらない。機能シートの下書きが作る箱は全てこれを持つ。ピンの位置はそのブロックの間隔のグリッドに乗るので、`get_pins` で読む。ファイル形式は29で、古いビルドはピンの無い箱を描かず、版番号でこのファイルを断る。

部分表示(1つの機器を複数のシートに分けて描く): 属性 `block_view` が `"1"` のブロックは部分表示である(`block::is_view`)。`update_entity` や `place_symbol` の attrs で手で付けられる。同じ参照記号の部分表示は、検査と部品表では1つの部品になる。`erc.duplicate_reference` は指摘しない(同じシートでもシートを跨いでも)。ただし部分表示と普通のシンボルが同じ参照記号なら今までどおり指摘する。部品表は1つと数え(数量1、参照記号は1回だけ)、型番はシートの並び順、次に描画の順で最初の「値が空でない」部分表示から取る。他のシートの場所の文字(`/2.B3`)は部分表示の脇に出さない。プロジェクトのネットリストは各ピンを共通の参照記号の下に並べるので、全部分表示のネットリストは機器全体のネットリストと同じになる。プロジェクトの `run_verification` は、同じ参照記号の2つの部分表示が同じピンの鍵(番号、番号なしのピンは名前)を出していると `erc.view_pin_repeated`(エラー。両方の部分表示を挙げる)、空でない型番が食い違うと `erc.view_part_mismatch`(警告)を出す。参照記号の無い部分表示は普通のブロックと同じに扱う。属性は `attrs` の文字列で、KiCad・DXF の書き出しと読み込みでも残り、ファイル形式の版は変わらない(29のまま)。

図面の見た目: `execute_commands` の `set_sheet_wire_style`(`sheet_id`、`weight_mm` は0.18〜1.0mm、nullで既定の0.35mm。`hide_notes` が true なら配線注記 = 線径・色名・長さを描かない。線の色はそのまま)。このコマンドの値は必ず一緒に設定され、省いた値は既定へ戻る(たとえば `hide_notes` を省いた呼び出しは注記を再び描く)。分岐点の丸は太い配線に合わせて大きくなり(半径は太さの0.9倍、0.6mmを下回らない)、ケーブルの芯線の分かれ目の線も同じ太さになる。`place_symbol` の `attrs.block_caption: "inside"`(既定は `"above"`)で、参照記号と型番をブロックの外形の上ではなく中央に書く(他のものは避けないので、中央の行が空いている箱で使う。中央の行にあるピン名や内側の番号とは重なる)。注記(`text` エンティティ)の `rotation` が 90・180・270 のとき、基準点を中心に時計回りへ回して描く。選択範囲と引き出し線も、回した文字に合う。

配線の行き先を示すタグ: `attrs.tag`(`left` / `right`)を持つネットラベルは旗の形で描かれる。`execute_commands` で `set_sheet_xref_style`(`sheet_id`, `style`)を送ると行き先の書き方を選べる。`zone`(既定)はゾーン住所(`/2.B3`)を旗の外へ小さく書く。`sheet_name` は同名ラベルを持つ他のシートの名前を、旗の中のネット名のあとに書く(シート順に「・」でつなぎ、3枚以上は先頭2枚のあとに「他」。ネット名は太字、行き先は文字高0.8倍で、旗は両方が入るまで広がる)。`sheet_name` では旗の外には何も書かず、他のシートに同名が無い旗には名前だけが出る。タグ付きラベルは `attrs.tag_dest`(自動の行き先に代わる自由な文字。プロジェクトのシートではない行き先用。どちらの書き方でも有効で、空白なら自動)と `attrs.tag_flow: "out"`(先端を本体の遠い端へ移し、旗が配線から離れる向きを指す。それ以外の値は普通の形)も持てる。タグの無いラベルは、どちらの書き方でもゾーン住所を外に書く。点を含むネット名(例 `J7.2`)は、点の前がポートのネットのラベルで、その名前の接点がある場合だけ接点の取り出しになり、それ以外は普通のネット名として扱われる。

線と文字の描き方: `set_sheet_wire_style` は `symbol_weight_mm`(シンボル・ブロックの外形の線の太さ、0.18〜1.0mm、nullで既定の0.3mm)・`vivid_colors`(true で配線を純色で描く。色名は変わらない)・`hide_numbers`(true で線番 = 配線に書くネット名・番号を描かない。名前はデータに残る)も受け取る。5つの値は必ず一緒に設定する。`wire` エンティティは `dashed: true` と自分の `weight_mm`(0.18〜2.0mm)を持てる。`shape` の線・折れ線は `arrow_end: true` を持てる。`set_text_font` は `pen_mm`(0.1〜1.0mm)を受け取り、文字を大きさによらずその太さで描く。`spacing`(0.3〜3)は図面の全ての文字の字間の倍率、`stretch`(0.3〜3)は字の横の伸縮率(字形と送りの両方。細長い字で書く図面向け。図枠の文字には効かない)。`text` エンティティも自分の `stretch` を持てる(書体の値に掛かる)。`text` エンティティは自分の `spacing` を持て、書体の字間に掛かる(図枠テンプレートのセルの字間も同じく掛かる)。`place_symbol` の `attrs.block_leads: "none"`(`block_numbers: "inside"` のときだけ)でリード線を描かず、ピンをブロックの外形の辺の上に置く(配線が箱に直接つながる)。どのシンボルも `attrs.caption_layout`(JSON `{"ref": {"x","y","h","anchor","hide"}, "value": {...}}`)を持て、参照記号と型番を行ごとに置ける(配置点からのずれ mm、`y` はベースライン、`anchor` は start/middle/end、`hide` が true ならその行を描かない)。省いた行はこれまでの場所のまま。
Claude Codeはリポジトリの`.mcp.json`で自動接続。

### Link API(REST+SSE、外部ツール向け)
`127.0.0.1:9310/api/v1` — CLI・ブラウザでのUI検証・(将来)FreeCADアドオンが使う素のJSON REST:
- 読み: `/project`・`/symbols`(+`/symbols/user`)・`/frame-templates`・`/stroke-fonts/{id}`(同梱ストロークフォントの字形。例`kst32b`。知らないidは404)・`/panel/unplaced`・`/netlist`・`/verify`・`/search`・`/devices`・`/parts`・`/wire-parts`・`/terminals`・`/terminals/chart`・`/terminals/check`・`/plc/modules`
- 書き: `/commands`(Command配列。`?batch=true`で配列全体を**1回の編集**=undo1回にする。途中で拒否されたら1件も適用しない)・`/undo`・`/redo`・`/save`・`/load`・`/import/kicad`・`/import/dxf`・`/simulate/op`(本文: `case_id` = 保存した計算ケース・`open_switches`・`overrides` = 参照記号 → {属性: 値} をその1回だけ置き換え・`shorts` = 接地へ短絡させるネット名。結果に定格超過 `findings` を重い順に、電線・部品ごとの電流・電圧・限度・負荷率 `loads` を足す)・`/route/reroute`(`reroute_wires`と同じ本体・同じ返り値)・`/route/arrange`(`arrange_sheet`と同じ本体・同じ返り値)・`/symbols/user/{save,delete}`・`/symbols/duplicate`・`/macros/{build,save,apply,apply-inline}`・`/templates/apply`・`/variants/{start,finish}`・`/export/{svg,pdf,dxf,kicad,pdf-book,report,bom,wire-list}`・部品CRUD
- ライブ更新: `GET /events`(SSEパッチストリーム)
- ローカルオリジンガード(外部Webオリジンからのアクセスを拒否)

### madake CLI
Link APIの薄いターミナルクライアント: `status`・`project`・`netlist`・`verify`・`sim`・`reroute`(`--sheet <ID>`・繰り返せる`--net <NAME>`。`--json`で返事をそのまま出す)・`arrange`(`--sheet <ID>`・`--channel <MM>`。20mmを超える幅なら`max_gap_mm`もその幅に上げる。`--json`で返事をそのまま出す)・`draft <request.json>`(機能シートの下書き。下の節を参照。`--json`で返事をそのまま出す)・`draft-check <source.mdkproj>`(繰り返せる`--ignore <REF>`。シートが元の図と合わないか札の旗が図枠をはみ出すとき終了コード0以外)・`parts`・`terminals`(端子台一覧)・`export`・`save`/`open`(`.kicad_sch`・`.dxf`対応。DXFは`--wire-layer`)・`renumber`(線番採番: `--sheet`・`--mode append|renumber`・`--start`)・`exec`(JSONのCommand配列)・`undo`/`redo`。`--json`で機械可読出力。

`madake export <種別> <path>`は回路図と帳票の両方を扱う:

| 種別 | オプション |
|---|---|
| `svg`・`pdf`・`dxf`・`kicad` | `--sheet <ID>`(省略時は先頭シート)。`dxf`=ACADE/EPLAN向けAutoCAD 2000形式、`kicad`=`.kicad_sch` |
| `pdf-book` | `--reports wire-list,terminal-chart,terminal-diagram,bom,xref,panel-placement`(並べた順に付く)・`--no-cover` |
| `bom`・`wire-list`・`terminal-chart`・`terminal-diagram`・`xref-table`・`plc-io`・`panel-placement` | `--format csv\|pdf`(省略時は出力先の拡張子から判定。`.pdf`なら図枠付き図面シート)・端子台の2帳票は`--terminal <参照記号\|ID>` |

帳票は常に図面全体が対象で、絞り込めるのは`--terminal TB1`だけ(同じ参照記号が複数シートにある場合は`--sheet <ID>`を併用)。候補は`madake terminals`で調べる。

## 機能シートの下書き

大きな配線図は、機能ごとに1枚へ分けると紙で読みやすい。下書きは、全ての配線を持つ**元の図**のファイルから、開いているプロジェクトへ機能シートを1枚作る。**何を**載せるか(どの機器を、左から何列目に置くか)は呼び出し側が決め、**どう**描くか(箱の中身・ピンの辺と順・配線・札・電源と接地の印・間隔)は MadakeCAD が決める。元のファイルは読むだけで書き換えず、開いているプロジェクトであってはならない。

入口: MCPツール`draft_function_sheet`・`check_function_sheets`、Link API `POST /api/v1/draft/function-sheet`・`POST /api/v1/draft/check`、CLI `madake draft <request.json>`・`madake draft-check <source.mdkproj> [--ignore REF]...`。下書きは1回の編集で、undo一発でシートごと消える。

依頼(Link APIの本文とMCPの入力。CLIは同じ形のJSONファイルを読み、相対の`source_path`はそのファイルの場所から探す):

```json
{
  "source_path": "/path/to/source.mdkproj",
  "name": "2 Control",
  "columns": [["U1", "U2"], ["TB1"]],
  "pins": {"U1": ["1", "2", "5"]},
  "power_nets": ["P24X"],
  "ground_nets": ["GND"],
  "destinations": {"J7.2": "cabinet B"},
  "paper": "A3",
  "pitch_mm": 5.0,
  "planned_sheets": ["1 Power", "2 Control", "3 Motors"]
}
```

- `columns`(必須): 1〜8列。各列は機器(元の図で参照記号が同じ全てのシンボル)の参照記号の並びで、上から下。参照記号は1回だけ。
- `pins`: 機器ごとに載せるピンの鍵を限る。1つの機器を何枚かのシートに分けるときに使う。書かない機器は、使用中のピン(元のネットが2ピン以上)を全て載せる。
- `power_nets`・`ground_nets`: 元の図のネット名。これらのピンは配線しない。同じ機器の同じ辺で同じネットのピンは隣の行に並べ、足の先を縦の棒1本でつなぎ、組に印を1つ付ける(電源は棒の上の端からピン間隔ぶん上、接地は下の端からピン間隔ぶん下へ伸ばして箱から離れる向きへ曲げた先。印の文字は棒・足・番号にかからない)。`marked`は印の数で、ピンの数ではない。`ground_nets`を省くと、ネットの印を持つ全てのネットになる。
- `destinations`: 図の外へ続くネットの、ネット名と自由な文字(札の行き先の文字になる)。
- `planned_sheets`(省略可): これから下書きする全てのシートの名前を渡すと、札がそれらの名前の分の場所を空けておく(pass the names of all sheets you will draft so that tags keep room for them)。順不同で、この依頼自身の名前やすでにあるシート名を含んでもよい。空白だけの名前や同じ名前の繰り返しは断る。札は、開いているプロジェクトのシート名とこの一覧の和集合からこのシート自身の名前を除いた集合から、旗が将来持ちうる最も広い文字の幅を確保する(1つならその名前、2つ以上なら広い2つを旗と同じつなぎ方でつないだもの、3つ以上ならさらに「他」。10文字は下限)。渡さないと、同じネットを持つシートを後から下書きしたとき、先のシートの旗が確保した場所を越えて図枠の外へ伸びることがある。`draft-check` はそのような札を `tags_outside` に挙げる。
- ブロックの機器は`block_view`=`1`のブロックになる(1つの機器が複数シートにあっても、参照記号の重複検査・部品表・ネットリストでは1つの部品)。ライブラリのシンボル(抵抗・リレーのコイルなど)はそのまま置き、載せられるのは1枚のシートだけ。
- 足の長さは機器の辺ごとに、max(5mm, 印字される番号のいちばん広い幅+2.5mm)を2.5mmの格子へ切り上げた値。同じ機器の同じ辺の同じネットのピンは1つの組(棒。3本が会う所に接点の丸)で、組が配線の端1つ(相手に近い棒の端、相手が上下にあれば真ん中)。機器の中だけのジャンパ(全ピンが1つの組)は棒だけでつなぐ。ネットの全ピンがこのシートにあれば配線でつなぐ(直角・シンボルと文字を避ける)。他のシートにもピンがあるネットは、載せたピンを配線し、列の順で最初のピンに札を1つ付ける。載せたピンが1本なら札はそのピンの足(棒)の先、2本以上なら足の先からさらに同じ行を5mm先に下げ、旗は箱から離れる側へ向け、相手への配線は旗の手前で分かれるので、旗の上や脇を通る配線は無い。札と印(文字を含む)は配線の障害物で、札は後から中に書かれる他のシートの名前の幅(行き先・開いているプロジェクトの最も長いシート名・10文字のうち広いもの)を空けておく。シンボル・ピン・足の先・札・印は2.5mmの格子の上、引いた配線の曲がりは経路の1.25mmの格子の上でもよく、配線は図枠の内側から出ず表題欄に入らない。札の名前はネットのラベル名で、無ければ元のネット全体の最初のピンの`<参照記号>.<ピン>`なので、どのシートでも同じ名前になる。新しいシートは`xref_style`=`sheet_name`で、札の中に他のシートの名前が出る。列は描画範囲の左端から詰めて並べ、列k+1は列kの右の張り出し(箱・足・棒・札・印)+通り道(max(20mm, 10mm+そこを横切るネット1本につき2.5mm)を2.5mm格子へ切り上げ)から始まる。札は、開いているプロジェクトで同じネットをすでに持つシートの名前(2枚+「他」。最低でも最も長いシート名か10文字)の幅を空け、列が描画範囲を越える依頼は列の番号と超えた量(mm)を挙げて断る。シートの上の相手が全て同じ列にあるピンは右の辺(最後の列では左の辺)に置き、相手が混ざるときは同じ列の相手を右(最後の列では左)に数える。そのネットはその列の右の通り道で数える。箱は外の番号を辺の外で終わらせる(`block_number_align`=`edge`)。札の旗を横切る配線も触れる配線も無い: 旗(確保した幅を含む)は通れない場所で、旗に触れずに引けないネットは、各組の棒の上の端(足の先)に行き先なしの札を付けてつなぎ(この札も他のネットの障害物)、`labelled`に載せる。

返り値: `sheet_id`・`units`・`pins`・`wired`(ネット名)・`tagged`(`[{net, pins}]`)・`marked`(`{power, ground}`)・`labelled`・`skipped_pins`(`[{reference, pin, reason}]`)・`metrics`(新しいシートの`get_tidy_metrics`と同じ数値)。

断って何も変えないもの: 元のファイルが読めない・空白の参照記号・元の図に無い参照記号や2回書いた参照記号・元の図で複数のシンボルで描かれたライブラリのシンボルの機器(リレーのコイルと接点など)・開いているプロジェクトのシートに既にあるライブラリのシンボルの機器・機器に無いピンの鍵・元の図に無いネット名(`power_nets`・`ground_nets`・`destinations`)や電源と接地の両方にあるネット・使用中のシート名・他のシートのビューが既に出しているピン・載せるピンが1つも無い・ケーブルの芯や接点付きポートを通るネット・用紙に収まらない列(列と超過量mmをメッセージに入れる)・計画中に開いた図面が変わり続けた(3回まで計画し直す)。

検算: `check_function_sheets { source_path, ignore_references? }`は元の図と開いているプロジェクトを比べ、何も変えない。両側とも、ネットごとの`(参照記号, ピン)`の集まりにして比べる(2ピン未満のネット・ネットの印・素通しのシンボルは除き、ネット名は比べない)。`ignore_references`の機器は両側から外す。

```json
{
  "ok": false,
  "source_nets": 12,
  "matched": 10,
  "missing_pins": [{"reference": "U3", "pin": "4", "source_net": "P24X"}],
  "extra_pins": [],
  "split": [{"source_net": "SIG_A", "parts": [["U1:3"], ["U2:3", "U4:1"]]}],
  "merged": [],
  "tags_outside": [],
  "layout_ok": true
}
```

`missing_pins`は元の図にあってどのシートにも無いピン、`extra_pins`はシートにあって元の図に無いピン、`split`は元の1つのネットのピンが開いたプロジェクトで1つのネットにならないもの(シートごとに札の名前が違うとここに出る)、`merged`は元の別々のネットのピンが開いたプロジェクトで1つのネットになっているもの。`tags_outside`(`[{sheet, net, excess_mm}]`。シート順、次にネット名の自然順)は、SVG書き出しと同じ行き先の文字で描いた旗が、そのシートの図枠の内側の外や表題欄の中へはみ出している札で、`excess_mm`は最大のはみ出し量(mm、小数1桁)。`layout_ok`はこの一覧が空のとき true。`ok`は今までどおり「つながりが同じ」だけを表し、`layout_ok`の影響を受けない。`madake draft-check`は同じ内容を表示し、`ok`か`layout_ok`が false のとき終了コード0以外で終わる(`--json`で返事をそのまま出す)。

## 計画中

- M5アドオンが必要とするFreeCAD向けエンドポイント(例: 3Dモデル有り部品のフィルタ)
- OSS公開に向けたAPIの安定性・バージョニング方針(M6)
