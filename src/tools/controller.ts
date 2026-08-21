// 対話ツールの状態機械。ドラッグ中はローカルプレビュー、確定時にCommandを発行する。

import { drawSymbol, transformLocal } from "../canvas/renderer";
import { theme, wireColorScreen } from "../canvas/theme";
import { Viewport, type Pt } from "../canvas/viewport";
import type { Entity, Point, SymbolInstance } from "../ipc";
import { useDocumentStore } from "../stores/document";

export type ToolId = "select" | "wire" | "place";

type DocumentStore = ReturnType<typeof useDocumentStore>;

/** 選択ヒット判定の許容距離 (mm)。 */
const HIT_TOLERANCE = 2.0;
/** シンボルのクリック判定半径 (mm)。 */
const SYMBOL_HIT_RADIUS = 9.0;

function segmentDistance(p: Pt, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  if (len2 < 1e-12) return Math.hypot(p.x - a.x, p.y - a.y);
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

export class EditorController {
  vp = new Viewport();
  tool: ToolId = "select";
  /** placeツールで配置するシンボルid。 */
  placeSymbolId: string | null = null;
  placeRotation = 0;
  /** 配線ツールの既定属性。 */
  wireColor = "red";
  wireSq = 0.75;
  snapEnabled = true;
  orthoEnabled = true;

  cursorScreen: Pt | null = null;
  cursorWorld: Pt = { x: 0, y: 0 };

  /** 再描画要求 (CanvasViewが設定する)。 */
  requestRedraw: () => void = () => {};
  /** キャンバスの現在サイズ(px)。CanvasViewが描画時に更新する。 */
  viewSize = { w: 0, h: 0 };

  private wirePoints: Pt[] = [];
  private dragMode: "none" | "pan" | "move" | "rubber" = "none";
  private dragStartWorld: Pt = { x: 0, y: 0 };
  private dragStartScreen: Pt = { x: 0, y: 0 };
  private moveDelta: Pt = { x: 0, y: 0 };
  private rubberEnd: Pt = { x: 0, y: 0 };
  private spaceHeld = false;

  constructor(private store: DocumentStore) {}

  setTool(tool: ToolId, symbolId?: string) {
    this.tool = tool;
    this.placeSymbolId = tool === "place" ? (symbolId ?? this.placeSymbolId) : null;
    this.wirePoints = [];
    this.dragMode = "none";
    this.requestRedraw();
  }

  private snap(p: Pt): Pt {
    return this.snapEnabled ? this.vp.snap(p) : p;
  }

  /** 直交モード: 直前点から支配軸方向に拘束した点を返す。 */
  private ortho(from: Pt, to: Pt): Pt {
    if (!this.orthoEnabled) return to;
    return Math.abs(to.x - from.x) >= Math.abs(to.y - from.y)
      ? { x: to.x, y: from.y }
      : { x: from.x, y: to.y };
  }

  /** カーソル位置のエンティティid (上にあるものを優先)。 */
  hitTest(world: Pt): string | null {
    const sheet = this.store.activeSheet;
    if (!sheet) return null;
    const entities = Object.values(sheet.entities);
    for (const e of entities) {
      if (e.kind === "symbol") {
        if (Math.hypot(world.x - e.at.x, world.y - e.at.y) <= SYMBOL_HIT_RADIUS) return e.id;
      } else if (e.kind === "junction" || e.kind === "net_label") {
        if (Math.hypot(world.x - e.at.x, world.y - e.at.y) <= HIT_TOLERANCE * 1.5) return e.id;
      }
    }
    for (const e of entities) {
      if (e.kind !== "wire") continue;
      for (let i = 0; i + 1 < e.points.length; i++) {
        if (segmentDistance(world, e.points[i], e.points[i + 1]) <= HIT_TOLERANCE) return e.id;
      }
    }
    return null;
  }

  onPointerDown(screen: Pt, ev: PointerEvent) {
    const world = this.vp.toWorld(screen);
    this.dragStartScreen = screen;
    this.dragStartWorld = world;
    if (ev.button === 1 || this.spaceHeld) {
      this.dragMode = "pan";
      return;
    }
    if (ev.button !== 0) return;

    switch (this.tool) {
      case "select": {
        const hit = this.hitTest(world);
        if (hit) {
          if (ev.shiftKey) {
            const sel = new Set(this.store.selection);
            if (sel.has(hit)) sel.delete(hit);
            else sel.add(hit);
            this.store.selection = sel;
          } else if (!this.store.selection.has(hit)) {
            this.store.selection = new Set([hit]);
          }
          this.dragMode = "move";
          this.moveDelta = { x: 0, y: 0 };
        } else {
          if (!ev.shiftKey) this.store.selection = new Set();
          this.dragMode = "rubber";
          this.rubberEnd = world;
        }
        break;
      }
      case "wire": {
        const base = this.wirePoints.length
          ? this.ortho(this.wirePoints[this.wirePoints.length - 1], this.snap(world))
          : this.snap(world);
        this.wirePoints.push(base);
        break;
      }
      case "place":
        void this.commitPlace(this.snap(world));
        break;
    }
    this.requestRedraw();
  }

  onPointerMove(screen: Pt) {
    this.cursorScreen = screen;
    const world = this.vp.toWorld(screen);
    this.cursorWorld = this.snapEnabled ? this.vp.snap(world) : world;
    switch (this.dragMode) {
      case "pan":
        this.vp.pan(screen.x - this.dragStartScreen.x, screen.y - this.dragStartScreen.y);
        this.dragStartScreen = screen;
        break;
      case "move": {
        const snapped = this.snap({
          x: world.x - this.dragStartWorld.x,
          y: world.y - this.dragStartWorld.y,
        });
        this.moveDelta = snapped;
        break;
      }
      case "rubber":
        this.rubberEnd = world;
        break;
    }
    this.requestRedraw();
  }

  async onPointerUp() {
    const sheet = this.store.activeSheet;
    switch (this.dragMode) {
      case "move": {
        if (sheet && (this.moveDelta.x !== 0 || this.moveDelta.y !== 0) && this.store.selection.size) {
          await this.store.execute({
            type: "move_entities",
            sheet_id: sheet.id,
            ids: [...this.store.selection],
            dx: this.moveDelta.x,
            dy: this.moveDelta.y,
          });
        }
        this.moveDelta = { x: 0, y: 0 };
        break;
      }
      case "rubber": {
        if (sheet) {
          const x0 = Math.min(this.dragStartWorld.x, this.rubberEnd.x);
          const x1 = Math.max(this.dragStartWorld.x, this.rubberEnd.x);
          const y0 = Math.min(this.dragStartWorld.y, this.rubberEnd.y);
          const y1 = Math.max(this.dragStartWorld.y, this.rubberEnd.y);
          if (x1 - x0 > 0.5 || y1 - y0 > 0.5) {
            const sel = new Set(this.store.selection);
            for (const e of Object.values(sheet.entities)) {
              const pts: Point[] =
                e.kind === "wire" ? e.points : "at" in e ? [e.at] : [];
              if (pts.length && pts.every((p) => p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1)) {
                sel.add(e.id);
              }
            }
            this.store.selection = sel;
          }
        }
        break;
      }
    }
    this.dragMode = "none";
    this.requestRedraw();
  }

  onWheel(screen: Pt, deltaY: number) {
    this.vp.zoomAt(screen, deltaY < 0 ? 1.15 : 1 / 1.15);
    this.requestRedraw();
  }

  /** エンティティ群を選択し、先頭をキャンバス中央へ表示する (検証結果パネルの行クリック用)。 */
  reveal(entityIds: string[]) {
    const sheet = this.store.activeSheet;
    if (!sheet || entityIds.length === 0) return;
    this.store.selection = new Set(entityIds.filter((id) => id in sheet.entities));
    const first = entityIds.map((id) => sheet.entities[id]).find(Boolean);
    if (!first) return;
    const target: Pt | null =
      first.kind === "wire"
        ? first.points.length
          ? {
              x: (first.points[0].x + first.points[first.points.length - 1].x) / 2,
              y: (first.points[0].y + first.points[first.points.length - 1].y) / 2,
            }
          : null
        : "at" in first
          ? first.at
          : null;
    if (target && this.viewSize.w > 0) {
      this.vp.originX = this.viewSize.w / 2 - target.x * this.vp.scale;
      this.vp.originY = this.viewSize.h / 2 - target.y * this.vp.scale;
    }
    this.requestRedraw();
  }

  onDoubleClick() {
    if (this.tool === "wire") void this.commitWire();
  }

  async onKeyDown(ev: KeyboardEvent): Promise<boolean> {
    const mod = ev.metaKey || ev.ctrlKey;
    if (ev.code === "Space") {
      this.spaceHeld = true;
      return true;
    }
    if (mod && ev.key.toLowerCase() === "z") {
      if (ev.shiftKey) await this.store.redo();
      else await this.store.undo();
      this.requestRedraw();
      return true;
    }
    switch (ev.key) {
      case "Escape":
        if (this.tool === "wire" && this.wirePoints.length) void this.commitWire();
        else this.setTool("select");
        return true;
      case "Delete":
      case "Backspace":
        await this.deleteSelection();
        return true;
      case "w":
      case "W":
        this.setTool("wire");
        return true;
      case "r":
      case "R":
        if (this.tool === "place") {
          this.placeRotation = (this.placeRotation + 90) % 360;
          this.requestRedraw();
          return true;
        }
        return false;
      case "Enter":
        if (this.tool === "wire") {
          void this.commitWire();
          return true;
        }
        return false;
    }
    return false;
  }

  onKeyUp(ev: KeyboardEvent) {
    if (ev.code === "Space") this.spaceHeld = false;
  }

  async deleteSelection() {
    const sheet = this.store.activeSheet;
    if (!sheet || !this.store.selection.size) return;
    await this.store.execute({
      type: "delete_entities",
      sheet_id: sheet.id,
      ids: [...this.store.selection],
    });
    this.store.selection = new Set();
    this.requestRedraw();
  }

  /** 参照記号の自動採番 (接頭辞 + 既存最大値+1)。 */
  nextReference(prefix: string): string {
    const sheet = this.store.activeSheet;
    let max = 0;
    if (sheet) {
      for (const e of Object.values(sheet.entities)) {
        if (e.kind === "symbol" && e.reference.startsWith(prefix)) {
          const num = parseInt(e.reference.slice(prefix.length), 10);
          if (!Number.isNaN(num)) max = Math.max(max, num);
        }
      }
    }
    return `${prefix}${max + 1}`;
  }

  private async commitPlace(at: Pt) {
    const sheet = this.store.activeSheet;
    const def = this.placeSymbolId ? this.store.resolveSymbol(this.placeSymbolId) : undefined;
    if (!sheet || !def) return;
    const entity: Entity = {
      kind: "symbol",
      id: crypto.randomUUID(),
      symbol_id: def.id,
      at,
      rotation: this.placeRotation,
      mirror: false,
      reference: this.nextReference(def.ref_prefix),
      value: "",
      attrs: {},
    };
    await this.store.execute({ type: "add_entity", sheet_id: sheet.id, entity });
    this.requestRedraw();
  }

  private async commitWire() {
    const sheet = this.store.activeSheet;
    const pts = this.wirePoints;
    this.wirePoints = [];
    if (!sheet || pts.length < 2) {
      this.requestRedraw();
      return;
    }
    const entity: Entity = {
      kind: "wire",
      id: crypto.randomUUID(),
      points: pts,
      color: this.wireColor,
      sq: this.wireSq,
      length_m: null,
      part_no: null,
      net: null,
    };
    await this.store.execute({ type: "add_entity", sheet_id: sheet.id, entity });
    this.requestRedraw();
  }

  /** ツールのプレビューを描く (renderSheetの後に呼ぶ)。 */
  renderPreview(ctx: CanvasRenderingContext2D) {
    const vp = this.vp;
    // 配線プレビュー
    if (this.tool === "wire" && this.wirePoints.length) {
      const last = this.wirePoints[this.wirePoints.length - 1];
      const next = this.ortho(last, this.cursorWorld);
      ctx.strokeStyle = wireColorScreen(this.wireColor);
      ctx.setLineDash([6, 4]);
      ctx.lineWidth = Math.max(1.2, 0.35 * vp.scale);
      ctx.beginPath();
      [...this.wirePoints, next].forEach((p, i) => {
        const s = vp.toScreen(p);
        if (i === 0) ctx.moveTo(s.x, s.y);
        else ctx.lineTo(s.x, s.y);
      });
      ctx.stroke();
      ctx.setLineDash([]);
    }
    // 配置ゴースト
    if (this.tool === "place" && this.placeSymbolId) {
      const def = this.store.resolveSymbol(this.placeSymbolId);
      if (def) {
        const ghost: SymbolInstance = {
          id: "ghost",
          symbol_id: def.id,
          at: this.cursorWorld,
          rotation: this.placeRotation,
          mirror: false,
          reference: "",
          value: "",
          attrs: {},
        };
        ctx.globalAlpha = 0.6;
        drawSymbol(ctx, vp, ghost, def, false, theme.selection);
        ctx.globalAlpha = 1;
      }
    }
    // 移動プレビュー: 選択物のピン/頂点を差分表示
    if (this.dragMode === "move" && (this.moveDelta.x || this.moveDelta.y)) {
      const sheet = this.store.activeSheet;
      if (sheet) {
        ctx.strokeStyle = theme.selection;
        ctx.setLineDash([4, 3]);
        ctx.lineWidth = 1.5;
        for (const id of this.store.selection) {
          const e = sheet.entities[id];
          if (!e) continue;
          if (e.kind === "wire") {
            ctx.beginPath();
            e.points.forEach((p, i) => {
              const s = vp.toScreen({ x: p.x + this.moveDelta.x, y: p.y + this.moveDelta.y });
              if (i === 0) ctx.moveTo(s.x, s.y);
              else ctx.lineTo(s.x, s.y);
            });
            ctx.stroke();
          } else if (e.kind === "symbol") {
            const def = this.store.resolveSymbol(e.symbol_id);
            const moved = { ...e, at: { x: e.at.x + this.moveDelta.x, y: e.at.y + this.moveDelta.y } };
            if (def) {
              ctx.globalAlpha = 0.5;
              drawSymbol(ctx, vp, moved, def, false, theme.selection);
              ctx.globalAlpha = 1;
            }
          } else if ("at" in e) {
            const s = vp.toScreen({ x: e.at.x + this.moveDelta.x, y: e.at.y + this.moveDelta.y });
            ctx.strokeRect(s.x - 4, s.y - 4, 8, 8);
          }
        }
        ctx.setLineDash([]);
      }
    }
    // 矩形選択
    if (this.dragMode === "rubber") {
      const a = vp.toScreen(this.dragStartWorld);
      const b = vp.toScreen(this.rubberEnd);
      ctx.fillStyle = theme.selectionFill;
      ctx.strokeStyle = theme.selection;
      ctx.lineWidth = 1;
      ctx.fillRect(a.x, a.y, b.x - a.x, b.y - a.y);
      ctx.strokeRect(a.x, a.y, b.x - a.x, b.y - a.y);
    }
    // ピンスナップマーカー: カーソル近傍のピンに菱形
    const sheet = this.store.activeSheet;
    if (sheet && (this.tool === "wire" || this.tool === "place")) {
      for (const e of Object.values(sheet.entities)) {
        if (e.kind !== "symbol") continue;
        const def = this.store.resolveSymbol(e.symbol_id);
        if (!def) continue;
        for (const pin of def.pins) {
          const wp = transformLocal(pin.at, e);
          if (Math.hypot(wp.x - this.cursorWorld.x, wp.y - this.cursorWorld.y) < 2.5) {
            const s = vp.toScreen(wp);
            ctx.strokeStyle = theme.annotation;
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.moveTo(s.x, s.y - 6);
            ctx.lineTo(s.x + 6, s.y);
            ctx.lineTo(s.x, s.y + 6);
            ctx.lineTo(s.x - 6, s.y);
            ctx.closePath();
            ctx.stroke();
          }
        }
      }
    }
  }
}
