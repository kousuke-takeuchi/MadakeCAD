// モデルピッカーの選択肢 (A1はAnthropicグループのみ)。
// idはそのまま Claude Code CLI の `--model` へ渡る (nullはCLI側の既定モデル)。

export interface ModelOption {
  id: string | null;
  label: string;
  sub?: string;
}

/** モデルIDは claude-api の現行モデル表に準拠。 */
export const ANTHROPIC_MODELS: ModelOption[] = [
  { id: null, label: "既定モデル", sub: "CLIの設定" },
  { id: "claude-fable-5", label: "Claude Fable 5", sub: "サブスク" },
  { id: "claude-opus-5", label: "Claude Opus 5" },
  { id: "claude-sonnet-5", label: "Claude Sonnet 5" },
  { id: "claude-haiku-4-5", label: "Claude Haiku 4.5" },
];
