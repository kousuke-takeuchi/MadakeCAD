// エージェント編集オーバーレイの状態管理。
// エージェントのツール呼び出し(ToolUseStarted)とドキュメントpatch(entity_upserted)から
// 「いま編集されている領域」を用紙座標(mm)で保持し、パルス表示用のパラメータを返す。
// 描画はrenderer.tsが担当(このファイルはDOM/Canvas非依存の純ロジック)。

import type { Entity, Point } from "../ipc";

/** 領域の外側に足すマージン (mm)。 */
export const REGION_MARGIN_MM = 4;

/** シンボル1個の想定半径 (mm)。参照記号の注記(at.y-9)まで覆う。 */
export const SYMBOL_EXTENT_MM = 10;

/** ツール完了後に領域を残す時間 (ms)。 */
export const HOLD_MS = 1500;

/** 完了イベントが来なかった領域を強制的に畳む時間 (ms)。 */
const MAX_RUNNING_MS = 60_000;

/** パルス1周期 (ms)。 */
const PULSE_PERIOD_MS = 1600;

/** パルス透明度の下限・上限。 */
export const PULSE_MIN_ALPHA = 0.1;
export const PULSE_MAX_ALPHA = 0.25;

/** 用紙座標(mm)の軸平行境界箱。 */
export interface Box {
  min: Point;
  max: Point;
}

/** 描画用に切り出したアクティブ領域。 */
export interface Region {
  /** 領域の一意キー(ツール呼び出しid / エンティティid)。 */
  key: string;
  /** 用紙座標(mm)。マージン込み。 */
  min: Point;
  max: Point;
  /** 塗りの不透明度。PULSE_MIN_ALPHA〜PULSE_MAX_ALPHAをsin波で往復する。 */
  opacity: number;
  /** 残存強度。進行中は1、完了後はHOLD_MSかけて0へ落ちる(枠線・チップの濃さ)。 */
  strength: number;
}

/** ノート系メソッドの共通オプション。 */
export interface NoteOptions {
  /** ツール呼び出しid(同一呼び出しの重複登録を防ぐキー)。 */
  id?: string;
  /** 現在時刻 (ms)。テストから固定するために外から渡せる。 */
  now?: number;
}

interface Entry {
  key: string;
  tool: string | null;
  box: Box;
  startedAt: number;
  /** 完了時刻。nullなら進行中。 */
  endedAt: number | null;
}

// ---------------------------------------------------------------------------
// bbox算出
// ---------------------------------------------------------------------------

function box(minX: number, minY: number, maxX: number, maxY: number): Box {
  return { min: { x: minX, y: minY }, max: { x: maxX, y: maxY } };
}

function around(at: Point, extent: number): Box {
  return box(at.x - extent, at.y - extent, at.x + extent, at.y + extent);
}

/** マージンを足した箱を返す(元の箱は変更しない)。 */
export function expandBox(b: Box, margin: number = REGION_MARGIN_MM): Box {
  return box(b.min.x - margin, b.min.y - margin, b.max.x + margin, b.max.y + margin);
}

/** 2つの箱の和(どちらかがnullならもう一方)。 */
export function unionBox(a: Box | null, b: Box | null): Box | null {
  if (!a) return b;
  if (!b) return a;
  return box(
    Math.min(a.min.x, b.min.x),
    Math.min(a.min.y, b.min.y),
    Math.max(a.max.x, b.max.x),
    Math.max(a.max.y, b.max.y),
  );
}

function pointsBox(points: unknown): Box | null {
  if (!Array.isArray(points) || points.length === 0) return null;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const p of points) {
    const r = p as Partial<Point> | null;
    if (!r || typeof r.x !== "number" || typeof r.y !== "number") continue;
    if (!Number.isFinite(r.x) || !Number.isFinite(r.y)) continue;
    minX = Math.min(minX, r.x);
    minY = Math.min(minY, r.y);
    maxX = Math.max(maxX, r.x);
    maxY = Math.max(maxY, r.y);
  }
  if (minX === Infinity) return null;
  return box(minX, minY, maxX, maxY);
}

/** 文字列注記のおおよその幅 (mm)。 */
function textWidthMm(text: string, heightMm: number): number {
  return Math.max(heightMm, text.length * heightMm * 0.6);
}

/** エンティティ1個の占有領域(マージンなし)。 */
export function entityBox(entity: Entity): Box | null {
  switch (entity.kind) {
    case "symbol":
      return around(entity.at, SYMBOL_EXTENT_MM);
    case "wire":
      return pointsBox(entity.points);
    case "junction":
      return around(entity.at, 1.5);
    case "net_label": {
      const w = textWidthMm(entity.name ?? "", 2.5);
      return box(entity.at.x, entity.at.y - 2.5, entity.at.x + w, entity.at.y);
    }
    case "text": {
      const h = entity.height > 0 ? entity.height : 2.5;
      const w = textWidthMm(entity.text ?? "", h);
      return box(entity.at.x, entity.at.y - h, entity.at.x + w, entity.at.y);
    }
    default:
      return null;
  }
}

function rec(input: unknown): Record<string, unknown> {
  return input && typeof input === "object" && !Array.isArray(input)
    ? (input as Record<string, unknown>)
    : {};
}

/** `mcp__madakecad__place_symbol` → `place_symbol`。 */
function shortName(tool: string): string {
  const m = /^mcp__[^_]+(?:_[^_]+)*__(.+)$/.exec(tool);
  if (m) return m[1];
  return tool.startsWith("mcp__") ? tool.slice(5) : tool;
}

/**
 * ツール呼び出しのinputから対象領域(マージンなし)を求める。
 * 座標を持たないツール(get_netlist / export_* / undo等)はnullを返し、領域を作らない。
 */
export function toolBox(tool: string, input: unknown): Box | null {
  const p = rec(input);
  switch (shortName(tool)) {
    case "place_symbol": {
      if (typeof p.x !== "number" || typeof p.y !== "number") return null;
      return around({ x: p.x, y: p.y }, SYMBOL_EXTENT_MM);
    }
    case "draw_wire":
      return pointsBox(p.points);
    case "add_entity":
    case "update_entity":
      return commandBox(p);
    case "execute_commands": {
      const commands = Array.isArray(p.commands) ? p.commands : [];
      let acc: Box | null = null;
      for (const c of commands) acc = unionBox(acc, commandBox(rec(c)));
      return acc;
    }
    default:
      return null;
  }
}

/** Command 1個(add_entity / update_entity)の対象領域。座標が読めないものはnull。 */
function commandBox(command: Record<string, unknown>): Box | null {
  const type = typeof command.type === "string" ? command.type : "";
  if (type && type !== "add_entity" && type !== "update_entity") return null;
  const entity = command.entity;
  if (!entity || typeof entity !== "object") return null;
  const e = entity as Entity;
  if (typeof e.kind !== "string") return null;
  return entityBox(e);
}

// ---------------------------------------------------------------------------
// オーバーレイ本体
// ---------------------------------------------------------------------------

export class AgentOverlay {
  private entries = new Map<string, Entry>();
  private seq = 0;

  /** ツール呼び出しの開始。完了(noteToolFinish)まで進行中として表示する。 */
  noteToolStart(tool: string, input: unknown, opts: NoteOptions = {}): void {
    const now = opts.now ?? Date.now();
    const raw = toolBox(tool, input);
    if (!raw) return;
    const key = `tool:${opts.id || `${tool}#${++this.seq}`}`;
    const existing = this.entries.get(key);
    this.entries.set(key, {
      key,
      tool,
      box: expandBox(raw),
      startedAt: existing?.startedAt ?? now,
      endedAt: null,
    });
  }

  /**
   * ツール呼び出しの完了。以降HOLD_MSだけ残ってフェードする。
   * `idOrTool`はnoteToolStartに渡したid(無ければツール名で進行中のものを畳む)。
   */
  noteToolFinish(idOrTool: string, opts: NoteOptions = {}): void {
    const now = opts.now ?? Date.now();
    const direct = this.entries.get(`tool:${idOrTool}`);
    if (direct) {
      if (direct.endedAt === null) direct.endedAt = now;
      return;
    }
    for (const entry of this.entries.values()) {
      if (entry.tool === idOrTool && entry.endedAt === null) entry.endedAt = now;
    }
  }

  /** 進行中の領域を全て完了扱いにする(ターン終了・中断時)。 */
  finishAll(now: number = Date.now()): void {
    for (const entry of this.entries.values()) {
      if (entry.endedAt === null) entry.endedAt = now;
    }
  }

  /** doc:patchのentity_upsert。届いた時点で確定した編集なのでHOLD_MSだけ残す。 */
  noteEntityUpserted(entity: Entity, opts: NoteOptions = {}): void {
    const now = opts.now ?? Date.now();
    const raw = entityBox(entity);
    if (!raw) return;
    const key = `entity:${entity.id}`;
    const existing = this.entries.get(key);
    this.entries.set(key, {
      key,
      tool: null,
      box: expandBox(raw),
      startedAt: existing?.startedAt ?? now,
      endedAt: now,
    });
  }

  /** 表示中の領域を返す(期限切れは破棄する)。 */
  activeRegions(now: number = Date.now()): Region[] {
    const out: Region[] = [];
    for (const entry of [...this.entries.values()]) {
      const strength = lifeStrength(entry, now);
      if (strength <= 0) {
        this.entries.delete(entry.key);
        continue;
      }
      out.push({
        key: entry.key,
        min: entry.box.min,
        max: entry.box.max,
        opacity: pulseAlpha(now - entry.startedAt),
        strength,
      });
    }
    return out;
  }

  /** 表示すべき領域があるか(アニメーションループの継続判定用)。 */
  hasActive(now: number = Date.now()): boolean {
    return this.activeRegions(now).length > 0;
  }

  /** 全領域を破棄する。 */
  clear(): void {
    this.entries.clear();
    this.seq = 0;
  }
}

/** 経過時間からパルス透明度(PULSE_MIN_ALPHA〜PULSE_MAX_ALPHA)を求める。 */
export function pulseAlpha(elapsedMs: number): number {
  const phase = (2 * Math.PI * elapsedMs) / PULSE_PERIOD_MS;
  const t = 0.5 + 0.5 * Math.sin(phase);
  return PULSE_MIN_ALPHA + (PULSE_MAX_ALPHA - PULSE_MIN_ALPHA) * t;
}

function lifeStrength(entry: Entry, now: number): number {
  if (entry.endedAt === null) {
    return now - entry.startedAt > MAX_RUNNING_MS ? 0 : 1;
  }
  const remain = entry.endedAt + HOLD_MS - now;
  if (remain <= 0) return 0;
  return Math.min(1, remain / HOLD_MS);
}
