import { setActivePinia, createPinia } from "pinia";
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
  EMPTY_CONVERSATION_TITLE,
  type AgentEvent,
  type ChatConversation,
  type ChatMessage,
} from "./chat";

const CONV = "11111111-1111-4111-8111-111111111111";
const CONV2 = "22222222-2222-4222-8222-222222222222";

type Store = ReturnType<typeof useChatStore>;

function feed(store: Store, events: AgentEvent[], id = CONV) {
  for (const event of events) store.applyAgentEvent({ conversation_id: id, event });
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

  it("既知の会話でtext_deltaを進行中メッセージへ連結する", () => {
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

  it("turn_completedでストリーミングを解除し、usageを確定する", () => {
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

  it("デルタが来なかった場合はturn_completedのresultで本文を埋める", async () => {
    // 送信時に積まれる空のアシスタントターンを再現する
    vi.spyOn(agentApi, "send").mockResolvedValue(CONV);
    const store = useChatStore();
    await store.send("配置して");

    feed(store, [{ type: "turn_completed", result: "配置しました", usage: null }]);

    expect(store.messages.map((m) => m.text)).toEqual(["配置して", "配置しました"]);
    expect(store.messages[1].streaming).toBe(false);
    expect(store.streaming).toBe(false);
  });

  it("ツール呼び出しをチップ化し、成功/失敗をidで確定する", () => {
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
    expect(calls[0].summary).toBe("fuse F2 5A を (140,90) に配置");
    expect(calls[1].status).toBe("error");
  });

  it("同じtool_use_idの重複開始は無視する", () => {
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

  it("id無しのtool_use_finishedは同名の実行中チップに対応づける", () => {
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "tool_use_started", id: "", tool: "mcp__madakecad__undo", input: {} },
      { type: "tool_use_finished", id: "", tool: "mcp__madakecad__undo", is_error: false },
    ]);

    expect(store.messages[0].tool_calls[0].status).toBe("ok");
  });

  it("errorイベントでメッセージにエラーを付与しストリーミングを解除する", () => {
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

  it("turn_appliedでrevision範囲を記録し、適用済みとして扱う", () => {
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

  it("turn_appliedにundo深さが乗っていれば正確な件数を記録する", () => {
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

  it("イベントを畳み込んだ会話はupdated_atが進む(履歴の最新順に反映)", () => {
    const store = useChatStore();
    seed(store);
    expect(store.conversations[0].updated_at).toBe(0);
    const before = Date.now();
    feed(store, [{ type: "text_delta", text: "作業中" }]);
    expect(store.conversations[0].updated_at).toBeGreaterThanOrEqual(before);
  });

  it("完了後の新しいデルタは新しいターンを開始する", () => {
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

  it("複数会話を独立に畳み込み、片方が進行中ならstreamingを保つ", () => {
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
    expect(store.streaming).toBe(true);

    feed(store, [{ type: "turn_completed", result: "B", usage: null }], CONV2);
    expect(store.streaming).toBe(false);
  });

  it("未知のconversation_idでは幽霊会話を作らず、一覧を取り直す", async () => {
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

  it("自分のターンが進行中の間は、未知会話のイベントで一覧を取り直さない", () => {
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

  it("進行中のターンが無い会話へのturn_completed/errorは捨てる", () => {
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

  it("sendは会話を新規作成し、サーバー採番のidを引き取る", async () => {
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

  it("send解決前に届いたイベントも同じ会話へ入る", async () => {
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

  it("空プロンプトとストリーミング中の送信は無視する", async () => {
    const send = vi.spyOn(agentApi, "send").mockResolvedValue(CONV);
    const store = useChatStore();

    expect(await store.send("   ")).toBeNull();
    expect(send).not.toHaveBeenCalled();

    await store.send("1通目");
    expect(await store.send("2通目")).toBeNull();
    expect(send).toHaveBeenCalledTimes(1);
  });

  it("send失敗時はメッセージにエラーを載せてストリーミングを解除する", async () => {
    vi.spyOn(agentApi, "send").mockRejectedValue(new Error("claude が見つかりません"));
    const store = useChatStore();

    expect(await store.send("やって")).toBeNull();
    expect(store.messages[1].error).toBe("claude が見つかりません");
    expect(store.streaming).toBe(false);
  });

  it("初回送信に失敗した会話でも、再送はローカルidを渡さず新規扱いで送れる", async () => {
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

  it("cancelは対象の会話だけを止め、他会話のストリーミングは残す", async () => {
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
    // 他会話がまだ進行中なのでグローバルなstreamingは立ったまま
    expect(store.streaming).toBe(true);
  });

  it("採番前(local-)の会話ではcancel APIを呼ばず、ローカル整理だけ行う", async () => {
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

  it("採番前のcancelは採番後にサーバーへ中断を送る", async () => {
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

  it("cancel APIが失敗してもストリーミング解除は完了する", async () => {
    vi.spyOn(agentApi, "cancel").mockRejectedValue(new Error("Link API 400"));
    const store = useChatStore();
    seed(store);
    feed(store, [{ type: "text_delta", text: "途中" }]);

    await expect(store.cancel()).resolves.toBeUndefined();

    expect(store.streaming).toBe(false);
    expect(store.messages[0].streaming).toBe(false);
  });

  it("会話が無い状態のcancelは何もせず落ちない", async () => {
    const cancel = vi.spyOn(agentApi, "cancel").mockResolvedValue();
    const store = useChatStore();

    await expect(store.cancel()).resolves.toBeUndefined();

    expect(cancel).not.toHaveBeenCalled();
    expect(store.streaming).toBe(false);
  });

  it("undoTurnはAPIを呼び、適用済み表示を取り下げる", async () => {
    const undoTurn = vi.spyOn(agentApi, "undoTurn").mockResolvedValue();
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      { type: "turn_applied", start_revision: 4, end_revision: 6 },
    ]);

    await store.undoTurn(CONV, 0);

    expect(undoTurn).toHaveBeenCalledWith(CONV, 0);
    expect(store.messages[0].undone).toBe(true);
    expect(appliedCommandCount(store.messages[0])).toBe(0);
    expect(store.messages[0].applied_revisions.end).toBe(
      store.messages[0].applied_revisions.start,
    );

    // 編集の無いターンではAPIを呼ばない
    await store.undoTurn(CONV, 0);
    expect(undoTurn).toHaveBeenCalledTimes(1);
  });

  it("undoTurnのサーバー拒否は呼び出し元へ投げ、適用済み表示は変えない", async () => {
    // 最新の適用済みターンでない/送信中などのガードは400で返る
    vi.spyOn(agentApi, "undoTurn").mockRejectedValue(
      new Error("Link API 400: 最新の適用済みターンではないため巻き戻せません"),
    );
    const store = useChatStore();
    seed(store);
    feed(store, [
      { type: "text_delta", text: "追加しました" },
      { type: "turn_completed", result: "追加しました", usage: null },
      { type: "turn_applied", start_revision: 4, end_revision: 6 },
    ]);

    await expect(store.undoTurn(CONV, 0)).rejects.toThrow("最新の適用済みターンではない");

    expect(store.messages[0].undone).toBe(false);
    expect(appliedCommandCount(store.messages[0])).toBeGreaterThan(0);
    expect(store.messages[0].applied_revisions).toEqual({ start: 4, end: 6 });
  });

  it("採番前(local-)の会話ではundoTurn APIを呼ばない", async () => {
    const undoTurn = vi.spyOn(agentApi, "undoTurn").mockResolvedValue();
    vi.spyOn(agentApi, "send").mockImplementation(() => new Promise<string>(() => {}));
    const store = useChatStore();

    void store.send("配線して");
    const localId = store.activeId as string;
    store.messages[1].applied_undo_depth = { start: 0, end: 2 };

    await store.undoTurn(localId, 1);

    expect(undoTurn).not.toHaveBeenCalled();
    expect(store.messages[1].undone).toBe(false);
  });

  it("loadConversationsはRust表現を表示用モデルへ正規化する", async () => {
    vi.spyOn(agentApi, "listConversations").mockResolvedValue([
      {
        id: CONV,
        session_id: "sess-1",
        model: null,
        messages: [
          {
            role: "user",
            text: "F2を追加",
            applied_revisions: { start: 3, end: 3 },
            applied_undo_depth: { start: 1, end: 1 },
          },
          {
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
    expect(store.messages[1].tool_calls[0].summary).toBe("fuse F2 を (140,90) に配置");
    // 巻き戻し回数はrevision差(4)ではなくundo深さの増分(2)
    expect(appliedCommandCount(store.messages[1])).toBe(2);
    expect(store.messages[1].applied_revisions).toEqual({ start: 3, end: 7 });
    expect(store.streaming).toBe(false);
  });

  it("normalizeConversationは未完了ツールをrunningとして扱う", () => {
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

  it("normalizeConversationはupdated_atをそのまま引き継ぐ", () => {
    const conv = normalizeConversation({
      id: CONV,
      session_id: null,
      model: null,
      messages: [],
      updated_at: 1_700_000_000_000,
    });

    expect(conv.updated_at).toBe(1_700_000_000_000);
  });

  it("subscribeを同時に呼んでも購読は1本だけ", async () => {
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

  it("購読の解決前にunsubscribeしても取りこぼさず閉じる", async () => {
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

  it("購読に失敗したら次のsubscribeで張り直せる", async () => {
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

  it("newConversationは採番待ち(pendingLocalId)を巻き込まない", async () => {
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

  it("setModel / setPanel / newConversation", async () => {
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
  it("MCPのプレフィックスを外す", () => {
    expect(shortToolName("mcp__madakecad__place_symbol")).toBe("place_symbol");
    expect(shortToolName("place_symbol")).toBe("place_symbol");
  });

  it("place_symbol", () => {
    expect(
      summarizeToolUse("mcp__madakecad__place_symbol", {
        symbol_id: "fuse",
        reference: "F2",
        value: "5A",
        x: 140,
        y: 90,
      }),
    ).toBe("fuse F2 5A を (140,90) に配置");
    expect(summarizeToolUse("place_symbol", { x: 12.5, y: 7.25 })).toBe(
      "シンボルを (12.5,7.25) に配置",
    );
  });

  it("draw_wire", () => {
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
    ).toBe("0.75sq 赤 2区間を接続");
    expect(
      summarizeToolUse("draw_wire", {
        points: [
          { x: 0, y: 0 },
          { x: 5, y: 0 },
        ],
      }),
    ).toBe("1区間を接続");
  });

  it("update_entity / execute_commands", () => {
    expect(
      summarizeToolUse("update_entity", { entity: { kind: "symbol", reference: "K1" } }),
    ).toBe("シンボル K1 を更新");
    expect(
      summarizeToolUse("mcp__madakecad__execute_commands", {
        commands: [{ type: "move_entities" }],
      }),
    ).toBe("要素移動 を実行");
    expect(
      summarizeToolUse("execute_commands", { commands: [{ type: "add_entity" }, { type: "undo" }] }),
    ).toBe("編集コマンド 2件を実行");
  });

  it("読み取り系と書き出し系", () => {
    expect(summarizeToolUse("mcp__madakecad__get_netlist", {})).toBe("ネットリストを取得");
    expect(summarizeToolUse("mcp__madakecad__get_project", {})).toBe("図面全体を読み取り");
    expect(summarizeToolUse("export_svg", { path: "/tmp/out/a.svg" })).toBe(
      "SVGを書き出し (a.svg)",
    );
    expect(summarizeToolUse("export_bom", { path: "/tmp/bom.csv" })).toBe(
      "部品表CSVを書き出し (bom.csv)",
    );
    expect(summarizeToolUse("export_wire_list", { path: "/tmp/w.csv" })).toBe(
      "電線リストCSVを書き出し (w.csv)",
    );
  });

  // チップはツール名を別途描くので、要約側は空を返す(両方返すと
  // 「ToolSearch ToolSearch」のような二重表示になる)。
  it("要約を作れないツールは空文字(表示はツール名のみ)", () => {
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

  it("タイトルは最初のユーザー発話の先頭40字", () => {
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

  it("ユーザー発話が無ければ「(空の会話)」", () => {
    expect(conversationTitle(conv("a", 0))).toBe(EMPTY_CONVERSATION_TITLE);
    expect(conversationTitle(conv("a", 0, [{ role: "user", text: "   " }]))).toBe(
      EMPTY_CONVERSATION_TITLE,
    );
  });

  it("相対時刻はたった今/N分前/N時間前/昨日/M-D", () => {
    const now = new Date(2026, 7, 21, 14, 0, 0).getTime();
    expect(formatRelativeTime(now - 30_000, now)).toBe("たった今");
    expect(formatRelativeTime(now - 8 * 60_000, now)).toBe("8分前");
    expect(formatRelativeTime(now - 3 * 3_600_000, now)).toBe("3時間前");
    expect(formatRelativeTime(new Date(2026, 7, 20, 22, 0, 0).getTime(), now)).toBe("昨日");
    expect(formatRelativeTime(new Date(2026, 7, 19, 9, 0, 0).getTime(), now)).toBe("8/19");
  });

  it("時刻不明(旧履歴のupdated_at=0)は相対時刻を出さない", () => {
    const now = Date.now();
    expect(formatRelativeTime(0, now)).toBe("");
    expect(conversationMeta(conv("a", 0, [{ role: "user", text: "x" }]), now)).toBe("1メッセージ");
  });

  it("メタ行は時刻・件数・適用済みrevを中黒で連ねる", () => {
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
    expect(conversationMeta(applied, now)).toBe("8分前 · 2メッセージ · 適用済み rev 24");

    // 巻き戻し済みのターンは「適用済み」に数えない
    applied.messages[1].undone = true;
    expect(conversationMeta(applied, now)).toBe("8分前 · 2メッセージ");
  });

  it("並びは更新の新しい順(時刻不明は後ろの登録順)", () => {
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
