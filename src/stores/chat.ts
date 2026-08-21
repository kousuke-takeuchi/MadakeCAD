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
  /** 旧`chat.json`には無いフィールド(Rust側は`serde(default)`で{0,0})。 */
  applied_undo_depth?: AppliedUndoDepth;
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

/**
 * ターンの前後で挟んだEngineのrevision範囲(表示専用: 「rev N」の表示に使う)。
 *
 * undo/redoでもrevisionは進むため、`end - start`は積まれたコマンド数ではない。
 * 巻き戻し可否・件数の正は [`AppliedUndoDepth`]。
 */
export interface AppliedRevisions {
  start: number;
  end: number;
}

/**
 * ターンの前後で挟んだundoスタックの深さ。
 *
 * `end - start`がこのターンで新たに積まれた編集コマンド数(= 必要なundo回数)。
 * ターン中にエージェントがundo/redoを混ぜても過不足なく数えられる。
 */
export interface AppliedUndoDepth {
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
  /** 表示用のrevision範囲 */
  applied_revisions: AppliedRevisions;
  /** 「元に戻す」可否・適用済み表示の判定に使う(`end - start > 0`が適用済み) */
  applied_undo_depth: AppliedUndoDepth;
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
      // pointsは折れ線の頂点列。数えているのは電線本数ではなく区間数
      return spec ? `${spec} ${segments}区間を接続` : `${segments}区間を接続`;
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

function emptyUndoDepth(): AppliedUndoDepth {
  return { start: 0, end: 0 };
}

function newMessage(role: ChatRole, text: string, streaming: boolean): ChatMessage {
  return {
    role,
    text,
    tool_calls: [],
    applied_revisions: emptyRevisions(),
    applied_undo_depth: emptyUndoDepth(),
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
    applied_undo_depth: message.applied_undo_depth ?? emptyUndoDepth(),
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

/**
 * このターンで確定した編集コマンド数(= 元に戻すのに必要なundo回数)。
 *
 * revisionではなくundoスタックの深さ増分で数える(revisionはundo/redoでも進むため、
 * 差分は積まれたコマンド数と一致しない)。Rust側`ChatMessage::applied_command_count`と同じ定義。
 */
export function appliedCommandCount(message: ChatMessage): number {
  return Math.max(message.applied_undo_depth.end - message.applied_undo_depth.start, 0);
}

let localSeq = 0;

/** サーバー採番前のローカル会話idの接頭辞。 */
const LOCAL_ID_PREFIX = "local-";

/**
 * サーバー採番前のローカルidか。
 *
 * Rust側の`conversation_id`はUuidなので、この形のidをAPIへ渡すと必ず
 * デシリアライズに失敗する。境界で必ず弾くこと。
 */
function isLocalId(id: string | null): boolean {
  return typeof id === "string" && id.startsWith(LOCAL_ID_PREFIX);
}

/** ユーザーが中断したターンに載せるラベル(Rust側`CANCELLED_MESSAGE`と同文言)。 */
const CANCELLED_MESSAGE = "キャンセルされました";

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
  /** 購読処理そのもの(解決前にunsubscribeされても確実に閉じるため保持する) */
  subscription: Promise<UnlistenFn> | null;
  /** 未知の会話を検知して一覧を取り直している最中か(再取得の多重起動よけ) */
  reloading: boolean;
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
    subscription: null,
    reloading: false,
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

    /**
     * エージェントイベントを購読する(多重購読しない)。
     *
     * `unlisten`だけを見張ると、guardと`await`の間に入った2回目の呼び出しが
     * 二重購読を張ってしまう。購読Promise自体を同期的に保持して塞ぐ。
     */
    async subscribe() {
      if (this.subscription) {
        await this.subscription;
        return;
      }
      const subscribing = agentApi.onEvent((payload) => this.applyAgentEvent(payload));
      this.subscription = subscribing;
      let unlisten: UnlistenFn;
      try {
        unlisten = await subscribing;
      } catch (e) {
        // 失敗した購読を残すと以降の再購読が永久に塞がる
        if (this.subscription === subscribing) this.subscription = null;
        throw e;
      }
      if (this.subscription !== subscribing) {
        // 解決までにunsubscribeされていた。張ってしまった購読はここで閉じる
        unlisten();
        return;
      }
      this.unlisten = unlisten;
    },

    unsubscribe() {
      const settled = this.unlisten;
      this.unlisten = null;
      // 解決前ならsubscribe()側がこの取り消しに気づいて閉じる
      this.subscription = null;
      settled?.();
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
      // 未知の会話(別クライアントが開始したターン)は捏造せず捨てる
      if (!conv) return;
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
          const turn = streamingTurn(conv);
          if (!turn) break;
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
            // このイベントは「編集が入ったターン」にだけ流れるが、undo深さの増分は
            // 運ばれてこない(revision差はundo/redo混在で過大になるため使えない)。
            // 表示側は「適用済みか」しか見ないので1件として記録し、正確な件数は
            // loadConversations()の再取得で上書きする。
            if (appliedCommandCount(turn) === 0) turn.applied_undo_depth = { start: 0, end: 1 };
            turn.undone = false;
          }
          break;
        }

        case "error": {
          const turn = streamingTurn(conv);
          if (!turn) break;
          turn.error = event.message;
          turn.streaming = false;
          this.recomputeStreaming();
          break;
        }
      }
    },

    /**
     * イベントの宛先会話を解決する。
     *
     * サーバー採番待ちのローカル会話があれば、そのidを差し替えて引き取る
     * (send()の解決より先にイベントが届くケース)。
     * どちらでもない未知のidは**会話を作らずnullを返す**。ここで空の会話を作ると、
     * madake CLIやMCPなど別クライアントが始めたターンのたびに、ユーザー発話を欠いた
     * 幽霊会話がUIへ並んでしまう。Rust側が正なので一覧の再取得だけ促す。
     */
    ensureConversation(id: string): ChatConversation | null {
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

      this.scheduleReload();
      return null;
    },

    /**
     * 未知の会話を見つけたときに、Rust側の一覧を1回だけ取り直す。
     *
     * 自分のターンが進行中の間は取り直さない(一覧の再取得は会話を丸ごと差し替えるため、
     * 進行中メッセージのstreaming/usageが消える)。次の機会に拾えばよい。
     */
    scheduleReload() {
      if (this.reloading || this.streaming) return;
      this.reloading = true;
      void this.loadConversations()
        .catch(() => {
          // 取得失敗時は次のイベントで再試行する
        })
        .finally(() => {
          this.reloading = false;
        });
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
      if (!conv) {
        const localId = `${LOCAL_ID_PREFIX}${++localSeq}`;
        conv = { id: localId, session_id: null, messages: [], model: this.model };
        this.conversations.push(conv);
        this.activeId = localId;
      }
      // 採番前のローカル会話は、初回送信が失敗した後の再送でも「新規扱い」で送る。
      // (ローカルidをAPIへ渡すとRust側のUuidデシリアライズで必ず失敗する)
      const pending = isLocalId(conv.id);
      this.pendingLocalId = pending ? conv.id : null;

      conv.messages.push(newMessage("user", text, false));
      conv.messages.push(newMessage("assistant", "", true));
      this.streaming = true;

      try {
        const id = await agentApi.send(pending ? null : conv.id, text, this.model);
        this.adoptConversationId(id);
        return id;
      } catch (e) {
        // 失敗してもローカル会話はpendingのまま維持する。ここでクリアすると会話idが
        // `local-N`のまま確定し、以降の送信が延々とUuidデシリアライズで落ちる
        this.pendingLocalId = isLocalId(conv.id) ? conv.id : null;
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

    /**
     * 現在の会話のストリーミング中のターンを中断する。
     *
     * 採番前(`local-`)の会話やアクティブな会話が無い場合、Rustへ渡せるidが無いので
     * APIは呼ばずローカルの整理だけ行う。API呼び出しが失敗しても整理は必ず済ませる
     * (UIをストリーミング表示のまま固めない)。
     */
    async cancel() {
      const conv = this.conversations.find((c) => c.id === this.activeId) ?? null;
      const remoteId = conv && !isLocalId(conv.id) ? conv.id : null;
      if (remoteId) {
        try {
          await agentApi.cancel(remoteId);
        } catch {
          // 中断要求の失敗は握りつぶす(ローカル整理は下で必ず行う)
        }
      }
      if (!conv) {
        this.recomputeStreaming();
        return;
      }
      // 中断するのは対象の会話だけ。他会話のターンは走り続けている
      const turn = streamingTurn(conv);
      // Rust側もキャンセル理由をErrorイベントで流すが、下のガード(進行中でない
      // ターンへのerrorは捨てる)で落ちるため、ラベルはここで載せる
      if (turn && !turn.error) turn.error = CANCELLED_MESSAGE;
      for (const m of conv.messages) m.streaming = false;
      this.recomputeStreaming();
    },

    /**
     * そのターンの編集を全て巻き戻す(undo深さの増分だけundoする)。
     *
     * サーバー側でも「最新の適用済みターンか」「送信中でないか」を検証しており、
     * 条件を外れると400が返る。**エラーはそのまま呼び出し元へ投げる**
     * (ChatPanelがui.logへ「元に戻す失敗: ...」として出す)。失敗時はローカルの
     * 適用済み表示も変更しない。
     */
    async undoTurn(conversationId: string, messageIndex: number) {
      const conv = this.conversations.find((c) => c.id === conversationId);
      const message = conv?.messages[messageIndex];
      if (!conv || !message || isLocalId(conv.id) || appliedCommandCount(message) === 0) return;
      await agentApi.undoTurn(conversationId, messageIndex);
      message.undone = true;
      // Rust側`record_undone`と同じ後始末(全部戻したのでrevision範囲も畳む)
      message.applied_undo_depth = {
        start: message.applied_undo_depth.start,
        end: message.applied_undo_depth.start,
      };
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

/**
 * 進行中のターン(末尾のストリーミング中アシスタントメッセージ)。
 *
 * `turn_completed`/`error`の宛先判定に使う。Rust側のイベントにはターン識別子が無いため、
 * キャンセル直後に遅れて届いた前ターンの`error`と、いま進行中のターンの`error`を
 * 区別できない。せめて「進行中のターンが無い会話」への終了系イベントは捨てて、
 * 発話の無い空ターンが生えるのを防ぐ(取りこぼしはloadConversations()で回復する)。
 * 恒久対策はイベントへのターン通し番号の追加(Rust側`ManagerState::next_seq`が既にある)。
 */
function streamingTurn(conv: ChatConversation): ChatMessage | undefined {
  const last = conv.messages[conv.messages.length - 1];
  return last && last.role === "assistant" && last.streaming ? last : undefined;
}

/** 直近のアシスタントメッセージ(ストリーミング中かどうかは問わない)。 */
function lastAssistant(conv: ChatConversation): ChatMessage | undefined {
  for (let i = conv.messages.length - 1; i >= 0; i--) {
    if (conv.messages[i].role === "assistant") return conv.messages[i];
  }
  return undefined;
}
