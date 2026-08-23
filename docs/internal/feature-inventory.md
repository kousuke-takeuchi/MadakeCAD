# MadakeCAD 機能インベントリ

作成: 2026-08-21(フェーズ0〜3完了時点)。更新: 2026-08-23(**フェーズ5 完了**: 参考図面の再現パリティ — ブロックシンボル+配線注記+ネットラベルのタグ形状+注記の装飾+電線品番の引き当て)。その前の更新は M4フェーズ4 完了(= M4完了: 図枠テンプレート+シンボルエディタ+2D盤レイアウト)。全機能の棚卸しと、未実装バックログの一覧。
specは`docs/superpowers/specs/2026-08-20-madakecad-design.md`、実装経緯は`docs/superpowers/plans/`の各プラン。

## 1. コア・アーキテクチャ(madake-core)

| 機能 | 状態 | 備考 |
|---|---|---|
| Commandエンジン | ✅ | 全編集がCommand経由。逆コマンドによるundo/redo、Patch(revision付き)のbroadcast |
| ドキュメントモデル | ✅ | Project(図枠テンプレートid・PLC割付表) / Sheet(種別=回路図\|盤レイアウト・JIS図枠・表題欄・改訂欄・ゾーン) / Entity 11種(Symbol・Wire・Junction・NetLabel・Text・Harness+盤の PanelOutline・DinRail・Duct・Footprint・PanelLegend) |
| 保存形式 `.mdkproj` | ✅ | 整形JSON(git差分可読)、`format_version` **6**(履歴と旧ファイルの扱いは`data-model.md`)。チャット履歴は`<名前>.chat.json`を併存 |
| シンボルライブラリ | ✅ | JIS C 0617 / IEC 60617系の静的47種(電源・変換・接地7/保護6/操作スイッチ8/リレー・電磁接触器5/半導体3/受動6/負荷・報知6/計測3/接続3)+動的シンボル`connector_{n}p`/`terminal_block_{n}p`(n=1..50、5mmピッチ中央揃え)・`plc_di_{n}p`/`plc_do_{n}p`(n=1..64、点ピッチ5mm、カテゴリ`plc`)。全記号がピンの接続方向(up/down/left/right)・属性スロット(TAG/PART/DESC/RATING)・検索キーワード(en+ja)を持つ。多極機器(遮断器・断路器・電磁接触器)は端子1/2・3/4・5/6で極ごとに導通。端子台は左右貫通端子。**ブロックシンボル**`block`はインスタンスの属性(`block_pins`=辺ごとの`{no,name}`・任意の`block_size`)から定義を毎回生成する(名前付きピンの汎用機器。ピン名は外形の内側・ピン番号は外側、5mmピッチ)。**ユーザーシンボル**(`~/MadakeCAD/symbols/*.json`、分類`user`固定・同梱とのid衝突は拒否・保存時検証)が同じ解決経路に合流する(同梱→動的→ユーザー) |
| 座標系 | ✅ | mm・左上原点・Y下向き。2.5mmグリッド、回転0/90/180/270 |
| ヘッドレス実行 | ✅ | コアはUI非依存(全ロジックがmadake-coreに集中) |

## 2. エディタUI(Vue 3 + Canvas2D)

| 機能 | 状態 | 備考 |
|---|---|---|
| リボン(回路図タブ) | ✅ | 配線 / 部品を挿入 / 回路図を編集 / 検証・レポート(部品表・電線リスト・シミュレーション・SVG/PDF出力) |
| リボン(表示タブ) | ✅ | 表示クラス11種(配線/シンボル/参照記号/ネットラベル/線番/配線注記/ハーネス/注記/盤/図枠/グリッド)のトグル(レイヤ。画面のみ。ただし**配線注記は出力に常に含まれる**) |
| リボン(レポートタブ) | ✅ | 帳票7種(From-To/端子台チャート/端子接続図/BOM/XRef表/PLC I/O/盤内部品配置表)+端子台(端子台エディタ)+PLC I/O+出力(PDF一括出力)。生成ダイアログで出力形式(CSV/図面シートPDF)と対象を選ぶ |
| リボン(パネルタブ) | ✅ | 盤(盤枠・DINレール・ダクトの作図)/フットプリント(未配置部品・型番から挿入*・ルックアップ編集*・銘板*)/配置(バルーン採番・整列*・等間隔*・レール整列*・寸法*・間隔チェック)/レポート(配置表・盤内配線図*・凡例表)。盤シートを開くと自動で切り替わる。*=未実装(淡く出して押すと理由をログ) |
| リボン(管理タブ) | ✅ | テンプレート(図枠テンプレート・表題欄の編集*・回路テンプレート)/ライブラリ(シンボルエディタ・部品DB*・マクロ*・ユーザーライブラリ) |
| リボン(プロジェクトタブ) | 🔶 | 作図の開始グループに図枠テンプレートを配置。他はプレースホルダ |
| リボン(他タブ) | ⬜ | ホーム/読み込み・書き出しはプレースホルダ |
| シンボルエディタ | ✅ | 属性パレット(TAG必須/PART・DESC・RATING任意)+2.5mmグリッドのキャンバス(線・矩形・円・弧・文字・属性・接続点。選択/移動/削除)+接続点表(番号・方向・名前)+回転バリアント0/90/180/270の実プレビュー+保存前チェック(コアと同じ判断)。「複製して新規」はポップオーバー、未保存の確認はフッタで行う(モーダルを重ねない) |
| 盤レイアウトUI | ✅ | シート種別の選択(図面タブ行の+がポップオーバー)、盤ツール(矩形/水平ドラッグ→`add_entity`1回)、未配置部品パネル(参照記号・型番・寸法+出どころバッジ)、クリック配置(ゴーストは部品の中心・`R`で0/90・レール取付線へスナップ)、フットプリントのプロパティ(参照記号・型番・幅高さ・回転・アイテム番号・所属レール) |
| 図枠テンプレート選択ダイアログ | ✅ | 一覧(同梱/ユーザー・使用中バッジ)+表示中シートでのライブプレビュー(値が空になる欄にだけ`{バインド}`を青で重ねる)+バインド一覧+ゾーン設定(読み取り専用)+適用範囲(このプロジェクト=`set_frame_template`1回 / グローバル既定=`AppSettings.default_frame_template`) |
| キャンバス操作 | ✅ | パン(中ボタン/Space)・ホイールズーム・グリッド・スナップ・直交拘束・ピンスナップ(菱形マーカー) |
| 選択・編集 | ✅ | クリック選択・Shift追加・矩形選択・ドラッグ移動(Command化)・削除・⌘Z/⇧⌘Z |
| 配線ツール | ✅ | 直交ポリライン、ダブルクリック/Escで確定。線色・sq既定値 |
| 配置ツール | ✅ | ゴーストプレビュー、Rで回転、参照記号の自動採番(接頭辞+連番) |
| 部品挿入ダイアログ | ✅ | 「部品」/「マクロ」タブ。部品=検索(名称/記号)・カテゴリ別グリッド・動的シンボルの極数バー(1..50ステッパー)・部品DB検索セクション(選択で型番・定格・接点構成つき配置)。マクロ=カテゴリツリー+バリアント数バッジ付きタイル+プレビュー+値セットのドロップダウン(値セットを持つマクロにだけ出る) |
| 回路マクロUI | ✅ | 保存ダイアログ(名前・カテゴリ・基準点表示・バリアントAチップ・選択範囲プレビュー)、配置ツール(ゴースト・`R`回転・`Tab`バリアント・Esc・連続配置)、`⌘C`/`⌘V`(無名マクロ) |
| 検索バー+結果パネル | ✅ | ⌘Fの浮きバー(フィルタチップ=参照記号/型番/ネット/テキスト・件数)+下部ドックの結果パネル(参照/種別/所在/補足)。Enter=次へ/Shift+Enter=前へ(端で回り込み)/Escで閉じる |
| デバイスナビゲータ(左3タブ目) | ✅ | 参照記号→機能(コイル/接点(端子対)/端子/本体)のツリー+所在。行クリックでreveal、右クリックでジャンプ/削除(`delete_entities`経由なので⌘Zで戻る)。未配置機能のドラッグ配置はフェーズ3 |
| 参照サーフィン(Surfer) | ✅ | キャンバスでAlt(Option)+クリック→所在一覧ポップアップ(シンボル=同一デバイスの全機能/ネットラベル=同名ラベル/ワイヤ=同一線番)。↑↓で巡回・Enterで確定・Escで閉じる |
| プロパティパネル | ✅ | シンボルの参照記号・型番・機能ラベル・ピン表(ブロック)、ワイヤの線番(ネット単位)・線色/線径/長さ/品番(自動引き当てはプレースホルダ表示)、注記の基準位置/色トークン/太字、ハーネスの名前・備考(含む電線数)、ネットラベルのタグ形状と相手先リンク(クリックでシート切替+ズーム) |
| reveal(ジャンプ)の一本化 | ✅ | 検証パネル・検索・デバイスナビゲータ・Surferの4経路が`composables/reveal.ts`(シート切替+選択+ズーム) |
| プロジェクトマネージャ(左) | ✅ | ツリー+詳細。エージェントタブと切替 |
| 図面タブ | ✅ | シート切替・追加(add_sheet) |
| ステータスバー | ✅ | 座標、スナップ/直交/グリッドトグル、直近ログ、ズーム、MCPポート表示 |
| 検証結果パネル | ✅ | severityバッジ、行クリックで該当エンティティ選択+ズーム、再検証 |
| シミュレーション結果パネル | ✅ | ネット電圧(min/max)・部品電流/電力、開路チップ、再実行 |
| 設定画面 | 🔶 | AI設定(プロバイダ・一般・チャット・MCPタブ)。プロバイダは**6経路すべて実動**(Claude Code CLI / Anthropic API / GitHub Copilot CLI / OpenAI互換 / Ollamaプリセット / Gemini)。経路ごとの詳細欄(パス・URL・モデル・伏せ字キー+保存/削除)と接続テストつき。残りはテーマ(ダーク)等 |
| テーマ | ⬜ | ライトのみ(CAD調トークン)。ダーク切替は設定UIにあるが未実装 |

✅=実装済み / 🔶=部分実装 / ⬜=未実装

## 3. 図枠・出力

| 機能 | 状態 | 備考 |
|---|---|---|
| JIS図枠描画 | ✅ | 枠線・ゾーン番号(横=数字/縦=英字)・表題欄(図番/品名/尺度/日付/設計〜承認/社名)。画面とSVG/PDFで同一 |
| 図枠テンプレート(プロットフレーム) | ✅ | `frame_template.rs`。JSON定義(余白・ゾーン文字・表題欄の罫線とセル・改訂欄)+バインド解決(`{sheet.no}`/`{rev.latest}`/`{title_block.*}`ほか。知らないバインドは空文字)。同梱`jis-a3`(データ駆動化前の出力とゴールデン一致)・`jis-a4-portrait`、ユーザーは`~/MadakeCAD/templates/frames/`。差し替えは`set_frame_template`1回で全シート(キャンバス=`canvas/frameTemplate.ts`とSVG/PDFが同一計算)。**用紙・ゾーン分割はシートが正**。ロゴ画像の実描画は未 |
| 改訂欄 | ✅ | ISO 7200様式(表題欄直上・古い行が下・最大6行・列=記号/日付/内容/承認)。キャンバス/SVG/PDFで同一、表題欄Revは最新行に連動。編集ダイアログ(`set_revisions`コマンド1回=undo1回) |
| SVG出力 | ✅ | 印刷品質、mm 1:1、XMLエスケープ |
| PDF出力 | ✅ | svg2pdfでベクタ変換、日本語フォント埋め込み(macOS=Hiragino。Win/Linuxのフォント割当は未調整) |
| BOM(部品表) | ✅ | **アイテム番号**+参照記号+型番+シンボル+数量。アイテム番号は盤のバルーンと連動(未配置は空欄、複数まとまる行は「1, 3」)。**旧4列CSVとは非互換**。CSV / 図枠付き図面シート |
| From-Toワイヤリスト | ✅ | シート・From・To・線番・線色・sq・長さ・電線品番・ハーネス。From/Toはピン(`参照記号:ピン`)/ネットラベル/空。CSV / 図面シート |
| 端子台チャート | ✅ | 端子台1台=1表。端子番号順に内部側/外部側・線番・電線・ハーネス・ジャンパ・予備端子。CSV / 図面シート(`--terminal`で1台に絞れる) |
| 端子接続図 | ✅ | EPLAN端子図様式(外部=左/内部=右・ハーネスブラケット・予備・ジャンパ)。1端子台=1ページ(端子15個で分割)。PDFのみ |
| クロスリファレンス表 | ✅ | プロジェクト全体ネットの所在一覧。CSV / 図面シート。コイル⇔接点対応は図面上の接点マップ(§8)で実装済み |
| 盤内部品配置表 | ✅ | アイテム番号/参照記号/型番/X/Y/レールの6列。X/Yは描かれる外形の左上角、レール番号は盤の上から順。CSV / 図面シート / PDF一括 |
| PLC I/Oレポート | ✅ | 割り付けた1点=1行(モジュール・アドレス・信号名・接続先・線番・コメントの6列)。接続先・線番は出力のたびに図面から読み直す(モジュールが描かれている全シートを合わせて見る)。CSV / 図面シート |
| 帳票の図面シート化 | ✅ | `report_sheet.rs`の汎用テーブルレンダラ(A4横・25行/ページ・列幅は相対比・長文セルは省略・自動ページ分割)。回路図と同じ図枠・表題欄 |
| PDF一括出力(図面一式) | ✅ | 表紙(プロジェクト名・シート一覧・最新改訂)→回路図全シート→選択帳票を1PDFへ(`pdf::export_project_pdf`) |
| 印刷(OSダイアログ) | ⬜ | PDF経由で代替 |

## 4. ネットリスト・検証・シミュレーション

| 機能 | 状態 | 備考 |
|---|---|---|
| ネットリスト抽出 | ✅ | 座標一致+Junction+同名NetLabel統合。端子台の同番号ピン内部短絡。決定的なネット名(ラベル名 or N001〜) |
| ERC | ✅ | 未接続ピン / 空参照 / 参照記号重複(リレー除外) / 宙ぶらりんワイヤ(ジャンクション忘れ検出) / ネットラベル競合 / 接点構成の超過`contact_overflow`(Error) / 親コイル無しの接点`orphan_contact`(Error) / 接点無しコイル`coil_without_contact`(Warning) / 読めない接点構成`invalid_contact_config`(Warning) |
| 電気検証 | ✅ | 電源到達性 / 線径許容電流(sq表) / 電圧降下(電源電圧の3%) / ヒューズ定格。**ngspiceのDC動作点解析がバックエンド**、未導入時はグラフ近似+Info明示 |
| SPICEネットリスト生成 | ✅ | 接続点ノード・ワイヤ抵抗(ρL/A、途中接続で分割按分)・導通橋・負荷等価抵抗・ラベル橋 |
| ngspiceランナー | ✅ | OS非依存(env→PATH→OS既定パス探索、サブプロセス実行) |
| DCシミュレーション | ✅ | ネット電圧(min/max)・部品電流/電力、スイッチ/接点の開路what-if |
| 過渡解析(.tran) | ⬜ | 保留(部品DBのSPICEモデル実運用後。spec §3.6) |
| ヒューズ協調(選択性) | ⬜ | 保留(部品DB連携後) |

## 5. 部品DB(SQLite、グローバル共有)

| 機能 | 状態 | 備考 |
|---|---|---|
| 部品マスタ | ✅ | 型番・メーカ・名称・カテゴリ・既定シンボル・定格電圧/電流・接点構成(`contact_config`、例"2NO+2NC")・PLCモジュール定義(`plc_module`、JSON)・購入先URL・データシートURL・価格・備考。外形寸法(`footprint_w`/`footprint_h`、mm)・スキーマ**v6**(バージョン管理+マイグレーション。v3→v4でPLCモジュール列、v4→v5は列を変えずPLCサンプルの追補、v5→v6で寸法2列+同梱サンプルへの寸法追補) |
| 電線品番マスタ | ✅ | 線色+sq→品番(DBがマスタ。図面側`Project::wire_parts`はスナップショット) |
| 予約列 | ✅ | `model_3d`(フェーズM)・`mounting`・`spice_model`(過渡解析用) |
| サンプルデータ | ✅ | 初回作成時にダミー型番5件+PLCモジュール4件(三菱DI/DO16点・Siemens DI8点・Allen-Bradley DI16点)+電線4件。v5より前のDBを開くとPLCサンプルのうち**不足しているものだけ**追補する(同じ型番の既存部品は上書きしない) |
| 検索→配置連携 | ✅ | 部品挿入ダイアログから配置すると型番(value)と定格(attrs.current_a)・接点構成(attrs.contact_config)が図面に設定され、検証・シミュレーション・接点マップと連動(図面はDBのスナップショット) |
| 管理UI(一覧・編集画面) | ⬜ | 登録・編集はCLI/MCP/API経由(意図した割り切り) |
| 盤レイアウトとの連動 | ✅ | 未配置部品の外形寸法を型番で引く(両方が正のときだけ採用、無ければ20×40mm)。一覧に出どころのバッジ(部品DBの外形/寸法から自動/未登録) |
| BOMとの連動 | ⬜ | BOMに型番は出るが、DBのメーカ・価格・購入先の引き当ては未 |

## 6. インポート/エクスポート

| 機能 | 状態 | 備考 |
|---|---|---|
| KiCadインポート(.kicad_sch) | ✅ | 自前S式パーサ。用紙・表題欄・配線・ジャンクション・ラベル・テキスト・主要シンボル(マッピング表+Conn/Screw_Terminalのピン数解釈)・電源シンボル→ネットラベル化。未対応はスキップ報告(ImportReport) |
| KiCadインポートの制限 | ⚠ | ピン形状の違いで接続が崩れ得る(ERCで洗い出す運用)。階層シート・バス未対応 |
| KiCadエクスポート | ⬜ | 未実装(実装すると`kicad-cli sch erc`クロスチェックが可能に) |

## 7. AI・自動化・外部連携

| 機能 | 状態 | 備考 |
|---|---|---|
| 内蔵MCPサーバー | ✅ | 127.0.0.1:9310/mcp。ツール35種: get_project / list_symbols / place_symbol / draw_wire / execute_commands(set_revisions・renumber_wires・set_wire_numbers・harness追加・ジャンパ(update_entityのattrs)もここから) / get_netlist / run_verification / get_tidy_metrics / simulate_op / search_parts / upsert_part / delete_part / import_kicad / list_terminal_blocks / get_terminal_chart / check_terminal_block / list_templates / apply_template / list_macros / save_macro / apply_macro / duplicate_symbol / save_user_symbol / delete_user_symbol / export_svg・pdf・report・pdf_book・bom・wire_list / undo / redo。**検索・デバイスツリーはMCP未露出**(IPC+Link APIのみ。AIはget_projectで足りるため意図的) |
| Link API (/api/v1) | ✅ | REST+SSEパッチ。project / symbols(+/user・/user/save・/user/delete・/duplicate) / frame-templates / panel/unplaced / netlist / verify / tidy-metrics / search / devices / simulate/op / commands / undo / redo / save / load / import/kicad / terminals(+/chart・/check) / templates(+/apply) / macros(+/build・/save・/apply・/apply-inline) / export/*(svg・pdf・pdf-book・report・bom・wire-list) / parts / wire-parts / agent/*(send・cancel・conversations・undo-turn・detect・provider・api-key(PUT/DELETE)・test-connection・events) / settings / events |
| madake CLI | ✅ | status / project / netlist / verify / sim / parts / terminals / export(svg・pdf・pdf-book+帳票6種を`--format csv\|pdf`・`--terminal`付きで) / save / open(.kicad_sch対応) / renumber / exec / undo / redo。マクロ・検索のサブコマンドは未(Link APIを`exec`/curlで直接叩ける) |
| AIチャット(A1) | ✅ | 左ドック+浮きカード、Claude Code CLIバックエンド(Pro/Max OAuth再利用)、ツールチップ表示、ターン単位undo、編集オーバーレイ(シアンパルス)、会話履歴のプロジェクト保存 |
| A1の持ち越し負債(M3フェーズ1で解消) | ✅ | ターン安定ID(`turn_id`。chat.json format_version 2へ移行)・編集origin(`Engine::execute_as` / `revert_range`でagent編集だけを逆適用。衝突は`RevertConflict`)・キャンセルseq(`turn_seq`で遅延イベントを破棄) |
| 規格知識+検証ループ(M3フェーズ1) | ✅ | 同梱`resources/knowledge/standards.md`を毎ターン注入(設定`knowledge_path`で追記可、後勝ち)。図面コンテキストに検証サマリ。プロンプトで「編集後は`run_verification`→修正→再検証(最大3回)→件数報告」を必須化 |
| ナレッジ回答・AIレビュー・部品選定(M3フェーズ1) | ✅ | `docs/`目次+`--add-dir`+読み取り専用ツール(Read/Glob/Grep)で出典付き回答(未対応機能はroadmapで回答)。レビューは決定的検証+5観点チェックリストを重要度表に。検証計画は手順表。部品選定は`search_parts`の比較表(最大10件) |
| 開始テンプレート(M3フェーズ1) | ✅ | 同梱3種(24V制御基本・モータ起動回路・非常停止回路。適用後ERCエラー/警告0)。`Engine::execute_batch`で履歴1エントリ=undo一発。IPC/Link API/MCPツール+リボン導線。`~/MadakeCAD/templates/*.json`でユーザーテンプレート |
| 自動反復=整えループ(M3フェーズ2) | ✅ | `madake-core::tidy`の決定的メトリクス(交差数・ラベル重なり・シンボル重なり・グリッド外)をMCP`get_tidy_metrics`/Link API`/tidy-metrics`で露出。チャット入力欄の杖ボタン→ポップアップ(配置整理/配線整理/ラベル整頓)が「測る→直す→測り直す(改善が止まる or 最大3回)」の定型プロンプトを**1ターン**として送る=undo一発。`knowledge.rs`のWORKFLOW_RULESにも同じループを記載(自然文依頼でも回る)。バリアント数(2〜4案)はフェーズ3 |
| 並列エージェント(M3フェーズ2) | ✅ | 会話ごとに`tokio::spawn`で同時実行(Busy判定は会話単位)。フロントの送信ガードは**開いている会話**だけに効く(`chat.streaming` / `anyStreaming` / `runningIds`)。会話色=`theme.agentPalette`(#29D3E6 / #FFB454 / #B48CFF / #FF6FD8)を開始順に割当、編集オーバーレイ・会話一覧のドット/スピナー・タブ行の「N running」バッジで共有。**巻き戻し粒度**: 並行ターンの編集が混ざった区間は両方まとめて戻る(`mark_swept_turns`で巻き込まれたターンも巻き戻し済みに)。比較案UX(シート複製・パッチプレビュー)はフェーズ3 |
| Anthropic APIプロバイダ+キーチェーン(M3フェーズ2) | ✅ | `AgentBackend`トレイト(`run_turn`1本)にCLI/API両実装。`AnthropicApiBackend`=Messages API直(SSE、ツール往復は上限16、`MADAKE_ANTHROPIC_BASE_URL`で接続先差し替え可)。ツールは`ToolBridge`が内蔵MCPサーバーをプロセス内パイプで呼ぶので**CLI経路とAPI経路で同一**。キーは`keyring 4.1.6`でOSキーチェーン(`MadakeCAD`/`anthropic_api_key`)。設定ファイルには項目自体を作らない。設定UIにプロバイダ選択・伏せ字のキー入力/保存/削除・モデル欄・接続テスト。**実キーでの通し確認はユーザー確認事項**(実キー無しの範囲=UI・401・キー未設定案内・平文非保存は確認済み) |
| Copilot CLI / OpenAI互換 / Ollama / Geminiプロバイダ(M3フェーズ3) | ✅ | 6経路そろう。`CopilotCliBackend`=`copilot -p … --output-format json`(JSONL)を実行。MCPは`--additional-mcp-config @一時ファイル`でセッション限定に渡し`--disable-builtin-mcps`。システムプロンプト用フラグが無いため`# システム指示`として前置(`--no-custom-instructions`)。**GitHubトークンは保存しない**(CLI側の`/login`)。`OpenAiCompatBackend`=`POST {base_url}/chat/completions`(SSE+`stream_options.include_usage`、`tool_calls`を`index`ごとに連結、失敗は`ERROR: `前置で返す)。設定は`openai_base_url`+`openai_model`(既定なし=必須入力)。**接続先がローカル(localhost/127.0.0.1/*.local等)ならAuthorizationヘッダごと省略**=Ollamaはキー不要。`GeminiBackend`=`models/{model}:streamGenerateContent?alt=sse`(キーは`x-goog-api-key`ヘッダのみ。`functionResponse`はrole`"user"`で返す。`usageMetadata`は累計置換)。ツールスキーマはOpenAPI部分集合へ削る`gemini_schema()`(`$schema`/`$ref`/`additionalProperties`等を送ると400)。ツール往復は各経路とも上限16 |
| プロバイダの実機確認状況 | 🔶 | **Ollama(0.32.14)は通し確認済み**(実チャンク形一致・キー無しで接続テスト成功・`gemma4:26b`で作図1ターン=ツール往復3回→revision 0→1→`undo-turn`で後始末)。**Copilot CLI 1.0.80も通し確認済み**(2026-08-23、`/login`済み。JSONL実イベント形=`{type,data}`封筒+封筒無しの最終行`result`をパーサへ反映し、作図1ターン→継続ターンで文脈保持→`undo-turn`で後始末)。Geminiは無効キーで実APIの`API_KEY_INVALID`→`gemini_auth`変換まで。Anthropic APIは401・キー未設定案内まで。**Gemini・Anthropicの実キー通しはユーザー確認事項** |
| キーチェーン(全プロバイダ共通) | ✅ | `keyring 4.1.6`直用。`service="MadakeCAD"`、`account`=`anthropic_api_key`/`openai_compat_api_key`/`gemini_api_key`。設定ファイル(`~/.madakecad/settings.json`)には項目自体を作らない(平文非保存はテストで固定)。`SecretStore`トレイトでテストはメモリ保管、実キーチェーンのテストは`MADAKE_KEYCHAIN_TESTS=1`のときだけ。macOSは許可ダイアログ抑制のためプロセスに1回だけ読み出しをキャッシュ。CLI経路(Claude Code / Copilot)は資格情報を一切持たない |
| プロバイダ接続テスト | ✅ | Link API `POST /api/v1/agent/test-connection` + `GET /api/v1/agent/provider`(モデル名・キー保存済みフラグ)。CLI経路は検出+バージョン、API経路は実エンドポイントへの疎通。失敗は種類つき(`*_auth`/`*_rate_limit`/`*_model_not_found`/`*_server`/`*_request`/`*_network`/`*_no_key`/`*_no_model`)で日本語の案内文を返す |
| FreeCAD連携(フェーズM) | 🔶 | Link API(M1の土台)は実装済み。**2D盤レイアウトはM4フェーズ4で実装済み**。アドオンWB・3D対応付け・電線長書き戻し・3D筐体との同期が未 |

## 8. ドメイン機能(参考図面の再現に必要な残り)

| 機能 | 状態 | 備考 |
|---|---|---|
| 端子台・コネクタ(ピン番号単位の結線) | ✅ | 動的シンボル+貫通端子+ネットリスト |
| 端子台エディタ(グリッド) | 🔶 | 行の導出表示・サドルジャンパ生成/削除(`attrs["jumpers"]`をupdate_entity。undo可)・端子台チェック・チャート/接続図の生成起点。並べ替え・多段端子・アクセサリ・部品割当はM4の以降のフェーズ(モデル拡張が必要) |
| 電線管理(色・sq・長さ・品番) | ✅ | Wire属性+電線品番マスタ |
| 線番(ワイヤ番号)の挿入・自動採番 | ✅ | ネット単位。`renumber_wires`(追い番/振り直し・開始番号・シート指定/全体)+`set_wire_numbers`(個別編集)。キャンバス/SVG/PDF描画、電線リスト連動、CLI `madake renumber` |
| ハーネス境界(破線囲み) | ✅ | Entity `Harness`(矩形・名前・備考)。所属は全点内包(入れ子は最小優先)、破線描画+名前、電線リストのハーネス列、リボンのハーネスツール |
| 複数母線・信号矢印 | ⬜ | リボンにボタンのみ(todo) |
| 回路マクロ(保存・挿入・バリアント) | ✅ | `macros.rs`。保存=選択範囲をCommand列へ逆変換+相対座標化(基準点=左下ピン)、`~/MadakeCAD/macros/*.json`。挿入=UUID振り直し+参照記号の再採番(既存最大+1、マクロ内の関係は保持)+線番クリア、`execute_batch`でundo一発。バリアントは列挙・選択まで(`A`以外の追加UIはフェーズ4)。**値セット**=`placeholders`(key・ラベル・行き先`{entity, field}`。fieldは`value`か`attrs.<名前>`)+`value_sets`(id・ラベル・`values`)。どちらも`#[serde(default)]`で旧マクロJSONもそのまま読める。挿入は行き先を全部解決してから書き込むので`execute_batch`1回のまま(undo一発)、不正な値セット・行き先は図面を変えずにエラー |
| 回路コピー・ペースト | ✅ | `⌘C`/`⌘V`。無名マクロとして組み立て(`build_macro`)→そのまま挿入(`apply_macro_inline`)。ファイルには書かない |
| 回路トリム | ⬜ | リボンにボタンのみ(todo) |
| コイル⇔接点クロスリファレンス | ✅ | `relay_xref.rs`。同一参照記号のコイル+接点群=1デバイス。接点はシート番号→ゾーン→idの読み順に並び、その連番が端子対(IEC 60947-1。コイル=A1-A2)。コイル外形下端+2.5mmに接点マップ(端子対\|所在。`contact_config`が読めるとき未使用は「—」)、接点の外形右端+1mmにコイル所在。SVGとキャンバスは`contact_map_layout`/`contactMapLayout`の同一数式 |
| プロジェクト内検索 | ✅ | `search.rs`。対象5種=参照記号・型番(value+attrs)・ネット名・線番・テキスト。部分一致・大文字小文字無視・空クエリは0件。並びはシート番号→ゾーン→entity id→種別で決定的。シンボルのヒットにはデバイス内の機能(コイル/接点13-14…)が付く |
| デバイスツリー(ナビゲータのデータ) | ✅ | `search.rs`。参照記号ごとに機能を列挙(リレー=コイル+接点(端子対は接点マップと同じ連番)、端子台=端子群1行、その他=本体) |
| シート間クロスリファレンス | ✅ | `extract_netlist_project`が同名ラベルでシート横断統合。ラベル脇に「/シート.ゾーン」(キャンバス/SVG/PDF)、プロパティパネルの相手先リンク、ERC(`verify_project`)・帳票も統合ネットで評価 |
| PLC I/O(割付表・図面生成・レポート) | ✅ | `plc.rs`。モジュール定義=部品DBの`plc_module`(点数・DI/DO・アドレス接頭辞・体系)。アドレス採番は三菱=8進/Siemens=バイト.ビット/AB=ワード/ビット(開始点をずらせる)。割付表=`Project.plc_assignments`(**format_version 2**。旧ファイルは空の表で開く)、編集は`set_plc_assignments`1回。CSVは往復用3列と読み取り専用レポート6列の2種。図面生成=`RestoreSheet`+必要なら`set_plc_assignments`を`execute_batch`で**undo一発**(縦母線`P24`+1点=1ラング、生成ページはERC指摘ゼロ)。接続先・線番は図面から読むだけ(全シート合算)。露出=IPC 6本 / Link API 4本 / MCPツール3種 / CLIの`plc-io`帳票 / 割付表エディタ+生成設定ダイアログ |
| PLC I/O(横母線・モジュール同居・ページ分割) | ⬜ | 生成設定のenumだけ用意し「未実装」エラーを返す。生成設定のプロジェクト保存も未(セッション限り) |
| シンボルエディタ(ユーザー定義シンボル) | ✅ | `symbol_library.rs`(ユーザーライブラリの読み書き・列挙・保存時検証・複製開始)+`canvas/symbolEditor.ts`(画面側。**検証はコアと同じ判断・同じコード**)。保存先`~/MadakeCAD/symbols/{id}.json`、分類は`user`へ正規化、同梱・動的シンボルとのid衝突は拒否。検証=id/名称/接頭辞/図形0個・潰れた図形/接続点0個・番号空・2.5mmグリッド外・同じ点の重複/属性スロット(TAG必須)。落ちたら**何も書かない**。図面はスナップショットを持たないので保存し直すと配置済みの部品も追従(使用中の削除は`force`なしでは拒否)。露出=IPC 6本 / Link API 5本 / MCPツール3種。**残り**: 回転バリアントの手動上書き |
| 2D盤レイアウト | ✅ | `panel.rs`(数式の正)+`canvas/panel.ts`(同じ式)。シート種別`panel`、盤枠(外形+取付板10mm内側)・DINレール(取付線=帯の中心、35mm帯)・ダクト(用紙座標に固定した45°ハッチ)・フットプリント・凡例表。未配置部品=回路図の参照記号ごとに1台(どの盤シートにもフットプリントが無いもの)。バルーン採番`renumber_panel_items`(読み順=部品の中心の高さで段分け→左から右。逆コマンド=`set_panel_item_numbers`)。検査=`panel.overlap`(面で重なる対。辺が触れるだけは報告しない)/`panel.off_rail`(レール0本の盤では報告しない)/`panel.rail_fill`(80%超は注意)/`panel.orphan_footprint`。盤シートは`verify_sheet`が即空を返す(配線が無いため)。露出=IPC`unplaced_components` / Link API`GET /panel/unplaced` / MCPは`execute_commands`の説明に書き方を追記 |
| 盤内配線図(wire annotation)・銘板・寸法線 | ⬜ | リボンに項目だけ(淡く表示。押すと理由をログ) |
| 計測ツール | ⬜ | 未実装 |

## 9. 基盤・配布

| 機能 | 状態 | 備考 |
|---|---|---|
| テスト | ✅ | cargo 1003件+vitest 734件+vue-tsc。全テストに対訳仕様文が付き、`docs/13-specification.md`(1737項目)を自動生成。TDD運用 |
| macOSビルド | ✅ | 開発は`npm run tauri dev` |
| Windows/Linuxビルド | ⬜ | 非目標(現時点)。コードはOS非依存を維持(ngspice探索・PDFフォントに一部OS別処理あり) |
| 配布パッケージ/自動更新 | ⬜ | 未着手 |
| 自動保存・クラッシュ復旧 | ⬜ | 未着手 |
| i18n | 🔶 | en/jaのメッセージカタログ+言語設定。新規UI文字列はi18n必須。他言語カタログはM6 |

## 10. 次期計画

**M4は2026-08-23のフェーズ4完了をもって完了。フェーズ5(参考図面の再現パリティ)も同日完了**
(再現検証で必須と判定されたG1〜G4+基盤のG9/G17を解消。判定は`specs/reproduction-gaps.md`)。 M4から先送りした項目(フェーズ未設定): ナビゲータからの
予約接点配置 / `A`以外のマクロバリアント追加UI / PLCの横母線ラダー・モジュール同居・ページ分割 /
ケーブル一覧(Cableエンティティ待ち) / 部品DB連動BOM / 盤内配線図・銘板・寸法線・整列/等間隔 /
グラフィカルな図枠エディタ。フェーズ5から残ったのは`specs/reproduction-gaps.md`の品質向上・軽微の各項目
(特にG19 タグの基準点・G20 注記の文字高さ設定・G21 c接点のクロスリファレンスは小さく効く)。

本節の旧A〜D方向案は、ビジョン明確化(2026-08-21)によりマイルストーンM2〜M6へ再編された。
計画の正は[要件定義書](requirements.md) §2 と [機能仕様集](specs/README.md)(競合比較は[ギャップ分析](specs/gap-analysis.ja.md))。
