// チャット入力(送信/中断/トークン表示)の共通ロジック。
// 左ドックのエージェントタブ (ChatDock) と、キャンバス左下の浮き入力カード
// (ChatPanel) が同じ振る舞いを共有するために切り出したもの。
// 図面の編集は一切行わない (エージェントがMCP→Commandエンジン経由で編集する)。

import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { i18n } from "../i18n";
import { useChatStore, type ChatMessage } from "../stores/chat";
import { useUiStore } from "../stores/ui";

/**
 * 会話の使用量を足し合わせる。
 *
 * プロバイダによって埋まる欄が違う: Claude系はトークン数、GitHub Copilotは
 * 「プレミアムリクエスト」(小数)。数え方が違うので足さずに別々に持つ。
 */
export function sumUsage(messages: ChatMessage[]) {
  let input = 0;
  let output = 0;
  let premium = 0;
  for (const m of messages) {
    if (!m.usage) continue;
    input += m.usage.input_tokens;
    output += m.usage.output_tokens;
    premium += m.usage.premium_requests ?? 0;
  }
  return { input, output, tokens: input + output, premium };
}

export function useChatComposer() {
  const store = useChatStore();
  const ui = useUiStore();
  const { t } = useI18n();

  /** 入力途中テキスト。ドックと浮きカードで共有する (uiストアが保持)。 */
  const draft = computed({
    get: () => ui.chatDraft,
    set: (v: string) => ui.setChatDraft(v),
  });

  const canSend = computed(() => draft.value.trim().length > 0 && !store.streaming);

  /** 会話全体の使用量の合計 (プロバイダによって埋まる欄が違う)。 */
  const usageTotals = computed(() => sumUsage(store.messages));

  /** ⚡バッジ: 会話のトークン累計 (未使用なら並列エージェント数=1x)。 */
  const tokenBadge = computed(() => {
    const total = usageTotals.value.tokens;
    if (total === 0) return "1x";
    return total >= 1000 ? `${(total / 1000).toFixed(1)}k` : String(total);
  });

  /**
   * ⚡バッジの内訳 (ホバーで出る説明)。
   *
   * GitHub Copilotはトークンではなく「プレミアムリクエスト」で残量を数えるため、
   * 消費があるときだけその行を足す (他のプロバイダでは常に0なので出ない)。
   */
  const usageTooltip = computed(() => {
    const { input, output, tokens, premium } = usageTotals.value;
    if (tokens === 0 && premium === 0) return "";
    const lines = [
      t("chat.usageTooltip", {
        tokens: tokens.toLocaleString(),
        input: input.toLocaleString(),
        output: output.toLocaleString(),
      }),
    ];
    if (premium > 0) {
      lines.push(t("chat.usagePremium", { premium: Math.round(premium * 100) / 100 }));
    }
    return lines.join("\n");
  });

  /**
   * プロンプトを送信する。
   *
   * 浮きカードには会話エリアが無いため、送信したら必ず左ドックのエージェントタブを
   * 開いて応答を見せる (panelOpen="expanded" = ドックのエージェントタブ表示)。
   */
  async function submit() {
    if (!canSend.value) return;
    const text = draft.value;
    draft.value = "";
    ui.openAgentTab();
    const id = await store.send(text);
    if (!id) ui.log(i18n.global.t("chat.composer.sendFailedLog"));
  }

  function onKeydown(ev: KeyboardEvent) {
    if (ev.key !== "Enter" || ev.shiftKey || ev.isComposing) return;
    ev.preventDefault();
    void submit();
  }

  /** ストリーミング中断。pending会話などで失敗してもUIを固めない。 */
  async function onCancel() {
    try {
      await store.cancel();
    } catch (e) {
      ui.log(i18n.global.t("chat.composer.cancelFailedLog", { error: String(e) }));
    }
  }

  return { store, ui, draft, canSend, tokenBadge, usageTooltip, submit, onKeydown, onCancel };
}
