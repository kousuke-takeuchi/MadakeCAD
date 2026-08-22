//! エージェントへ毎ターン注入する規格・慣習の知識と、作図ルール(検証ループ)。
//!
//! 組み立てる`--append-system-prompt`の中身は次の順:
//!
//! 1. 図面コンテキスト(呼び出し側が渡す。madake-mcpの`drawing_context`が作る)
//! 2. 作図ルール([`WORKFLOW_RULES`]。編集後は必ず`run_verification`を回す検証ループ)
//! 3. 同梱ドキュメントの案内([`docs_guide`]。docs/が見つかったときだけ)
//! 4. 設計レビュー・動作確認手順の書式([`REVIEW_RULES`])
//! 5. 部品選定・比較表の書式([`PARTS_RULES`])
//! 6. 同梱の規格ノート(`resources/knowledge/standards.md`)
//! 7. 設定[`AppSettings::knowledge_path`]の知識ファイル(あれば。後勝ちで上書きできる)
//!
//! 規格ノートは**編集可能なMarkdownとして同梱**する。実体は
//! `src-tauri/resources/knowledge/standards.md`で、Tauriが起動時にリソースの実体パスを
//! [`STANDARDS_PATH_ENV`]へ入れる(開発時はリポジトリのファイル、配布時はアプリバンドル内)。
//! 実ファイルが読めない環境でもエージェントが無知にならないよう、ビルド時に埋め込んだ
//! 同内容へフォールバックする。

use std::path::{Path, PathBuf};

use crate::settings::AppSettings;

/// 同梱の規格ノートの実ファイルパスを与える環境変数(Tauriのリソース解決・テスト用)。
pub const STANDARDS_PATH_ENV: &str = "MADAKE_STANDARDS_PATH";

/// 実ファイルが読めないときのフォールバック(ビルド時に埋め込む同内容)。
const BUNDLED_STANDARDS: &str = include_str!("../../../resources/knowledge/standards.md");

/// 検証ループの上限回数(これを超えたら直さずに報告させる)。
pub const VERIFY_LOOP_LIMIT: u32 = 3;

/// 図面編集後の作図ルール。**検証ループ**の指示はここ。
///
/// ツール名は内蔵MCPサーバー(`mcp__madakecad__*`)のもの。
const WORKFLOW_RULES: &str = "\
## 作図と検証のルール(必須)
- 図面を編集したら、**必ず`run_verification`を実行して結果を確認する**(編集しっぱなしにしない)
- severity=error の指摘が残っているときは、原因を直してからもう一度`run_verification`を実行する。\
この「検証 → 修正 → 再検証」は最大3回まで繰り返す
- 3回試してもエラーが消えない場合は、それ以上いじらず、残っている指摘と考えられる原因・\
必要な判断をユーザーへ説明する(壊れた図面を黙って残さない)
- warning は消せるものだけ消し、残す場合は理由を説明する。info は放置してよい
- **最終応答には必ず「検証: エラーN・警告M」の形で件数を書く**(例: 検証: エラー0・警告1で完了)
- 図面を編集していないターン(質問への回答だけ)では検証は不要
- 図面の読み書きは必ずMCPサーバー\"madakecad\"のツール(`mcp__madakecad__*`)で行う。\
.mdkprojファイルを直接編集してはならない(全ての編集はCommandエンジンを通す必要がある)
";

/// 同梱ドキュメント(`docs/`)の場所を与える環境変数(Tauriのリソース解決・テスト用)。
pub const DOCS_PATH_ENV: &str = "MADAKE_DOCS_PATH";

/// 開発時(リポジトリで動かすとき)のdocs/の位置。配布ビルドには存在しないので候補どまり。
const REPO_DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs");

/// 同梱ドキュメントかどうかの判定に使うファイル(これがあれば目次どおりに並んでいる)。
const DOCS_MARKER: &str = "01-overview.md";

/// 設計レビューと動作確認手順(検証計画)の進め方。
///
/// 決定的な検証(`run_verification`)で拾える指摘と、慣行・見やすさのように
/// 人が見ないと分からない指摘を、ひとつの指摘一覧にまとめさせる。
const REVIEW_RULES: &str = "\
## 設計レビュー(「レビューして」「見てほしい」と言われたとき)

1. まず`run_verification`を実行し、決定的な指摘(ERC・電気検証)を取得する
2. 次に`get_project`・`get_netlist`等で図面を読み、検証では拾えない次の5観点を点検する:
   - **参照記号の体系**: 接頭辞が下の「参照記号」表と合っているか、重複・空欄・番号の飛びが無いか
   - **線色/sqの慣習**: 電源・接地・制御線の色と太さ(sq)が慣習どおりか、負荷電流に対して細すぎないか
   - **線番・ネットラベルの命名**: 命名規則が図面全体で揃っているか、重複・付け忘れが無いか
   - **レイアウト**: シンボルや配線の重なり・不要な交差・2.5mmグリッド外れ・読み順(左上→右下)
   - **図枠情報**: 表題欄(図番・図面名・日付・担当)と改訂欄が埋まっているか
3. 指摘は重要度の高い順に、次の表で列挙する(指摘が無い観点は「問題なし」と一行で書く):

   | 重要度 | 対象 | 指摘 | 提案 |
   |---|---|---|---|
   | 高 | K1 | コイルの片側が電源に届いていない | K1-A2をN線へ接続する |

   重要度は **高**(誤配線・安全・出図不可)/ **中**(慣習違反・読みにくさ)/ **低**(体裁)の3段階
4. **指摘するだけで図面を直さない。修正はユーザーが承認してから**MCPツールで実行する
   (「直しますか?」と聞き、承認された指摘だけを直して`run_verification`で確かめる)

## 動作確認手順(「検証計画を作って」「動作確認手順を作って」と言われたとき)

- 手順表で答える:

  | 手順 | 操作 | 期待結果 |
  |---|---|---|
  | 1 | 主電源を投入する | PL1が点灯し、K1は非励磁のまま |

- 操作は実際に人がやること(電源投入・PB1押下・SW1切替)、期待結果は目で確かめられる状態
  (K1励磁・PL1点灯・端子TB1-3にDC24V)で書く。図面から読み取れない前提は書かない
- 手順表は**回答として返す**。図面へ注記や帳票として書き込むのは、ユーザーがそう指示したときだけ
";

/// 部品選定・比較表・代替品提案の書式。
const PARTS_RULES: &str = "\
## 部品選定(「どれを選べばいい?」「〜の代替は?」と言われたとき)

- 部品DBを`search_parts`で検索する(query=型番・名称・メーカの部分一致、category=カテゴリ完全一致)。\
型番を思い出しで書かず、必ず検索して実在する部品だけを挙げる
- 候補は**最大10件**を比較表で示す(DBに値が無い列は「-」):

  | 型番 | メーカ | 定格 | 価格 | 購入先 | 差分 |
  |---|---|---|---|---|---|
  | MY2N-D2 DC24 | オムロン | DC24V 5A 2c | ¥1,200 | (purchase_url) | 現行品にサージ吸収ダイオード付き |

- 「差分」には現行品との違い(接点構成・コイル電圧・定格電流・価格差・入手性)を書く
- 候補が無ければ「部品DBに該当なし」と伝え、DBへの登録(`upsert_part`)を提案する
- 配置済み部品の置換は**提案までにとどめる**。ユーザーの承認後に`execute_commands`の\
`update_entity`(型番`part_no`・属性の書き換え)や、シンボルごと替える場合は`place_symbol`+\
`delete_entities`で実行し、最後に`run_verification`で確かめる
";

/// 同梱の規格ノート本文。実ファイル(リソース)があればそれ、無ければ埋め込み。
pub fn standards_text() -> String {
    match standards_path().and_then(|path| read_knowledge(&path)) {
        Some(text) => text,
        None => BUNDLED_STANDARDS.to_string(),
    }
}

/// 同梱の規格ノートの実ファイルパス([`STANDARDS_PATH_ENV`]が指す場所)。
pub fn standards_path() -> Option<PathBuf> {
    std::env::var_os(STANDARDS_PATH_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 同梱ドキュメント(`docs/`)のディレクトリ。見つからなければ`None`。
///
/// 探す順は [`DOCS_PATH_ENV`](Tauriが起動時に入れる実体パス) →
/// リポジトリの`docs/`(開発時)。どちらも[`DOCS_MARKER`]を含むディレクトリでなければ
/// 「ドキュメント未同梱」として扱う(存在しないファイルを案内させない)。
pub fn docs_dir() -> Option<PathBuf> {
    let from_env = std::env::var_os(DOCS_PATH_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    [from_env, Some(PathBuf::from(REPO_DOCS))]
        .into_iter()
        .flatten()
        .find(|dir| dir.join(DOCS_MARKER).is_file())
}

/// 同梱ドキュメントの案内(目次と引用のしかた)。場所が分からなければ`None`。
///
/// ドキュメントが無い環境で目次だけ教えると、読めないファイルを出典として書いてしまう。
/// そのため**実物があるときだけ**案内する。
pub fn docs_guide(dir: Option<&Path>) -> Option<String> {
    let dir = dir?;
    if !dir.join(DOCS_MARKER).is_file() {
        return None;
    }
    let mut out = String::from(
        "## MadakeCADのドキュメント(操作方法・規格・対応状況の出典)\n\n\
         - 操作の仕方・機能の有無・規格の決まりを聞かれたら、**推測で答えず**、下の\
         ドキュメントを`Read`で読んでから答える\n\
         - 回答の末尾に**出典**を書く(例: `出典: 04-standards-output.md`)。\
         読んでいないファイルを出典に書かない\n\
         - 日本語で答えるときは同名の`.ja.md`(例: `03-schematic-editor.ja.md`)を読んでよい\n\
         - **未対応の機能**を聞かれたら`12-roadmap.md`で対応予定のマイルストーン(M1〜M6)を確かめ、\
         「今はできないが Mx で対応予定」と答える\n",
    );
    out.push_str(&format!("- 置き場所: `{}`\n\n", dir.display()));
    for (file, summary) in DOCS_INDEX {
        out.push_str(&format!("  - `{file}` — {summary}\n"));
    }
    let inventory = dir.join("internal/feature-inventory.md");
    let specs = dir.join("internal/specs");
    if inventory.is_file() {
        out.push_str(&format!(
            "\n- 実装済み・未実装の棚卸し: `{}`(機能の有無はここが最も詳しい)\n",
            inventory.display()
        ));
    } else {
        // 配布ビルドには内部資料を同梱しない。ロードマップだけで答える
        out.push_str("\n- 実装状況の詳細(feature-inventory.md)は同梱していないため、`12-roadmap.md`で答える\n");
    }
    if specs.is_dir() {
        out.push_str(&format!(
            "- 未実装機能の詳細仕様(内部資料): `{}`\n",
            specs.display()
        ));
    }
    Some(out)
}

/// 公開ドキュメントの目次(ファイル名 → 1行の内容)。読む前に当たりを付けるための索引。
const DOCS_INDEX: &[(&str, &str)] = &[
    ("01-overview.md", "MadakeCADとは何か・設計思想(AIが人と同じCommand APIで編集する)"),
    ("02-getting-started.md", "動作環境・インストール・起動と最初の図面"),
    ("03-schematic-editor.md", "回路図エディタの操作(リボン・選択・配置・配線・グリッド・端子台エディタ)"),
    ("04-standards-output.md", "JIS図枠・表題欄・改訂欄・ゾーン・クロスリファレンス・SVG/PDF出力"),
    ("05-wire-management.md", "電線の色/sq/長さ/品番・ハーネス・線番の採番・From-To電線リスト"),
    ("06-verification-simulation.md", "ERC・電気検証(電源到達性/許容電流/電圧降下/ヒューズ)・ngspiceのDC動作点"),
    ("07-parts-database.md", "部品DB(型番・メーカ・定格・価格・購入先)の検索と登録"),
    ("08-import-export.md", "KiCadインポート・CSV/PDF帳票・PDFブック出力"),
    ("09-ai-assistant.md", "AIチャットの使い方・できること・ターンの取り消し・設定"),
    ("10-automation-api.md", "MCPサーバー・Link API(REST)・madake CLIによる自動化"),
    ("11-mechanical-integration.md", "FreeCAD連携(部品対応付け・3D配線・電線長の書き戻し)"),
    ("12-roadmap.md", "マイルストーンM1〜M6と未対応機能の対応予定"),
    ("13-specification.md", "テストから自動生成した詳細仕様(挙動の正確な確認先)"),
];

/// エージェントへ渡すシステムプロンプトを組み立てる。
///
/// `drawing_context`は図面の要約(検証サマリ込み)。「図面の自動読み取り」がOFFのときは
/// `None`になるが、**規格知識と作図ルールは常に入る**(図面を渡さないだけで、
/// エージェントが規格を知らない状態にはしない)。
pub fn system_prompt(settings: &AppSettings, drawing_context: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(context) = drawing_context {
        out.push_str(context.trim_end());
        out.push_str("\n\n");
    }
    out.push_str(WORKFLOW_RULES);
    if let Some(guide) = docs_guide(docs_dir().as_deref()) {
        out.push('\n');
        out.push_str(guide.trim_end());
        out.push('\n');
    }
    out.push('\n');
    out.push_str(REVIEW_RULES);
    out.push('\n');
    out.push_str(PARTS_RULES);
    out.push_str("\n# 規格・慣習の知識(MadakeCAD同梱)\n\n");
    out.push_str(standards_text().trim_end());
    out.push('\n');
    if let Some((path, text)) = user_knowledge(settings) {
        out.push_str(&format!(
            "\n# 追加の知識(設定の知識ファイル: {})\n\n",
            path.display()
        ));
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}

/// 設定の知識ファイル(あれば内容とパス)。読めなければ`None`。
fn user_knowledge(settings: &AppSettings) -> Option<(PathBuf, String)> {
    let path = settings.knowledge_path.clone()?;
    let text = read_knowledge(&path)?;
    Some((path, text))
}

/// 知識ファイルを読む。読めない・空ならNone(理由は標準エラーへ)。
fn read_knowledge(path: &Path) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) if text.trim().is_empty() => None,
        Ok(text) => Some(text),
        Err(e) => {
            // 設定ミスでエージェントが起動しなくなるより、知識抜きで動くほうがまし
            eprintln!("知識ファイルを読めません ({}): {e}", path.display());
            None
        }
    }
}
