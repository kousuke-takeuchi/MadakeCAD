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
  | {
      type: "turn_applied";
      /** 適用されたターンの安定ID(「元に戻す」の対象指定に使う)。 */
      turn_id?: string;
      start_revision: number;
      end_revision: number;
      start_undo_depth?: number;
      end_undo_depth?: number;
    };

/** `agent:event` / SSE `agent` のpayload。 */
export interface AgentEventPayload {
  conversation_id: string;
  /**
   * このイベントを生んだターンの通し番号(送信のたびに単調増加。1始まり)。
   *
   * キャンセル済みターンの遅延イベントを捨てるための鍵。旧サーバーは付けてこないため
   * 省略可(その場合は捨てずに畳み込む)。
   */
  turn_seq?: number;
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
  /**
   * このメッセージが属するターンの安定ID(ユーザー発話とアシスタント応答で共通)。
   *
   * Rust側は`chat.json`のformat_version 2で導入。旧履歴も読み込み時に採番されるので、
   * サーバー由来のメッセージには必ず入る。
   */
  turn_id?: string;
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
  /** 最終更新時刻(unixミリ秒)。旧`chat.json`には無い(Rust側は`serde(default)`で0)。 */
  updated_at?: number;
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
  /**
   * ターンの安定ID(「元に戻す」の対象指定に使う)。
   *
   * サーバー採番前(送信直後のローカル表示・ストリーミング中)は空文字。
   * `turn_applied`イベントか会話一覧の再取得でRust側のIDが入る。
   */
  turn_id: string;
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
  /** 最終更新時刻(unixミリ秒)。`0`は「時刻不明」(旧履歴)で、相対時刻を表示しない。 */
  updated_at: number;
}

// ---------------------------------------------------------------------------
// バックエンド呼び出し(Tauri IPC / ブラウザ検証時はLink API)
// ---------------------------------------------------------------------------

interface AgentApi {
  /** プロンプトを送る。戻り値は会話id(新規会話ならサーバー採番)。 */
  send(conversationId: string | null, prompt: string, model: string | null): Promise<string>;
  cancel(conversationId: string | null): Promise<void>;
  listConversations(): Promise<WireConversation[]>;
  undoTurn(conversationId: string, turnId: string): Promise<void>;
  detect(): Promise<AgentDetect>;
  onEvent(handler: (payload: AgentEventPayload) => void): Promise<UnlistenFn>;
}

const tauriAgentApi: AgentApi = {
  send: (conversationId, prompt, model) =>
    invoke<string>("agent_send", { conversationId, prompt, model }),
  cancel: (conversationId) => invoke<void>("agent_cancel", { conversationId }),
  listConversations: () => invoke<WireConversation[]>("agent_list_conversations"),
  undoTurn: (conversationId, turnId) =>
    invoke<void>("agent_undo_turn", { conversationId, turnId }),
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
  undoTurn: async (conversationId, turnId) => {
    await http("/agent/undo-turn", {
      method: "POST",
      body: JSON.stringify({ conversation_id: conversationId, turn_id: turnId }),
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

/** エンティティ種別の日本語名(未知の種別は「要素」)。 */
export function entityKindLabel(kind: string): string {
  return ENTITY_KIND_JA[kind] ?? "要素";
}

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
  renumber_wires: "線番自動採番",
  set_wire_numbers: "線番設定",
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
 *
 * 要約を作れないツール(MCP以外のCLI組み込みツール等)は**空文字**を返す。
 * チップはツール名を別途表示するので、ここでツール名を返すと
 * 「ToolSearch ToolSearch」のような二重表示になる。
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
      const kind = entityKindLabel(str(entity.kind));
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
      return "";
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
    // ターンIDはRust側が採番する。届くまで(=巻き戻せるようになるまで)は空
    turn_id: "",
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
    turn_id: message.turn_id ?? "",
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
    updated_at: conversation.updated_at ?? 0,
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

// ---------------------------------------------------------------------------
// 会話履歴ポップアップの表示ヘルパー(純関数。コンポーネントからも直接使う)
// ---------------------------------------------------------------------------

/** 会話履歴の行タイトルに使う最大文字数。 */
const TITLE_MAX_CHARS = 40;

/** 発話が無い会話のタイトル。 */
export const EMPTY_CONVERSATION_TITLE = "(空の会話)";

/**
 * 会話履歴の行タイトル。最初のユーザー発話の先頭40字(改行は空白へ畳む)。
 * ユーザー発話がまだ無い会話は「(空の会話)」。
 */
export function conversationTitle(conversation: ChatConversation): string {
  const first = conversation.messages.find((m) => m.role === "user");
  const text = (first?.text ?? "").replace(/\s+/g, " ").trim();
  if (!text) return EMPTY_CONVERSATION_TITLE;
  return text.length > TITLE_MAX_CHARS ? `${text.slice(0, TITLE_MAX_CHARS)}…` : text;
}

/**
 * 会話の更新時刻を相対表記にする(`0`は時刻不明で空文字)。
 *
 * デザインの例に合わせて「たった今 / N分前 / N時間前 / 昨日 / M/D」の5段階。
 * `now`は暦日の比較にも使うため、テストから固定値を渡せるようにしてある。
 */
export function formatRelativeTime(updatedAt: number, now: number = Date.now()): string {
  if (!updatedAt) return "";
  const diffMin = Math.floor((now - updatedAt) / 60_000);
  if (diffMin < 1) return "たった今";
  if (diffMin < 60) return `${diffMin}分前`;

  const at = new Date(updatedAt);
  const today = new Date(now);
  const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const dayDiff = Math.round((startOfDay(today) - startOfDay(at)) / 86_400_000);
  if (dayDiff <= 0) return `${Math.floor(diffMin / 60)}時間前`;
  if (dayDiff === 1) return "昨日";
  return `${at.getMonth() + 1}/${at.getDate()}`;
}

/**
 * 会話履歴の行メタ(「8分前 · 4メッセージ · 適用済み rev 24」)。
 *
 * 巻き戻し済み(`undone`)のターンは「適用済み」に数えない。
 */
export function conversationMeta(conversation: ChatConversation, now: number = Date.now()): string {
  const parts: string[] = [];
  const time = formatRelativeTime(conversation.updated_at, now);
  if (time) parts.push(time);
  parts.push(`${conversation.messages.length}メッセージ`);
  // findLastはES2023。tsconfigのlibはES2020なので後ろから探す
  for (let i = conversation.messages.length - 1; i >= 0; i--) {
    const message = conversation.messages[i];
    if (appliedCommandCount(message) > 0 && !message.undone) {
      parts.push(`適用済み rev ${message.applied_revisions.end}`);
      break;
    }
  }
  return parts.join(" · ");
}

/** 会話履歴の表示順(更新が新しい順。時刻不明の旧履歴は後ろの登録順)。 */
export function sortedConversations(conversations: ChatConversation[]): ChatConversation[] {
  return conversations
    .map((conversation, index) => ({ conversation, index }))
    .sort((a, b) => b.conversation.updated_at - a.conversation.updated_at || b.index - a.index)
    .map((entry) => entry.conversation);
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
  /** 採番前(local-)の会話でキャンセルが押された印。採番後にサーバーへ中断を送る */
  cancelRequested: boolean;
  /** 会話id → 直近に受け取ったイベントのターン通し番号 */
  lastTurnSeq: Record<string, number>;
  /** 会話id → 中断したターンの通し番号(これ以下のseqのイベントは捨てる) */
  cancelledTurnSeq: Record<string, number>;
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
    cancelRequested: false,
    lastTurnSeq: {},
    cancelledTurnSeq: {},
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
      const seq = payload.turn_seq ?? 0;
      // 中断したターンの遅延イベントは捨てる。中断後もCLIの出力が数行遅れて届くこと
      // があり、そのまま畳み込むと次のターンの表示へ前のターンの本文が混ざる
      // (次のターンのseqは必ず大きいので取り違えない)。seq無し=旧サーバーは捨てない
      if (seq > 0 && seq <= (this.cancelledTurnSeq[payload.conversation_id] ?? 0)) return;
      const conv = this.ensureConversation(payload.conversation_id);
      // 未知の会話(別クライアントが開始したターン)は捏造せず捨てる
      if (!conv) return;
      if (seq > 0) this.lastTurnSeq[conv.id] = seq;
      // Rust側touch()のミラー: イベントを畳み込んだ会話は「今」更新されたことにする
      // (正確な値はloadConversations()の再取得で上書きされる)
      conv.updated_at = Date.now();
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
            // 「元に戻す」の対象指定はこの安定ID(添字ではない)。旧サーバー(ID無し)では
            // 空のままになり、ChatDockが巻き戻し前に会話を再取得して補う
            if (event.turn_id) turn.turn_id = event.turn_id;
            turn.applied_revisions = {
              start: event.start_revision,
              end: event.end_revision,
            };
            if (event.start_undo_depth != null && event.end_undo_depth != null) {
              turn.applied_undo_depth = {
                start: event.start_undo_depth,
                end: event.end_undo_depth,
              };
            } else if (appliedCommandCount(turn) === 0) {
              // 旧形式イベント(undo深さなし)への後方互換。このイベントは
              // 「編集が入ったターン」にだけ流れるので1件として記録し、
              // 正確な件数はloadConversations()の再取得で上書きする。
              turn.applied_undo_depth = { start: 0, end: 1 };
            }
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
        conv = {
          id: localId,
          session_id: null,
          messages: [],
          model: this.model,
          updated_at: Date.now(),
        };
        this.conversations.push(conv);
        this.activeId = localId;
      }
      // 採番前のローカル会話は、初回送信が失敗した後の再送でも「新規扱い」で送る。
      // (ローカルidをAPIへ渡すとRust側のUuidデシリアライズで必ず失敗する)
      const pending = isLocalId(conv.id);
      this.pendingLocalId = pending ? conv.id : null;
      this.cancelRequested = false;
      conv.updated_at = Date.now();

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
      if (pending && pending.id !== id && !this.conversations.some((c) => c.id === id)) {
        if (this.activeId === pending.id) this.activeId = id;
        pending.id = id;
      }
      // 採番前にキャンセルが押されていた場合、サーバー側のターンはまだ走っている
      // のでここで中断を送る(失敗は握りつぶす: ターンは自然完了するだけ)
      if (this.cancelRequested) {
        this.cancelRequested = false;
        void agentApi.cancel(id).catch(() => undefined);
      }
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
      if (conv && !remoteId && streamingTurn(conv)) {
        // 採番前: サーバー側ターンはsend解決後にadoptConversationIdが中断する
        this.cancelRequested = true;
      }
      if (remoteId) {
        // 中断したターンの通し番号を控える(以降このseq以下のイベントは捨てる)。
        // API呼び出しを待つ前に控えるので、待っている間の遅延イベントも落とせる
        const seq = this.lastTurnSeq[remoteId] ?? 0;
        if (seq > 0) {
          this.cancelledTurnSeq[remoteId] = Math.max(this.cancelledTurnSeq[remoteId] ?? 0, seq);
        }
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
     * そのターンの編集を巻き戻す(サーバーがそのターンの**エージェント編集だけ**を
     * 逆Commandで戻す。ユーザーの手編集は残る)。
     *
     * 対象はターンの安定ID(`turn_id`)で指定する。メッセージ添字と違い、後続ターンの
     * 追記や履歴の再読込でズレないため、別ターンを巻き戻す事故が起きない。
     * サーバー側は「送信中でないか」「まだ適用済みか」「逆適用が現在の図面と衝突
     * しないか」を検証し、条件を外れると400を返す(衝突時は図面を一切変更しない)。
     * **エラーはそのまま呼び出し元へ投げる**(ChatDockがui.logへ
     * 「元に戻す失敗: ...」として出す)。失敗時はローカルの適用済み表示も変更しない。
     */
    async undoTurn(conversationId: string, turnId: string) {
      const conv = this.conversations.find((c) => c.id === conversationId);
      const message = conv?.messages.find((m) => m.turn_id === turnId && m.role === "assistant");
      if (!conv || !turnId || !message || isLocalId(conv.id)) return;
      if (appliedCommandCount(message) === 0) return;
      await agentApi.undoTurn(conversationId, turnId);
      message.undone = true;
      // Rust側`record_reverted`と同じ後始末(全部戻したのでrevision範囲も畳む)
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

    /**
     * 新しい空の会話へ切り替える(送信時にサーバーがidを採番する)。
     *
     * `pendingLocalId`には触らない: 採番待ちの会話をここでクリアすると、
     * 進行中のsend()の採番結果を引き取れず孤立会話がstreamingのまま残り、
     * 以降の送信が永久にブロックされる。
     */
    newConversation() {
      this.activeId = null;
    },

    // panelOpen="expanded" は「左ドックのエージェントタブ表示」の意味。
    // タブ状態(ui.leftPanelTab)との同期はuiストアのopenAgentTab/closeAgentTabが行うので、
    // UIコンポーネントは直接これを呼ばずそちらを使うこと。
    setPanel(state: PanelState) {
      this.panelOpen = state;
    },
  },
});

/**
 * 進行中のターン(末尾のストリーミング中アシスタントメッセージ)。
 *
 * `turn_completed`/`error`の宛先判定に使う。「進行中のターンが無い会話」への終了系
 * イベントは捨てて、発話の無い空ターンが生えるのを防ぐ(取りこぼしは
 * loadConversations()で回復する)。中断済みターンの遅延イベントは、その手前で
 * ターン通し番号(`turn_seq`)により`applyAgentEvent`が捨てている。
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
