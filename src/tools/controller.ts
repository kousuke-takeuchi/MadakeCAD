// 対話ツールの状態機械。ドラッグ中はローカルプレビュー、確定時にCommandを発行する。

import { harnessAddCommand, harnessRectPoints, harnessWireCount } from "../canvas/harness";
import {
  DEFAULT_VARIANT,
  drawMacroGhost,
  macroEntities,
  macroVariantKeys,
  placeMacroEntities,
} from "../canvas/macroPreview";
import {
  DIN_RAIL_H,
  footprintRect,
  footprintSize,
  legendRect,
  panelOutlineRect,
  railRect,
} from "../canvas/panel";
import {
  dinRailAddCommand,
  ductAddCommand,
  footprintAddCommand,
  footprintPlaceAt,
  panelLegendAddCommand,
  panelOutlineAddCommand,
  panelToolsAllowed,
  railSnapDelta,
  schematicToolsAllowed,
  type PanelToolId,
} from "../canvas/panelTools";
import {
  deviceBlockBounds,
  drawHarness,
  drawShape,
  drawSymbol,
  symbolBounds,
  transformLocal,
} from "../canvas/renderer";
import {
  SHAPE_TOOLS,
  isShapeTool,
  newShape,
  shapeAddCommand,
  shapeHitTest,
  type ShapeToolId,
} from "../canvas/shapes";
import { noteBounds } from "../canvas/textStyle";
import { panelTheme, theme, wireColorScreen } from "../canvas/theme";
import { Viewport, type Pt } from "../canvas/viewport";
import { i18n } from "../i18n";
import {
  ipc,
  type Entity,
  type Macro,
  type Point,
  type SymbolInstance,
  type UnplacedComponent,
} from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useMacrosStore } from "../stores/macros";
import { useUiStore } from "../stores/ui";

/**
 * 対話ツール。回路図のツール (配線・部品挿入・ハーネス・マクロ) と
 * 盤のツール (盤枠・DINレール・ダクト・フットプリント、M4 §8) は
 * **シートの種別で使える側が決まる** (`setTool`が弾く)。
 */
export type ToolId =
  | "select"
  | "wire"
  | "place"
  | "harness"
  | "macro"
  | ShapeToolId
  | PanelToolId;

/** 回路図シートでしか使えないツール。 */
const SCHEMATIC_TOOLS: ToolId[] = [
  "wire",
  "place",
  "harness",
  "macro",
  "shape_rect",
  "shape_line",
  "shape_circle",
  "shape_polyline",
];
/** 盤レイアウトシートでしか使えないツール。 */
const PANEL_TOOLS: ToolId[] = [
  "panel_outline",
  "din_rail",
  "duct",
  "footprint",
  "panel_legend",
];

/** 配置中のマクロ。`fromLibrary=false`は⌘Vの無名マクロ (ファイルに無いのでidを引けない)。 */
export interface MacroPlacement {
  macro: Macro;
  fromLibrary: boolean;
  /** 挿入ダイアログで選んだ値セットid (null=保存時の値のまま置く)。 */
  valueSet: string | null;
}

/** マクロ確定時にRustへ渡す引数 (ライブラリはid、無名マクロはマクロそのもの)。 */
export type MacroApplyArgs =
  | {
      kind: "library";
      macroId: string;
      variant: string;
      valueSet: string | null;
      sheetId: string;
      at: Point;
      rotation: number;
    }
  | {
      kind: "inline";
      macro: Macro;
      variant: string;
      valueSet: string | null;
      sheetId: string;
      at: Point;
      rotation: number;
    };

type DocumentStore = ReturnType<typeof useDocumentStore>;

/** 選択ヒット判定の許容距離 (mm)。 */
const HIT_TOLERANCE = 2.0;
/** シンボル外形の外側でも掴める幅 (mm)。 */
const SYMBOL_HIT_PAD = 1.0;
/** 定義を引けないシンボル(属性が壊れたブロック等)のクリック判定半径 (mm)。 */
const SYMBOL_HIT_RADIUS = 9.0;

/** 点が矩形の中 (辺の上を含む) にあるか。盤の要素の当たり判定。 */
function inRect(p: Pt, r: { x: number; y: number; w: number; h: number }): boolean {
  return p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h;
}

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
  /** 部品DBから選択した部品情報 (配置時にvalue/attrsへ反映)。 */
  placePart: { value: string; attrs: Record<string, string> } | null = null;
  /** macroツールで配置中の回路マクロ。 */
  placeMacro: MacroPlacement | null = null;
  /** footprintツールで配置中の未配置部品 (未配置部品パネルの行から入る)。 */
  placeFootprint: UnplacedComponent | null = null;
  /** 配置中のバリアント (Tabで巡回する`macroVariantKeys`の添字)。 */
  macroVariantIndex = 0;
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
  /** 折れ線ツールで打ち終えた点 (ダブルクリック・Enter・Escで確定する)。 */
  private shapePoints: Pt[] = [];
  private dragMode:
    | "none"
    | "pan"
    | "move"
    | "rubber"
    | "harness"
    | "shape"
    | "panel_rect"
    | "panel_rail" = "none";
  private dragStartWorld: Pt = { x: 0, y: 0 };
  private dragStartScreen: Pt = { x: 0, y: 0 };
  private moveDelta: Pt = { x: 0, y: 0 };
  private rubberEnd: Pt = { x: 0, y: 0 };
  private spaceHeld = false;

  constructor(private store: DocumentStore) {}

  /**
   * ツールが表示中のシートで使えるか (M4 §8)。回路図のツールは盤レイアウトシートでは
   * 使えず、盤のツールは回路図シートでは使えない。**Commandエンジン側にシート種別の
   * ガードは無い**ので、置けてしまわないようにするのはこのUIの役目。
   */
  toolAllowed(tool: ToolId): boolean {
    const sheet = this.store.activeSheet;
    if (SCHEMATIC_TOOLS.includes(tool)) return schematicToolsAllowed(sheet);
    if (PANEL_TOOLS.includes(tool)) return panelToolsAllowed(sheet);
    return true;
  }

  setTool(
    tool: ToolId,
    symbolId?: string,
    part?: { value: string; attrs: Record<string, string> },
  ) {
    if (!this.toolAllowed(tool)) {
      useUiStore().log(
        i18n.global.t(
          SCHEMATIC_TOOLS.includes(tool) ? "panel.schematicToolLog" : "panel.panelToolLog",
        ),
      );
      return;
    }
    this.tool = tool;
    this.placeSymbolId = tool === "place" ? (symbolId ?? this.placeSymbolId) : null;
    this.placePart = tool === "place" ? (part ?? null) : null;
    if (tool !== "macro") {
      this.placeMacro = null;
      this.macroVariantIndex = 0;
    }
    if (tool !== "footprint") this.placeFootprint = null;
    this.wirePoints = [];
    this.shapePoints = [];
    this.dragMode = "none";
    this.requestRedraw();
  }

  /**
   * 未配置部品をフットプリントとして置くモードに入る (未配置部品パネルの行から)。
   * ゴーストがカーソルに付き、Rで0/90度を切り替え、クリックで確定する。
   */
  startFootprintPlacement(component: UnplacedComponent) {
    if (!this.toolAllowed("footprint")) {
      useUiStore().log(i18n.global.t("panel.panelToolLog"));
      return;
    }
    this.setTool("footprint");
    this.placeFootprint = component;
    this.placeRotation = 0;
    this.requestRedraw();
  }

  /**
   * 回路マクロの配置モードに入る (ゴースト表示 → クリックで確定)。
   * バリアントは既定の"A"、回転は0から始める。値セットは挿入ダイアログで選んだものを
   * そのまま持ち回る (⌘Vの無名マクロには値セットが無いのでnull)。
   */
  startMacroPlacement(macro: Macro, opts: { fromLibrary?: boolean; valueSet?: string | null } = {}) {
    this.tool = "macro";
    this.placeMacro = {
      macro,
      fromLibrary: opts.fromLibrary ?? true,
      valueSet: opts.valueSet ?? null,
    };
    this.macroVariantIndex = 0;
    this.placeRotation = 0;
    this.placeSymbolId = null;
    this.placePart = null;
    this.wirePoints = [];
    this.dragMode = "none";
    this.requestRedraw();
  }

  /** 配置中のマクロのバリアントキー ("A"が既定)。 */
  get macroVariantKey(): string {
    if (!this.placeMacro) return DEFAULT_VARIANT;
    const keys = macroVariantKeys(this.placeMacro.macro);
    return keys[this.macroVariantIndex % keys.length] ?? DEFAULT_VARIANT;
  }

  /** Tab: バリアントを次へ巡回する (1つしか無ければ変わらない)。 */
  cycleMacroVariant() {
    if (!this.placeMacro) return;
    const keys = macroVariantKeys(this.placeMacro.macro);
    this.macroVariantIndex = (this.macroVariantIndex + 1) % keys.length;
    this.requestRedraw();
  }

  /**
   * 確定時にRustへ渡す引数。ライブラリのマクロはidで、⌘Vの無名マクロは
   * マクロそのものを渡す (ファイルが無いのでidを引けない)。
   */
  macroApplyArgs(at: Pt): MacroApplyArgs | null {
    const sheet = this.store.activeSheet;
    if (!sheet || !this.placeMacro) return null;
    const common = {
      variant: this.macroVariantKey,
      valueSet: this.placeMacro.valueSet,
      sheetId: sheet.id,
      at: { x: at.x, y: at.y },
      rotation: this.placeRotation,
    };
    return this.placeMacro.fromLibrary
      ? { kind: "library", macroId: this.placeMacro.macro.id, ...common }
      : { kind: "inline", macro: this.placeMacro.macro, ...common };
  }

  /**
   * マクロを1回の編集として挿入する (undo一発で全体が戻る)。
   * 確定後もマクロツールのままなので、同じマクロを続けて何個でも置ける。
   */
  async commitMacro(at: Pt) {
    const args = this.macroApplyArgs(at);
    if (!args) return;
    const ui = useUiStore();
    try {
      const patch =
        args.kind === "library"
          ? await ipc.applyMacro(
              args.macroId,
              args.variant,
              args.valueSet,
              args.sheetId,
              args.at,
              args.rotation,
            )
          : await ipc.applyMacroInline(
              args.macro,
              args.variant,
              args.valueSet,
              args.sheetId,
              args.at,
              args.rotation,
            );
      this.store.applyEdit(patch);
    } catch (e) {
      ui.log(String(e));
    }
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

  /**
   * カーソル位置のエンティティid (上にあるものを優先)。
   *
   * シンボルは**外形の囲み矩形**で判定する (ブロックのように大きさがピン表で決まる
   * シンボルでも、外形の中ならどこでも掴めるようにするため)。実機ブロック表示 (G7) の
   * コイルは、描かれているブロックの外形そのもので判定する (小さなコイル図形の範囲だけ
   * しか掴めない、ということが起きないように)。定義を引けないシンボル
   * (属性が壊れたブロック) だけは、置いた位置まわりの固定半径で拾って直せるようにする。
   */
  hitTest(world: Pt): string | null {
    const sheet = this.store.activeSheet;
    if (!sheet) return null;
    const blocks = this.store.sheetAnnotations?.relay.deviceBlocks;
    const entities = Object.values(sheet.entities);
    for (const e of entities) {
      if (e.kind === "symbol") {
        const def = this.store.resolveInstance(e);
        if (!def) {
          if (Math.hypot(world.x - e.at.x, world.y - e.at.y) <= SYMBOL_HIT_RADIUS) return e.id;
          continue;
        }
        const block = blocks?.get(e.id);
        const { min, max } = block ? deviceBlockBounds(e, block) : symbolBounds(e, def);
        if (
          world.x >= min.x - SYMBOL_HIT_PAD &&
          world.x <= max.x + SYMBOL_HIT_PAD &&
          world.y >= min.y - SYMBOL_HIT_PAD &&
          world.y <= max.y + SYMBOL_HIT_PAD
        ) {
          return e.id;
        }
      } else if (e.kind === "junction" || e.kind === "net_label") {
        if (Math.hypot(world.x - e.at.x, world.y - e.at.y) <= HIT_TOLERANCE * 1.5) return e.id;
      } else if (e.kind === "text") {
        // 注記は書かれている文字の範囲で掴む (基準位置で左右どちらへ伸びるかが変わる)
        const { min, max } = noteBounds(e);
        if (
          world.x >= min.x - SYMBOL_HIT_PAD &&
          world.x <= max.x + SYMBOL_HIT_PAD &&
          world.y >= min.y - SYMBOL_HIT_PAD &&
          world.y <= max.y + SYMBOL_HIT_PAD
        ) {
          return e.id;
        }
      }
    }
    for (const e of entities) {
      if (e.kind !== "wire") continue;
      for (let i = 0; i + 1 < e.points.length; i++) {
        if (segmentDistance(world, e.points[i], e.points[i + 1]) <= HIT_TOLERANCE) return e.id;
      }
    }
    // 図形は輪郭で掴む (塗りの下にある配線・記号を選びたいことのほうが多い)
    for (const e of entities) {
      if (e.kind === "shape" && shapeHitTest(e, world, HIT_TOLERANCE)) return e.id;
    }
    // ハーネス境界は最後 (囲みの中身を選びたいことのほうが多い)。破線そのものを掴む
    for (const e of entities) {
      if (e.kind !== "harness" || e.points.length < 2) continue;
      for (let i = 0; i < e.points.length; i++) {
        const a = e.points[i];
        const b = e.points[(i + 1) % e.points.length];
        if (segmentDistance(world, a, b) <= HIT_TOLERANCE) return e.id;
      }
    }
    // 盤の要素は描いた順の逆 (凡例表→フットプリント→レール→ダクト→盤外形) に拾う。
    // 盤外形は中身を選びたいことのほうが多いので最後に見る
    for (const e of entities) {
      if (e.kind === "panel_legend" && inRect(world, legendRect(sheet, e))) return e.id;
    }
    for (const e of entities) {
      if (e.kind === "footprint" && inRect(world, footprintRect(e))) return e.id;
    }
    for (const e of entities) {
      if (e.kind === "din_rail" && inRect(world, railRect(e))) return e.id;
    }
    for (const e of entities) {
      if (e.kind === "duct" && inRect(world, { x: e.at.x, y: e.at.y, w: e.w, h: e.h })) return e.id;
    }
    for (const e of entities) {
      if (e.kind === "panel_outline" && inRect(world, panelOutlineRect(e))) return e.id;
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
      case "macro":
        void this.commitMacro(this.snap(world));
        break;
      case "harness": {
        // ハーネス境界は矩形ドラッグ。押した点をグリッドに乗せて始点にする
        this.dragStartWorld = this.snap(world);
        this.rubberEnd = this.dragStartWorld;
        this.dragMode = "harness";
        break;
      }
      // 図形 (G10): 矩形・線・円はドラッグ、折れ線はクリックを重ねてダブルクリックで確定
      case "shape_rect":
      case "shape_line":
      case "shape_circle": {
        this.dragStartWorld = this.snap(world);
        this.rubberEnd = this.dragStartWorld;
        this.dragMode = "shape";
        break;
      }
      case "shape_polyline":
        this.shapePoints.push(this.snap(world));
        break;
      // 盤枠・ダクトは矩形ドラッグ、DINレールは水平ドラッグ (取付線は押した点の高さ)
      case "panel_outline":
      case "duct":
      case "din_rail": {
        this.dragStartWorld = this.snap(world);
        this.rubberEnd = this.dragStartWorld;
        this.dragMode = this.tool === "din_rail" ? "panel_rail" : "panel_rect";
        break;
      }
      case "footprint":
        void this.commitFootprint(this.snap(world));
        break;
      case "panel_legend":
        void this.commitPanelLegend(this.snap(world));
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
        this.moveDelta = this.railSnappedDelta(snapped);
        break;
      }
      case "rubber":
        this.rubberEnd = world;
        break;
      case "harness":
      case "shape":
      case "panel_rect":
      case "panel_rail":
        this.rubberEnd = this.snap(world);
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
                e.kind === "wire" || e.kind === "harness" || e.kind === "shape"
                  ? e.points
                  : "at" in e
                    ? [e.at]
                    : [];
              if (pts.length && pts.every((p) => p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1)) {
                sel.add(e.id);
              }
            }
            this.store.selection = sel;
          }
        }
        break;
      }
      case "panel_rect":
      case "panel_rail": {
        // 盤の要素も1回のadd_entityコマンドで作る (undo一発で消える)
        if (sheet) {
          const command =
            this.tool === "panel_outline"
              ? panelOutlineAddCommand(sheet, this.dragStartWorld, this.rubberEnd)
              : this.tool === "duct"
                ? ductAddCommand(sheet, this.dragStartWorld, this.rubberEnd)
                : dinRailAddCommand(sheet, this.dragStartWorld, this.rubberEnd);
          if (command) {
            await this.store.execute(command);
            useUiStore().log(i18n.global.t(`panel.added.${this.tool}`));
          }
        }
        break;
      }
      case "shape": {
        // 図形を1回のadd_entityコマンドで作る (undo一発で消える)
        if (sheet && isShapeTool(this.tool)) {
          const kind = SHAPE_TOOLS[this.tool];
          const command = shapeAddCommand(sheet, kind, [
            { x: this.dragStartWorld.x, y: this.dragStartWorld.y },
            { x: this.rubberEnd.x, y: this.rubberEnd.y },
          ]);
          if (command) {
            await this.store.execute(command);
            useUiStore().log(i18n.global.t(`shapes.added.${kind}`));
          }
        }
        break;
      }
      case "harness": {
        // 囲みを1回のadd_entityコマンドで作る (undo一発で消える)
        if (sheet) {
          const command = harnessAddCommand(sheet, this.dragStartWorld, this.rubberEnd);
          if (command && command.type === "add_entity" && command.entity.kind === "harness") {
            const { id, name } = command.entity;
            await this.store.execute(command);
            const after = this.store.activeSheet;
            const count = after ? harnessWireCount(after, id) : 0;
            useUiStore().log(i18n.global.t("harness.createdLog", { name, count }));
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
      first.kind === "wire" || first.kind === "harness" || first.kind === "shape"
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
    // 折れ線はダブルクリックで確定する (最後のクリックは点として数えない)
    if (this.tool === "shape_polyline") void this.commitPolyline();
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
    // ⌘C/V: 選択範囲を無名マクロとして覚え、貼り付けは同じ配置モードで置く
    if (mod && ev.key.toLowerCase() === "c") return await this.copySelection();
    if (mod && ev.key.toLowerCase() === "v") return this.pasteMacro();
    switch (ev.key) {
      case "Escape":
        if (this.tool === "wire" && this.wirePoints.length) void this.commitWire();
        else if (this.tool === "shape_polyline" && this.shapePoints.length) void this.commitPolyline();
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
        // 盤のフットプリントは0度と90度だけ (v1の描画がこの2つしか持たない)
        if (this.tool === "footprint") {
          this.placeRotation = this.placeRotation === 0 ? 90 : 0;
          this.requestRedraw();
          return true;
        }
        if (this.tool === "place" || this.tool === "macro") {
          this.placeRotation = (this.placeRotation + 90) % 360;
          this.requestRedraw();
          return true;
        }
        return false;
      case "Tab":
        if (this.tool === "macro" && this.placeMacro) {
          this.cycleMacroVariant();
          return true;
        }
        return false;
      case "Enter":
        if (this.tool === "wire") {
          void this.commitWire();
          return true;
        }
        if (this.tool === "shape_polyline") {
          void this.commitPolyline();
          return true;
        }
        return false;
    }
    return false;
  }

  onKeyUp(ev: KeyboardEvent) {
    if (ev.code === "Space") this.spaceHeld = false;
  }

  /**
   * ⌘C: 選択範囲を無名マクロとしてメモリに覚える (ファイルには書かない)。
   * 選択が空のときはキー入力を横取りしない (通常のコピーを邪魔しない)。
   */
  async copySelection(): Promise<boolean> {
    const sheet = this.store.activeSheet;
    if (!sheet || this.store.selection.size === 0) return false;
    const ids = [...this.store.selection];
    const copied = await useMacrosStore().copy(sheet.id, ids);
    if (!copied) return false;
    useUiStore().log(i18n.global.t("macros.copiedLog", { count: ids.length }));
    return true;
  }

  /** ⌘V: 覚えている無名マクロの配置モードへ入る (何もコピーしていなければ何もしない)。 */
  pasteMacro(): boolean {
    const pasted = useMacrosStore().paste();
    if (!pasted) return false;
    this.startMacroPlacement(pasted, { fromLibrary: false });
    useUiStore().log(i18n.global.t("macros.pasteLog"));
    return true;
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

  /**
   * 配置ゴーストのシンボル (カーソル位置・現在の回転・配置に使う属性つき)。
   * ブロックは形が属性 (ピン表) で決まるので、ゴーストもこの属性から描く。
   */
  ghostInstance(): SymbolInstance | null {
    if (this.tool !== "place" || !this.placeSymbolId) return null;
    return {
      id: "ghost",
      symbol_id: this.placeSymbolId,
      at: this.cursorWorld,
      rotation: this.placeRotation,
      mirror: false,
      reference: "",
      value: "",
      attrs: this.placePart?.attrs ?? {},
    };
  }

  /**
   * 移動量をDINレールへ吸い付かせる (フットプリントを1つだけ動かしているとき)。
   * 盤以外の要素・複数選択のときは何もしない。
   */
  private railSnappedDelta(delta: Pt): Pt {
    const sheet = this.store.activeSheet;
    if (!sheet || this.store.selection.size !== 1) return delta;
    const [id] = [...this.store.selection];
    const e = sheet.entities[id];
    if (!e || e.kind !== "footprint") return delta;
    return railSnapDelta(sheet, e, delta);
  }

  /**
   * 未配置部品をフットプリントとして確定する。置いたら選択ツールへ戻る
   * (同じ部品は1台しか無いので、続けて置けると二重配置になる)。
   */
  private async commitFootprint(at: Pt) {
    const sheet = this.store.activeSheet;
    const component = this.placeFootprint;
    if (!sheet || !component) return;
    await this.store.execute(footprintAddCommand(sheet, component, at, this.placeRotation));
    useUiStore().log(
      i18n.global.t("panel.placedLog", {
        reference: component.reference,
        part: component.part_no,
      }),
    );
    this.setTool("select");
    this.requestRedraw();
  }

  /**
   * 凡例表を1つ置いて選択ツールへ戻る。中身は描画のたびにシートのフットプリントから
   * 導出するので、ここで決めるのは置き場所だけ。
   */
  private async commitPanelLegend(at: Pt) {
    const sheet = this.store.activeSheet;
    if (!sheet) return;
    await this.store.execute(panelLegendAddCommand(sheet, at));
    useUiStore().log(i18n.global.t("panel.legendPlacedLog"));
    this.setTool("select");
    this.requestRedraw();
  }

  private async commitPlace(at: Pt) {
    const sheet = this.store.activeSheet;
    const ghost = this.ghostInstance();
    const def = ghost ? this.store.resolveInstance(ghost) : undefined;
    if (!sheet || !ghost || !def) return;
    const entity: Entity = {
      kind: "symbol",
      id: crypto.randomUUID(),
      symbol_id: ghost.symbol_id,
      at,
      rotation: this.placeRotation,
      mirror: false,
      reference: this.nextReference(def.ref_prefix),
      value: this.placePart?.value ?? "",
      attrs: this.placePart?.attrs ?? {},
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

  /**
   * 折れ線を確定する (ダブルクリック・Enter・Esc)。点が1つ以下なら何も作らずに捨てる。
   * 確定後も折れ線ツールのままなので、続けて何本でも描ける。
   */
  private async commitPolyline() {
    const sheet = this.store.activeSheet;
    const pts = this.shapePoints;
    this.shapePoints = [];
    if (sheet && pts.length >= 2) {
      const command = shapeAddCommand(
        sheet,
        "polyline",
        pts.map((p) => ({ x: p.x, y: p.y })),
      );
      if (command) {
        await this.store.execute(command);
        useUiStore().log(i18n.global.t("shapes.added.polyline"));
      }
    }
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
    // 配置ゴースト (ブロックはピン表の属性どおりの形で出す)
    const ghost = this.ghostInstance();
    if (ghost) {
      const def = this.store.resolveInstance(ghost);
      if (def) {
        ctx.globalAlpha = 0.6;
        drawSymbol(ctx, vp, ghost, def, false, theme.selection);
        ctx.globalAlpha = 1;
      }
    }
    // 回路マクロの配置ゴースト: 確定後と同じ回路をカーソル位置へ半透明で描く
    if (this.tool === "macro" && this.placeMacro) {
      const entities = placeMacroEntities(
        macroEntities(this.placeMacro.macro, this.macroVariantKey),
        this.cursorWorld,
        this.placeRotation,
      );
      drawMacroGhost(ctx, vp, entities, (id) => this.store.resolveSymbol(id));
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
          if (e.kind === "wire" || e.kind === "harness" || e.kind === "shape") {
            ctx.beginPath();
            e.points.forEach((p, i) => {
              const s = vp.toScreen({ x: p.x + this.moveDelta.x, y: p.y + this.moveDelta.y });
              if (i === 0) ctx.moveTo(s.x, s.y);
              else ctx.lineTo(s.x, s.y);
            });
            if (e.kind === "harness") ctx.closePath();
            ctx.stroke();
          } else if (e.kind === "symbol") {
            const def = this.store.resolveInstance(e);
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
    // ハーネス境界のドラッグプレビュー (確定後と同じ破線で見せる)
    if (this.dragMode === "harness") {
      const preview = harnessRectPoints(this.dragStartWorld, this.rubberEnd);
      drawHarness(ctx, vp, preview, "", false);
    }
    // 図形のドラッグプレビュー (確定後と同じ形を選択色で見せる)
    if (this.dragMode === "shape" && isShapeTool(this.tool)) {
      const preview = newShape(SHAPE_TOOLS[this.tool], [
        { x: this.dragStartWorld.x, y: this.dragStartWorld.y },
        { x: this.rubberEnd.x, y: this.rubberEnd.y },
      ]);
      drawShape(ctx, vp, preview, true);
    }
    // 折れ線の作図中プレビュー: 打った点 + カーソルまでの1本
    if (this.tool === "shape_polyline" && this.shapePoints.length) {
      const preview = newShape("polyline", [
        ...this.shapePoints.map((p) => ({ x: p.x, y: p.y })),
        { x: this.cursorWorld.x, y: this.cursorWorld.y },
      ]);
      drawShape(ctx, vp, preview, true);
    }
    // 盤の作図プレビュー: 盤枠・ダクトは矩形、DINレールは帯 (確定後と同じ高さ)
    if (this.dragMode === "panel_rect" || this.dragMode === "panel_rail") {
      const a = vp.toScreen(this.dragStartWorld);
      const b = vp.toScreen(this.rubberEnd);
      ctx.strokeStyle = theme.selection;
      ctx.lineWidth = 1.5;
      ctx.setLineDash([5, 3]);
      if (this.dragMode === "panel_rail") {
        const half = (DIN_RAIL_H / 2) * vp.scale;
        ctx.strokeRect(Math.min(a.x, b.x), a.y - half, Math.abs(b.x - a.x), half * 2);
      } else {
        ctx.strokeRect(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.abs(b.x - a.x), Math.abs(b.y - a.y));
      }
      ctx.setLineDash([]);
    }
    // フットプリントの配置ゴースト: 確定後と同じ矩形+参照記号をカーソル位置へ出す
    const sheetForGhost = this.store.activeSheet;
    if (this.tool === "footprint" && this.placeFootprint && sheetForGhost) {
      const component = this.placeFootprint;
      const at = footprintPlaceAt(sheetForGhost, component, this.cursorWorld, this.placeRotation);
      const size = footprintSize({ w: component.w, h: component.h, rotation: this.placeRotation });
      const s = vp.toScreen(at);
      ctx.globalAlpha = 0.7;
      ctx.fillStyle = panelTheme.footprint;
      ctx.strokeStyle = theme.selection;
      ctx.lineWidth = 1.5;
      ctx.fillRect(s.x, s.y, size.w * vp.scale, size.h * vp.scale);
      ctx.strokeRect(s.x, s.y, size.w * vp.scale, size.h * vp.scale);
      ctx.fillStyle = panelTheme.reference;
      ctx.font = `600 ${Math.max(9, 3 * vp.scale)}px var(--mono-font, monospace)`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(component.reference, s.x + (size.w * vp.scale) / 2, s.y + (size.h * vp.scale) / 2);
      ctx.textAlign = "start";
      ctx.textBaseline = "alphabetic";
      ctx.globalAlpha = 1;
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
    if (sheet && (this.tool === "wire" || this.tool === "place" || this.tool === "macro")) {
      for (const e of Object.values(sheet.entities)) {
        if (e.kind !== "symbol") continue;
        const def = this.store.resolveInstance(e);
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
