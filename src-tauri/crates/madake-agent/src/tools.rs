//! 図面編集ツールの窓口と、Anthropic Messages APIの`tools`配列への変換。
//!
//! Claude Code CLIバックエンドはCLI自身がMCPサーバー(9310/mcp)へ接続してツールを
//! 呼ぶが、API直結バックエンドにはその仕組みが無い。そこで**同じ内蔵MCPツールを
//! [`ToolBridge`]としてRust側から呼べるように**しておき、定義をAPIのツール定義へ
//! 変換して渡す。実装(`madake-mcp`側)はMCPサーバーそのものを呼ぶので、
//! 編集は今までどおり全てCommandエンジンを通る。

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// エージェントへ見せるツール1件の定義(MCPの`tools/list`と同じ3項目)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    /// JSON Schema(オブジェクト型)。
    pub input_schema: Value,
}

/// ツール1回の実行結果。
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutcome {
    /// モデルへ返す本文(通常はJSON文字列)
    pub content: String,
    /// 失敗したか(APIの`tool_result.is_error`になる)
    pub is_error: bool,
}

impl ToolOutcome {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// [`ToolBridge::call`]の戻り値(トレイトオブジェクトのままawaitできる形)。
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = ToolOutcome> + Send + 'a>>;

/// 図面編集ツールの窓口。実装はmadake-mcp側(内蔵MCPサーバーを呼ぶ)。
///
/// **新しい編集経路は作らない**: 呼ぶ先はUIやClaude Code CLIと同じMCPツールなので、
/// 編集はCommandエンジン・undo履歴・patch配信をそのまま通る。
pub trait ToolBridge: Send + Sync + 'static {
    /// 公開するツールの一覧。
    fn tools(&self) -> Vec<ToolDef>;
    /// ツールを1回実行する。失敗も[`ToolOutcome::error`]として返し、Errにはしない
    /// (モデルへ結果として返して直させるため)。
    fn call(&self, name: &str, input: Value) -> ToolFuture<'_>;
}

/// APIのツール名に使える文字数の上限。
const MAX_TOOL_NAME_LEN: usize = 128;

/// MCPツール定義 → Anthropic Messages APIの`tools`配列。
///
/// - 名前はそのまま。APIが許さない文字は`_`へ寄せ、長すぎる名前は切り詰める
/// - 説明が空のツールには名前入りの代わりの文を入れる(説明無しのツールを渡さない)
/// - 入力スキーマはオブジェクト型に正規化する(`$schema`宣言は落とす)
pub fn api_tool_definitions(defs: &[ToolDef]) -> Vec<Value> {
    defs.iter()
        .map(|def| {
            serde_json::json!({
                "name": api_tool_name(&def.name),
                "description": api_tool_description(&def.name, &def.description),
                "input_schema": api_input_schema(&def.input_schema),
            })
        })
        .collect()
}

/// APIが許す文字(英数字・`_`・`-`)だけに寄せた名前。
fn api_tool_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(MAX_TOOL_NAME_LEN)
        .collect();
    if sanitized.is_empty() {
        "tool".to_string()
    } else {
        sanitized
    }
}

fn api_tool_description(name: &str, description: &str) -> String {
    if description.trim().is_empty() {
        format!("MadakeCADのツール「{name}」")
    } else {
        description.to_string()
    }
}

/// APIが受け付けるオブジェクト型のJSON Schemaへ整える。
fn api_input_schema(schema: &Value) -> Value {
    let mut object: Map<String, Value> = match schema {
        Value::Object(map) => map.clone(),
        // スキーマが無い・形が違うツールも渡せるように、空のオブジェクト型にする
        _ => Map::new(),
    };
    // `$schema`はスキーマの方言宣言で、APIが必要とするのは形そのものだけ
    object.remove("$schema");
    object.insert("type".to_string(), Value::String("object".to_string()));
    object
        .entry("properties".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !object["properties"].is_object() {
        object.insert("properties".to_string(), Value::Object(Map::new()));
    }
    Value::Object(object)
}
