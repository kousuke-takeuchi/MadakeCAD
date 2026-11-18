# MadakeCAD 機能インベントリ

作成: 2026-08-21(フェーズ0〜3完了時点)。更新: 2026-08-23(**フェーズ6 完了**: 可読性 —
ネットタグの基準点・シートの注記文字高・c接点のリレー認識)。その前の更新は **フェーズ5 完了**(参考図面の再現パリティ — ブロックシンボル+配線注記+ネットラベルのタグ形状+注記の装飾+電線品番の引き当て)。その前の更新は M4フェーズ4 完了(= M4完了: 図枠テンプレート+シンボルエディタ+2D盤レイアウト)。全機能の棚卸しと、未実装バックログの一覧。
specは`docs/superpowers/specs/2026-08-20-madakecad-design.md`、実装経緯は`docs/superpowers/plans/`の各プラン。

## 1. コア・アーキテクチャ(madake-core)

| 機能 | 状態 | 備考 |
|---|---|---|
| Commandエンジン | ✅ | 全編集がCommand経由。逆コマンドによるundo/redo、Patch(revision付き)のbroadcast |
| ドキュメントモデル | ✅ | Project(図枠テンプレートid・PLC割付表・FreeCAD対応付け`mech_links`) / Sheet(種別=回路図\|盤レイアウト・JIS図枠・表題欄・改訂欄・ゾーン) / Entity 12種(Symbol・Wire・Junction・NetLabel・Text・Harness・Shape(図形。電気的意味なし)+盤の PanelOutline・DinRail・Duct・Footprint・PanelLegend) |
| 保存形式 `.mdkproj` | ✅ | 整形JSON(git差分可読)、`format_version` **15**(v12でFreeCAD連携=`mech_links`・`MechLink.placement`・`Wire.length_source`を統合、v13で整えバリアントの複製シートの印`Sheet.variant_of`を追加。履歴と旧ファイルの扱いは`data-model.md`)。チャット履歴は`<名前>.chat.json`を併存 |
| シンボルライブラリ | ✅ | JIS C 0617 / IEC 60617系の静的51種(電源・変換・接地7/保護6/操作スイッチ8/リレー・電磁接触器5/半導体3/受動6/負荷・報知6+絵記号4(拡声器・ブザー・サイレン・送風機、G12。確立した形のみ。拡声器・送風機はIEC 60617の項目番号、ブザー・サイレンはJIS C 0617-8:2011 第10節で裏取り)/計測3/接続3)+動的シンボル`connector_{n}p`/`terminal_block_{n}p`(n=1..50、5mmピッチ中央揃え)・`plc_di_{n}p`/`plc_do_{n}p`(n=1..64、点ピッチ5mm、カテゴリ`plc`)。全記号がピンの接続方向(up/down/left/right)・属性スロット(TAG/PART/DESC/RATING)・検索キーワード(en+ja)を持つ。多極機器(遮断器・断路器・電磁接触器)は端子1/2・3/4・5/6で極ごとに導通。端子台は左右貫通端子。**多段端子台**(G6)`terminal_block_{n}p_{levels}l`(段=1〜3、例`terminal_block_8p_2l`)は段ごとに1列の端子を横に並べ(列の中心間隔10mm・極が行)、段記号付きピン番号(`1a`/`1b`…)を持つ独立端子になる。1シンボル=1デバイスのままチャート・接続図・BOMに載る(既定の1段は従来どおり`terminal_block_{n}p`と同一図形)。端子台の`attrs["jumpers"]`は端子台チャート・端子接続図に加え、図面上のシンボルにもショートバー(隣り合う端子の印の隙間を埋める太線、G5)として描かれ、多段端子台では段記号付きの指定(`1a-1b`/`1a-2a`)だけを受け付ける。`attrs["pin_groups"]`(G8、`"1-4:電源,5-8:信号"`)は`block`/`terminal_block_*`/`connector_*`のチャート順ピン範囲へ角括弧+縦書き近似(4文字ごとの行分割)のキャプションを描き、ピンの向きに追従して回転・ミラーでも崩れない。括弧はピンの足先より外・キャプションはさらに外に置き、その箱をシンボル外形に数え入れるので参照記号・型番・機能ラベルのほうが避ける。多段端子台では括弧がその段の列の外側に出る(`1b-4b`は柱の間に入らない)。範囲の重なり・不明ピン・書式エラー・実機ブロック表示への指定はその項目だけを飛ばすERC診断(`erc.pin_group_overlap`/`erc.pin_group_unknown_pin`/`erc.pin_group_invalid`/`erc.pin_group_unsupported`)。ピン番号そのものに「-」を含む端子は範囲指定に使えない(区切り記号と区別できないため、その旨を診断で返す)。**ブロックシンボル**`block`はインスタンスの属性(`block_pins`=辺ごとの`{no,name}`・任意の`block_size`)から定義を毎回生成する(名前付きピンの汎用機器。ピン名は外形の内側・ピン番号は外側、5mmピッチ)。ピンの`connector`(13種: USB-A/USB-C/micro-B のメス・オス、3.5mm・DC のジャック・プラグ、端子台、JST PH/XH メス)で外形の辺に白黒のコネクタの絵を描き、続く端子台・JST のピンは同じ`connector_ref`で1つの本体と名前(`J3 PH`・`TB1`)になる。コネクタのあるピンの接続点は外形から5mm。絵だけで接点・ネットは変えない(キャンバスと SVG/PDF/PNG/DXF/KiCad。DXF・KiCad はブロックの形ごとに`MDK_block_<16桁>`・`MadakeCAD:block_<16桁>`の定義を書き、読み戻すと`block`とピン表に戻る)。Rust↔TS の一致は`madake-core/tests/golden/connector-blocks.json`で確かめる。形式19、コネクタのあるブロックはファイルでは種類`port_block`。**ユーザーシンボル**(`~/MadakeCAD/symbols/*.json`、分類`user`固定・同梱とのid衝突は拒否・保存時検証)が同じ解決経路に合流する(同梱→動的→ユーザー) |
| 座標系 | ✅ | mm・左上原点・Y下向き。2.5mmグリッド、回転0/90/180/270 |
| ヘッドレス実行 | ✅ | コアはUI非依存(全ロジックがmadake-coreに集中) |

## 2. エディタUI(Vue 3 + Canvas2D)

| 機能 | 状態 | 備考 |
|---|---|---|
| リボン(回路図タブ) | ✅ | 配線 / 部品を挿入 / 回路図を編集 / 図形(矩形・線・円・折れ線) / 検証・レポート(部品表・電線リスト・シミュレーション・SVG/PDF出力) |
| リボン(表示タブ) | ✅ | 表示クラス12種(配線/シンボル/参照記号/ネットラベル/線番/配線注記/ハーネス/図形/注記/盤/図枠/グリッド)のトグル(レイヤ。画面のみ。ただし**配線注記は出力に常に含まれる**) |
| リボン(レポートタブ) | ✅ | 帳票7種(From-To/端子台チャート/端子接続図/BOM/XRef表/PLC I/O/盤内部品配置表)+端子台(端子台エディタ)+PLC I/O+出力(PDF一括出力)。生成ダイアログで出力形式(CSV/図面シートPDF)と対象を選ぶ |
| リボン(パネルタブ) | ✅ | 盤(盤枠・DINレール・ダクトの作図)/フットプリント(未配置部品・型番から挿入*・ルックアップ編集*・銘板)/配置(バルーン採番・整列・等間隔・レール整列*・寸法・間隔チェック)/レポート(配置表・盤内配線図・凡例表)。盤シートを開くと自動で切り替わる。*=未実装(淡く出して押すと理由をログ) |
| リボン(管理タブ) | ✅ | テンプレート(図枠テンプレート・表題欄の編集*・回路テンプレート)/ライブラリ(シンボルエディタ・部品DB*・マクロ*・ユーザーライブラリ) |
| リボン(プロジェクトタブ) | 🔶 | 作図の開始グループに図枠テンプレートを配置。他はプレースホルダ |
| リボン(ホームタブ)・ファイルメニュー | ✅ | ファイル(新規/開く/保存/名前を付けて保存)+最近使ったファイル(最大8件、localStorageに保持)。タイトルバーの新規/開く/保存ボタンと`⌘N`/`⌘O`/`⌘S`/`⇧⌘S`も同じ動作。開いているファイルのパスを覚えて「保存」は上書き、未保存の編集があれば新規/開く/読み込みの前と**ウィンドウを閉じる/リロードするとき**に確認し、タイトルバーに`*`を出す(`stores/projectFile.ts` / `composables/closeGuard.ts`)。確認ダイアログはTauriのdialogプラグイン(`composables/nativeDialogs.ts`。webviewの`window.confirm`は実装が無い)。ショートカットはモーダル表示中は効かない。`*`はrevisionの比較なので**undoで保存時の内容に戻しても消えない** |
| リボン(読み込み/書き出しタブ) | ✅ | 読み込み(開く / KiCad回路図 / DXF)+書き出し(DXF / KiCad回路図 / SVG / PDF)+帳票(部品表CSV / 電線リストCSV)。読み込みは未保存の編集を先に確認 |
| シンボルエディタ | ✅ | 属性パレット(TAG必須/PART・DESC・RATING任意)+2.5mmグリッドのキャンバス(線・矩形・円・弧・文字・属性・接続点。選択/移動/削除)+接続点表(番号・方向・名前)+回転バリアント0/90/180/270の実プレビュー+保存前チェック(コアと同じ判断)。「複製して新規」はポップオーバー、未保存の確認はフッタで行う(モーダルを重ねない) |
| 盤レイアウトUI | ✅ | シート種別の選択(図面タブ行の+がポップオーバー)、盤ツール(矩形/水平ドラッグ→`add_entity`1回)、未配置部品パネル(参照記号・型番・寸法+出どころバッジ)、クリック配置(ゴーストは部品の中心・`R`で0/90・レール取付線へスナップ)、フットプリントのプロパティ(参照記号・型番・幅高さ・回転・アイテム番号・所属レール) |
| 図枠テンプレート選択ダイアログ | ✅ | 一覧(同梱/ユーザー・使用中バッジ)+表示中シートでのライブプレビュー(値が空になる欄にだけ`{バインド}`を青で重ねる)+バインド一覧+ゾーン設定(読み取り専用)+適用範囲(このプロジェクト=`set_frame_template`1回 / グローバル既定=`AppSettings.default_frame_template`) |
| キャンバス操作 | ✅ | パン(中ボタン/Space)・ホイールズーム・グリッド・スナップ・直交拘束・ピンスナップ(菱形マーカー) |
| 選択・編集 | ✅ | クリック選択・Shift追加・矩形選択・ドラッグ移動(Command化)・削除・⌘Z/⇧⌘Z |
| 配線ツール | ✅ | 直交ポリライン、ダブルクリック/Escで確定。線色・sq既定値 |
| 配置ツール | ✅ | ゴーストプレビュー、Rで回転、参照記号の自動採番(接頭辞+連番) |
| 部品挿入ダイアログ | ✅ | 「部品」/「マクロ」タブ。部品=検索(名称/記号)・カテゴリ別グリッド・動的シンボルの極数バー(1..50ステッパー)・部品DB検索セクション(選択で型番・定格・接点構成つき配置)。マクロ=カテゴリツリー+バリアント数バッジ付きタイル+プレビュー+値セットのドロップダウン(値セットを持つマクロにだけ出る) |
| 回路マクロUI | ✅ | 保存ダイアログ(名前・カテゴリ・基準点表示・バリアントチップ・選択範囲プレビュー。名前欄の既存マクロ候補を選ぶとそのマクロへバリアントB〜Zを追加し、登録済みは上書き確認=2026-10-15)、配置ツール(ゴースト・`R`回転・`Tab`バリアント・Esc・連続配置)、`⌘C`/`⌘V`(無名マクロ) |
| 検索バー+結果パネル | ✅ | ⌘Fの浮きバー(フィルタチップ=参照記号/型番/ネット/テキスト・件数)+下部ドックの結果パネル(参照/種別/所在/補足)。Enter=次へ/Shift+Enter=前へ(端で回り込み)/Escで閉じる |
| デバイスナビゲータ(左3タブ目) | ✅ | 参照記号→機能(コイル/接点(端子対)/端子/本体)+**未配置の予約接点**(接点構成から導出、クリック/ドラッグで配置・参照記号固定・undo可、絞り込み すべて/未配置あり/配置済み)のツリー+所在。行クリックでreveal、右クリックでジャンプ/削除(`delete_entities`経由なので⌘Zで戻る)。未配置機能のドラッグ配置はフェーズ3 |
| 参照サーフィン(Surfer) | ✅ | キャンバスでAlt(Option)+クリック→所在一覧ポップアップ(シンボル=同一デバイスの全機能/ネットラベル=同名ラベル/ワイヤ=同一線番)。↑↓で巡回・Enterで確定・Escで閉じる |
| プロパティパネル | ✅ | 何も選択していないときはシートの設定(注記文字高 1.0〜3.5mm=配線注記・線番・ネットタグの大きさ)、シンボルの参照記号・型番・機能ラベル・ピン表(ブロック)・嵌合相手(コネクタ、G14)、ワイヤの線番(ネット単位)・線色/線径/長さ/品番(自動引き当てはプレースホルダ表示)、注記の基準位置/色トークン/太字、ハーネスの名前・備考(含む電線数)、ネットラベルのタグ形状と相手先リンク(クリックでシート切替+ズーム)。**未対応**: 注記の引き出し先(`leader_to`、G20)。パネルにエンティティを選ぶ部品が無いので、コマンド/CLI/MCP経由のみ(対象を後から消すと`erc.leader_target_missing`として検証が報告する) |
| プロジェクト情報ダイアログ | ✅ | プロジェクト名・番号(`Project.no`、G16)+全シート共通の表題欄(会社名・図番・表題)を`set_project_meta`1回で設定。変更した欄だけを送る。「回路図」タブ→「編集」グループ |
| 用紙変更ダイアログ | ✅ | シートの用紙サイズ・向きを作成後に変更(`set_sheet_paper`、G15)。既存エンティティの座標は不変、ゾーン分割はシートが正のまま変えない(クロスリファレンスのゾーン住所が用紙変更で動かない)。「回路図」タブ→「編集」グループ |
| reveal(ジャンプ)の一本化 | ✅ | 検証パネル・検索・デバイスナビゲータ・Surferの4経路が`composables/reveal.ts`(シート切替+選択+ズーム) |
| プロジェクトマネージャ(左) | ✅ | ツリー+詳細。エージェントタブと切替 |
| 図面タブ | ✅ | シート切替・追加(add_sheet) |
| ステータスバー | ✅ | 座標、スナップ/直交/グリッドトグル、直近ログ、ズーム、MCPポート表示 |
| 検証結果パネル | ✅ | severityバッジ、行クリックで該当エンティティ選択+ズーム、再検証 |
| シミュレーション結果パネル | ✅ | 計算ケース (プロジェクトに保存、見出しで選択・新規・名前の変更・複製・削除、右クリックで短絡・断線、選択中の部品の値をケースで置き換え、⚡/✕ の印)・図面への重ね表示 (負荷率で橙・赤、超過に札、マウスでプローブ、編集後は古い結果として薄く)・定格超過 (重さで絞り込み、行クリックで選択+ズーム)・選択中の詳細・すべての値の折りたたみ (ネット電圧・部品電流/電力)、条件バー(`参照記号.属性=値` / `短絡 ネット名` の入力とチップ)、開路チップ、再実行 |
| 設定画面 | 🔶 | AI設定(プロバイダ・一般・チャット・MCPタブ)。プロバイダは**6経路すべて実動**(Claude Code CLI / Anthropic API / GitHub Copilot CLI / OpenAI互換 / Ollamaプリセット / Gemini)。経路ごとの詳細欄(パス・URL・モデル・伏せ字キー+保存/削除)と接続テストつき。残りはテーマ(ダーク)等 |
| テーマ | ⬜ | ライトのみ(CAD調トークン)。ダーク切替は設定UIにあるが未実装 |

✅=実装済み / 🔶=部分実装 / ⬜=未実装

## 3. 図枠・出力

| 機能 | 状態 | 備考 |
|---|---|---|
| JIS図枠描画 | ✅ | 枠線・ゾーン番号(横=数字/縦=英字)・表題欄(図番/品名/尺度/日付/設計〜承認/社名)。画面とSVG/PDFで同一 |
| 図枠エディタ(グラフィカル) | ✅ | `canvas/frameEditor.ts`(純粋ロジック: 罫線・矩形・文字・プレースホルダ・表題欄セル・ロゴ枠の追加/移動/削除、当たり判定、保存前チェック=Rust `validate`と同じ規則+未知プレースホルダ/枠外の要素)+`stores/frameEditor.ts`(下書き・エディタ内undo最大100段・別名で保存)+`FrameEditorDialog.vue`/`FrameEditorCanvas.vue`。保存=`frame_template::save`(IPC`save_frame_template` / Link API`POST /frame-templates/save`。同梱idは拒否、検証に落ちたら書かない)。形式は変えない。2026-10-15 |
| 図枠テンプレート(プロットフレーム) | ✅ | `frame_template.rs`。JSON定義(余白・ゾーン文字・表題欄の罫線とセル・改訂欄)+バインド解決(`{sheet.no}`/`{rev.latest}`/`{title_block.*}`ほか。知らないバインドは空文字)。同梱`jis-a3`(データ駆動化前の出力とゴールデン一致)・`jis-a4-portrait`、ユーザーは`~/MadakeCAD/templates/frames/`。差し替えは`set_frame_template`1回で全シート(キャンバス=`canvas/frameTemplate.ts`とSVG/PDFが同一計算)。**用紙・ゾーン分割はシートが正**。ロゴ画像の実描画は未 |
| 改訂欄 | ✅ | ISO 7200様式(表題欄直上・古い行が下・最大6行・列=記号/日付/内容/承認)。キャンバス/SVG/PDFで同一、表題欄Revは最新行に連動。編集ダイアログ(`set_revisions`コマンド1回=undo1回) |
| SVG出力 | ✅ | 印刷品質、mm 1:1、XMLエスケープ |
| PDF出力 | ✅ | svg2pdfでベクタ変換、日本語フォント埋め込み(OS別: macOS=Hiragino Sans、Windows=Yu Gothic UI、Linux=Noto Sans CJK JP。`pdf.rs::preferred_families`) |
| BOM(部品表) | ✅ | **アイテム番号**+参照記号+型番+シンボル+数量。アイテム番号は盤のバルーンと連動(未配置は空欄、複数まとまる行は「1, 3」)。**旧4列CSVとは非互換**。**電線集約**(G9残): 部品行の後ろに電線品番ごとのプロジェクト全体使用長合計(m・小数第2位、数量欄)を1行ずつ、品番が引けない配線は「(未割当)」の1行にまとめて追加。**カタログ列**(2026-10-15、BOM拡張): 数量の後ろにメーカ/品名/単価/通貨/金額/購入先の6列(全11列、旧5列CSVとは非互換)。型番を部品DBで引き当て(`PartsDb::bom_catalog`→`report_bytes_with`。アプリ・MCP・Link APIの出力口で引く)、無い型番は空欄、電線の単価は1mあたり、単価が1つでもあれば通貨ごとの「合計」行。CSV / 図枠付き図面シート |
| 多芯ケーブル・ケーブル一覧 | ✅ | `Entity::Cable`(定義線・芯線割当・シールド・長さ、format 15)、`AddCable`が横切った配線を交差順に芯へ割当、プロパティで型番・芯数・線径・シールド・長さ、ERC`erc.cable_core_overflow`、帳票「ケーブル一覧」(11列・CSV/図面シート・CLI `cable-list`)、部品表のケーブル行。2026-10-15 |
| From-Toワイヤリスト | ✅ | シート・From・To・**嵌合相手**(G14)・**備考**(G18)・線番・線色・sq・長さ・電線品番・ハーネス(旧9列とは非互換)。From/Toはピン(`参照記号:ピン`)/ネットラベル/空。ネットラベルに届いた端はネットを1対1で辿れるときだけデバイスへ解決し(G18)、辿れなければラベル名のまま備考に理由が立つ(`multi`=他のピンが複数あって絞れない / `none`=辿る先のデバイスが無い)。嵌合相手列は宣言済みコネクタの相手参照記号。CSV / 図面シート |
| 嵌合コネクタ対 | ✅ | `attrs["mates_with"]`(コネクタ一族`connector_*`のみ、G14)。ERC: 極数不一致`erc.mate_pole_mismatch`(Warning)・片側宣言`erc.mate_one_sided`(Info)・自己参照/不明参照記号`erc.mate_unknown`(Warning、いずれもプロジェクト全体で評価)。帳票の嵌合相手列+同一シート50mm以内なら図面に破線を自動描画(SVG `render_mate_links`+キャンバス双子`mate.ts`)。動的コネクタ系統への性別欄は未実装 |
| 端子台チャート | ✅ | 端子台1台=1表。端子番号順に端子・**Lv(段)**・内部側/外部側・線番・電線・ハーネス・ジャンパ・予備端子。Lv列は多段端子台(G6)の段記号、単段は空欄。**Lv列は端子列の直後に新設で、CSVは旧6列とは非互換**(端子台1台分は7列、プロジェクト全体版は8列。フェーズ6積み残し、列位置で読むスクリプトは直す必要がある)。CSV / 図面シート(`--terminal`で1台に絞れる) |
| 端子接続図 | ✅ | EPLAN端子図様式(外部=左/内部=右・ハーネスブラケット・予備・ジャンパ)。**千鳥配置**(行ごとに1箱を上から順に積み、外部左・内部右の向きは崩れない)で多段端子台は端子ストリップを段の列に分け(列の上に段記号)、箱に段記号付きの端子番号(`1a`等)を表示。サドルジャンパは**段ごとの配線レーン**を使う: 同じ極の段違いを繋ぐジャンパ(必ず隣接行)は箱のすぐ外の共通レーン、同じ段の隣り合う極を繋ぐジャンパ(間に他の段の行を挟む)は段ごとに割り当てた専用レーン(全ての行の引出線より外側)を通るので、他の段の行を横切らない。1端子台=1ページ(端子15個で分割)。PDFのみ |
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
| ERC | ✅ | 未接続ピン / 空参照 / 参照記号重複(リレー除外) / 宙ぶらりんワイヤ(ジャンクション忘れ検出) / ネットラベル競合 / 接点構成の超過`contact_overflow`(Error) / 親コイル無しの接点`orphan_contact`(Error) / 接点無しコイル`coil_without_contact`(Warning) / 読めない接点構成`invalid_contact_config`(Warning) / 用紙外エンティティ`erc.outside_paper`(Warning、G15) / 嵌合対の極数不一致`erc.mate_pole_mismatch`・片側宣言`erc.mate_one_sided`・自己参照/不明参照記号`erc.mate_unknown`(G14)。接点はa/b/c接点の3種を数え、**c接点1極はa接点にもb接点にも充当できる**(`2CO`の部品にa接点1+b接点1を描いても超過にならない) |
| 電気検証 | ✅ | 電源到達性 / 線径許容電流(sq表) / 電圧降下(電源電圧の3%) / ヒューズ定格。**ngspiceのDC動作点解析がバックエンド**、未導入時はグラフ近似+Info明示 |
| SPICEネットリスト生成 | ✅ | 接続点ノード・ワイヤ抵抗(ρL/A、途中接続で分割按分)・導通橋・負荷等価抵抗・ラベル橋 |
| ngspiceランナー | ✅ | OS非依存(env→PATH→OS既定パス探索、サブプロセス実行) |
| DCシミュレーション | ✅ | ネット電圧(min/max)・部品電流/電力、スイッチ/接点の開路what-if。シンボル定義の `sim`(役割: 電源・負荷=定電流・ヒューズ・導通・DC/DC変換器)でユーザーシンボルも解ける・複数電源。条件: 属性の置き換え(`--set REF.ATTR=VALUE`)・ネットの地絡(`--short NET`)。定格超過 `findings`(ヒューズ・ピン・電源・変換器・負荷の端子電圧・電線)。結果パネルに定格超過の列 (重い順・件数バッジ・行クリックで選択+ズーム) と条件バー (置き換え・短絡のチップと入力) |
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
| BOMとの連動 | ✅ | 部品表の型番を部品DBで引き当て、メーカ・品名・単価・通貨・金額・購入先の列と通貨ごとの合計行を出す(2026-10-15) |

## 6. インポート/エクスポート

| 機能 | 状態 | 備考 |
|---|---|---|
| KiCadインポート(.kicad_sch) | ✅ | 自前S式パーサ。用紙・表題欄・配線・ジャンクション・ラベル・テキスト・主要シンボル(マッピング表+Conn/Screw_Terminalのピン数解釈)・電源シンボル→ネットラベル化。未対応はスキップ報告(ImportReport) |
| KiCadインポートの制限 | ⚠ | ピン形状の違いで接続が崩れ得る(ERCで洗い出す運用)。階層シート・バス未対応 |
| KiCadエクスポート(.kicad_sch) | ✅ | `kicad.rs::export_kicad_sch`。KiCad 9形式。使用シンボルの図形・ピンを`MadakeCAD:<id>`として`lib_symbols`へ埋め込む(KiCad側にライブラリ不要。機器ブロックは形ごとに`MadakeCAD:block_<hex>`)。線番はネットのラベル、ハーネスは破線多角形+名前。書き出し→読み戻しで配置が保たれる(インポーターは`MadakeCAD:`接頭辞を直接解決し、`block_<hex>`はブロックに戻す) |
| DXFエクスポート(ACADE/EPLAN向け) | ✅ | `dxf.rs::sheet_to_dxf`。AutoCAD 2000形式(AC1015)・mm・`\U+XXXX`。配線=`WIRES`レイヤのLINE、線番=`WIRENO`、シンボル=`MDK_<id>`ブロック(機器ブロックは形ごとに`MDK_block_<hex>`)+属性(TAG1/CAT/DESC1/RATING1/TERMnn)、ジャンクション=`WDDOT`、ラベル=`LABELS`、注記=`MISC`、ハーネス=`HARNESS`(破線)。ACADE/EPLANはDXFをネイティブに読める(`.dwg`/`.elk`は非公開形式のため直接対応しない。仕様: `specs/interop.md`) |
| DXFインポート | ✅ | `dxf.rs::import_dxf`。配線レイヤ(名前に`WIRE`、または`wire_layers`指定)の線分→配線、`WIRENO`文字→最寄り配線の線番、`MDK_<id>`ブロック→シンボル(`MDK_block_<hex>`はブロックに戻す)、`WDDOT`/小円→ジャンクション、インチ→mm換算、外形が収まる最小A判。ACADE/EPLAN固有のブロック名は対応表が無いためスキップ報告(未決事項) |

## 7. AI・自動化・外部連携

| 機能 | 状態 | 備考 |
|---|---|---|
| 内蔵MCPサーバー | ✅ | 127.0.0.1:9310/mcp。ツール49種: get_project / list_symbols / place_symbol / draw_wire / execute_commands(set_revisions・set_sheet_annotation_text・set_sheet_paper・set_project_meta・renumber_wires・set_wire_numbers・harness追加・ジャンパ(update_entityのattrs)もここから) / get_netlist / run_verification / get_tidy_metrics / simulate_op / search_parts / upsert_part / delete_part / import_kicad / import_dxf / list_terminal_blocks / get_terminal_chart / check_terminal_block / list_templates / apply_template / list_macros / save_macro / apply_macro / duplicate_symbol / save_user_symbol / delete_user_symbol / export_svg・pdf・dxf・kicad・report・pdf_book・bom・wire_list / undo / redo。**検索・デバイスツリーはMCP未露出**(IPC+Link APIのみ。AIはget_projectで足りるため意図的) |
| Link API (/api/v1) | ✅ | REST+SSEパッチ。project / symbols(+/user・/user/save・/user/delete・/duplicate) / frame-templates / panel/unplaced / netlist / verify / tidy-metrics / search / devices / simulate/op / commands / undo / redo / save / load / import/kicad / import/dxf / terminals(+/chart・/check) / templates(+/apply) / variants(+/start・/finish) / macros(+/build・/save・/apply・/apply-inline) / export/*(svg・pdf・dxf・kicad・pdf-book・report・bom・wire-list) / parts / wire-parts / agent/*(send・cancel・conversations・undo-turn・detect・provider・api-key(PUT/DELETE)・test-connection・events) / settings / events |
| madake CLI | ✅ | status / project / netlist / verify / sim / parts / terminals / export(svg・pdf・dxf・kicad・pdf-book+帳票6種を`--format csv\|pdf`・`--terminal`付きで) / save / open(.kicad_sch・.dxf対応、`--wire-layer`) / renumber / exec / undo / redo。マクロ・検索のサブコマンドは未(Link APIを`exec`/curlで直接叩ける) |
| AIチャット(A1) | ✅ | 左ドック+浮きカード、送信は⌘Enter/Ctrl+Enter(Enterは改行。IME確定で誤送信しない)、応答中の割り込み送信(進行中ターンを中断→同じ会話へ再送)、Claude Code CLIバックエンド(Pro/Max OAuth再利用)、ツールチップ表示、ターン単位undo、編集オーバーレイ(シアンパルス)、会話履歴のプロジェクト保存。**添付**(2026-10-18): 画像・PDF(1件20MB・5件)を+メニュー・ドラッグ&ドロップ・⌘Vで。`madake_agent::attachments`(検証・プロバイダ別の渡し方)、Claude CLIは`--input-format stream-json`、Copilot CLIは一時フォルダ+`--add-dir`、OpenAI互換はPDF不可。会話には`AttachmentMeta`(名前・種類・大きさ)だけ保存。**表計算**(2026-10-20): Excel(.xlsx)・UTF-8のCSVも添付でき、`spreadsheet_text`(calamine)がシート見出し+タブ区切りのテキストにして全プロバイダへ本文の前のテキストで渡す(Copilot CLIは本文へ直接)。**作業過程のデータ**(2026-10-20、段階1 PR A): 応答を起きた順の`blocks`(本文・思考・ツール)で残し、ツールの結果を先頭8KBまで`ToolCall.output`に保存。思考はClaude CLI・Anthropic API・OpenAI互換のreasoningから。画面(段階1 PR B): 思考・ツールのまとまりを折りたたみ表示 (応答中は開き、終わると閉じる)、手順の入力・結果とコピー、返答のMarkdown (markdown-it 15.0.2、生HTML無効・http(s)リンクだけOSのブラウザで・画像はリンク) |
| A1の持ち越し負債(M3フェーズ1で解消) | ✅ | ターン安定ID(`turn_id`。chat.json format_version 2へ移行)・編集origin(`Engine::execute_as` / `revert_range`でagent編集だけを逆適用。衝突は`RevertConflict`)・キャンセルseq(`turn_seq`で遅延イベントを破棄) |
| 規格知識+検証ループ(M3フェーズ1) | ✅ | 同梱`resources/knowledge/standards.md`を毎ターン注入(設定`knowledge_path`で追記可、後勝ち)。図面コンテキストに検証サマリ。プロンプトで「編集後は`run_verification`→修正→再検証(最大3回)→件数報告」を必須化 |
| ナレッジ回答・AIレビュー・部品選定(M3フェーズ1) | ✅ | `docs/`目次+`--add-dir`+読み取り専用ツール(Read/Glob/Grep)で出典付き回答(未対応機能はroadmapで回答)。レビューは決定的検証+5観点チェックリストを重要度表に。検証計画は手順表。部品選定は`search_parts`の比較表(最大10件) |
| 開始テンプレート(M3フェーズ1) | ✅ | 同梱3種(24V制御基本・モータ起動回路・非常停止回路。適用後ERCエラー/警告0)。`Engine::execute_batch`で履歴1エントリ=undo一発。IPC/Link API/MCPツール+リボン導線。`~/MadakeCAD/templates/*.json`でユーザーテンプレート |
| 自動反復=整えループ(M3フェーズ2) | ✅ | `madake-core::tidy`の決定的メトリクス(交差数・ラベル重なり・シンボル重なり・グリッド外)をMCP`get_tidy_metrics`/Link API`/tidy-metrics`で露出。チャット入力欄の杖ボタン→ポップアップ(配置整理/配線整理/ラベル整頓)が「測る→直す→測り直す(改善が止まる or 最大3回)」の定型プロンプトを**1ターン**として送る=undo一発。`knowledge.rs`のWORKFLOW_RULESにも同じループを記載(自然文依頼でも回る)。バリアント数(2〜4案)はフェーズ4で実装(下記) |
| 並列エージェント(M3フェーズ2) | ✅ | 会話ごとに`tokio::spawn`で同時実行(Busy判定は会話単位)。フロントの送信ガードは**開いている会話**だけに効く(`chat.streaming` / `anyStreaming` / `runningIds`)。会話色=`theme.agentPalette`(#29D3E6 / #FFB454 / #B48CFF / #FF6FD8)を開始順に割当、編集オーバーレイ・会話一覧のドット/スピナー・タブ行の「N running」バッジで共有。**巻き戻し粒度**: 並行ターンの編集が混ざった区間は両方まとめて戻る(`mark_swept_turns`で巻き込まれたターンも巻き戻し済みに)。比較案UX(シート複製)はフェーズ4で実装(下記) |
| 整えバリアント+比較パネル(M3フェーズ4) | ✅ | `madake-core::variants`(複製=`RestoreSheet`の`execute_batch`、採用=元idを保つ`UpdateEntity`/`DeleteEntities`/`AddEntity`+全複製`RemoveSheet`の1バッチ、破棄=`RemoveSheet`バッチ。いずれもundo一発)。Link API `/variants/start`・`/variants/finish`、Tauri `start_variants`/`finish_variants`/`get_tidy_metrics`。フロント`stores/variants.ts`が案ごとに新会話で`tidyPrompt`(範囲=複製、選択は対応表で写す)を並列送信し、`VariantsPanel.vue`(下部ドックパネル)に状態・指標・表示・採用を並べる。整えポップアップのバリアント数チップ(2/3/4)を有効化。`.pen`ボードへの反映は未 |
| Anthropic APIプロバイダ+キーチェーン(M3フェーズ2) | ✅ | `AgentBackend`トレイト(`run_turn`1本)にCLI/API両実装。`AnthropicApiBackend`=Messages API直(SSE、ツール往復は上限16、`MADAKE_ANTHROPIC_BASE_URL`で接続先差し替え可)。ツールは`ToolBridge`が内蔵MCPサーバーをプロセス内パイプで呼ぶので**CLI経路とAPI経路で同一**。キーは`keyring 4.1.6`でOSキーチェーン(`MadakeCAD`/`anthropic_api_key`)。設定ファイルには項目自体を作らない。設定UIにプロバイダ選択・伏せ字のキー入力/保存/削除・モデル欄・接続テスト。**実キーでの通し確認はユーザー確認事項**(実キー無しの範囲=UI・401・キー未設定案内・平文非保存は確認済み) |
| Copilot CLI / OpenAI互換 / Ollama / Geminiプロバイダ(M3フェーズ3) | ✅ | 6経路そろう。`CopilotCliBackend`=`copilot -p … --output-format json`(JSONL)を実行。MCPは`--additional-mcp-config @一時ファイル`でセッション限定に渡し`--disable-builtin-mcps`。システムプロンプト用フラグが無いため`# システム指示`として前置(`--no-custom-instructions`)。**GitHubトークンは保存しない**(CLI側の`/login`)。`OpenAiCompatBackend`=`POST {base_url}/chat/completions`(SSE+`stream_options.include_usage`、`tool_calls`を`index`ごとに連結、失敗は`ERROR: `前置で返す)。設定は`openai_base_url`+`openai_model`(既定なし=必須入力)。**接続先がローカル(localhost/127.0.0.1/*.local等)ならAuthorizationヘッダごと省略**=Ollamaはキー不要。`GeminiBackend`=`models/{model}:streamGenerateContent?alt=sse`(キーは`x-goog-api-key`ヘッダのみ。`functionResponse`はrole`"user"`で返す。`usageMetadata`は累計置換)。ツールスキーマはOpenAPI部分集合へ削る`gemini_schema()`(`$schema`/`$ref`/`additionalProperties`等を送ると400)。ツール往復は各経路とも上限16 |
| プロバイダの実機確認状況 | 🔶 | **Ollama(0.32.14)は通し確認済み**(実チャンク形一致・キー無しで接続テスト成功・`gemma4:26b`で作図1ターン=ツール往復3回→revision 0→1→`undo-turn`で後始末)。**Copilot CLI 1.0.80も通し確認済み**(2026-08-23、`/login`済み。JSONL実イベント形=`{type,data}`封筒+封筒無しの最終行`result`をパーサへ反映し、作図1ターン→継続ターンで文脈保持→`undo-turn`で後始末)。Geminiは無効キーで実APIの`API_KEY_INVALID`→`gemini_auth`変換まで。Anthropic APIは401・キー未設定案内まで。**Gemini・Anthropicの実キー通しはユーザー確認事項** |
| キーチェーン(全プロバイダ共通) | ✅ | `keyring 4.1.6`直用。`service="MadakeCAD"`、`account`=`anthropic_api_key`/`openai_compat_api_key`/`gemini_api_key`。設定ファイル(`~/.madakecad/settings.json`)には項目自体を作らない(平文非保存はテストで固定)。`SecretStore`トレイトでテストはメモリ保管、実キーチェーンのテストは`MADAKE_KEYCHAIN_TESTS=1`のときだけ。macOSは許可ダイアログ抑制のためプロセスに1回だけ読み出しをキャッシュ。CLI経路(Claude Code / Copilot)は資格情報を一切持たない |
| プロバイダ接続テスト | ✅ | Link API `POST /api/v1/agent/test-connection` + `GET /api/v1/agent/provider`(モデル名・キー保存済みフラグ)。CLI経路は検出+バージョン、API経路は実エンドポイントへの疎通。失敗は種類つき(`*_auth`/`*_rate_limit`/`*_model_not_found`/`*_server`/`*_request`/`*_network`/`*_no_key`/`*_no_model`)で日本語の案内文を返す |
| FreeCAD連携(M5) | ✅ | Link API(M1の土台)は実装済み。**M5-1 アドオンWB骨格を実装(2026-09-27)**: `freecad-addon/`(package.xml・InitGui.py・`madakecad_link/`)。接続設定(ポートはFreeCADユーザーパラメータ)・プロジェクト概要・ネットリストビュー・SSEライブ追従。純Python部分(client/model/events/settings/linking/routes)は38テストでCI(`freecad`ジョブ)。実FreeCADでの目視確認はユーザー確認事項。**M5-2も実装(2026-09-27)**: `Project.mech_links`+`Wire.length_source`(format_version 12)、Command `set_mech_link`/`remove_mech_link`/`set_wire_lengths`、アドオンのParts/Wiresタブ(3Dモデル挿入・対応付け・経路計測→書き戻し)、プロパティの「FreeCAD計測」バッジと手上書き警告。**M5-3も実装(2026-09-27)**: `MechLink.placement`(format_version 12)、アドオンの経路スタブ生成(ネット内の対応付け済み部品を名前順の鎖で結び配線へ順に割当、冪等)・ネット行選択→3Dビュー選択・配置同期(`set_mech_link`)、プロパティの「3D対応付け」行に配置表示。2D盤レイアウトシートはM4フェーズ4で実装済み(M4 §8) |

## 8. ドメイン機能(参考図面の再現に必要な残り)

| 機能 | 状態 | 備考 |
|---|---|---|
| 端子台・コネクタ(ピン番号単位の結線) | ✅ | 動的シンボル+貫通端子+ネットリスト |
| 端子台エディタ(グリッド) | ✅ | **部品割当**(部品DB v7の`terminal_kind`。`value`+`attrs["terminal_kind"]`をupdate_entity 1回)・**付属品**(`attrs["accessories"]`。チャートに付属品行、BOMに型番ごとの数量行、読めない項はWarning)・行の導出表示・サドルジャンパ生成/削除(`attrs["jumpers"]`をupdate_entity。undo可)・**並べ替え**(`attrs["row_order"]`=数値順/英数順/ページ順/外部ケーブル順/ジャンパ順。update_entity 1回でundo可、`sort_terminal_rows`がチャートCSV・図枠付き図面シート・接続図とも同じ並びにする)・**Lv列表示**(多段端子台だけに出る)・**図面へジャンプ**(ツールバー+行ダブルクリック。検索結果と同じreveal動線)・**端子番号の自動採番**(`attrs["terminal_numbers"]`。新Command`RenumberTerminals`=開始番号・増分・接頭辞・`SetTerminalNumbers`=直接指定/逆コマンド。個数不一致・重複・空要素は図面を変えずに拒否、同じコマンドで`jumpers`・`pin_groups`の端子番号も追従、ツールバー「採番」ポップオーバー)・**既存端子台への多段の生成/解除**(`symbol_id`を`terminal_block_{n}p` <-> `terminal_block_{n}p_{levels}l`へ書き換える新Command`SetTerminalLevels`=段数1〜3。同じコマンドで`terminal_numbers`・`jumpers`・`pin_groups`・`accessories`の段記号も追従。段を増やすときは極ごとに段aのラベルを土台に段記号を付け足し、既存の段記号なしジャンパは段aへ移行。**段ピッチの半分だけ`at`も平行移動して段aの端子を用紙上の同じ座標に留める**(回転・反転を通した向きで動かすので、既存の配線が別の段へ繋ぎ変わらない)。段を減らして消える段に結線 (座標一致で判定=配線・ネットラベル・ジャンクション・他シンボルのピン)・ジャンパ・ピングループ・付属品指定が残っていれば図面を変えずに拒否、逆コマンドは変更前の段数へ戻す同じコマンド(`symbol_id`と`at`がそろって戻る)。ツールバー「多段生成」→段数ステッパー+プレビュー+「解除で消える段」の先読み(該当すれば適用ボタンを無効化)のポップオーバー)・端子台チェック・チャート/接続図の生成起点。多段端子台(G6)はモデル・帳票側では対応済み(段数は部品挿入時にも選べる)。**予備端子の挿入**(`InsertSpareTerminals`。繰り下げ/空き番号のまま残すの2モード)・**内外スワップ**(`attrs["swapped"]`=端子番号のカンマ列。チャートと端子接続図の内部側・外部側だけを入れ替え、結線は変えない。端子台に無い端子番号はWarning)を含め、M4フェーズ5で完了(2026-09-28)。エディタを開いたまま⌘Z/⇧⌘Zで戻せる |
| 電線管理(色・sq・長さ・品番) | ✅ | Wire属性+電線品番マスタ |
| 線番(ワイヤ番号)の挿入・自動採番 | ✅ | ネット単位。`renumber_wires`(追い番/振り直し・開始番号・シート指定/全体)+`set_wire_numbers`(個別編集)。キャンバス/SVG/PDF描画、電線リスト連動、CLI `madake renumber` |
| ハーネス境界(破線囲み) | ✅ | Entity `Harness`(矩形・名前・備考)。所属は全点内包(入れ子は最小優先)、破線描画+名前、電線リストのハーネス列、リボンのハーネスツール |
| 複数母線・信号矢印 | ⬜ | リボンにボタンのみ(todo) |
| 回路マクロ(保存・挿入・バリアント) | ✅ | `macros.rs`。保存=選択範囲をCommand列へ逆変換+相対座標化(基準点=左下ピン)、`~/MadakeCAD/macros/*.json`。挿入=UUID振り直し+参照記号の再採番(既存最大+1、マクロ内の関係は保持)+線番クリア、`execute_batch`でundo一発。バリアントは列挙・選択まで(`A`以外の追加UIはフェーズ4)。**値セット**=`placeholders`(key・ラベル・行き先`{entity, field}`。fieldは`value`か`attrs.<名前>`)+`value_sets`(id・ラベル・`values`)。どちらも`#[serde(default)]`で旧マクロJSONもそのまま読める。挿入は行き先を全部解決してから書き込むので`execute_batch`1回のまま(undo一発)、不正な値セット・行き先は図面を変えずにエラー |
| 回路コピー・ペースト | ✅ | `⌘C`/`⌘V`。無名マクロとして組み立て(`build_macro`)→そのまま挿入(`apply_macro_inline`)。ファイルには書かない |
| 回路トリム | ⬜ | リボンにボタンのみ(todo) |
| コイル⇔接点クロスリファレンス | ✅ | `relay_xref.rs`。同一参照記号のコイル+接点群=1デバイス。接点はシート番号→ゾーン→idの読み順に並び、その連番が端子対(IEC 60947-1。コイル=A1-A2)。コイル外形下端+2.5mmに接点マップ(端子対\|所在。`contact_config`が読めるとき未使用は「—」)、接点の外形右端+1mmにコイル所在。SVGとキャンバスは`contact_map_layout`/`contactMapLayout`の同一数式。**G22**: コイル所在は右→上→下の順で、そのシートの配線注記(`wire_note::wire_notes`)と重ならない側を`coil_location_at`/`coilLocationAt`が選ぶ(CONTACT_MAP_FONT高さの近似矩形で重なり判定) |
| リレー実機ブロック表示(G7) | ✅ | `relay_xref.rs`の`DeviceBlock`/`device_block_layout`+`svg.rs`の`render_device_block`+`canvas/relayXref.ts`の`DeviceBlock`/`deviceBlockLayout`+`renderer.ts`の`drawDeviceBlock`。コイルの`attrs["device_block"]=="true"`(`is_device_block`)で分散配置(既定)と切替。外形は参照記号+型番(型番が空ならヘッダ1行)のヘッダと、接点マップと同じ並びの接点イメージを1つにまとめる。外形の幅はA1/A2ピン間隔(15mm)を下回らず、内容で広がった分は`lead_half_len`/`leadHalfLen`ぶんの水平な引き出し線でピンへつなぐ(ピン座標自体は不変なので配線は切れない)。分散配置のコイルの接点マップ(`sheet_contact_maps`)からはdevice_blockのコイルを除外(二重描画防止)。プロパティパネルの「実機ブロックで表示」チェックボックスが`update_entity`で`attrs`を書き換える |
| プロジェクト内検索 | ✅ | `search.rs`。対象5種=参照記号・型番(value+attrs)・ネット名・線番・テキスト。部分一致・大文字小文字無視・空クエリは0件。並びはシート番号→ゾーン→entity id→種別で決定的。シンボルのヒットにはデバイス内の機能(コイル/接点13-14…)が付く |
| デバイスツリー(ナビゲータのデータ) | ✅ | `search.rs`。参照記号ごとに機能を列挙(リレー=コイル+接点(端子対は接点マップと同じ連番。c接点は共通・b側・a側の3端子「11-12-14」)、端子台=端子群1行、その他=本体) |
| シート間クロスリファレンス | ✅ | `extract_netlist_project`が同名ラベルでシート横断統合。ラベル脇に「/シート.ゾーン」(キャンバス/SVG/PDF)、プロパティパネルの相手先リンク、ERC(`verify_project`)・帳票も統合ネットで評価 |
| デバイス単位シート間参照(G13) | ✅ | `xref.rs`の`device_sites`/`sheet_device_xrefs`/`device_xref_at`+`canvas/xref.ts`の同名関数。ネットラベルのシート間クロスリファレンスを参照記号一般へ拡張したもの: 2シート以上に跨る参照記号のインスタンス全てへ相手シートの住所(`XREF_SEPARATOR`=半角スペース区切りで複数列挙)を自動で添える。コイル・接点(`relay_xref::relay_role`が`Some`のシンボル)は対象外(コイル⇔接点クロスリファレンスが既に注記するため二重に描かない)。表示クラスは既存の「参照記号(refs)」に含め、新しい表示クラスは追加していない。位置はシンボル外形の右+1mm、参照記号ラベルと同じ高さ(`REF_LABEL_DY`) |
| PLC I/O(割付表・図面生成・レポート) | ✅ | `plc.rs`。モジュール定義=部品DBの`plc_module`(点数・DI/DO・アドレス接頭辞・体系)。アドレス採番は三菱=8進/Siemens=バイト.ビット/AB=ワード/ビット(開始点をずらせる)。割付表=`Project.plc_assignments`(**format_version 2**。旧ファイルは空の表で開く)、編集は`set_plc_assignments`1回。CSVは往復用3列と読み取り専用レポート6列の2種。図面生成=`RestoreSheet`+必要なら`set_plc_assignments`を`execute_batch`で**undo一発**(縦母線`P24`+1点=1ラング、生成ページはERC指摘ゼロ)。接続先・線番は図面から読むだけ(全シート合算)。露出=IPC 6本 / Link API 4本 / MCPツール3種 / CLIの`plc-io`帳票 / 割付表エディタ+生成設定ダイアログ |
| PLC I/O(横母線・モジュール同居・ページ分割) | ✅ | 横バス+縦ラング(モジュール横倒し・接続点が上・見出し2段千鳥)、収まる場合のみ同居/同居し分割(分割の単位4/8/16点、分割片`plc_di_{n}p_s{start}`、続き参照)、生成設定のプロジェクト保存(`Project.plc_generate`・format 14・`SetPlcSheetOptions`)。2026-10-15 |
| シンボルエディタ(ユーザー定義シンボル) | ✅ | `symbol_library.rs`(ユーザーライブラリの読み書き・列挙・保存時検証・複製開始)+`canvas/symbolEditor.ts`(画面側。**検証はコアと同じ判断・同じコード**)。保存先`~/MadakeCAD/symbols/{id}.json`、分類は`user`へ正規化、同梱・動的シンボルとのid衝突は拒否。検証=id/名称/接頭辞/図形0個・潰れた図形/接続点0個・番号空・2.5mmグリッド外・同じ点の重複/属性スロット(TAG必須)。落ちたら**何も書かない**。図面はスナップショットを持たないので保存し直すと配置済みの部品も追従(使用中の削除は`force`なしでは拒否)。露出=IPC 6本 / Link API 5本 / MCPツール3種。**残り**: 回転バリアントの手動上書き |
| 2D盤レイアウト | ✅ | `panel.rs`(数式の正)+`canvas/panel.ts`(同じ式)。シート種別`panel`、盤枠(外形+取付板10mm内側)・DINレール(取付線=帯の中心、35mm帯)・ダクト(用紙座標に固定した45°ハッチ)・フットプリント・凡例表。未配置部品=回路図の参照記号ごとに1台(どの盤シートにもフットプリントが無いもの)。バルーン採番`renumber_panel_items`(読み順=部品の中心の高さで段分け→左から右。逆コマンド=`set_panel_item_numbers`)。検査=`panel.overlap`(面で重なる対。辺が触れるだけは報告しない)/`panel.off_rail`(レール0本の盤では報告しない)/`panel.rail_fill`(80%超は注意)/`panel.orphan_footprint`。盤シートは`verify_sheet`が即空を返す(配線が無いため)。露出=IPC`unplaced_components` / Link API`GET /panel/unplaced` / MCPは`execute_commands`の説明に書き方を追記 |
| 盤内配線図(wire annotation)・銘板・寸法線・整列/等間隔 | ✅ | `panel.rs`(`nameplate_rect`/`nameplate_lines`/`dimension_geometry`/`align_moves`/`distribute_moves`)+`panel_wiring.rs`+`canvas/panelMarks.ts`(同じ式)。銘板=`Entity::Nameplate`(フットプリントの子・上下・説明追従・型番は部品表へ)、寸法線=`Entity::Dimension`(端点=点かフットプリントの中心)、整列/等間隔=`AlignFootprints`/`DistributeFootprints`(undo一発・レール上の部品は上下に動かさない)、盤内配線図=帳票`panel-wiring`(CSV 8列/図面ページ・CLI`panel-wiring`)。format 16。2026-10-15 |
| 計測ツール | ⬜ | 未実装 |

## 9. 基盤・配布

| 機能 | 状態 | 備考 |
|---|---|---|
| テスト | ✅ | cargo 1003件+vitest 734件+vue-tsc。全テストに対訳仕様文が付き、`docs/13-specification.md`(1737項目)を自動生成。TDD運用 |
| macOSビルド | ✅ | 開発は`npm run tauri dev` |
| Windows/Linuxビルド | ✅ | CI(`ci.yml`): `rust-cross`がmacOS(4クレート)とWindows(`madake-core`/`madake-cli`)でcargo test、`frontend`(vitest+vue-tsc)と`freecad`(Python)は3OSマトリクス、`bundle`が3OSで`tauri build`(未署名・成果物は保存しない)。`spec`(gen_spec --check)は生成結果がOSに依らないのでUbuntuのみ。OS別のビルド依存は`docs/02-getting-started.md`。ngspice探索・PDFフォントはOS別に解決(M6) |
| 配布パッケージ/自動更新 | 🔶 | `release.yml`(GitHubのリリース公開`release: published`/手動=既存リリースのタグ指定)がtauri-actionでmacOS `.dmg`(arm64/x86_64)・Windows `.msi`/NSIS・Linux `.AppImage`/`.deb`/`.rpm`をビルドし、そのリリースへ添付(`releaseId`)。バージョンはタグ`vX.Y.Z`(数字のみ。MSIの制約)をランナー上の`tauri.conf.json`へ書き込む。**アプリ内アップデート**(2026-10-16): tauri-plugin-updater(確認先=最新リリースの`latest.json`、署名はリポジトリのシークレット`TAURI_SIGNING_PRIVATE_KEY`、公開鍵は`tauri.conf.json`)+`src-tauri/src/updates.rs`(開発ビルド=確認しない/Linuxの.deb・.rpm=知らせるだけ)+`stores/updates.ts`(起動時の確認・今すぐ確認・未保存確認つき更新と再起動・後で)+設定「アップデート」欄+`UpdateNotice.vue`。コード署名・notarizationは公開後 |
| 自動保存・クラッシュ復旧 | ⬜ | 未着手 |
| i18n | ✅ | 6言語(en/ja/zh/es/fr/de)のカタログ+言語設定(即時反映)。全カタログのキー・プレースホルダ・複数形の一致をテストで強制。UI文字列はカタログ必須、翻訳者向け手順は`specs/i18n.md` §6。**翻訳率はen/jaが100%、zh/es/fr/deは約6割**(残りは英語のまま。テストでは区別できない)(M6) |
| コミュニティ整備 | ✅ | `CONTRIBUTING.md`(+ja)・`CODE_OF_CONDUCT.md`・`SECURITY.md`・Issue/PRテンプレート・`scripts/audit_confidential.py`(社外秘用語の履歴監査)(M6) |
| 公開手順書 | ✅ | `internal/release-runbook.md`: 機密の最終確認(唯一のゲート)→公開方法のA/B案→可視性・トピック・脆弱性報告窓口→タグと配布物→公開後の初動。**不可逆操作なので実行はしていない**(M6) |

## 10. 次期計画

**M4は2026-08-23のフェーズ4完了をもって完了。フェーズ5(参考図面の再現パリティ)も同日完了**
(再現検証で必須と判定されたG1〜G4+基盤のG9/G17を解消。判定は`specs/reproduction-gaps.md`)。 M4から先送りした項目(フェーズ未設定): ナビゲータからの
~~予約接点配置~~(2026-10-15実装) / ~~`A`以外のマクロバリアント追加UI~~(2026-10-15実装) / ~~PLCの横母線ラダー・モジュール同居・ページ分割~~(2026-10-15実装) /
~~ケーブル一覧~~(2026-10-15実装) / ~~部品DB連動BOM~~(2026-10-15実装) / ~~盤内配線図・銘板・寸法線・整列/等間隔~~(2026-10-15実装) /
~~グラフィカルな図枠エディタ~~(2026-10-15実装)。**フェーズ6(可読性ギャップ)も2026-08-23に完了**: G19 タグの基準点・G20 注記文字高の
シート設定・G21 c接点のクロスリファレンスを解消し、再現図面のシート1は文字の重なり0件・
ERC 2件(どちらも実在の指摘)になった。**フェーズ6タスク4(2026-09-22)**: G8 ピングループ括弧・
G12 同梱絵記号(拡声器・ブザー・サイレン・送風機。ブザーとサイレンはJIS C 0617-8:2011 第10節を出典とする——IECの識別番号が第2の出典で裏取りできなかったため)・G20残(注記の引き出し線)を追加。**フェーズ6タスク5(2026-09-22)**: 軽微バッチ — G14 嵌合コネクタ対(`mates_with`+ERC+帳票+作図。性別欄は今後)・G15 用紙変更コマンド・G16 プロジェクト情報コマンド(`Project.no`追加、format_version 10)・G18 From-To帳票のネットラベル辿りを追加。**フェーズ6タスク6(2026-09-27・フェーズ6完了)**: G9残(電線品番凡例ブロック`wire_legend`エンティティ+BOMの電線集約行、format_version 11)を追加し、整えメトリクスのラベル重なりへピングループキャプション(G8)を算入。**第4回の再現(2026-09-27)**: フェーズ6の全機能を適用して再現図面を組み直し、267コマンド / 文字の重なり0件(既定2.0mmなら12件)/ シンボル外形の重なり0件 / ERC+電気検証5件(うち3件は無結線のデモ機器)/ 電線リストの品番111/111行 / ハーネス欄0/111行(G11を図形で描き直し)/ BOMに電線集約2行 / ブック19ページ。この実測で見つかった2件の不具合(凡例の使用長が丸められず浮動小数のまま印字される・リボンの新ボタンが列からあふれて押せない)をテスト先行で修正し、メトリクスが配線注記と描画時外形を見ていない件をG23として台帳に追加(詳細は`specs/reproduction-gaps.md`)。

本節の旧A〜D方向案は、ビジョン明確化(2026-08-21)によりマイルストーンM2〜M6へ再編された。
計画の正は[要件定義書](requirements.md) §2 と [機能仕様集](specs/README.md)(競合比較は[ギャップ分析](specs/gap-analysis.ja.md))。

**図枠テンプレートの拡張(2026-11-14)**: A0参考図面の再現(領域1)で見つけたG24〜G31を解消 — 辺ごとの余白・ゾーン目盛の非表示・裁断マークと中心マーク・罫線の太さ・セルの回転/枠/幅への収め/複数行・大文字高さの文字サイズ・改訂欄の列/見出し/行高/固定行数/三角の記号・表題欄の自由な欄(`{field.名前}`)・ロゴ画像(format_version 20)。SVG・PDF・キャンバスが同じ規則で描く。図枠エディタでの編集UIは未実装(新しい項目を保持するだけ)。再現の実測: 領域1の重ね比較で再現側の被覆率 0.51 → 0.95(詳細は`specs/reproduction-gaps.md`の第5回)。

**ストロークフォント(2026-11-15)**: プロジェクトの書体に同梱のCADストロークフォントKST32B(JIS第1・第2水準、約7200字)を選べる(`set_text_font`コマンド、`Project.text_font`、format_version 21)。選ぶと図面の全ての文字をSVG・PDF・キャンバスが線で描く。線の太さは文字サイズに比例し、太字は1.5倍。字形の無い文字を含む文字列は、その文字列ごとシステムの書体へ戻る。文字幅の見積もり(`tidy::text_width` / `textWidthMm`)は変えないので、書体を選んでもピン・シンボル・配線は動かない。英数字は半角か全角(`latin`)、図枠テンプレートは図枠の文字だけ別に指定でき(`text_latin`)、セルごとに字間(`spacing`)を持つ。字形は`GET /api/v1/stroke-fonts/{id}`とIPC `get_stroke_font`でフロントへ渡す。書体を選ぶUIは未実装(コマンドとLink APIから設定する)。実装は`madake-core/src/stroke_font.rs`と`src/canvas/strokeFont.ts`。

**ブロックの細かいピン間隔(2026-11-15)**: ブロックシンボルは`attrs.block_pitch = "2.5"`でピン間隔を2.5mmにでき(既定は`"5"`)、`attrs.block_numbers = "inside"`でピン番号を外形の内側に描ける(既定は`"outside"`。内側のときピン名は描かない。信号名は配線のネットラベルで出す)。細かい間隔では大きさの刻みが2.5mm、最小が5×5mmで、ピンn本の辺は(n+1)×2.5mm以上が要り、ピンと辺が2.5mmの格子に乗るよう外形とピンの列は必要に応じて1.25mmずらす。コネクタの絵(`connector`)は2.5mmのブロックにも、番号を内側に描くブロックにも使えない。ほかの値は断る。KiCad・DXFの書き出しと取り込みは属性を保つ。format_version 22(形式21のアプリは版番号で断る)。ブロックエディタは属性を保つが、操作の部品はまだ無い(`place_symbol`の`attrs`から設定する)。実装は`madake-core/src/block.rs`、`src/canvas/dynamicSymbol.ts`、`src/canvas/blockEditor.ts`。

**ブロックの貫通ピン(2026-11-15)**: ブロックのピン表で、同じピン番号を向かい合う辺(左と右、または上と下)に1回ずつ置ける。コネクタ箱で導体が片側から入り反対側へ抜ける極(同じ極の両側に配線がある形)を、同じ番号の2本のピンで表す。2つの接続点は1つのノードで、ネットのピン一覧には1回だけ載る。描き方は今までどおり(各辺が番号・名前・リードを描く)。ただし`block_numbers = "inside"`で、2本が同じ線上(左右なら同じ高さ、上下なら同じ列)に並ぶ組は、その線と交わる辺にピンが無いときだけ、番号を箱の中央に1つ描く(あるときは各ピンが自分の番号を描く。幅は組の番号の最大幅を使うので、一桁の番号だけの左右の組なら幅5mmに収まる)。両側のピン名は前後の空白を除いて同じにし、`contacts`・`connector`は付けない。同じ辺・隣り合う辺・3回以上は従来どおり「duplicate block pin number」で断る。`REF.NO`は曖昧になり、`REF.NO#1`(左・右・上・下の順で最初)と`REF.NO#2`で選ぶ。`block_pitch`・`block_numbers`とは組み合わせられる。ブロックエディタは、重複・名前の不一致・接点とコネクタの絵を指摘し、配線のある側の行を消す・番号を変える編集を、もう片側の行が残っていても「接続が外れる」と警告する(配線の無い側の行を消しても警告しない。操作の部品は増やしていない)。片側にだけ配線がある貫通ピンは、未接続ピンの検査では接続済みとして数える。`REF.NO#1`・`REF.NO#2`はピン表の定義順(左・右・上・下。回転・反転の前)で、反転したブロックでは`#1`が画面の右に描かれる。format_version 23(形式22のアプリは版番号で断る)。実装は`madake-core/src/block.rs`、`src/canvas/dynamicSymbol.ts`、`src/canvas/blockEditor.ts`。

**図面の見た目の設定 (2026-11-15)**: 配線を太く描く図面・品番を文字で別に書く図面に合わせる4つ(format_version 24)。(1) シートごとの配線の線の太さ `wire_weight_mm`(0.18〜1.0mm、未設定は0.35mm)。(2) シートごとに配線注記(線径・色名・長さ)を描かない `hide_wire_notes`(線の色はそのまま。注記を避ける他の配置もこの設定に従う)。どちらも `set_sheet_wire_style` コマンドで設定する。(3) ブロックの属性 `block_caption: "inside"` で、参照記号と型番を外形の中央に書く(既定の `above` は外形の上)。(4) 注記の回転(90・180・270度、時計回り、基準点が中心)を SVG・PDF・キャンバスで描く。回した注記の選択範囲と引き出し線は、回した文字の箱で求める。分岐点の丸は太い配線に合わせて大きくなり(半径は太さの0.9倍、0.6mm以上)、ケーブルの芯線の分かれ目の線も配線の太さになる。`set_sheet_wire_style` はその値を必ず一緒に設定する(省いた値は既定へ戻る)。`block_caption: "inside"` は他の文字を避けないので、中央の行にピン名や内側の番号がある箱では重なる。設定の画面はまだ無い(コマンド・MCP・Link API から設定する)。

**線と文字の描き方 (2026-11-16)**: 太い線・鮮やかな色・箱に直接つながる配線で描く図面に合わせる7つ(format_version 25)。(1) シートの外形の線の太さ `symbol_weight_mm`。(2) シートの `vivid_wire_colors` で配線を純色に(色名と品番の引き当ては同じ)。(3) 配線ごとの破線 `dashed` と太さ `weight_mm`(母線など)。(4) 図形の線・折れ線の終点の矢印 `arrow_end`。(5) 書体のペンの太さ `pen_mm`(文字の大きさによらず同じ太さ)と字間 `spacing`(全ての文字。全角の字形を詰めて書く図面向け)、注記ごとの字間 `spacing`(書体の字間に掛ける)。(6) ブロックの `block_leads: "none"`(リード線を描かず、ピンが外形の辺の上。番号内側のブロックだけ)。(7) シンボルの `caption_layout`(参照記号・型番の行ごとの位置・大きさ・寄せ・非表示)。SVG・PDF・キャンバスが同じ規則で描く。KiCad・DXF の読み戻しは `block_leads` を保つ。`caption_layout` で動かした行は、整列・クロスリファレンス・KiCad/DXF のフィールド位置には反映しない(これまでの置き場所で計算する)。設定の画面はまだ無い。

**グリッド外の接続点 (2026-11-16)**: ユーザーシンボルの定義に `free_pins: true` を書くと、接続点を2.5mmグリッドに縛らない(G47)。グリッドで描かれていない図面のピン間隔を、そのシンボルの中だけ写せる。検証は座標が有限の数であることだけを見る。配線は接続点ちょうど(0.01mm以内)で終わればつながる(これまでと同じ規則)。同梱シンボル・ブロックは対象外で、常にグリッドに乗る。シンボルエディタには操作が無い(JSON か `save_user_symbol` で作る。複製は指定を保ち、保存前チェックも通る)。配線ツールは、カーソルから1.25mm以内のグリッド外の接続点へ、グリッドより先に吸着する。整えの指標 off_grid は、こうしたシンボルと、その接続点で終わる配線を数えない。保存形式は変わらない(シンボルのファイルの項目)。

**タグの行き先 (2026-11-18)**: `Sheet.xref_style`(`zone` / `sheet_name`、format_version 28、`set_sheet_xref_style`)。`sheet_name` では、タグ付きネットラベルの旗の中に太字のネット名と、同名ラベルを持つ他のシートの名前(2枚まで「・」でつなぎ、3枚以上は「他」)を書く。ネットラベルの `attrs.tag_dest`(行き先の自由な文字)と `tag_flow: "out"`(先端を本体の遠い端へ)も使える。文字の重なりの計測・並べ直しの置き場は新しい旗の広がりに従う。点を含むネット名(`J7.2`)は、ポートの接点に当たらなければ普通のネット名として繋がることをテストで固定した。

**ブロックのピン(2026-11-18)**: ブロックのピン表の3つの拡張と文字の高さ(format_version 29)。(1) 番号なしのピン(`no` を省く。名前がピンの鍵で、`get_pins`・`connect_pins`・ネットリスト・KiCad/DXF はそのまま名前で働く。番号は外に書かない。番号を内側に描くブロックでは番号の位置に名前を書く。名前が重なると断る)。(2) すき間の行 `{"gap":n}`(1〜20行、ピンの位置を空ける)。(3) 見出しの行 `{"caption":"…"}`(1行を使い、箱の内側にピン名と同じ大きさの太字で書く。`Primitive::Text.bold` を新設)。どちらもピンではなく `get_pins` に出ない。属性 `block_pin_text`(1.5〜5.0mm)でピン名・見出し・番号の文字の高さと箱の最小幅が変わる。ピン表エディタは、すき間・見出しの行を行として表示し(並べ替え・行数や文字の書き換え・削除ができる。追加ボタンはまだ無い)、開いて保存し直しても落とさず、`block_pin_text` もそのまま保つ。TS の描画は Rust と共有の golden で一致を固定した。

**シンボルのネット名 (2026-11-16)**: シンボルの定義に `net`(例 `"GND"`)を書くと、置いた全てのピンがその名前のネットラベルと同じ働きをする(G48)。接地の印をあちこちに描いた回路図で、印につながる配線が1つのネットになる。同じ名前の普通のネットラベルともつながり、ネット名はその名前になる。図面には何も描かない。空白だけの名前は無しと同じ。同梱のシンボルは持たない。シンボルエディタには操作が無い(JSON か `save_user_symbol`)。保存形式は変わらない(シンボルのファイルの項目)。この名前はシートを跨いだ統合・ラベル衝突の確認・シミュレーション・KiCad 出力でも働き、1つのネットに2つ以上の名前があれば既存のラベル衝突の警告が出る。参照記号の未設定・重複の確認 (erc.empty_reference / erc.duplicate_reference) は、こうした印を数えない。部品表にも載せない(部品ではないため)。電線リストは、印に届いた電線の端を印のネット名で書く。

**図面の補助記号 (2026-11-18)**: 同梱ライブラリに、トランジスタ(NPN・PNP。ピン B・C・E)・フォトカプラ(A・K・C・E)・オペアンプ(-・+・OUT・V+・V-)・双方向のサージ吸収ダイオード(TVS)・電源の印 `power_rail`・コネクタ接点 `connector_contact` を足した (G51〜G53)。図形は JIS C 0617 に倣い、ピンは2.5mmグリッドのリード線の端にある。`power_rail` は定義の `net_from_value` で、置いた部品の**値**がネット名になる(同じ値の印・同名のネットラベル・固定のネット名の印は、シートを跨いでも1つのネット)。値は棒の上に印字し、空だと検証が警告 `erc.empty_net_name` を出す。参照記号を振らず、部品表・参照記号の検査(erc.empty_reference / erc.duplicate_reference)の対象外で、電線リストは印に届いた電線の端を値で書き、KiCad 出力は値の名前のラベルを書く。`connector_contact` は定義の `through` で、半径1mmの丸の左右のピン(同じ番号1)が1つの節点になる。参照記号がコネクタ名(J7)、値が極の番号で、ピンの鍵は極の番号 (値 `5` なら `J7:5`。値が空なら 1。同じコネクタの同じ極が別々のネットにあれば erc.contact_pole_repeated)、同じ参照記号を何個置いてもよい(重複を指摘しない)が、空なら erc.empty_reference。部品表は同じ参照記号を1つのコネクタと数え、SPICE では 1mΩ の橋(0Ωの素通し)。半導体の4種は SPICE に模型が無く、役割の無い部品と同じに素子を作らない。KiCad・DXF の書き出しと読み戻しでは、ほかのシンボルと同じに種類・参照記号・値が残る。シンボルエディタの保存前チェックと Rust の検証は、`net` と `net_from_value` の併記、同じ番号のピンの組が無い `through` を断る。配置ツールは `net_from_value` の記号に参照記号を振らない。保存形式は変わらない(定義はライブラリの項目)。
**部分表示 (2026-11-18)**: ブロックの属性 `block_view` = `"1"` で、1つの機器(同じ参照記号、例 `U7`)を複数のシートに分けて描く(`block::is_view`)。同じ参照記号の部分表示は1つの部品で、参照記号の重複(erc.duplicate_reference)は指摘せず、部品表は1つ(数量1、型番は最初の空でない部分表示から)、シート間の場所の文字は出さず、ネットリストは各ピンを共通の参照記号の下に並べる。プロジェクトの検証が erc.view_pin_repeated(同じピンの鍵を2つの部分表示が出す。エラー)と erc.view_part_mismatch(型番の食い違い。警告)を出す。同じコネクタの同じ極の接点 (erc.contact_pole_repeated) もプロジェクト全体のネットで見直す (ラベルでシートを跨いでつながれば同じネット。プロジェクトの検証が1組1件)。キャンバスの場所の文字も同じ規則 (`isBlockView`)。KiCad・DXF は属性を保つ。保存形式は変わらない。

**配線の自動の引き直し (2026-11-16)**: シートの配線を、接続を変えずに直角の配線へ引き直す (G49)。計画は madake-core の `route.rs` が作り (図面は書き換えない)、消す配線と新しい配線を `delete_entities` + `add_entity` の1回の編集で適用するので、undo一発で元の配線がid付きで戻る。入口は MCP ツール `reroute_wires`・Link API `POST /api/v1/route/reroute`・CLI `madake reroute [--sheet ID] [--net NAME]...`。1.25mm の格子を A* で探索し (曲がり・交差・他ネットのすぐ隣に費用)、シンボル・文字・図枠・表題欄・他のネットのピンを避け、他のネットとは直角にしか交わらず端点も共有しない。3端子以上のネットは T と接合点の木にする。物理的に離れた配線のかたまりごとに引き直し、ケーブルの芯が使う配線・属性 (線色・線径・品番・線番・破線・太さ) や長さの出所が混在するもの・他のエンティティ (ケーブルの芯・注記の引き出し線・寸法) や FreeCAD との対応付けが指すもの・ピンの引き出し線が他のネットに触れてしまうもの・端が何にもつながっていないもの・経路が見つからないものは元のまま残して `unchanged` に理由を付けて返す。属性は新しい配線へ引き継ぎ、元の `length_m` の合計は新しい配線のうち一番長い1本だけが持つ (部品表・電線リストで何度も数えないため)。設定は使える範囲 (`grid_mm` 0.5〜10mm、費用・すき間・余白は0以上) を確かめ、範囲外はエラーで断る。探索は端点のまわり15mmの範囲から始め、見つからなければ図面全体を1回探し、広げる状態は15万までにする。計画はエンジンのロックの外でシートの写しに対して作り、版番号が変わっていなければ適用する。名前の無いネットの自動の名前 (N001 など) は引き直しの後に入れ替わることがある。返り値は `rerouted`・`unchanged`・整えの数値の前後・配線長の前後。変える配線が無いときは何も実行しない。UI のボタンは無い。保存形式は変わらない。

**部品の並べ直し (2026-11-17)**: シートの部品を、並び順を保ったまま列と行にそろえ、`channel_mm` の通り道をあけて全配線を引き直す。計画は madake-core の `arrange.rs` の `plan_arrange` が作り (図面は書き換えない)、`SharedDoc::arrange_sheet` が 消す配線 → 移動 → 形の置き換え → 足す配線 の順に1回の編集で適用するので、undo一発で元に戻る。経路が見つからないかたまりはネットラベル (既存のラベル名・ネット名シンボルの名前・`ARR_<n>`) でつなぎ、返り値の `labelled` に載せる。ケーブル・盤面配置・ポートの接点・指されている配線・長さの出所が混在する配線・名前の違うラベル・図枠に収まらない結果は断る。入口は MCP ツール `arrange_sheet`・Link API `POST /api/v1/route/arrange`・CLI `madake arrange [--sheet ID] [--channel MM]`。

**機能シートの下書き (2026-11-18)**: 全ての配線を持つ別の `.mdkproj` (元の図) から、開いているプロジェクトへ機能シートを1枚作る。何を載せるか (機器と列) は呼び出し側、どう描くか (使うピンだけの箱・ピンの辺と順・配線・他のシートの名前を入れた札・電源と接地の印・間隔) は madake-core の `draft.rs` の `plan_function_sheet` が決める (図面は書き換えない)。`SharedDoc::draft_function_sheet` が元の図を `io::load_project` で読み (書き換えない)、計画をエンジンのロックの外で作り、版番号が変わっていなければ `Command::RestoreSheet` 1つの編集で適用する (undo一発でシートごと消える。変わっていれば計画し直し、3回続けて変わったら断る)。1つの機器を複数のシートに出すビューは `block_view` のブロック。検算 `draft_check.rs` の `check_function_sheets` は、元の図と開いているプロジェクトの `(参照記号, ピン)` のネットごとの集まりを比べ、`missing_pins`・`extra_pins`・`split`・`merged` を返す (`ok` が false なら CLI は終了コード1)。入口は MCP ツール `draft_function_sheet`・`check_function_sheets`、Link API `POST /api/v1/draft/function-sheet`・`POST /api/v1/draft/check`、CLI `madake draft <request.json>`・`madake draft-check <source.mdkproj> [--ignore REF]...`。見た目の決まり (2026-11-18): 同じ機器の同じ辺の同じネットのピンは隣の行の組 (棒) にして印・配線の端を1つにし、短い線は辺ごとの番号の幅に合わせ、引けないネットは行き先なしの札で各端をつなぎ、列は左から詰めて (通り道は max(20mm, 10mm+2.5mm×横切るネット数))、札は同じネットを持つ他シートの名前の幅まで空けて描画範囲を越えない。見直し (2026-11-18): シートの上の相手が全て同じ列にあるピンは右 (最後の列では左) に置き、その列の右の通り道で数える。電源・接地の印は棒の端からピン間隔ぶん延ばして箱から離れる向きへ曲げた先に置き、文字が棒・足・番号にかからない。箱は外の番号を辺の外で終わらせる (`block_number_align`=`edge`)。札の旗 (引けないネットの代わりの札も) は全てのネットの通れない場所で、旗に触れる経路は引けないものとして札でつなぐ。UI のボタンは無い。保存形式は変わらない。

**線番を描かない (2026-11-16)**: シートの `hide_wire_numbers`(format_version 26)。配線に付けたネット名・番号の文字を描かない。名前はデータ・ネットリスト・帳票に残る。信号名を注記で書く図面や、切れ目のある母線を名前で1つのネットにするときに使う。`set_sheet_wire_style` の `hide_numbers` で設定する。DXF 書き出しは線番のレイヤへ名前を書き続ける(読み戻しでネット名を復元するため)。整えの指標は、描かれない線番を文字の重なりに数えない。

**字の横の伸縮率 (2026-11-16)**: 書体の `stretch` と注記の `stretch`(0.3〜3、format_version 26)。字形の横幅と送りを一緒に縮める・伸ばす(高さは変わらない)。字を細長く書く図面を写すときに使う。字間 `spacing` は送りだけを変えるので、字を詰めすぎると重なる。伸縮率は重ならない。図枠の文字(表題欄・改訂欄)には書体の伸縮率は効かない(図枠はセルの大きさと字間で幅を決める)。

