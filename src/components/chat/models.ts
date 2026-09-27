// モデルピッカーの選択肢 (A1はAnthropicグループのみ)。
// idはそのまま Claude Code CLI の `--model` へ渡る (nullはCLI側の既定モデル)。

export interface ModelOption {
  id: string | null;
  label: string;
  sub?: string;
}

/** モデルIDは claude-api の現行モデル表に準拠。表示名はカタログ (`chat.modelPicker.*`) から引く。 */
export function anthropicModels(t: (key: string) => string): ModelOption[] {
  return [
    { id: null, label: t("chat.modelPicker.defaultLabel"), sub: t("chat.modelPicker.defaultSub") },
    { id: "claude-fable-5", label: "Claude Fable 5", sub: t("chat.modelPicker.subscription") },
    { id: "claude-opus-5", label: "Claude Opus 5" },
    { id: "claude-sonnet-5", label: "Claude Sonnet 5" },
    { id: "claude-haiku-4-5", label: "Claude Haiku 4.5" },
  ];
}
