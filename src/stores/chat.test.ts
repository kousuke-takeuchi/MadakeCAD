import { setActivePinia, createPinia } from "pinia";
import { nextTick, watch } from "vue";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import {
  useChatStore,
  agentApi,
  summarizeToolUse,
  shortToolName,
  appliedCommandCount,
  normalizeConversation,
  conversationTitle,
  conversationMeta,
  formatRelativeTime,
  sortedConversations,
  emptyConversationTitle,
  type AgentEvent,
  type ChatConversation,
  type ChatMessage,
  conversationColor,
} from "./chat";
import { agentColorAt } from "../canvas/theme";

const CONV = "11111111-1111-4111-8111-111111111111";
const TURN = "33333333-3333-4333-8333-333333333333";
const CONV2 = "22222222-2222-4222-8222-222222222222";

type Store = ReturnType<typeof useChatStore>;

function feed(store: Store, events: AgentEvent[], id = CONV) {
  for (const event of events) store.applyAgentEvent({ conversation_id: id, event });
}

/** ターン通し番号つきでイベントを流す(キャンセル後の遅延イベント判定に使う)。 */
function feedSeq(store: Store, turnSeq: number, events: AgentEvent[], id = CONV) {
  for (const event of events) {
    store.applyAgentEvent({ conversation_id: id, turn_seq: turnSeq, event });
  }
}

/** イベントの宛先となる既知の会話をストアへ用意する(未知idのイベントは無視されるため)。 */
function seed(store: Store, id = CONV) {
  store.conversations.push({ id, session_id: null, messages: [], model: null, updated_at: 0 });
  if (!store.activeId) store.activeId = id;
}

/** 保留中のマイクロタスクを吐き出す(fire-and-forgetの再取得の完了を待つ)。 */
async function flush() {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("chat store: applyAgentEvent", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: 既知の会話ではtext_deltaが進行中メッセージへ連結される
  it("text deltas append to the in-progress message of a known conversation", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "session_started", session_id: "sess-1" },
      { type: "text_delta", text: "ヒューズ" },
      { type: "text_delta", text: "を追加します" },
    ]);

    expect(store.conversations).toHaveLength(1);
    expect(store.activeId).toBe(CONV);
    expect(store.conversations[0].session_id).toBe("sess-1");
    expect(store.messages).toHaveLength(1);
    expect(store.messages[0].role).toBe("assistant");
    expect(store.messages[0].text).toBe("ヒューズを追加します");
    expect(store.messages[0].streaming).toBe(true);
    expect(store.streaming).toBe(true);
    expect(store.streamingMessage).toBe(store.messages[0]);
  });

  // ja: turn_completedでストリーミングを解除し、トークン使用量を確定する
  it("turn_completed stops streaming and finalizes the token usage", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "done" },
      {
        type: "turn_completed",
        result: "done",
        usage: {
          input_tokens: 10,
          output_tokens: 20,
          cache_creation_input_tokens: 0,
          cache_read_input_tokens: 5,
          total_cost_usd: 0.01,
        },
      },
    ]);

    expect(store.streaming).toBe(false);
    expect(store.messages[0].streaming).toBe(false);
    expect(store.messages[0].usage?.output_tokens).toBe(20);
    expect(store.streamingMessage).toBeNull();
  });

  // ja: デルタが来なかった場合はturn_completedのresultで本文を埋める
  it("when no deltas arrived, the body is filled from turn_completed's result", async () => {
    // 送信時に積まれる空のアシスタントターンを再現する
    vi.spyOn(agentApi, "send").mockResolvedValue(CONV);
    const store = useChatStore();
    await store.send("配置して");

    feed(store, [{ type: "turn_completed", result: "配置しました", usage: null }]);

    expect(store.messages.map((m) => m.text)).toEqual(["配置して", "配置しました"]);
    expect(store.messages[1].streaming).toBe(false);
    expect(store.streaming).toBe(false);
  });

  // ja: ツール呼び出しはチップになり、idで成功/失敗が確定する
  it("tool calls become chips, resolved to success/failure by id", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      {
        type: "tool_use_started",
        id: "t1",
        tool: "mcp__madakecad__place_symbol",
        input: { symbol_id: "fuse", reference: "F2", value: "5A", x: 140, y: 90 },
      },
      {
        type: "tool_use_started",
        id: "t2",
        tool: "mcp__madakecad__export_svg",
        input: { path: "/tmp/a.svg" },
      },
      { type: "tool_use_finished", id: "t1", tool: "mcp__madakecad__place_symbol", is_error: false },
      { type: "tool_use_finished", id: "t2", tool: "mcp__madakecad__export_svg", is_error: true },
    ]);

    const calls = store.messages[0].tool_calls;
    expect(calls).toHaveLength(2);
    expect(calls[0].status).toBe("ok");
    expect(calls[0].summary).toBe("Placed fuse F2 5A at (140,90)");
    expect(calls[1].status).toBe("error");
  });

  // ja: 同じtool_use_idの重複開始は無視される
  it("duplicate tool starts with the same id are ignored", () => {
    const store = useChatStore();
    seed(store);
    const started: AgentEvent = {
      type: "tool_use_started",
      id: "t1",
      tool: "mcp__madakecad__get_project",
      input: {},
    };
    feed(store, [started, started]);

    expect(store.messages[0].tool_calls).toHaveLength(1);
  });

  // ja: id無しのtool_use_finishedは同名の実行中チップへ対応付けられる
  it("tool completions without an id match the running chip of the same name", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "tool_use_started", id: "", tool: "mcp__madakecad__undo", input: {} },
      { type: "tool_use_finished", id: "", tool: "mcp__madakecad__undo", is_error: false },
    ]);

    expect(store.messages[0].tool_calls[0].status).toBe("ok");
  });

  // ja: errorイベントはメッセージにエラーを付与し、ストリーミングを解除する
  it("an error event marks the message and stops streaming", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "途中" },
      { type: "error", message: "claude が異常終了しました" },
    ]);

    expect(store.messages[0].error).toBe("claude が異常終了しました");
    expect(store.messages[0].streaming).toBe(false);
    expect(store.streaming).toBe(false);
  });

  // ja: turn_appliedでrevision範囲を記録し、適用済みとして扱う
  it("turn_applied records the revision range and marks the turn applied", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      { type: "turn_applied", start_revision: 7, end_revision: 10 },
    ]);

    // revision範囲は表示専用。件数はイベントに含まれないので「1件以上」として記録する
    expect(store.messages[0].applied_revisions).toEqual({ start: 7, end: 10 });
    expect(appliedCommandCount(store.messages[0])).toBeGreaterThan(0);
    expect(store.messages[0].undone).toBe(false);
  });

  // ja: turn_appliedはターンの安定IDを載せ、「元に戻す」の対象指定に使える
  it("turn_applied carries the stable turn id used to address the undo", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      { type: "turn_applied", turn_id: TURN, start_revision: 4, end_revision: 6 },
    ]);

    expect(store.messages[0].turn_id).toBe(TURN);
  });

  // ja: turn_appliedにundo深さがあれば正確な編集件数を記録する
  it("an undo depth on turn_applied records the exact edit count", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      {
        type: "turn_applied",
        start_revision: 7,
        end_revision: 12,
        start_undo_depth: 3,
        end_undo_depth: 6,
      },
    ]);

    // revision差(5)ではなくundo深さの増分(3)が件数
    expect(appliedCommandCount(store.messages[0])).toBe(3);
    expect(store.messages[0].applied_undo_depth).toEqual({ start: 3, end: 6 });
  });

  // ja: イベントを畳み込んだ会話はupdated_atが進む(履歴の最新順に反映)
  it("folding events advances the conversation's updated_at (newest-first history)", () => {
    const store = useChatStore();
    seed(store);
    expect(store.conversations[0].updated_at).toBe(0);
    const before = Date.now();
    feed(store, [{ type: "text_delta", text: "作業中" }]);
    expect(store.conversations[0].updated_at).toBeGreaterThanOrEqual(before);
  });

  // ja: 完了後の新しいデルタは新しいターンを開始する
  it("a delta after completion starts a new turn", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "1回目" },
      { type: "turn_completed", result: "1回目", usage: null },
      { type: "text_delta", text: "2回目" },
    ]);

    expect(store.messages.map((m) => m.text)).toEqual(["1回目", "2回目"]);
    expect(store.streaming).toBe(true);
  });

  // ja: 複数会話は独立に畳み込まれ、streamingは開いている会話だけを表す
  it("multiple conversations fold independently; streaming reflects only the open conversation", () => {
    const store = useChatStore();
    seed(store, CONV);
    seed(store, CONV2);
    feed(store, [{ type: "text_delta", text: "A" }], CONV);
    feed(store, [{ type: "text_delta", text: "B" }], CONV2);
    feed(store, [{ type: "turn_completed", result: "A", usage: null }], CONV);

    expect(store.conversations.map((c) => c.id)).toEqual([CONV, CONV2]);
    expect(store.activeId).toBe(CONV);
    expect(store.conversations[0].messages[0].text).toBe("A");
    expect(store.conversations[1].messages[0].text).toBe("B");
    // 開いている会話(CONV)は答え終わった。別会話(CONV2)はまだ答えている
    expect(store.streaming).toBe(false);
    expect(store.anyStreaming).toBe(true);
    expect(store.runningIds).toEqual([CONV2]);

    feed(store, [{ type: "turn_completed", result: "B", usage: null }], CONV2);
    expect(store.anyStreaming).toBe(false);
    expect(store.runningIds).toEqual([]);
  });

  // ja: 未知のconversation_idでは幽霊会話を作らず一覧を取り直す
  it("an unknown conversation id refetches the list instead of creating a ghost", async () => {
    const list = vi.spyOn(agentApi, "listConversations").mockResolvedValue([]);
    const store = useChatStore();

    feed(store, [{ type: "text_delta", text: "別クライアントのターン" }], CONV2);

    expect(store.conversations).toEqual([]);
    expect(store.messages).toEqual([]);
    expect(store.activeId).toBeNull();
    expect(list).toHaveBeenCalledTimes(1);

    // 再取得の最中に続けて届いても多重には走らせない
    feed(store, [{ type: "text_delta", text: "続き" }], CONV2);
    expect(list).toHaveBeenCalledTimes(1);

    // 再取得が終われば次の未知イベントで改めて取り直す
    await flush();
    feed(store, [{ type: "text_delta", text: "また" }], CONV2);
    expect(list).toHaveBeenCalledTimes(2);
  });

  // ja: 自分のターンが進行中の間は、未知会話のイベントで一覧を取り直さない
  it("while our own turn is running, unknown-conversation events do not trigger a refetch", () => {
    const list = vi.spyOn(agentApi, "listConversations").mockResolvedValue([]);
    const store = useChatStore();
    seed(store);
    feed(store, [{ type: "text_delta", text: "進行中" }], CONV);

    feed(store, [{ type: "text_delta", text: "別クライアント" }], CONV2);

    // 再取得は会話を丸ごと差し替えるので、進行中メッセージを壊さないよう見送る
    expect(list).not.toHaveBeenCalled();
    expect(store.conversations).toHaveLength(1);
    expect(store.messages[0].streaming).toBe(true);
  });

  // ja: 進行中ターンの無い会話へのturn_completed/errorは捨てられる
  it("turn_completed/error for a conversation with no running turn is dropped", () => {
    const store = useChatStore();
    seed(store);

    // キャンセル後に遅れて届いた前ターンのイベントを想定
    feed(store, [
      { type: "error", message: "キャンセルされました" },
      { type: "turn_completed", result: "遅れて届いた結果", usage: null },
    ]);

    expect(store.messages).toEqual([]);
    expect(store.streaming).toBe(false);
  });
});

describe("chat store: アクション", () => {
  beforeEach(() => {
    // agentApiはモジュールレベルの共有オブジェクト。テスト間でスパイを戻す
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: 送信に失敗したら、その理由が最初の1通目からチャットに表示される
  it("a failed first send shows the reason in the chat right away", async () => {
    vi.spyOn(agentApi, "send").mockRejectedValue(
      new Error("Anthropic APIキーが設定されていません(設定 > エージェント で入力してください)"),
    );
    const store = useChatStore();

    // 画面が更新されるか (= 変更がリアクティブに届くか) を監視する
    let shown: string | null | undefined;
    watch(
      () => store.conversations[0]?.messages[1]?.error,
      (error) => {
        shown = error;
      },
    );

    const id = await store.send("こんにちは");
    await nextTick();

    expect(id).toBeNull();
    expect(shown).toContain("APIキー");
    expect(store.messages[1].streaming).toBe(false);
    expect(store.streaming).toBe(false);
  });

  // ja: sendは会話を新規作成し、サーバー採番のidを引き取る
  it("send creates a conversation and adopts the server-assigned id", async () => {
    const send = vi.spyOn(agentApi, "send").mockResolvedValue(CONV);
    const store = useChatStore();
    store.setModel("claude-opus-4");

    const id = await store.send("  F2を追加して  ");

    expect(id).toBe(CONV);
    expect(send).toHaveBeenCalledWith(null, "F2を追加して", "claude-opus-4");
    expect(store.conversations).toHaveLength(1);
    expect(store.activeId).toBe(CONV);
    expect(store.messages.map((m) => m.role)).toEqual(["user", "assistant"]);
    expect(store.messages[0].text).toBe("F2を追加して");
    expect(store.streaming).toBe(true);

    // 2通目は既存の会話idで送る
    send.mockResolvedValue(CONV);
    feed(store, [{ type: "turn_completed", result: "ok", usage: null }]);
    await store.send("次の指示");
    expect(send).toHaveBeenLastCalledWith(CONV, "次の指示", "claude-opus-4");
    expect(store.conversations).toHaveLength(1);
  });

  // ja: send解決前に届いたイベントも同じ会話へ入る
  it("events arriving before send resolves still land in the same conversation", async () => {
    let resolveSend: (id: string) => void = () => {};
    vi.spyOn(agentApi, "send").mockImplementation(
      () => new Promise<string>((r) => (resolveSend = r)),
    );
    const store = useChatStore();

    const pending = store.send("配線して");
    feed(store, [{ type: "text_delta", text: "はい" }]);
    resolveSend(CONV);
    await pending;

    expect(store.conversations).toHaveLength(1);
    expect(store.conversations[0].id).toBe(CONV);
    expect(store.activeId).toBe(CONV);
    expect(store.messages.map((m) => m.text)).toEqual(["配線して", "はい"]);
  });

  // ja: 空プロンプトとストリーミング中の送信は無視される
  it("empty prompts and sends during streaming are ignored", async () => {
    const send = vi.spyOn(agentApi, "send").mockResolvedValue(CONV);
    const store = useChatStore();

    expect(await store.send("   ")).toBeNull();
    expect(send).not.toHaveBeenCalled();

    await store.send("1通目");
    expect(await store.send("2通目")).toBeNull();
    expect(send).toHaveBeenCalledTimes(1);
  });

  // ja: send失敗時はメッセージにエラーを載せてストリーミングを解除する
  it("a failed send marks the message with the error and stops streaming", async () => {
    vi.spyOn(agentApi, "send").mockRejectedValue(new Error("claude が見つかりません"));
    const store = useChatStore();

    expect(await store.send("やって")).toBeNull();
    expect(store.messages[1].error).toBe("claude が見つかりません");
    expect(store.streaming).toBe(false);
  });

  // ja: 初回送信に失敗した会話でも、再送はローカルidを渡さず新規として送れる
  it("after a failed first send, retrying sends as new without leaking the local id", async () => {
    const send = vi
      .spyOn(agentApi, "send")
      .mockRejectedValueOnce(new Error("claude が見つかりません"));
    const store = useChatStore();

    expect(await store.send("やって")).toBeNull();
    // ローカル会話はpendingのまま残り、idはまだ採番前
    expect(store.conversations).toHaveLength(1);
    expect(store.activeId).toBe(store.pendingLocalId);
    expect(store.activeId?.startsWith("local-")).toBe(true);

    send.mockResolvedValueOnce(CONV);
    const id = await store.send("やっぱりやって");

    // 2回目もconversation_id=null(local-idは絶対にAPIへ渡さない)
    expect(send).toHaveBeenNthCalledWith(2, null, "やっぱりやって", null);
    expect(id).toBe(CONV);
    expect(store.conversations).toHaveLength(1);
    expect(store.activeId).toBe(CONV);
    expect(store.pendingLocalId).toBeNull();
  });

  // ja: cancelは対象の会話だけを止め、他会話のストリーミングは残す
  it("cancel stops only the target conversation, leaving others streaming", async () => {
    const cancel = vi.spyOn(agentApi, "cancel").mockResolvedValue();
    const store = useChatStore();
    seed(store, CONV);
    seed(store, CONV2);
    feed(store, [{ type: "text_delta", text: "途中" }], CONV);
    feed(store, [{ type: "text_delta", text: "別会話" }], CONV2);

    await store.cancel();

    expect(cancel).toHaveBeenCalledWith(CONV);
    expect(store.conversations[0].messages[0].streaming).toBe(false);
    expect(store.conversations[0].messages[0].error).toBe("キャンセルされました");
    expect(store.conversations[1].messages[0].streaming).toBe(true);
    // 開いている会話は止まったが、別会話はまだ答えている
    expect(store.streaming).toBe(false);
    expect(store.anyStreaming).toBe(true);
    expect(store.runningIds).toEqual([CONV2]);
  });

  // ja: 採番前(local-)の会話ではcancel APIを呼ばずローカル整理だけ行う
  it("cancel on a not-yet-assigned (local-) conversation skips the API and cleans up locally", async () => {
    const cancel = vi.spyOn(agentApi, "cancel").mockResolvedValue();
    vi.spyOn(agentApi, "send").mockImplementation(() => new Promise<string>(() => {}));
    const store = useChatStore();

    void store.send("配線して");
    expect(store.streaming).toBe(true);

    await expect(store.cancel()).resolves.toBeUndefined();

    expect(cancel).not.toHaveBeenCalled();
    expect(store.streaming).toBe(false);
    expect(store.messages[1].streaming).toBe(false);
  });

  // ja: 採番前のcancelは採番後にサーバーへ中断を送る
  it("a cancel issued before id assignment is sent to the server once the id arrives", async () => {
    const cancel = vi.spyOn(agentApi, "cancel").mockResolvedValue();
    let resolveSend: (id: string) => void = () => undefined;
    vi.spyOn(agentApi, "send").mockImplementation(
      () => new Promise<string>((resolve) => (resolveSend = resolve)),
    );
    const store = useChatStore();

    const sending = store.send("配線して");
    await store.cancel();
    expect(cancel).not.toHaveBeenCalled();

    // send解決(採番) → 走り続けているサーバー側ターンへ中断が飛ぶ
    resolveSend(CONV);
    await sending;
    expect(cancel).toHaveBeenCalledWith(CONV);
    expect(store.cancelRequested).toBe(false);
  });

  // ja: キャンセル後に遅れて届いた同じターンのイベントは捨てられる
  it("events that arrive late from a cancelled turn are discarded", async () => {
    vi.spyOn(agentApi, "cancel").mockResolvedValue();
    const store = useChatStore();
    seed(store);
    feedSeq(store, 7, [{ type: "text_delta", text: "途中まで" }]);

    await store.cancel();

    // CLIの停止が間に合わず、中断したターンの出力が遅れて届く
    feedSeq(store, 7, [
      { type: "text_delta", text: "遅れて届いた続き" },
      { type: "turn_completed", result: "完了しました", usage: null },
      { type: "turn_applied", turn_id: TURN, start_revision: 4, end_revision: 6 },
    ]);

    expect(store.messages).toHaveLength(1);
    expect(store.messages[0].text).toBe("途中まで");
    expect(store.messages[0].error).toBe("キャンセルされました");
    expect(store.messages[0].streaming).toBe(false);
    expect(appliedCommandCount(store.messages[0])).toBe(0);
    expect(store.streaming).toBe(false);
  });

  // ja: キャンセル後に始めた新しいターン(より大きい通し番号)のイベントは通る
  it("events of a new turn started after a cancel (higher sequence number) still apply", async () => {
    vi.spyOn(agentApi, "cancel").mockResolvedValue();
    const store = useChatStore();
    seed(store);
    feedSeq(store, 7, [{ type: "text_delta", text: "途中まで" }]);
    await store.cancel();

    feedSeq(store, 8, [
      { type: "text_delta", text: "やり直しました" },
      { type: "turn_completed", result: "やり直しました", usage: null },
    ]);

    expect(store.messages).toHaveLength(2);
    expect(store.messages[1].text).toBe("やり直しました");
    expect(store.messages[1].streaming).toBe(false);
  });

  // ja: 通し番号の無いイベント(旧サーバー)はキャンセル後でも捨てない
  it("events without a sequence number (older server) are kept even after a cancel", async () => {
    vi.spyOn(agentApi, "cancel").mockResolvedValue();
    const store = useChatStore();
    seed(store);
    feedSeq(store, 7, [{ type: "text_delta", text: "途中まで" }]);
    await store.cancel();

    feed(store, [{ type: "text_delta", text: "旧サーバーの続き" }]);

    expect(store.messages[store.messages.length - 1].text).toContain("旧サーバーの続き");
  });

  // ja: cancel APIが失敗してもストリーミング解除は完了する
  it("streaming stops even if the cancel API fails", async () => {
    vi.spyOn(agentApi, "cancel").mockRejectedValue(new Error("Link API 400"));
    const store = useChatStore();
    seed(store);
    feed(store, [{ type: "text_delta", text: "途中" }]);

    await expect(store.cancel()).resolves.toBeUndefined();

    expect(store.streaming).toBe(false);
    expect(store.messages[0].streaming).toBe(false);
  });

  // ja: 会話が無い状態のcancelは何もせず落ちない
  it("cancel with no conversation does nothing and never crashes", async () => {
    const cancel = vi.spyOn(agentApi, "cancel").mockResolvedValue();
    const store = useChatStore();

    await expect(store.cancel()).resolves.toBeUndefined();

    expect(cancel).not.toHaveBeenCalled();
    expect(store.streaming).toBe(false);
  });

  // ja: undoTurnはターン安定IDでAPIを呼び、適用済み表示を取り下げる
  it("undoTurn calls the API with the stable turn id and withdraws the applied badge", async () => {
    const undoTurn = vi.spyOn(agentApi, "undoTurn").mockResolvedValue();
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      { type: "turn_applied", turn_id: TURN, start_revision: 4, end_revision: 6 },
    ]);

    await store.undoTurn(CONV, TURN);

    expect(undoTurn).toHaveBeenCalledWith(CONV, TURN);
    expect(store.messages[0].undone).toBe(true);
    expect(appliedCommandCount(store.messages[0])).toBe(0);
    expect(store.messages[0].applied_revisions.end).toBe(
      store.messages[0].applied_revisions.start,
    );

    // 巻き戻し済み(編集の残っていない)ターンではAPIを呼ばない
    await store.undoTurn(CONV, TURN);
    expect(undoTurn).toHaveBeenCalledTimes(1);
  });

  // ja: 会話に無いターンIDのundoTurnはAPIを呼ばない
  it("undoTurn never calls the API for a turn id the conversation does not have", async () => {
    const undoTurn = vi.spyOn(agentApi, "undoTurn").mockResolvedValue();
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      { type: "turn_applied", turn_id: TURN, start_revision: 4, end_revision: 6 },
    ]);

    await store.undoTurn(CONV, "44444444-4444-4444-8444-444444444444");
    await store.undoTurn(CONV, "");

    expect(undoTurn).not.toHaveBeenCalled();
    expect(store.messages[0].undone).toBe(false);
  });

  // ja: undoTurnのサーバー拒否は呼び出し元へ投げられ、適用済み表示は変わらない
  it("a server-rejected undoTurn propagates the error and keeps the applied badge", async () => {
    // 手編集との衝突・送信中などのガードは400で返る
    vi.spyOn(agentApi, "undoTurn").mockRejectedValue(
      new Error("Link API 400: このターンの編集は現在の図面と衝突するため巻き戻せません"),
    );
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      { type: "turn_applied", turn_id: TURN, start_revision: 4, end_revision: 6 },
    ]);

    await expect(store.undoTurn(CONV, TURN)).rejects.toThrow("衝突するため巻き戻せません");

    expect(store.messages[0].undone).toBe(false);
    expect(appliedCommandCount(store.messages[0])).toBeGreaterThan(0);
    expect(store.messages[0].applied_revisions).toEqual({ start: 4, end: 6 });
  });

  // ja: 採番前(local-)の会話ではundoTurn APIを呼ばない
  it("undoTurn never calls the API for a not-yet-assigned (local-) conversation", async () => {
    const undoTurn = vi.spyOn(agentApi, "undoTurn").mockResolvedValue();
    vi.spyOn(agentApi, "send").mockImplementation(() => new Promise<string>(() => {}));
    const store = useChatStore();

    void store.send("配線して");
    const localId = store.activeId as string;
    store.messages[1].applied_undo_depth = { start: 0, end: 2 };
    store.messages[1].turn_id = TURN;

    await store.undoTurn(localId, TURN);

    expect(undoTurn).not.toHaveBeenCalled();
    expect(store.messages[1].undone).toBe(false);
  });

  // ja: loadConversationsはRust表現を表示用モデルへ正規化する
  it("loadConversations normalizes the Rust representation into the display model", async () => {
    vi.spyOn(agentApi, "listConversations").mockResolvedValue([
      {
        id: CONV,
        session_id: "sess-1",
        model: null,
        messages: [
          {
            turn_id: TURN,
            role: "user",
            text: "F2を追加",
            applied_revisions: { start: 3, end: 3 },
            applied_undo_depth: { start: 1, end: 1 },
          },
          {
            turn_id: TURN,
            role: "assistant",
            text: "追加しました",
            // ターン中にエージェントがundoを挟むとrevision差(4)は積まれた数と一致しない
            applied_revisions: { start: 3, end: 7 },
            applied_undo_depth: { start: 1, end: 3 },
            tool_calls: [
              {
                id: "t1",
                tool: "mcp__madakecad__place_symbol",
                input: { symbol_id: "fuse", reference: "F2", x: 140, y: 90 },
                finished: true,
                is_error: false,
              },
            ],
            error: null,
          },
        ],
      },
    ]);
    const store = useChatStore();

    await store.loadConversations();

    expect(store.activeId).toBe(CONV);
    expect(store.messages).toHaveLength(2);
    expect(store.messages[1].tool_calls[0].status).toBe("ok");
    expect(store.messages[1].tool_calls[0].summary).toBe("Placed fuse F2 at (140,90)");
    // 1ターンの2メッセージは同じターンIDを持ち、巻き戻しの対象指定に使える
    expect(store.messages.map((m) => m.turn_id)).toEqual([TURN, TURN]);
    // 巻き戻し回数はrevision差(4)ではなくundo深さの増分(2)
    expect(appliedCommandCount(store.messages[1])).toBe(2);
    expect(store.messages[1].applied_revisions).toEqual({ start: 3, end: 7 });
    expect(store.streaming).toBe(false);
  });

  // ja: normalizeConversationは未完了ツールをrunningとして扱う
  it("normalizeConversation treats unfinished tools as running", () => {
    const conv = normalizeConversation({
      id: CONV,
      session_id: null,
      model: null,
      messages: [
        {
          role: "assistant",
          text: "",
          tool_calls: [
            { id: "t1", tool: "draw_wire", input: {}, finished: false, is_error: false },
          ],
        },
      ],
    });

    expect(conv.messages[0].tool_calls[0].status).toBe("running");
    // 旧`chat.json`にはどちらのフィールドも無い(Rust側はserde defaultで{0,0})
    expect(conv.messages[0].applied_revisions).toEqual({ start: 0, end: 0 });
    expect(conv.messages[0].applied_undo_depth).toEqual({ start: 0, end: 0 });
    expect(appliedCommandCount(conv.messages[0])).toBe(0);
    // updated_atも旧`chat.json`には無い。0は「時刻不明」で相対時刻を出さない印
    expect(conv.updated_at).toBe(0);
  });

  // ja: normalizeConversationはupdated_atをそのまま引き継ぐ
  it("normalizeConversation carries updated_at through unchanged", () => {
    const conv = normalizeConversation({
      id: CONV,
      session_id: null,
      model: null,
      messages: [],
      updated_at: 1_700_000_000_000,
    });

    expect(conv.updated_at).toBe(1_700_000_000_000);
  });

  // ja: subscribeを同時に呼んでも購読は1本だけになる
  it("concurrent subscribe calls result in a single subscription", async () => {
    const unlisten = vi.fn();
    let resolveOn: (fn: () => void) => void = () => {};
    const onEvent = vi
      .spyOn(agentApi, "onEvent")
      .mockImplementation(() => new Promise<() => void>((r) => (resolveOn = r)));
    const store = useChatStore();

    const first = store.subscribe();
    const second = store.subscribe();
    resolveOn(unlisten);
    await Promise.all([first, second]);

    expect(onEvent).toHaveBeenCalledTimes(1);

    store.unsubscribe();
    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(store.unlisten).toBeNull();
    expect(store.subscription).toBeNull();
  });

  // ja: 購読の解決前にunsubscribeしても取りこぼさず閉じられる
  it("unsubscribing before the subscription resolves still closes it cleanly", async () => {
    const unlisten = vi.fn();
    let resolveOn: (fn: () => void) => void = () => {};
    vi.spyOn(agentApi, "onEvent").mockImplementation(
      () => new Promise<() => void>((r) => (resolveOn = r)),
    );
    const store = useChatStore();

    const pending = store.subscribe();
    store.unsubscribe();
    resolveOn(unlisten);
    await pending;

    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(store.unlisten).toBeNull();
    expect(store.subscription).toBeNull();
  });

  // ja: 購読に失敗しても次のsubscribeで張り直せる
  it("after a failed subscription, the next subscribe re-establishes it", async () => {
    const onEvent = vi
      .spyOn(agentApi, "onEvent")
      .mockRejectedValueOnce(new Error("SSE接続失敗"))
      .mockResolvedValueOnce(vi.fn());
    const store = useChatStore();

    await expect(store.subscribe()).rejects.toThrow("SSE接続失敗");
    expect(store.subscription).toBeNull();

    await store.subscribe();
    expect(onEvent).toHaveBeenCalledTimes(2);
    expect(store.unlisten).not.toBeNull();
  });

  // ja: newConversationは採番待ち(pendingLocalId)を巻き込まない
  it("newConversation does not disturb a pending local id", async () => {
    let resolveSend: (id: string) => void = () => undefined;
    vi.spyOn(agentApi, "send").mockImplementation(
      () => new Promise<string>((resolve) => (resolveSend = resolve)),
    );
    const store = useChatStore();
    const sending = store.send("配線して");
    const localId = store.pendingLocalId;
    expect(localId).not.toBeNull();

    // 採番前に新規会話を押しても、進行中のsendの採番引き取りは生きている
    store.newConversation();
    expect(store.activeId).toBeNull();
    expect(store.pendingLocalId).toBe(localId);

    resolveSend(CONV);
    await sending;
    expect(store.conversations.some((c) => c.id === CONV)).toBe(true);
  });

  // ja: setModel / setPanel / newConversationがそれぞれの状態を更新する
  it("setModel / setPanel / newConversation update their state", async () => {
    vi.spyOn(agentApi, "send").mockResolvedValue(CONV);
    const store = useChatStore();

    expect(store.panelOpen).toBe("collapsed");
    store.setPanel("expanded");
    expect(store.panelOpen).toBe("expanded");
    store.setPanel("collapsed");
    expect(store.panelOpen).toBe("collapsed");

    await store.send("こんにちは");
    store.setModel("claude-sonnet-4");
    expect(store.activeConversation?.model).toBe("claude-sonnet-4");

    store.newConversation();
    expect(store.activeId).toBeNull();
    expect(store.messages).toEqual([]);
  });
});

describe("summarizeToolUse", () => {
  // ja: 表示時にMCPツール名のプレフィックスを外す
  it("the MCP tool-name prefix is stripped for display", () => {
    expect(shortToolName("mcp__madakecad__place_symbol")).toBe("place_symbol");
    expect(shortToolName("place_symbol")).toBe("place_symbol");
  });

  // ja: place_symbolはシンボル・参照記号・位置で要約される
  it("place_symbol calls summarize as symbol, reference and position", () => {
    expect(
      summarizeToolUse("mcp__madakecad__place_symbol", {
        symbol_id: "fuse",
        reference: "F2",
        value: "5A",
        x: 140,
        y: 90,
      }),
    ).toBe("Placed fuse F2 5A at (140,90)");
    expect(summarizeToolUse("place_symbol", { x: 12.5, y: 7.25 })).toBe(
      "Placed a symbol at (12.5,7.25)",
    );
  });

  // ja: draw_wireは線色・線径・頂点数で要約される
  it("draw_wire calls summarize as color, gauge and point count", () => {
    expect(
      summarizeToolUse("mcp__madakecad__draw_wire", {
        points: [
          { x: 0, y: 0 },
          { x: 10, y: 0 },
          { x: 10, y: 20 },
        ],
        color: "red",
        sq: 0.75,
      }),
      // pointsは頂点列なので数えるのは本数ではなく区間数
    ).toBe("0.75sq red: connected 2 segment(s)");
    expect(
      summarizeToolUse("draw_wire", {
        points: [
          { x: 0, y: 0 },
          { x: 5, y: 0 },
        ],
      }),
    ).toBe("Connected 1 segment(s)");
  });

  // ja: update_entity / execute_commandsはコマンド内容で要約される
  it("update_entity and execute_commands summarize by command content", () => {
    expect(
      summarizeToolUse("update_entity", { entity: { kind: "symbol", reference: "K1" } }),
    ).toBe("Updated symbol K1");
    expect(
      summarizeToolUse("mcp__madakecad__execute_commands", {
        commands: [{ type: "move_entities" }],
      }),
    ).toBe("Ran move entities");
    expect(
      summarizeToolUse("execute_commands", { commands: [{ type: "add_entity" }, { type: "undo" }] }),
    ).toBe("Ran 2 edit commands");
  });

  // ja: 読み取り系・書き出し系ツールも適切に要約される
  it("read and export tools get appropriate summaries", () => {
    expect(summarizeToolUse("mcp__madakecad__get_netlist", {})).toBe("Read the netlist");
    expect(summarizeToolUse("mcp__madakecad__get_project", {})).toBe("Read the whole drawing");
    expect(summarizeToolUse("export_svg", { path: "/tmp/out/a.svg" })).toBe(
      "Exported SVG (a.svg)",
    );
    expect(summarizeToolUse("export_bom", { path: "/tmp/bom.csv" })).toBe(
      "Exported BOM CSV (bom.csv)",
    );
    expect(summarizeToolUse("export_wire_list", { path: "/tmp/w.csv" })).toBe(
      "Exported wire list CSV (w.csv)",
    );
  });

  // チップはツール名を別途描くので、要約側は空を返す(両方返すと
  // 「ToolSearch ToolSearch」のような二重表示になる)。
  // ja: 要約を作れないツールは空文字になり、表示はツール名のみ
  it("tools without a summary show the tool name only", () => {
    expect(summarizeToolUse("mcp__madakecad__future_tool", { a: 1 })).toBe("");
    expect(summarizeToolUse("Bash", null)).toBe("");
    expect(summarizeToolUse("ToolSearch", { query: "select:Read" })).toBe("");
    // ツール名そのものは shortToolName が出す
    expect(shortToolName("mcp__madakecad__future_tool")).toBe("future_tool");
  });
});

describe("会話履歴ポップアップの表示ヘルパー", () => {
  /** 表示ヘルパーの入力に使う最小の会話。 */
  function conv(
    id: string,
    updatedAt: number,
    messages: Partial<ChatMessage>[] = [],
  ): ChatConversation {
    return {
      id,
      session_id: null,
      model: null,
      updated_at: updatedAt,
      messages: messages.map((m) => ({
        role: "user",
        text: "",
        tool_calls: [],
        applied_revisions: { start: 0, end: 0 },
        applied_undo_depth: { start: 0, end: 0 },
        error: null,
        usage: null,
        streaming: false,
        undone: false,
        ...m,
      })) as ChatMessage[],
    };
  }

  // ja: 会話タイトルは最初のユーザー発話の先頭40字
  it("the conversation title is the first 40 chars of the first user message", () => {
    expect(conversationTitle(conv("a", 0, [{ role: "user", text: "リレーK2のb接点を追加" }]))).toBe(
      "リレーK2のb接点を追加",
    );
    // アシスタントの発話しか無くてもユーザー発話を探す
    expect(
      conversationTitle(
        conv("a", 0, [
          { role: "assistant", text: "配置しました" },
          { role: "user", text: "ありがとう" },
        ]),
      ),
    ).toBe("ありがとう");
    // 改行は空白へ畳む
    expect(conversationTitle(conv("a", 0, [{ role: "user", text: "24V系に\nF2を追加" }]))).toBe(
      "24V系に F2を追加",
    );
    const long = "あ".repeat(50);
    expect(conversationTitle(conv("a", 0, [{ role: "user", text: long }]))).toBe(
      `${"あ".repeat(40)}…`,
    );
  });

  // ja: ユーザー発話が無ければ「(空の会話)」になる
  it("without any user message the title is '(empty conversation)'", () => {
    expect(conversationTitle(conv("a", 0))).toBe(emptyConversationTitle());
    expect(conversationTitle(conv("a", 0, [{ role: "user", text: "   " }]))).toBe(
      emptyConversationTitle(),
    );
  });

  // ja: 相対時刻は たった今/N分前/N時間前/昨日/M-D で表示される
  it("relative time renders as just now / N min / N h / yesterday / M-D", () => {
    const now = new Date(2026, 7, 21, 14, 0, 0).getTime();
    expect(formatRelativeTime(now - 30_000, now)).toBe("just now");
    expect(formatRelativeTime(now - 8 * 60_000, now)).toBe("8 min ago");
    expect(formatRelativeTime(now - 3 * 3_600_000, now)).toBe("3 h ago");
    expect(formatRelativeTime(new Date(2026, 7, 20, 22, 0, 0).getTime(), now)).toBe("yesterday");
    expect(formatRelativeTime(new Date(2026, 7, 19, 9, 0, 0).getTime(), now)).toBe("8/19");
  });

  // ja: 時刻不明(旧履歴のupdated_at=0)は相対時刻を出さない
  it("unknown timestamps (legacy updated_at=0) show no relative time", () => {
    const now = Date.now();
    expect(formatRelativeTime(0, now)).toBe("");
    expect(conversationMeta(conv("a", 0, [{ role: "user", text: "x" }]), now)).toBe("1 message");
  });

  // ja: メタ行は時刻・件数・適用済みrevを中黒で連ねる
  it("the meta line joins time, message count and applied rev with a middle dot", () => {
    const now = new Date(2026, 7, 21, 14, 0, 0).getTime();
    const applied = conv("a", now - 8 * 60_000, [
      { role: "user", text: "F2を追加" },
      {
        role: "assistant",
        text: "追加しました",
        applied_revisions: { start: 22, end: 24 },
        applied_undo_depth: { start: 0, end: 2 },
      },
    ]);
    expect(conversationMeta(applied, now)).toBe("8 min ago · 2 messages · applied rev 24");

    // 巻き戻し済みのターンは「適用済み」に数えない
    applied.messages[1].undone = true;
    expect(conversationMeta(applied, now)).toBe("8 min ago · 2 messages");
  });

  // ja: 会話は更新の新しい順に並ぶ(時刻不明は後ろに登録順)
  it("conversations sort newest-first (unknown times last, in insertion order)", () => {
    const list = [conv("old", 100), conv("legacy", 0), conv("new", 300), conv("legacy2", 0)];
    expect(sortedConversations(list).map((c) => c.id)).toEqual([
      "new",
      "old",
      "legacy2",
      "legacy",
    ]);
    // 元の配列は破壊しない
    expect(list.map((c) => c.id)).toEqual(["old", "legacy", "new", "legacy2"]);
  });
});

describe("chat store: 並列エージェント", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: 会話には開始順で編集オーバーレイ色が割り当てられ、表示順を変えても揺れない
  it("gives each conversation a stable overlay color in the order the conversations started", () => {
    const store = useChatStore();
    seed(store, CONV);
    seed(store, CONV2);

    expect(store.conversationColors[CONV]).toBe(agentColorAt(0));
    expect(store.conversationColors[CONV2]).toBe(agentColorAt(1));
    expect(conversationColor(store.conversations, CONV2)).toBe(agentColorAt(1));

    // 新しい会話が上に来る並べ替え(履歴ポップアップの表示順)でも色は変わらない
    store.conversations[1].updated_at = Date.now();
    expect(sortedConversations(store.conversations)[0].id).toBe(CONV2);
    expect(store.conversationColors[CONV2]).toBe(agentColorAt(1));
    // 知らない会話は既定色
    expect(conversationColor(store.conversations, "unknown")).toBe(agentColorAt(0));
  });

  // ja: 別の会話が答えている最中でも、開いている会話からは送信できる
  it("allows sending in the open conversation while another conversation is still answering", async () => {
    const send = vi.spyOn(agentApi, "send").mockResolvedValue(CONV2);
    const store = useChatStore();
    seed(store, CONV);
    seed(store, CONV2);
    // CONVが答えている最中にCONV2へ切り替える
    feed(store, [{ type: "text_delta", text: "作図中" }], CONV);
    store.setActive(CONV2);

    expect(store.streaming).toBe(false);
    expect(await store.send("端子台を1個足して")).toBe(CONV2);

    expect(send).toHaveBeenCalledWith(CONV2, "端子台を1個足して", null);
    expect(store.runningIds).toEqual([CONV, CONV2]);
    expect(store.runningCount).toBe(2);
    // 先に走っている会話のターンは中断されない
    expect(store.conversations[0].messages[0].streaming).toBe(true);
  });

  // ja: 開いている会話が答えている間は、その会話への追加送信をしない
  it("does not send again while the open conversation is still answering", async () => {
    const send = vi.spyOn(agentApi, "send").mockResolvedValue(CONV);
    const store = useChatStore();
    seed(store, CONV);
    feed(store, [{ type: "text_delta", text: "作図中" }], CONV);

    expect(store.streaming).toBe(true);
    expect(await store.send("もう1件")).toBeNull();
    expect(send).not.toHaveBeenCalled();
  });

  // ja: 会話一覧を取り直しても、実行中の会話は実行中のまま表示できる
  it("keeps conversations that are still answering marked as running across a history reload", async () => {
    vi.spyOn(agentApi, "listConversations").mockResolvedValue([
      { id: CONV, session_id: null, model: null, messages: [] },
      {
        id: CONV2,
        session_id: null,
        model: null,
        messages: [{ role: "assistant", text: "途中まで", tool_calls: [] }],
      },
    ]);
    const store = useChatStore();
    seed(store, CONV);
    seed(store, CONV2);
    feed(store, [{ type: "text_delta", text: "途中まで" }], CONV2);
    expect(store.runningIds).toEqual([CONV2]);

    await store.loadConversations();

    expect(store.runningIds).toEqual([CONV2]);
    const running = store.conversations.find((c) => c.id === CONV2)!;
    expect(running.messages[0].streaming).toBe(true);
  });

  // ja: ターンが終わった会話は実行中の一覧から外れる
  it("drops a conversation from the running list once its turn ends", () => {
    const store = useChatStore();
    seed(store, CONV);
    seed(store, CONV2);
    feed(store, [{ type: "text_delta", text: "A" }], CONV);
    feed(store, [{ type: "text_delta", text: "B" }], CONV2);
    expect(store.runningCount).toBe(2);

    feed(store, [{ type: "turn_completed", result: "A", usage: null }], CONV);
    expect(store.runningIds).toEqual([CONV2]);
    feed(store, [{ type: "error", message: "失敗" }], CONV2);
    expect(store.runningIds).toEqual([]);
  });
});
