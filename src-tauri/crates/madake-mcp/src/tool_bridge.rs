//! 内蔵MCPツールをRust側から呼べるようにする窓口([`madake_agent::ToolBridge`]の実装)。
//!
//! Claude Code CLIバックエンドはCLI自身がHTTPで`/mcp`へつなぐが、Anthropic API直結
//! バックエンドにはその経路が無い。そこで**同じ[`MadakeMcp`]サーバーをプロセス内の
//! パイプ越しに呼ぶMCPクライアント**を1本用意し、APIバックエンドはここを通してツールを
//! 実行する。
//!
//! ツールのディスパッチを書き写すのではなく本物のMCPサーバーを呼ぶので、
//! **ツールを1つ足せばCLI経由でもAPI経由でも同時に使えるようになる**(取りこぼしが出ない)。
//! 編集は当然[`crate::SharedDoc`]のCommandエンジンを通り、undo履歴とpatch配信に乗る。

use std::sync::Arc;

use madake_agent::tools::{ToolBridge, ToolDef, ToolFuture, ToolOutcome};
use rmcp::model::CallToolRequestParams;
use rmcp::service::{serve_directly, RoleClient, RoleServer, RunningService};
use serde_json::Value;

use crate::{MadakeMcp, SharedDoc, SharedParts};

/// プロセス内パイプの容量。図面全体のJSONが1往復で流れても詰まらない大きさにする。
const PIPE_CAPACITY: usize = 1024 * 1024;

/// プロセス内でつながったMCPサーバー+クライアントの組。
struct InProcess {
    /// サーバー側(落とすと接続が切れるので保持する)
    _server: RunningService<RoleServer, MadakeMcp>,
    client: RunningService<RoleClient, ()>,
}

/// 内蔵MCPツールの窓口。
pub struct McpToolBridge {
    doc: SharedDoc,
    parts: SharedParts,
    /// 初回のツール実行時にサーバー/クライアントを立てる(tokioランタイム上で作る必要がある)
    service: tokio::sync::OnceCell<InProcess>,
}

impl McpToolBridge {
    pub fn new(doc: SharedDoc, parts: SharedParts) -> Self {
        Self {
            doc,
            parts,
            service: tokio::sync::OnceCell::new(),
        }
    }

    /// [`SharedDoc`]/[`SharedParts`]から窓口を作って共有ハンドルにする。
    pub fn shared(doc: &SharedDoc, parts: &SharedParts) -> Arc<dyn ToolBridge> {
        Arc::new(Self::new(doc.clone(), parts.clone()))
    }

    async fn service(&self) -> &InProcess {
        self.service
            .get_or_init(|| async {
                let (server_io, client_io) = tokio::io::duplex(PIPE_CAPACITY);
                // initializeハンドシェイクは省く(同一プロセス内で相手が確定しているため)
                let server = serve_directly::<RoleServer, _, _, _, _>(
                    MadakeMcp::new(self.doc.clone(), self.parts.clone()),
                    server_io,
                    None,
                );
                let client = serve_directly::<RoleClient, _, _, _, _>((), client_io, None);
                InProcess {
                    _server: server,
                    client,
                }
            })
            .await
    }
}

impl ToolBridge for McpToolBridge {
    /// 公開しているMCPツールをそのまま返す(CLI経由で見えるものと同一)。
    fn tools(&self) -> Vec<ToolDef> {
        MadakeMcp::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| ToolDef {
                name: tool.name.to_string(),
                description: tool.description.map(|d| d.to_string()).unwrap_or_default(),
                input_schema: Value::Object((*tool.input_schema).clone()),
            })
            .collect()
    }

    fn call(&self, name: &str, input: Value) -> ToolFuture<'_> {
        let name = name.to_string();
        Box::pin(async move {
            let mut params = CallToolRequestParams::default();
            params.name = name.clone().into();
            params.arguments = match input {
                Value::Object(map) => Some(map),
                // 引数なしのツールはnullで来る。空扱いにする
                _ => None,
            };
            match self.service().await.client.call_tool(params).await {
                Ok(result) => ToolOutcome {
                    content: result_text(&result),
                    is_error: result.is_error.unwrap_or(false),
                },
                // 呼び出し自体が失敗した(未知のツール名・引数の型違いなど)。
                // ターンは止めず、モデルへ理由を返して直させる
                Err(e) => ToolOutcome::error(format!("ツール「{name}」の実行に失敗しました: {e}")),
            }
        })
    }
}

/// ツール結果のテキストを取り出して1本にまとめる。
///
/// 内蔵ツールはどれもJSON文字列1本を返すが、将来テキスト以外が混ざっても
/// 落とさずに読める形へ寄せる。
fn result_text(result: &rmcp::model::CallToolResult) -> String {
    let mut parts = Vec::new();
    for block in &result.content {
        let value = serde_json::to_value(block).unwrap_or(Value::Null);
        match value.get("text").and_then(Value::as_str) {
            Some(text) => parts.push(text.to_string()),
            None => parts.push(value.to_string()),
        }
    }
    if parts.is_empty() {
        // 戻り値の無いツール。モデルには「成功した」と分かる形で返す
        return "{\"ok\":true}".to_string();
    }
    parts.join("\n")
}
