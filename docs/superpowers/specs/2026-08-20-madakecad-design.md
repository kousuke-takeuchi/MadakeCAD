# MadakeCAD 全体設計(スペック)

日付: 2026-08-20
状態: フェーズ0実装済み。フェーズ1はUIデザイン(Pencil)確定後に着手。

## 1. 目的と背景

顧客向けロボットの電気配線図(現状KiCad 8で作図)を、産業基準に準拠した図面として作成できる電気CADを開発する。目標品質はユーザー提供の参考図面(機器構成図・基板図面。社外秘のため詳細はgit管理外の`docs/references/README.md`参照):

- JIS図枠: 表題欄(図番・品名・尺度・日付・設計/製図/検図/承認)、改訂欄、ゾーン番号(縦横アドレス)
- 電線管理: 線色、線径(0.3〜3.5sq)、長さ、線色+sqごとの電線品番マスタ
- 端子台・コネクタの型番+ピン番号単位の結線、ハーネス境界(破線囲み)
- 部品表(BOM)・電線リストの出力

将来目標: Amazon等で買える市販部品を含む部品DB、配線検証(電圧降下・線径適合・ヒューズ協調)、SPICEシミュレーション、AIによる自動作図。

参考図面PDFの一覧と方針の原点は `docs/references/README.md` を参照(PDF本体は社外秘・git管理外)。UIはAutoCAD Electrical等の産業用電気CADの慣習的デザインに寄せる。

## 2. 技術選定(確定済み)

| 項目 | 選定 | 理由 |
|---|---|---|
| シェル | Tauri 2 | 軽量・Rustコアと同居 |
| UI | Vue 3 + TypeScript + Pinia | ユーザー選好 |
| 描画 | Canvas2D自作レンダラ | 印刷品質・スナップ制御・依存最小 |
| コア | Rust (madake-core) | UI非依存、ヘッドレス実行可能 |
| AI連携 | 内蔵MCPサーバー (rmcp 3.x, Streamable HTTP) | Claude Code/Desktopから直接編集 |
| 部品DB | SQLite (rusqlite) ※フェーズ2 | ローカル完結 |
| シミュレーション | 配線検証(自作)→ngspice(フェーズ3) | 配線図用途を優先 |
| UIデザイン | Pencil (pen.dev) で design-first | ユーザー方針(AutoCAD Electrical風UI) |
| メカ連携 | FreeCAD 1.1+ (アドオンWB + Link API) | SOLIDWORKS Electrical⇔SOLIDWORKS相当の電気・機械連携 |

## 3. アーキテクチャ

### 3.1 Command中心設計(実装済み)

全編集はシリアライズ可能な`Command`(serde tagged enum)として`Engine::execute()`で実行される。エンジンは適用時に逆コマンドを生成してundo/redo履歴を持ち、変更差分を`Patch`(`PatchOp`列+単調増加revision)としてbroadcastする。Tauriはpatchを`doc:patch`イベントでwebviewへ転送し、フロントのPiniaストアはpatchだけを信頼してミラーを更新する(フロントでモデルを直接変更しない)。

### 3.2 モジュール構成

```
src-tauri/crates/madake-core   ドキュメントモデル、Command、シンボル、IO、(将来)ネットリスト/検証
src-tauri/crates/madake-mcp    MCPサーバー。SharedDoc { engine, patches } がUI/MCP共通の編集入口
src-tauri/src                  Tauri IPC(execute_command/undo/redo/save/load等)、MCP起動、patch転送
src/                           Vue UI(フェーズ1で本実装。現在は仮デバッグ画面)
design/                        Pencil (.pen) デザインファイル
```

### 3.3 データモデル(実装済み、フェーズ1で拡張)

- `Project`: format_version, name, sheets[], wire_parts[](電線品番マスタ)
- `Sheet`: JIS図枠(A4〜A0、縦横)、zone分割、表題欄、改訂欄、entities(BTreeMap<Uuid, Entity>)
- `Entity`(tagged enum): Symbol / Wire / Junction / NetLabel / Text
  - `SymbolInstance`: symbol_id, at(mm), rotation(0/90/180/270), mirror, reference("K1"), value(型番), attrs
  - `Wire`: points[](直交ポリライン), color, sq, length_m, part_no, net
- `SymbolDef`: JSONベクタプリミティブ(Line/Circle/Arc/Rect/Text)+ピン定義。JIS C 0617系15種を同梱。端子台/コネクタのピン数可変シンボルはフェーズ1
- 保存形式 `.mdkproj`: 整形JSON。KiCadインポート(S式→本モデル)はフェーズ2

### 3.4 ネットリスト(フェーズ1)

座標一致(2.5mmグリッド上の端点・ピン)とJunctionから接続グラフを構築し、NetLabelで論理結合。BOM(参照記号+型番集計)と電線リスト(品番・色・sq・長さ)はネットリストから導出する。

### 3.5 検証エンジン(フェーズ2)

- ERC: 未接続ピン、参照記号重複、短絡(異電位ネットの直結)。**自前実装を維持**(ERCはデータモデル密結合で流用可能な成熟OSSが存在しない。2026-08-21調査)。KiCadエクスポート実装後に`kicad-cli sch erc`をクロスチェックとして追加検討
- 電気検証: 電源からの到達性、電圧降下、線径-電流容量適合、ヒューズ協調。**電流・電圧の計算はngspice(フェーズ3から前倒し、2026-08-21決定)のDC動作点解析(.op)をバックエンドにする**: ワイヤ=抵抗(ρ×L/A、接続点でノード分割)、電源=電圧源、負荷=等価抵抗(V/current_a)、導通部品=微小抵抗としてSPICEネットリスト化し、実解の電圧・電流で判定する
- **ngspice連携はOS非依存**: サブプロセス実行(`ngspice -b`)。実行ファイル探索は 環境変数`MADAKE_NGSPICE` → PATH → OS別既定パス(macOS: Homebrew、Linux: /usr/bin、Windows: Spice64標準パス)。**未導入環境ではグラフ近似(導通部品越し電流伝播)へフォールバック**し、Info診断で近似モードと明示する
- 検証結果は`Diagnostic { severity, message, entity_ids, sheet_id }`のリストとしてMCP/UI双方に返す

### 3.6 シミュレーション(フェーズ3)

部品DBにSPICEモデルを紐付け、ngspice(libngspice FFIまたはサブプロセス)でDC動作点/過渡解析。ネットリスト抽出器がSPICEネットリストも出力する。

## 4. UI設計(Pencilでデザイン後に確定)

AutoCADをベースにした画面構成の想定(Pencilデザインで詳細を決める):

- 中央: 図面キャンバス(無限パン/ズーム、mm座標、グリッド、用紙枠)
- 上部: メニュー+ツールバー(選択/配線/シンボル/テキスト/計測)
- 左: シンボルパレット(カテゴリ別)、シートツリー
- 右: プロパティパネル(選択物の属性: 線色/sq/品番/参照記号/型番)
- 下部: 座標/スナップ状態バー(直近メッセージ表示含む)。~~コマンドライン~~は廃止(2026-08-20決定)し、キーボードショートカット+チャット+`madake` CLI(ターミナル用、Link APIシンクライアント)で代替
- シートタブ(TB1前部ボックス/TB2底部ボックス…のような複数シート)
- レイヤ=**固定の表示クラス**(2026-08-21決定): エンティティ種別ごとの表示トグル(配線[ジャンクション含む]/シンボル/参照記号・型番/ネットラベル/注記/図枠/グリッド)。画面表示のみでSVG/PDF出力には影響しない(出力は常に全要素)。レイヤ単位のロックは無し。UIはリボン「表示」タブに置き、グリッドはステータスバーの既存トグルと同一状態を共有する。AutoCAD式の自由レイヤ(エンティティへのlayer属性付与)は不採用(必要になったら別途スペック化)

対話ツールはステートマシンとして実装し、ドラッグ中はローカルプレビュー、確定時にCommand発行。

## 5. MCP API(実装済み、フェーズ1で拡張)

実装済みツール: `get_project` / `list_symbols` / `place_symbol` / `draw_wire` / `execute_commands`(全Commandスキーマ公開) / `undo` / `redo` / `get_netlist` / `export_bom` / `export_wire_list` / `export_svg`

### 5.2 アプリ内AIエージェント(チャットパネル)

Pencil(pen.dev)のエージェントUIを手本に、アプリ内でLLMと対話しながら図面を操作できるようにする。デザインは`MadakeCAD.pen`の「AIチャット(展開状態)」「AI連携設定」フレーム参照(2026-08-20作成、Pencilのデザイン言語準拠: 白カード・角丸・黒アクセント)。

- **UI**: 作図領域左下のフローティングチャットパネル(折りたたみ=入力バーのみ/展開=会話履歴)。入力欄+添付+トークン使用量+モデル選択(Claude Fable 5等)+送信。エージェントの各ツール実行は会話内にチップ(✓ place_symbol ...)で表示し、「図面に適用済み (rev N)/元に戻す」を添える
- **マルチプロバイダ対応(Pencil同等)**: `madake-agent`クレートに`AgentBackend`トレイトを設け、プロバイダを差し替え可能にする
  - **ClaudeCodeCliBackend(推奨・既定)**: ローカルのClaude Code CLIをヘッドレス実行(`claude -p --output-format stream-json`+MCP設定)し、**既存のClaude Pro/MaxサブスクリプションのOAuthセッションをそのまま利用**する(Pencilの「Claude Codeの設定を利用」「Sign in with Claude (Pro/Max)」と同方式)。APIキー不要。ツールはMadakeCAD自身のMCP(127.0.0.1:9310/mcp)を自己接続させるため、ツール定義の二重実装が不要
  - **AnthropicApiBackend**: Messages API直接呼び出し(APIキー、AWS Bedrock/Google Vertex経路含む)
  - **OpenAI互換Backend**: OpenAI ChatGPT / xAI / OpenRouter / Ollama(ローカル)を同一実装で対応
  - **GeminiBackend**: Google AI Studio APIキー
- **編集は全てCommandエンジンを通る**ため、どのプロバイダ経由でもundo/redo・patch配信・rev番号と完全に整合し、「元に戻す」はundo Nと等価
- **設定画面(デザイン済み、`MadakeCAD.pen`の「AI設定 - *」フレーム6枚)**: Pencilの設定と同構成
  - プロバイダ一覧: サインインボタン(Claude Pro/Max・ChatGPT)+プロバイダ行(接続状態バッジ)。Anthropic/OpenAI/Gemini/xAI/OpenRouter/Ollama
  - プロバイダ詳細(Claude): 認証方法ラジオ(Claude Codeの設定を利用/APIキー/Claudeでサインイン(Pro/Max)/Bedrock/Vertex/カスタム)+サインイン済みバナー+エージェント設定(自動許可モード/図面自動読み取り/claude実行ファイルパス)
  - プロバイダ詳細(Gemini等): APIキー方式(生成リンク+入力+保存)
  - 一般: 外観(ライト/ダーク)、グリッド/スナップ/ホイールズーム/直交などのキャンバス設定、文字サイズ・座標精度・単位
  - チャット: 通知、危険:確認スキップ、会話履歴のプロジェクト保存
  - MCP: 外部CLI(Claude Code/Codex/Gemini CLI/Claude Desktop/FreeCAD)への自動MCP設定トグル+カスタムMCP設定のコピー(JSON表示)
- プロバイダ一覧はPencil同等のフルラインナップ: Anthropic / OpenAI(Codex CLI連携) / Gemini / xAI / OpenRouter / GitHub Copilot(デバイスコード認証) / Ollama(ローカル、サーバーURL+モデル一覧) / Moonshot / Kimi For Coding / DeepSeek / Together / Fireworks / Z.AI / OpenCode Zen。認証アーキタイプは4種: CLI再利用(サブスク)、APIキー、デバイスコードOAuth、ローカルURL
- チャットのポップアップ(モデルピッカー/並列エージェント[作業分担・比較案]/自動反復[配置整理・配線整理・ラベル整頓]/コンテキスト追加[データシートPDF・図面・部品DB・規格])は`MadakeCAD.pen`「AIチャット - ポップアップ集」参照
- **共通デザインシステム**: `docs/design-system.md`と`.pen`の「デザインシステム - 共通コンポーネント」が正。メイン画面のCAD調トークン(ribbon-bg背景+白カード+acad-blueアクセント)を全UI(設定・チャット含む)で使用する。モデル/レイアウトタブ行はチャットパネル導入に伴い廃止
- APIキー・OAuthトークンはOSキーチェーンに保存し、設定ファイルには置かない
- **エージェント編集オーバーレイ**: エージェントの編集中、対象領域をシアン(#29D3E6)の半透明パルス枠+「エージェントが編集中...」チップで可視化する(Pencil同等。デザイン: .penの「エージェント編集オーバーレイ」、DSボード収載)
- **madake CLI**: Link APIのシンクライアントとしてターミナルから操作(`madake netlist` / `madake export svg` / `madake exec`等)。廃止したコマンドラインUIの代替
- **フェーズA**(フェーズ2と並行可): A1=チャットフルUI+ClaudeCodeCliBackend+編集オーバーレイ+madake CLI(プラン: `docs/superpowers/plans/2026-08-20-phaseA1-ai-chat-and-cli.md`)、A2=並列エージェント・自動反復、A3=APIキー系バックエンド(OpenAI互換/Gemini/Ollama)+設定画面フル実装

## 6. フェーズ計画

- フェーズ0(完了): Commandエンジン+モデル+シンボル+IO、MCPサーバー、Tauri足場、スモークテスト
- フェーズ1: Canvas2DエディタUI(Pencilデザイン準拠)、ネットリスト、BOM/電線リストCSV、PDF/SVG出力、端子台動的シンボル、AutoCAD風コマンドライン
- フェーズ2: 検証エンジン、部品DB(SQLite+購入先URL)、KiCadインポート
- フェーズ3: ngspiceシミュレーション
- フェーズA(フェーズ2と並行可): アプリ内AIエージェント(§5.2。チャットUI→ツールチップ表示→Claudeログイン)
- フェーズM(フェーズ2完了後、3と並行可): FreeCAD連携(§7参照。Link API→3Dモデル挿入→経路長還元→盤レイアウト)

## 7. FreeCAD連携(メカCAD連携)

SOLIDWORKS Electrical⇔SOLIDWORKSの関係に相当する、電気(MadakeCAD)⇔機械(FreeCAD)の双方向連携を設計の柱として組み込む。対象はFreeCAD 1.1系以降(2026-03リリース、アセンブリWB標準搭載、Python API刷新)。

### 7.1 提供する連携機能

1. **部品の対応付け**: 回路図の部品(参照記号+型番)をFreeCADアセンブリ内の3D部品と1対1で紐付ける。部品DB(フェーズ2)の各部品に3Dモデル参照(`model_3d`: STEP/FCStdパス)を持たせ、FreeCAD側へ挿入できるようにする
2. **3D配線ルーティング**: MadakeCADのネットリスト(どのピンとどのピンが繋がるか)をFreeCAD側で参照し、筐体内の配線経路を3Dで引く
3. **電線長の還元**: FreeCADで確定した経路長を`Wire.length_m`へ書き戻し、BOM/電線リストに反映する(SOLIDWORKS Electricalの目玉機能に相当)
4. **盤レイアウト**: 将来、パネル図(2D盤面レイアウト)と3D筐体配置の同期

### 7.2 アーキテクチャ

```
MadakeCAD (Tauri)                         FreeCAD 1.1+
┌─────────────────────────┐               ┌──────────────────────────┐
│ madake-core (Command)   │   HTTP/JSON   │ アドオンWB「MadakeCAD Link」 │
│ 内蔵サーバー (axum)       │◄─────────────►│  - Link APIクライアント     │
│  ├ /mcp     (AI用MCP)   │  localhost    │  - 部品挿入/経路計測UI      │
│  └ /api/v1  (Link API)  │               │  - madake_idプロパティ管理  │
└─────────────────────────┘               └──────────────────────────┘
```

- **Link API**: 既存の内蔵HTTPサーバー(127.0.0.1:9310)に`/api/v1`(素のJSON REST)を追加。MCPはAIエージェント用、Link APIは機械連携用と役割を分ける。ただし**書き込みは全て既存Commandエンジンを通す**ため、FreeCADからの変更もundo/redo・patch配信・AI編集と完全に整合する(アーキテクチャの絶対原則を維持)
  - 読み: `GET /api/v1/project` / `GET /api/v1/netlist?sheet=` / `GET /api/v1/parts`
  - 書き: `POST /api/v1/commands`(Command列)。主用途: `UpdateEntity`での電線長更新、mech_link登録
- **FreeCADアドオン**: Pythonワークベンチとして別リポジトリまたは`freecad-addon/`配下で開発。機能: 接続設定、部品リスト表示→アセンブリへ3Dモデル挿入、ネットリストビュー、経路オブジェクト(Draft Wire/スケッチ)の長さ計測と一括書き戻し
- **対応付けのキー**: MadakeCADのentity UUIDが軸。FreeCADオブジェクト側にカスタムプロパティ`madake_id`を保存し、MadakeCAD側は`Project.mech_links`(entity_id → {fcstd_path, object_name})を保存。両側にキーを持つことでファイルを別々に開いても再同期できる
- **マスタ権の原則**: 電気データ(参照記号・型番・ピン接続)はMadakeCADがマスタ。ジオメトリ(3D配置・経路・実測長)はFreeCADがマスタ。同期は明示操作(同期ボタン)で行い、暗黙の自動上書きはしない

### 7.3 データモデルへの影響(先行して設計に織り込む)

- `Project.mech_links: Vec<MechLink { entity_id, fcstd_path, object_name, synced_at }>`(フェーズM1で追加)
- 部品DBスキーマに`model_3d`(STEP/FCStdパス)と取付情報(DINレール/ねじ等)を予約
- `Wire`に長さの出所(手入力かFreeCAD実測か)を示す属性を追加し、FreeCAD由来の値を手入力で誤って上書きしない

### 7.4 実装フェーズ(フェーズM: フェーズ2完了後、フェーズ3と並行可)

- **M1**: Link API(読み取り+Command書き込み)、FreeCADアドオン骨格(接続・プロジェクト/ネットリスト表示)
  - Link API本体は実装済み(2026-08-20前倒し)。`madake-mcp/src/link_api.rs`。REST + SSEパッチストリーム。フロントエンドもTauri外ではこのAPIに自動フォールバックし、ブラウザ(Playwright等)でのUI検証に使える
- **M2**: 部品DBの3Dモデル挿入+`madake_id`バインド、経路長の一括書き戻し(電線リストへ反映)
- **M3**: 経路3D表示の同期、盤レイアウト(パネル図⇔3D筐体)

## 8. 非目標(現時点)

- PCBレイアウト・ガーバー出力(電子基板CADではない)
- クラウド同期・同時編集(ローカルファースト)
- Windows/Linuxビルド(まずmacOS。Tauriなので移植は容易)
