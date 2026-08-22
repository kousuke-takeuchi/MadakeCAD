//! エージェントへ毎ターン注入する規格・慣習の知識と、作図ルール(検証ループ)。
//!
//! 組み立てる`--append-system-prompt`の中身は次の順:
//!
//! 1. 図面コンテキスト(呼び出し側が渡す。madake-mcpの`drawing_context`が作る)
//! 2. 作図ルール([`WORKFLOW_RULES`]。編集後は必ず`run_verification`を回す検証ループ)
//! 3. 同梱の規格ノート(`resources/knowledge/standards.md`)
//! 4. 設定[`AppSettings::knowledge_path`]の知識ファイル(あれば。後勝ちで上書きできる)
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
