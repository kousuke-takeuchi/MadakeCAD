# MadakeCAD 機能インベントリ

作成: 2026-08-21(フェーズ0〜3完了時点)。更新: 2026-08-22(M4フェーズ2 回路マクロ+コイル⇔接点XRef+検索/ナビゲータ 完了)。全機能の棚卸しと、未実装バックログの一覧。
specは`docs/superpowers/specs/2026-08-20-madakecad-design.md`、実装経緯は`docs/superpowers/plans/`の各プラン。

## 1. コア・アーキテクチャ(madake-core)

| 機能 | 状態 | 備考 |
|---|---|---|
| Commandエンジン | ✅ | 全編集がCommand経由。逆コマンドによるundo/redo、Patch(revision付き)のbroadcast |
| ドキュメントモデル | ✅ | Project / Sheet(JIS図枠・表題欄・改訂欄・ゾーン) / Entity(Symbol・Wire・Junction・NetLabel・Text・Harness) |
| 保存形式 `.mdkproj` | ✅ | 整形JSON(git差分可読)。チャット履歴は`<名前>.chat.json`を併存 |
| シンボルライブラリ | ✅ | JIS C 0617系の静的15種(リレーはコイル・a接点・b接点)+動的シンボル`connector_{n}p`/`terminal_block_{n}p`(n=1..50、5mmピッチ中央揃え)。端子台は左右貫通端子 |
| 座標系 | ✅ | mm・左上原点・Y下向き。2.5mmグリッド、回転0/90/180/270 |
| ヘッドレス実行 | ✅ | コアはUI非依存(全ロジックがmadake-coreに集中) |

## 2. エディタUI(Vue 3 + Canvas2D)

| 機能 | 状態 | 備考 |
|---|---|---|
| リボン(回路図タブ) | ✅ | 配線 / 部品を挿入 / 回路図を編集 / 検証・レポート(部品表・電線リスト・シミュレーション・SVG/PDF出力) |
| リボン(表示タブ) | ✅ | 表示クラス9種(配線/シンボル/参照記号/ネットラベル/線番/ハーネス/注記/図枠/グリッド)のトグル(レイヤ。画面のみ、出力へ非反映) |
| リボン(レポートタブ) | ✅ | 帳票(From-To/端子台チャート/端子接続図/BOM/XRef表)+端子台(端子台エディタ)+出力(PDF一括出力)。生成ダイアログで出力形式(CSV/図面シートPDF)と対象を選ぶ |
| リボン(他タブ) | ⬜ | ホーム/プロジェクト/パネル/読み込み・書き出し/管理はプレースホルダ |
| キャンバス操作 | ✅ | パン(中ボタン/Space)・ホイールズーム・グリッド・スナップ・直交拘束・ピンスナップ(菱形マーカー) |
| 選択・編集 | ✅ | クリック選択・Shift追加・矩形選択・ドラッグ移動(Command化)・削除・⌘Z/⇧⌘Z |
| 配線ツール | ✅ | 直交ポリライン、ダブルクリック/Escで確定。線色・sq既定値 |
| 配置ツール | ✅ | ゴーストプレビュー、Rで回転、参照記号の自動採番(接頭辞+連番) |
| 部品挿入ダイアログ | ✅ | 「部品」/「マクロ」タブ。部品=検索(名称/記号)・カテゴリ別グリッド・動的シンボルの極数バー(1..50ステッパー)・部品DB検索セクション(選択で型番・定格・接点構成つき配置)。マクロ=カテゴリツリー+バリアント数バッジ付きタイル+プレビュー(値セット欄はフェーズ3のためdisabled) |
| 回路マクロUI | ✅ | 保存ダイアログ(名前・カテゴリ・基準点表示・バリアントAチップ・選択範囲プレビュー)、配置ツール(ゴースト・`R`回転・`Tab`バリアント・Esc・連続配置)、`⌘C`/`⌘V`(無名マクロ) |
| 検索バー+結果パネル | ✅ | ⌘Fの浮きバー(フィルタチップ=参照記号/型番/ネット/テキスト・件数)+下部ドックの結果パネル(参照/種別/所在/補足)。Enter=次へ/Shift+Enter=前へ(端で回り込み)/Escで閉じる |
| デバイスナビゲータ(左3タブ目) | ✅ | 参照記号→機能(コイル/接点(端子対)/端子/本体)のツリー+所在。行クリックでreveal、右クリックでジャンプ/削除(`delete_entities`経由なので⌘Zで戻る)。未配置機能のドラッグ配置はフェーズ3 |
| 参照サーフィン(Surfer) | ✅ | キャンバスでAlt(Option)+クリック→所在一覧ポップアップ(シンボル=同一デバイスの全機能/ネットラベル=同名ラベル/ワイヤ=同一線番)。↑↓で巡回・Enterで確定・Escで閉じる |
| プロパティパネル | ✅ | シンボルの参照記号・型番、ワイヤの線番(ネット単位)、ハーネスの名前・備考(含む電線数)、ネットラベルの相手先リンク(クリックでシート切替+ズーム) |
| reveal(ジャンプ)の一本化 | ✅ | 検証パネル・検索・デバイスナビゲータ・Surferの4経路が`composables/reveal.ts`(シート切替+選択+ズーム) |
| プロジェクトマネージャ(左) | ✅ | ツリー+詳細。エージェントタブと切替 |
| 図面タブ | ✅ | シート切替・追加(add_sheet) |
| ステータスバー | ✅ | 座標、スナップ/直交/グリッドトグル、直近ログ、ズーム、MCPポート表示 |
| 検証結果パネル | ✅ | severityバッジ、行クリックで該当エンティティ選択+ズーム、再検証 |
| シミュレーション結果パネル | ✅ | ネット電圧(min/max)・部品電流/電力、開路チップ、再実行 |
| 設定画面 | 🔶 | AI設定(プロバイダ・一般・チャット・MCPタブ)。プロバイダはClaude Code CLI / Anthropic API(キーはOSキーチェーン・接続テストつき)が実動。OpenAI互換・Geminiは未実装 |
| テーマ | ⬜ | ライトのみ(CAD調トークン)。ダーク切替は設定UIにあるが未実装 |

✅=実装済み / 🔶=部分実装 / ⬜=未実装

## 3. 図枠・出力

| 機能 | 状態 | 備考 |
|---|---|---|
| JIS図枠描画 | ✅ | 枠線・ゾーン番号(横=数字/縦=英字)・表題欄(図番/品名/尺度/日付/設計〜承認/社名)。画面とSVG/PDFで同一 |
| 改訂欄 | ✅ | ISO 7200様式(表題欄直上・古い行が下・最大6行・列=記号/日付/内容/承認)。キャンバス/SVG/PDFで同一、表題欄Revは最新行に連動。編集ダイアログ(`set_revisions`コマンド1回=undo1回) |
| SVG出力 | ✅ | 印刷品質、mm 1:1、XMLエスケープ |
| PDF出力 | ✅ | svg2pdfでベクタ変換、日本語フォント埋め込み(macOS=Hiragino。Win/Linuxのフォント割当は未調整) |
| BOM(部品表) | ✅ | 参照記号+型番の集計。CSV / 図枠付き図面シート |
| From-Toワイヤリスト | ✅ | シート・From・To・線番・線色・sq・長さ・電線品番・ハーネス。From/Toはピン(`参照記号:ピン`)/ネットラベル/空。CSV / 図面シート |
| 端子台チャート | ✅ | 端子台1台=1表。端子番号順に内部側/外部側・線番・電線・ハーネス・ジャンパ・予備端子。CSV / 図面シート(`--terminal`で1台に絞れる) |
| 端子接続図 | ✅ | EPLAN端子図様式(外部=左/内部=右・ハーネスブラケット・予備・ジャンパ)。1端子台=1ページ(端子15個で分割)。PDFのみ |
| クロスリファレンス表 | ✅ | プロジェクト全体ネットの所在一覧。CSV / 図面シート。コイル⇔接点対応は図面上の接点マップ(§8)で実装済み |
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
| 部品マスタ | ✅ | 型番・メーカ・名称・カテゴリ・既定シンボル・定格電圧/電流・接点構成(`contact_config`、例"2NO+2NC")・購入先URL・データシートURL・価格・備考。スキーマv3(バージョン管理+マイグレーション。v2→v3で既存行を保持したまま接点構成列を追加) |
| 電線品番マスタ | ✅ | 線色+sq→品番(DBがマスタ。図面側`Project::wire_parts`はスナップショット) |
| 予約列 | ✅ | `model_3d`(フェーズM)・`mounting`・`spice_model`(過渡解析用) |
| サンプルデータ | ✅ | 初回作成時にダミー型番5件+電線4件 |
| 検索→配置連携 | ✅ | 部品挿入ダイアログから配置すると型番(value)と定格(attrs.current_a)・接点構成(attrs.contact_config)が図面に設定され、検証・シミュレーション・接点マップと連動(図面はDBのスナップショット) |
| 管理UI(一覧・編集画面) | ⬜ | 登録・編集はCLI/MCP/API経由(意図した割り切り) |
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
| 内蔵MCPサーバー | ✅ | 127.0.0.1:9310/mcp。ツール29種: get_project / list_symbols / place_symbol / draw_wire / execute_commands(set_revisions・renumber_wires・set_wire_numbers・harness追加・ジャンパ(update_entityのattrs)もここから) / get_netlist / run_verification / get_tidy_metrics / simulate_op / search_parts / upsert_part / delete_part / import_kicad / list_terminal_blocks / get_terminal_chart / check_terminal_block / list_templates / apply_template / list_macros / save_macro / apply_macro / export_svg・pdf・report・pdf_book・bom・wire_list / undo / redo。**検索・デバイスツリーはMCP未露出**(IPC+Link APIのみ。AIはget_projectで足りるため意図的) |
| Link API (/api/v1) | ✅ | REST+SSEパッチ。project / symbols / netlist / verify / tidy-metrics / search / devices / simulate/op / commands / undo / redo / save / load / import/kicad / terminals(+/chart・/check) / templates(+/apply) / macros(+/build・/save・/apply・/apply-inline) / export/*(svg・pdf・pdf-book・report・bom・wire-list) / parts / wire-parts / agent/*(send・cancel・conversations・undo-turn・detect・events) / settings / events |
| madake CLI | ✅ | status / project / netlist / verify / sim / parts / terminals / export(svg・pdf・pdf-book+帳票5種を`--format csv\|pdf`・`--terminal`付きで) / save / open(.kicad_sch対応) / renumber / exec / undo / redo。マクロ・検索のサブコマンドは未(Link APIを`exec`/curlで直接叩ける) |
| AIチャット(A1) | ✅ | 左ドック+浮きカード、Claude Code CLIバックエンド(Pro/Max OAuth再利用)、ツールチップ表示、ターン単位undo、編集オーバーレイ(シアンパルス)、会話履歴のプロジェクト保存 |
| A1の持ち越し負債(M3フェーズ1で解消) | ✅ | ターン安定ID(`turn_id`。chat.json format_version 2へ移行)・編集origin(`Engine::execute_as` / `revert_range`でagent編集だけを逆適用。衝突は`RevertConflict`)・キャンセルseq(`turn_seq`で遅延イベントを破棄) |
| 規格知識+検証ループ(M3フェーズ1) | ✅ | 同梱`resources/knowledge/standards.md`を毎ターン注入(設定`knowledge_path`で追記可、後勝ち)。図面コンテキストに検証サマリ。プロンプトで「編集後は`run_verification`→修正→再検証(最大3回)→件数報告」を必須化 |
| ナレッジ回答・AIレビュー・部品選定(M3フェーズ1) | ✅ | `docs/`目次+`--add-dir`+読み取り専用ツール(Read/Glob/Grep)で出典付き回答(未対応機能はroadmapで回答)。レビューは決定的検証+5観点チェックリストを重要度表に。検証計画は手順表。部品選定は`search_parts`の比較表(最大10件) |
| 開始テンプレート(M3フェーズ1) | ✅ | 同梱3種(24V制御基本・モータ起動回路・非常停止回路。適用後ERCエラー/警告0)。`Engine::execute_batch`で履歴1エントリ=undo一発。IPC/Link API/MCPツール+リボン導線。`~/MadakeCAD/templates/*.json`でユーザーテンプレート |
| 自動反復=整えループ(M3フェーズ2) | ✅ | `madake-core::tidy`の決定的メトリクス(交差数・ラベル重なり・シンボル重なり・グリッド外)をMCP`get_tidy_metrics`/Link API`/tidy-metrics`で露出。チャット入力欄の杖ボタン→ポップアップ(配置整理/配線整理/ラベル整頓)が「測る→直す→測り直す(改善が止まる or 最大3回)」の定型プロンプトを**1ターン**として送る=undo一発。`knowledge.rs`のWORKFLOW_RULESにも同じループを記載(自然文依頼でも回る)。バリアント数(2〜4案)はフェーズ3 |
| 並列エージェント(M3フェーズ2) | ✅ | 会話ごとに`tokio::spawn`で同時実行(Busy判定は会話単位)。フロントの送信ガードは**開いている会話**だけに効く(`chat.streaming` / `anyStreaming` / `runningIds`)。会話色=`theme.agentPalette`(#29D3E6 / #FFB454 / #B48CFF / #FF6FD8)を開始順に割当、編集オーバーレイ・会話一覧のドット/スピナー・タブ行の「N running」バッジで共有。**巻き戻し粒度**: 並行ターンの編集が混ざった区間は両方まとめて戻る(`mark_swept_turns`で巻き込まれたターンも巻き戻し済みに)。比較案UX(シート複製・パッチプレビュー)はフェーズ3 |
| Anthropic APIプロバイダ+キーチェーン(M3フェーズ2) | ✅ | `AgentBackend`トレイト(`run_turn`1本)にCLI/API両実装。`AnthropicApiBackend`=Messages API直(SSE、ツール往復は上限16、`MADAKE_ANTHROPIC_BASE_URL`で接続先差し替え可)。ツールは`ToolBridge`が内蔵MCPサーバーをプロセス内パイプで呼ぶので**CLI経路とAPI経路で同一**。キーは`keyring 4.1.6`でOSキーチェーン(`MadakeCAD`/`anthropic_api_key`)。設定ファイルには項目自体を作らない。設定UIにプロバイダ選択・伏せ字のキー入力/保存/削除・モデル欄・接続テスト。**実キーでの通し確認はユーザー確認事項**(実キー無しの範囲=UI・401・キー未設定案内・平文非保存は確認済み) |
| その他プロバイダ(A3の残り) | ⬜ | OpenAI互換(OpenAI/xAI/OpenRouter/Ollama)・Gemini。フェーズ3 |
| FreeCAD連携(フェーズM) | 🔶 | Link API(M1の土台)は実装済み。アドオンWB・3D対応付け・電線長書き戻し・盤レイアウトが未 |

## 8. ドメイン機能(参考図面の再現に必要な残り)

| 機能 | 状態 | 備考 |
|---|---|---|
| 端子台・コネクタ(ピン番号単位の結線) | ✅ | 動的シンボル+貫通端子+ネットリスト |
| 端子台エディタ(グリッド) | 🔶 | 行の導出表示・サドルジャンパ生成/削除(`attrs["jumpers"]`をupdate_entity。undo可)・端子台チェック・チャート/接続図の生成起点。並べ替え・多段端子・アクセサリ・部品割当はM4の以降のフェーズ(モデル拡張が必要) |
| 電線管理(色・sq・長さ・品番) | ✅ | Wire属性+電線品番マスタ |
| 線番(ワイヤ番号)の挿入・自動採番 | ✅ | ネット単位。`renumber_wires`(追い番/振り直し・開始番号・シート指定/全体)+`set_wire_numbers`(個別編集)。キャンバス/SVG/PDF描画、電線リスト連動、CLI `madake renumber` |
| ハーネス境界(破線囲み) | ✅ | Entity `Harness`(矩形・名前・備考)。所属は全点内包(入れ子は最小優先)、破線描画+名前、電線リストのハーネス列、リボンのハーネスツール |
| 複数母線・信号矢印 | ⬜ | リボンにボタンのみ(todo) |
| 回路マクロ(保存・挿入・バリアント) | ✅ | `macros.rs`。保存=選択範囲をCommand列へ逆変換+相対座標化(基準点=左下ピン)、`~/MadakeCAD/macros/*.json`。挿入=UUID振り直し+参照記号の再採番(既存最大+1、マクロ内の関係は保持)+線番クリア、`execute_batch`でundo一発。バリアントは列挙・選択まで(追加UIと値セット=プレースホルダはフェーズ3) |
| 回路コピー・ペースト | ✅ | `⌘C`/`⌘V`。無名マクロとして組み立て(`build_macro`)→そのまま挿入(`apply_macro_inline`)。ファイルには書かない |
| 回路トリム | ⬜ | リボンにボタンのみ(todo) |
| コイル⇔接点クロスリファレンス | ✅ | `relay_xref.rs`。同一参照記号のコイル+接点群=1デバイス。接点はシート番号→ゾーン→idの読み順に並び、その連番が端子対(IEC 60947-1。コイル=A1-A2)。コイル外形下端+2.5mmに接点マップ(端子対\|所在。`contact_config`が読めるとき未使用は「—」)、接点の外形右端+1mmにコイル所在。SVGとキャンバスは`contact_map_layout`/`contactMapLayout`の同一数式 |
| プロジェクト内検索 | ✅ | `search.rs`。対象5種=参照記号・型番(value+attrs)・ネット名・線番・テキスト。部分一致・大文字小文字無視・空クエリは0件。並びはシート番号→ゾーン→entity id→種別で決定的。シンボルのヒットにはデバイス内の機能(コイル/接点13-14…)が付く |
| デバイスツリー(ナビゲータのデータ) | ✅ | `search.rs`。参照記号ごとに機能を列挙(リレー=コイル+接点(端子対は接点マップと同じ連番)、端子台=端子群1行、その他=本体) |
| シート間クロスリファレンス | ✅ | `extract_netlist_project`が同名ラベルでシート横断統合。ラベル脇に「/シート.ゾーン」(キャンバス/SVG/PDF)、プロパティパネルの相手先リンク、ERC(`verify_project`)・帳票も統合ネットで評価 |
| シンボルエディタ(ユーザー定義シンボル) | ⬜ | SymbolDefはJSONなので手書きは可能。UIなし |
| 計測ツール | ⬜ | 未実装 |

## 9. 基盤・配布

| 機能 | 状態 | 備考 |
|---|---|---|
| テスト | ✅ | cargo 561件+vitest 390件+vue-tsc。全テストに対訳仕様文が付き、`docs/13-specification.md`(951項目)を自動生成。TDD運用 |
| macOSビルド | ✅ | 開発は`npm run tauri dev` |
| Windows/Linuxビルド | ⬜ | 非目標(現時点)。コードはOS非依存を維持(ngspice探索・PDFフォントに一部OS別処理あり) |
| 配布パッケージ/自動更新 | ⬜ | 未着手 |
| 自動保存・クラッシュ復旧 | ⬜ | 未着手 |
| i18n | 🔶 | en/jaのメッセージカタログ+言語設定。新規UI文字列はi18n必須。他言語カタログはM6 |

## 10. 次期計画

本節の旧A〜D方向案は、ビジョン明確化(2026-08-21)によりマイルストーンM2〜M6へ再編された。
計画の正は[要件定義書](requirements.md) §2 と [機能仕様集](specs/README.md)(競合比較は[ギャップ分析](specs/gap-analysis.ja.md))。
