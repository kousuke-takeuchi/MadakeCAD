// AIチャットストア。
// 信頼できる唯一の情報源はRust側(madake-agent)。ここはエージェントイベント
// (agent:event / SSE)を畳み込むミラーで、図面の編集は一切行わない
// (編集はエージェントがMCP経由でCommandエンジンを通し、document storeへpatchで届く)。

import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { inTauri } from "../ipc";

// ---------------------------------------------------------------------------
// ワイヤ型 (madake-agent の serde 表現: 内部タグ`type`, snake_case)
// ---------------------------------------------------------------------------

/** 1ターン分のトークン使用量(CLIの`result`イベントより)。 */
export interface Usage {
  input_tokens: number;
  output_tokens: number;
  cache_creation_input_tokens: number;
  cache_read_input_tokens: number;
  total_cost_usd: number | null;
}

/** Rust側 `AgentEvent` のJSON表現。 */
export type AgentEvent =
  | { type: "session_started"; session_id: string }
  | { type: "text_delta"; text: string }
  | { type: "tool_use_started"; id: string; tool: string; input: unknown }
  | { type: "tool_use_finished"; id: string; tool: string; is_error: boolean }
  | { type: "turn_completed"; result: string; usage: Usage | null }
  | { type: "error"; message: string }
  /** ターンの編集が図面へ適用された時にマネージャが合成するイベント。 */
  | { type: "turn_applied"; start_revision: number; end_revision: number };

/** `agent:event` / SSE `agent` のpayload。 */
export interface AgentEventPayload {
  conversation_id: string;
  event: AgentEvent;
}

/** Rust側 `ToolCall` のJSON表現(会話一覧APIの戻り)。 */
export interface WireToolCall {
  id: string;
  tool: string;
  input: unknown;
  finished: boolean;
  is_error: boolean;
}

/** Rust側 `ChatMessage` のJSON表現。 */
export interface WireChatMessage {
  role: ChatRole;
  text: string;
  tool_calls?: WireToolCall[];
  applied_revisions?: AppliedRevisions;
  error?: string | null;
}

/** Rust側 `Conversation` のJSON表現。 */
export interface WireConversation {
  id: string;
  session_id: string | null;
  messages: WireChatMessage[];
  model: string | null;
}

/** `claude`実行ファイルの検出結果。 */
export interface AgentDetect {
  path: string;
  version: string;
}

// ---------------------------------------------------------------------------
// ストア内の表示用モデル
// ---------------------------------------------------------------------------

export type ChatRole = "user" | "assistant";
export type ToolStatus = "running" | "ok" | "error";
export type PanelState = "collapsed" | "expanded";

/** ターンの前後で挟んだEngineのrevision範囲(`end - start`が巻き戻しに必要なundo回数)。 */
export interface AppliedRevisions {
  start: number;
  end: number;
}

/** ツールチップ1個分。 */
export interface ChatToolCall {
  /** CLIのtool_use_id */
  id: string;
  tool: string;
  input: unknown;
  status: ToolStatus;
  /** 表示用の日本語要約 */
  summary: string;
}

export interface ChatMessage {
  role: ChatRole;
  text: string;
  tool_calls: ChatToolCall[];
  applied_revisions: AppliedRevisions;
  error: string | null;
  usage: Usage | null;
  /** ストリーミング進行中(ローダ表示用) */
  streaming: boolean;
  /** 「元に戻す」実行済み */
  undone: boolean;
}

export interface ChatConversation {
  id: string;
  session_id: string | null;
  messages: ChatMessage[];
  model: string | null;
}

// ---------------------------------------------------------------------------
// バックエンド呼び出し(Tauri IPC / ブラウザ検証時はLink API)
// ---------------------------------------------------------------------------

interface AgentApi {
  /** プロンプトを送る。戻り値は会話id(新規会話ならサーバー採番)。 */
  send(conversationId: string | null, prompt: string, model: string | null): Promise<string>;
  cancel(conversationId: string | null): Promise<void>;
  listConversations(): Promise<WireConversation[]>;
  undoTurn(conversationId: string, messageIndex: number): Promise<void>;
  detect(): Promise<AgentDetect>;
  onEvent(handler: (payload: AgentEventPayload) => void): Promise<UnlistenFn>;
}

const tauriAgentApi: AgentApi = {
  send: (conversationId, prompt, model) =>
    invoke<string>("agent_send", { conversationId, prompt, model }),
  cancel: (conversationId) => invoke<void>("agent_cancel", { conversationId }),
  listConversations: () => invoke<WireConversation[]>("agent_list_conversations"),
  undoTurn: (conversationId, messageIndex) =>
    invoke<void>("agent_undo_turn", { conversationId, messageIndex }),
  detect: () => invoke<AgentDetect>("agent_detect"),
  onEvent: (handler) => listen<AgentEventPayload>("agent:event", (e) => handler(e.payload)),
};

// Tauri外(ブラウザでのUI開発・E2E検証)では、起動中のMadakeCADのLink APIに接続する。
const API_BASE = "http://127.0.0.1:9310/api/v1";

async function http<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!res.ok) throw new Error(`Link API ${res.status}: ${await res.text()}`);
  const text = await res.text();
  return (text ? JSON.parse(text) : null) as T;
}

const httpAgentApi: AgentApi = {
  send: async (conversationId, prompt, model) => {
    const out = await http<string | { conversation_id: string }>("/agent/send", {
      method: "POST",
      body: JSON.stringify({ conversation_id: conversationId, prompt, model }),
    });
    return typeof out === "string" ? out : out.conversation_id;
  },
  cancel: async (conversationId) => {
    await http("/agent/cancel", {
      method: "POST",
      body: JSON.stringify({ conversation_id: conversationId }),
    });
  },
  listConversations: () => http<WireConversation[]>("/agent/conversations"),
  undoTurn: async (conversationId, messageIndex) => {
    await http("/agent/undo-turn", {
      method: "POST",
      body: JSON.stringify({ conversation_id: conversationId, message_index: messageIndex }),
    });
  },
  detect: () => http<AgentDetect>("/agent/detect"),
  onEvent: (handler) => {
    const es = new EventSource(`${API_BASE}/agent/events`);
    es.addEventListener("agent", (e) => {
      try {
        handler(JSON.parse((e as MessageEvent).data) as AgentEventPayload);
      } catch {
        // 不正なイベントは無視
      }
    });
    return Promise.resolve(() => es.close());
  },
};

/** 差し替え可能なバックエンド入口(テストではここをスタブする)。 */
export const agentApi: AgentApi = inTauri ? tauriAgentApi : httpAgentApi;

// ---------------------------------------------------------------------------
// ツール要約
// ---------------------------------------------------------------------------

const WIRE_COLOR_JA: Record<string, string> = {
  black: "黒",
  white: "白",
  red: "赤",
  blue: "青",
  yellow: "黄",
  green: "緑",
  light_blue: "水色",
  orange: "橙",
  brown: "茶",
  gray: "灰",
  grey: "灰",
  purple: "紫",
  violet: "紫",
  pink: "桃",
};

const ENTITY_KIND_JA: Record<string, string> = {
  symbol: "シンボル",
  wire: "配線",
  junction: "接続点",
  net_label: "ネットラベル",
  text: "テキスト",
};

const COMMAND_JA: Record<string, string> = {
  add_sheet: "シート追加",
  remove_sheet: "シート削除",
  rename_sheet: "シート改名",
  set_title_block: "表題欄設定",
  set_revisions: "改訂欄設定",
  add_entity: "要素追加",
  update_entity: "要素更新",
  delete_entities: "要素削除",
  move_entities: "要素移動",
  set_wire_parts: "電線品番表設定",
};

function rec(input: unknown): Record<string, unknown> {
  return input && typeof input === "object" && !Array.isArray(input)
    ? (input as Record<string, unknown>)
    : {};
}

function str(v: unknown): string {
  return typeof v === "string" ? v : "";
}

/** 数値を表示用に整形する(小数の末尾ゼロを落とす)。 */
function num(v: unknown): string {
  if (typeof v !== "number" || !Number.isFinite(v)) return "?";
  return String(Math.round(v * 1000) / 1000);
}

function baseName(path: unknown): string {
  const p = str(path);
  const seg = p.split(/[\\/]/).pop();
  return seg || p;
}

/** `mcp__madakecad__place_symbol` → `place_symbol`。 */
export function shortToolName(tool: string): string {
  const m = /^mcp__[^_]+(?:_[^_]+)*__(.+)$/.exec(tool);
  if (m) return m[1];
  return tool.startsWith("mcp__") ? tool.slice(5) : tool;
}

/**
 * ツール呼び出しをツールチップ用の日本語1行に要約する。
 * 未知のツールはツール名のみを返す。
 */
export function summarizeToolUse(tool: string, input: unknown): string {
  const name = shortToolName(tool);
  const p = rec(input);
  switch (name) {
    case "place_symbol": {
      const label = [str(p.symbol_id), str(p.reference), str(p.value)].filter(Boolean).join(" ");
      const at = `(${num(p.x)},${num(p.y)})`;
      return label ? `${label} を ${at} に配置` : `シンボルを ${at} に配置`;
    }
    case "draw_wire": {
      const points = Array.isArray(p.points) ? p.points : [];
      const segments = Math.max(points.length - 1, 1);
      const spec = [
        typeof p.sq === "number" ? `${num(p.sq)}sq` : "",
        p.color ? (WIRE_COLOR_JA[str(p.color)] ?? str(p.color)) : "",
      ]
        .filter(Boolean)
        .join(" ");
      return spec ? `${spec} ${segments}本を接続` : `${segments}本を接続`;
    }
    case "update_entity": {
      const entity = rec(p.entity);
      const kind = ENTITY_KIND_JA[str(entity.kind)] ?? "要素";
      const label = str(entity.reference) || str(entity.name) || str(entity.id);
      return label ? `${kind} ${label} を更新` : `${kind}を更新`;
    }
    case "execute_commands": {
      const commands = Array.isArray(p.commands) ? p.commands : [];
      if (commands.length === 1) {
        const type = str(rec(commands[0]).type);
        return `${COMMAND_JA[type] ?? (type || "コマンド")} を実行`;
      }
      return `編集コマンド ${commands.length}件を実行`;
    }
    case "get_netlist":
      return "ネットリストを取得";
    case "get_project":
      return "図面全体を読み取り";
    case "list_symbols":
      return "シンボル一覧を取得";
    case "export_svg":
      return `SVGを書き出し (${baseName(p.path)})`;
    case "export_bom":
      return `部品表CSVを書き出し (${baseName(p.path)})`;
    case "export_wire_list":
      return `電線リストCSVを書き出し (${baseName(p.path)})`;
    case "undo":
      return "直前の編集を取り消し";
    case "redo":
      return "取り消した編集をやり直し";
    default:
      return name;
  }
}

// ---------------------------------------------------------------------------
// ファクトリ / 正規化
// ---------------------------------------------------------------------------

function emptyRevisions(): AppliedRevisions {
  return { start: 0, end: 0 };
}

function newMessage(role: ChatRole, text: string, streaming: boolean): ChatMessage {
  return {
    role,
    text,
    tool_calls: [],
    applied_revisions: emptyRevisions(),
    error: null,
    usage: null,
    streaming,
    undone: false,
  };
}

function normalizeToolCall(call: WireToolCall): ChatToolCall {
  return {
    id: call.id,
    tool: call.tool,
    input: call.input,
    status: !call.finished ? "running" : call.is_error ? "error" : "ok",
    summary: summarizeToolUse(call.tool, call.input),
  };
}

function normalizeMessage(message: WireChatMessage): ChatMessage {
  return {
    role: message.role,
    text: message.text,
    tool_calls: (message.tool_calls ?? []).map(normalizeToolCall),
    applied_revisions: message.applied_revisions ?? emptyRevisions(),
    error: message.error ?? null,
    usage: null,
    streaming: false,
    undone: false,
  };
}

/** 会話一覧APIの戻り(Rust表現)を表示用モデルへ変換する。 */
export function normalizeConversation(conversation: WireConversation): ChatConversation {
  return {
    id: conversation.id,
    session_id: conversation.session_id ?? null,
    messages: (conversation.messages ?? []).map(normalizeMessage),
    model: conversation.model ?? null,
  };
}

/** このターンで確定した編集コマンド数(= 元に戻すのに必要なundo回数)。 */
export function appliedCommandCount(message: ChatMessage): number {
  return Math.max(message.applied_revisions.end - message.applied_revisions.start, 0);
}

let localSeq = 0;

// ---------------------------------------------------------------------------
// ストア
// ---------------------------------------------------------------------------

interface ChatState {
  conversations: ChatConversation[];
  activeId: string | null;
  streaming: boolean;
  panelOpen: PanelState;
  model: string | null;
  detect: AgentDetect | null;
  /** サーバー採番待ちのローカル会話id(イベントが先に届いた場合の引き取り用) */
  pendingLocalId: string | null;
  unlisten: UnlistenFn | null;
}

export const useChatStore = defineStore("chat", {
  state: (): ChatState => ({
    conversations: [],
    activeId: null,
    streaming: false,
    panelOpen: "collapsed",
    model: null,
    detect: null,
    pendingLocalId: null,
    unlisten: null,
  }),

  getters: {
    activeConversation(state): ChatConversation | null {
      return state.conversations.find((c) => c.id === state.activeId) ?? null;
    },
    messages(): ChatMessage[] {
      return this.activeConversation?.messages ?? [];
    },
    /** ストリーミング中のアシスタントメッセージ(なければnull)。 */
    streamingMessage(): ChatMessage | null {
      return this.messages.find((m) => m.streaming) ?? null;
    },
  },

  actions: {
    // --- 購読・初期化 -------------------------------------------------------

    /** 会話履歴の読込・CLI検出・イベント購読をまとめて行う。 */
    async bootstrap() {
      await this.subscribe();
      await Promise.all([this.loadConversations(), this.detectCli()]);
    },

    async subscribe() {
      if (this.unlisten) return;
      this.unlisten = await agentApi.onEvent((payload) => this.applyAgentEvent(payload));
    },

    unsubscribe() {
      this.unlisten?.();
      this.unlisten = null;
    },

    async detectCli() {
      try {
        this.detect = await agentApi.detect();
      } catch {
        // 未インストール時はnullのまま(UIは「未接続」を表示する)
        this.detect = null;
      }
    },

    async loadConversations() {
      const list = await agentApi.listConversations();
      this.conversations = list.map(normalizeConversation);
      if (!this.conversations.some((c) => c.id === this.activeId)) {
        this.activeId = this.conversations[this.conversations.length - 1]?.id ?? null;
      }
      this.recomputeStreaming();
    },

    // --- イベントリデューサ -------------------------------------------------

    /** エージェントイベントを会話へ畳み込む(唯一の状態更新経路)。 */
    applyAgentEvent(payload: AgentEventPayload) {
      const conv = this.ensureConversation(payload.conversation_id);
      const event = payload.event;
      switch (event.type) {
        case "session_started":
          conv.session_id = event.session_id;
          break;

        case "text_delta":
          this.openTurn(conv).text += event.text;
          break;

        case "tool_use_started": {
          const turn = this.openTurn(conv);
          if (event.id && turn.tool_calls.some((c) => c.id === event.id)) break;
          turn.tool_calls.push({
            id: event.id,
            tool: event.tool,
            input: event.input,
            status: "running",
            summary: summarizeToolUse(event.tool, event.input),
          });
          break;
        }

        case "tool_use_finished": {
          const turn = lastAssistant(conv);
          const call = turn?.tool_calls.find(
            (c) =>
              (event.id && c.id === event.id) ||
              (!event.id && c.status === "running" && c.tool === event.tool),
          );
          if (call) call.status = event.is_error ? "error" : "ok";
          break;
        }

        case "turn_completed": {
          const turn = lastAssistant(conv) ?? this.openTurn(conv);
          // デルタが来ない構成でも本文を埋める
          if (!turn.text) turn.text = event.result;
          turn.usage = event.usage;
          turn.streaming = false;
          this.recomputeStreaming();
          break;
        }

        case "turn_applied": {
          const turn = lastAssistant(conv);
          if (turn) {
            turn.applied_revisions = {
              start: event.start_revision,
              end: event.end_revision,
            };
            turn.undone = false;
          }
          break;
        }

        case "error": {
          const turn = lastAssistant(conv) ?? this.openTurn(conv);
          turn.error = event.message;
          turn.streaming = false;
          this.recomputeStreaming();
          break;
        }
      }
    },

    /**
     * 会話を取得する(無ければ作る)。
     * サーバー採番待ちのローカル会話があれば、そのidを差し替えて引き取る。
     */
    ensureConversation(id: string): ChatConversation {
      const found = this.conversations.find((c) => c.id === id);
      if (found) return found;

      if (this.pendingLocalId) {
        const pending = this.conversations.find((c) => c.id === this.pendingLocalId);
        this.pendingLocalId = null;
        if (pending) {
          if (this.activeId === pending.id) this.activeId = id;
          pending.id = id;
          return pending;
        }
      }

      const created: ChatConversation = {
        id,
        session_id: null,
        messages: [],
        model: this.model,
      };
      this.conversations.push(created);
      if (!this.activeId) this.activeId = id;
      return created;
    },

    /** 進行中のアシスタントメッセージ(無ければ新しいターンを開始する)。 */
    openTurn(conv: ChatConversation): ChatMessage {
      const last = conv.messages[conv.messages.length - 1];
      if (last && last.role === "assistant" && last.streaming) return last;
      const created = newMessage("assistant", "", true);
      conv.messages.push(created);
      this.streaming = true;
      return created;
    },

    recomputeStreaming() {
      this.streaming = this.conversations.some((c) => c.messages.some((m) => m.streaming));
    },

    // --- アクション ---------------------------------------------------------

    /**
     * プロンプトを送信する(必要なら会話を新規作成)。戻り値は会話id。
     * ユーザー発話とアシスタントの空ターンを即座に積み、以降はイベントで埋まる。
     */
    async send(prompt: string): Promise<string | null> {
      const text = prompt.trim();
      if (!text || this.streaming) return null;

      let conv = this.conversations.find((c) => c.id === this.activeId) ?? null;
      let localId: string | null = null;
      if (!conv) {
        localId = `local-${++localSeq}`;
        conv = { id: localId, session_id: null, messages: [], model: this.model };
        this.conversations.push(conv);
        this.activeId = localId;
        this.pendingLocalId = localId;
      }
      conv.messages.push(newMessage("user", text, false));
      conv.messages.push(newMessage("assistant", "", true));
      this.streaming = true;

      try {
        const id = await agentApi.send(localId ? null : conv.id, text, this.model);
        this.adoptConversationId(id);
        return id;
      } catch (e) {
        this.pendingLocalId = null;
        const turn = lastAssistant(conv);
        if (turn) {
          turn.error = e instanceof Error ? e.message : String(e);
          turn.streaming = false;
        }
        this.recomputeStreaming();
        return null;
      }
    },

    /** サーバー採番されたidをローカル会話へ反映する。 */
    adoptConversationId(id: string) {
      if (!this.pendingLocalId) return;
      const pending = this.conversations.find((c) => c.id === this.pendingLocalId);
      this.pendingLocalId = null;
      if (!pending || pending.id === id) return;
      if (this.conversations.some((c) => c.id === id)) return;
      if (this.activeId === pending.id) this.activeId = id;
      pending.id = id;
    },

    /** ストリーミング中のターンを中断する。 */
    async cancel() {
      const conv = this.conversations.find((c) => c.id === this.activeId) ?? null;
      try {
        await agentApi.cancel(conv && conv.id !== this.pendingLocalId ? conv.id : null);
      } finally {
        for (const c of this.conversations) {
          for (const m of c.messages) m.streaming = false;
        }
        this.streaming = false;
      }
    },

    /** そのターンの編集を全て巻き戻す(revision差の回数だけundoする)。 */
    async undoTurn(conversationId: string, messageIndex: number) {
      const conv = this.conversations.find((c) => c.id === conversationId);
      const message = conv?.messages[messageIndex];
      if (!conv || !message || appliedCommandCount(message) === 0) return;
      await agentApi.undoTurn(conversationId, messageIndex);
      message.undone = true;
      message.applied_revisions = {
        start: message.applied_revisions.start,
        end: message.applied_revisions.start,
      };
    },

    setModel(model: string | null) {
      this.model = model;
      const conv = this.conversations.find((c) => c.id === this.activeId);
      if (conv) conv.model = model;
    },

    setActive(conversationId: string | null) {
      this.activeId = conversationId;
    },

    /** 新しい空の会話へ切り替える(送信時にサーバーがidを採番する)。 */
    newConversation() {
      this.activeId = null;
      this.pendingLocalId = null;
    },

    togglePanel() {
      this.panelOpen = this.panelOpen === "expanded" ? "collapsed" : "expanded";
    },

    setPanel(state: PanelState) {
      this.panelOpen = state;
    },
  },
});

/** 直近のアシスタントメッセージ(ストリーミング中かどうかは問わない)。 */
function lastAssistant(conv: ChatConversation): ChatMessage | undefined {
  for (let i = conv.messages.length - 1; i >= 0; i--) {
    if (conv.messages[i].role === "assistant") return conv.messages[i];
  }
  return undefined;
}
