// チャット入力(送信/中断/トークン表示)の共通ロジック。
// 左ドックのエージェントタブ (ChatDock) と、キャンバス左下の浮き入力カード
// (ChatPanel) が同じ振る舞いを共有するために切り出したもの。
// 図面の編集は一切行わない (エージェントがMCP→Commandエンジン経由で編集する)。

import { computed } from "vue";
import { useChatStore } from "../stores/chat";
import { useUiStore } from "../stores/ui";

export function useChatComposer() {
  const store = useChatStore();
  const ui = useUiStore();

  /** 入力途中テキスト。ドックと浮きカードで共有する (uiストアが保持)。 */
  const draft = computed({
    get: () => ui.chatDraft,
    set: (v: string) => {
      ui.chatDraft = v;
    },
  });

  const canSend = computed(() => draft.value.trim().length > 0 && !store.streaming);

  /** ⚡バッジ: 会話のトークン累計 (未使用なら並列エージェント数=1x)。 */
  const tokenBadge = computed(() => {
    let total = 0;
    for (const m of store.messages) {
      if (m.usage) total += m.usage.input_tokens + m.usage.output_tokens;
    }
    if (total === 0) return "1x";
    return total >= 1000 ? `${(total / 1000).toFixed(1)}k` : String(total);
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
    store.setPanel("expanded");
    ui.setLeftPanelTab("chat");
    const id = await store.send(text);
    if (!id) ui.log("AGENT   送信に失敗しました (詳細はチャットのエラー表示を参照)");
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
      ui.log(`AGENT   キャンセル失敗: ${String(e)}`);
    }
  }

  function notImplemented(label: string) {
    ui.log(`AGENT   ${label}は未実装です (フェーズA2以降)`);
  }

  return { store, ui, draft, canSend, tokenBadge, submit, onKeydown, onCancel, notImplemented };
}
