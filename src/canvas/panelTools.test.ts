// 盤レイアウトの作図ツール・未配置部品・プロパティ編集の仕様テスト
// (M4仕様 §8、.pen「M4デザイン - 盤レイアウト」/「M4デザイン - リボン パネル/レポート/管理タブ」)。
// 図面を変えるのは必ずCommandなので、ここでは「操作 → どのコマンドを送るか」を固定する。
import { describe, expect, it } from "vitest";

import type { DinRail, Entity, Footprint, Sheet, UnplacedComponent } from "../ipc";
import { DEFAULT_FOOTPRINT_SIZE, DIN_RAIL_H } from "./panel";
import {
  dinRailAddCommand,
  ductAddCommand,
  footprintAddCommand,
  footprintPlaceAt,
  nextPanelName,
  panelOutlineAddCommand,
  panelToolsAllowed,
  panelUpdateCommand,
  railOfFootprint,
  panelLegendAddCommand,
  railSnapDelta,
  renumberPanelItemsCommand,
  schematicToolsAllowed,
  unplacedRow,
} from "./panelTools";

function panelSheet(...entities: Entity[]): Sheet {
  return {
    id: "panel-1",
    name: "Panel1",
    kind: "panel",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
}

const rail = (id: string, y: number): Entity & DinRail => ({
  kind: "din_rail",
  id,
  y,
  x1: 40,
  x2: 260,
});

const component: UnplacedComponent = {
  reference: "K1",
  part_no: "MDK-RLY-24V",
  symbol_id: "relay_coil",
  sheet_id: "s1",
  w: 25,
  h: 65,
};

describe("panel drawing tools (enclosure / DIN rail / duct)", () => {
  // ja: 盤枠ツールの矩形ドラッグで、盤の外形を1つ足すコマンドを送る (undo一発で消える)。
  it("turns a rectangle drag into one add_entity command for the enclosure", () => {
    const command = panelOutlineAddCommand(panelSheet(), { x: 40, y: 30 }, { x: 240, y: 230 });
    expect(command).toMatchObject({
      type: "add_entity",
      sheet_id: "panel-1",
      entity: { kind: "panel_outline", at: { x: 40, y: 30 }, w: 200, h: 200, name: "CP-1" },
    });
  });

  // ja: 逆向き(右下から左上へ)にドラッグしても同じ盤ができる。
  it("accepts a drag in any direction for the enclosure", () => {
    const command = panelOutlineAddCommand(panelSheet(), { x: 240, y: 230 }, { x: 40, y: 30 });
    expect(command).toMatchObject({
      entity: { at: { x: 40, y: 30 }, w: 200, h: 200 },
    });
  });

  // ja: 盤の名前は自動で採番する (CP-1, CP-2 …)。
  it("numbers each new enclosure (CP-1, CP-2 …)", () => {
    const sheet = panelSheet({
      kind: "panel_outline",
      id: "o1",
      at: { x: 0, y: 0 },
      w: 10,
      h: 10,
      name: "CP-1",
    });
    expect(nextPanelName(sheet)).toBe("CP-2");
    expect(panelOutlineAddCommand(sheet, { x: 0, y: 0 }, { x: 50, y: 50 })).toMatchObject({
      entity: { name: "CP-2" },
    });
  });

  // ja: クリックしただけ(つぶれた矩形)ではコマンドを送らない。
  it("sends no command when the enclosure drag is a mere click", () => {
    expect(panelOutlineAddCommand(panelSheet(), { x: 40, y: 30 }, { x: 40.2, y: 30.2 })).toBeNull();
    expect(ductAddCommand(panelSheet(), { x: 40, y: 30 }, { x: 40, y: 30 })).toBeNull();
    expect(dinRailAddCommand(panelSheet(), { x: 40, y: 30 }, { x: 40, y: 30 })).toBeNull();
  });

  // ja: DINレールは水平ドラッグで作る。取付線の高さは押し始めた点で決まる。
  it("draws a DIN rail horizontally from the point where the drag started", () => {
    const command = dinRailAddCommand(panelSheet(), { x: 60, y: 120 }, { x: 220, y: 133 });
    expect(command).toMatchObject({
      type: "add_entity",
      entity: { kind: "din_rail", y: 120, x1: 60, x2: 220 },
    });
  });

  // ja: ダクトツールの矩形ドラッグで配線ダクトを1つ足す。
  it("turns a rectangle drag into one duct", () => {
    const command = ductAddCommand(panelSheet(), { x: 50, y: 40 }, { x: 230, y: 70 });
    expect(command).toMatchObject({
      type: "add_entity",
      entity: { kind: "duct", at: { x: 50, y: 40 }, w: 180, h: 30 },
    });
  });
});

describe("unplaced parts panel", () => {
  // ja: 未配置部品の行には参照記号・型番・外形寸法が並ぶ。
  it("shows the reference, part number and outline size of an unplaced part", () => {
    expect(unplacedRow(component)).toEqual({
      reference: "K1",
      partNo: "MDK-RLY-24V",
      w: 25,
      h: 65,
      sizeLabel: "25 × 65 mm",
      status: "sized",
    });
  });

  // ja: 部品DBに寸法が無い型番は既定の矩形で置ける (行には「寸法から自動」と出る)。
  it("marks a part that falls back to the default rectangle", () => {
    const row = unplacedRow({ ...component, ...DEFAULT_FOOTPRINT_SIZE });
    expect(row.status).toBe("default");
    expect(row.sizeLabel).toBe("20 × 40 mm");
  });

  // ja: 型番が入っていない部品は「未登録」として注意付きで並ぶ。
  it("flags a part with no part number as unregistered", () => {
    expect(unplacedRow({ ...component, part_no: "" }).status).toBe("unregistered");
  });
});

describe("footprint placement and rail snapping", () => {
  // ja: フットプリントはカーソルを中心に置かれる (外形の左上ではなく真ん中を持つ)。
  it("centres the footprint on the cursor", () => {
    const at = footprintPlaceAt(panelSheet(), component, { x: 100, y: 250 }, 0);
    expect(at).toEqual({ x: 87.5, y: 217.5 });
  });

  // ja: 90度回すと幅と高さが入れ替わり、置かれる位置も回した外形の中心になる。
  it("centres a footprint turned 90 degrees on its swapped outline", () => {
    const at = footprintPlaceAt(panelSheet(), component, { x: 100, y: 250 }, 90);
    expect(at).toEqual({ x: 67.5, y: 237.5 });
  });

  // ja: DINレールの近くへ落とすと、部品の中心がレールの取付線へ吸い付く。
  it("snaps the footprint centre onto a nearby DIN rail", () => {
    const sheet = panelSheet(rail("r1", 150));
    const at = footprintPlaceAt(sheet, component, { x: 100, y: 152 }, 0);
    expect(at).toEqual({ x: 87.5, y: 117.5 });
  });

  // ja: レールから離れた場所ではカーソルの位置のまま置かれる (勝手に吸い付かない)。
  it("leaves the footprint where the cursor is when no rail is near", () => {
    const sheet = panelSheet(rail("r1", 150));
    const at = footprintPlaceAt(sheet, component, { x: 100, y: 250 }, 0);
    expect(at).toEqual({ x: 87.5, y: 217.5 });
    expect(DIN_RAIL_H).toBe(35);
  });

  // ja: 未配置部品を置くと、参照記号と型番を持つフットプリントが1つ足される。
  it("adds one footprint carrying the reference and part number", () => {
    const sheet = panelSheet(rail("r1", 150));
    expect(footprintAddCommand(sheet, component, { x: 100, y: 152 }, 0)).toMatchObject({
      type: "add_entity",
      sheet_id: "panel-1",
      entity: {
        kind: "footprint",
        at: { x: 87.5, y: 117.5 },
        w: 25,
        h: 65,
        rotation: 0,
        reference: "K1",
        part_no: "MDK-RLY-24V",
      },
    });
  });

  // ja: 置いたフットプリントをレールの近くへ動かすと、移動量がレールへ吸い付くように直る。
  it("corrects a drag so the moved footprint lands on the rail", () => {
    const fp: Entity & Footprint = {
      kind: "footprint",
      id: "fp1",
      at: { x: 60, y: 40 },
      w: 25,
      h: 65,
      rotation: 0,
      reference: "K1",
      part_no: "MDK-RLY-24V",
    };
    const sheet = panelSheet(rail("r1", 150), fp);
    // 中心 (72.5) を +75 動かすと 147.5 = レール150の近く → 中心が150に乗るよう +77.5 へ直す
    expect(railSnapDelta(sheet, fp, { x: 10, y: 75 })).toEqual({ x: 10, y: 77.5 });
    // レールから遠い移動はそのまま
    expect(railSnapDelta(sheet, fp, { x: 10, y: 200 })).toEqual({ x: 10, y: 200 });
  });

  // ja: フットプリントが乗っているレールが分かる (プロパティの「所属レール」)。
  it("tells which rail a footprint sits on", () => {
    const fp: Entity & Footprint = {
      kind: "footprint",
      id: "fp1",
      at: { x: 60, y: 117.5 },
      w: 25,
      h: 65,
      rotation: 0,
      reference: "K1",
      part_no: "",
    };
    const sheet = panelSheet(rail("r1", 60), rail("r2", 150), fp);
    expect(railOfFootprint(sheet, fp)).toEqual({ rail: sheet.entities.r2, index: 2 });
    const off = { ...fp, at: { x: 60, y: 240 } };
    expect(railOfFootprint(sheet, off)).toBeNull();
  });
});

describe("panel entity properties", () => {
  const fp: Entity & Footprint = {
    kind: "footprint",
    id: "fp1",
    at: { x: 60, y: 40 },
    w: 25,
    h: 65,
    rotation: 0,
    reference: "K1",
    part_no: "MDK-RLY-24V",
  };

  // ja: プロパティの編集は update_entity コマンドになる (モデルを直接書き換えない)。
  it("turns a property edit into one update_entity command", () => {
    expect(panelUpdateCommand("panel-1", fp, { w: 45, rotation: 90 })).toEqual({
      type: "update_entity",
      sheet_id: "panel-1",
      entity: { ...fp, w: 45, rotation: 90 },
    });
  });

  // ja: 何も変わっていなければコマンドを送らない (無駄なundo段を作らない)。
  it("sends no command when nothing changed", () => {
    expect(panelUpdateCommand("panel-1", fp, { w: 25, h: 65, rotation: 0 })).toBeNull();
  });

  // ja: 幅・高さに0や文字を入れても図面は壊れない (その欄は変えない)。
  it("ignores a width or height that is not a positive number", () => {
    expect(panelUpdateCommand("panel-1", fp, { w: 0 })).toBeNull();
    expect(panelUpdateCommand("panel-1", fp, { h: Number.NaN })).toBeNull();
  });

  // ja: 回転は0度と90度だけ (v1の描画がこの2つしか持たない)。
  it("keeps the rotation at 0 or 90 degrees", () => {
    expect(panelUpdateCommand("panel-1", fp, { rotation: 270 })).toMatchObject({
      entity: { rotation: 90 },
    });
  });

  // ja: 盤枠は名前と寸法を、レールは取付線と長さを編集できる。
  it("edits the enclosure name and size, and the rail line and length", () => {
    const outline: Entity = {
      kind: "panel_outline",
      id: "o1",
      at: { x: 20, y: 20 },
      w: 200,
      h: 150,
      name: "CP-1",
    };
    expect(panelUpdateCommand("panel-1", outline, { name: "制御盤", w: 300 })).toMatchObject({
      entity: { name: "制御盤", w: 300, h: 150 },
    });
    const r = rail("r1", 150);
    expect(panelUpdateCommand("panel-1", r, { y: 160, x2: 300 })).toMatchObject({
      entity: { y: 160, x1: 40, x2: 300 },
    });
  });

  // ja: 回路図の要素(配線など)はこの編集の対象外。
  it("refuses to edit a schematic entity through the panel properties", () => {
    const wire: Entity = {
      kind: "wire",
      id: "w1",
      points: [],
      color: "red",
      sq: 0.75,
      length_m: null,
      part_no: null,
      net: null,
    };
    expect(panelUpdateCommand("panel-1", wire, { w: 10 })).toBeNull();
  });
});

describe("tools allowed per sheet kind", () => {
  const schematic: Sheet = { ...panelSheet(), id: "s1", name: "Sheet1", kind: "schematic" };

  // ja: 盤レイアウトシートでは回路図のツール(配線・部品挿入など)を使えない。
  it("disables the schematic tools on a panel sheet", () => {
    expect(schematicToolsAllowed(panelSheet())).toBe(false);
    expect(schematicToolsAllowed(schematic)).toBe(true);
  });

  // ja: 回路図シートでは盤のツール(盤枠・レール・ダクト)を使えない。
  it("disables the panel tools on a schematic sheet", () => {
    expect(panelToolsAllowed(schematic)).toBe(false);
    expect(panelToolsAllowed(panelSheet())).toBe(true);
  });

  // ja: シートが1枚も開いていなければどちらのツールも使えない。
  it("disables both tool sets when no sheet is open", () => {
    expect(schematicToolsAllowed(null)).toBe(false);
    expect(panelToolsAllowed(null)).toBe(false);
  });
});

describe("balloon numbering and the legend table", () => {
  const fp = (id: string, x: number): Entity & Footprint => ({
    kind: "footprint",
    id,
    at: { x, y: 120 },
    w: 20,
    h: 40,
    rotation: 0,
    reference: id.toUpperCase(),
    part_no: "",
    item_no: null,
  });

  // ja: バルーン採番は renumber_panel_items コマンド1回 (undo一発で採番前へ戻せる)。
  it("turns balloon numbering into one renumber_panel_items command", () => {
    const sheet = panelSheet(fp("k1", 100), fp("k2", 200));
    expect(renumberPanelItemsCommand(sheet)).toEqual({
      type: "renumber_panel_items",
      sheet_id: "panel-1",
    });
  });

  // ja: 置く部品が1つも無い盤では採番のコマンドを送らない。
  it("sends no command when there is nothing to number", () => {
    expect(renumberPanelItemsCommand(panelSheet())).toBeNull();
  });

  // ja: 凡例表の挿入は add_entity コマンド1つで、決めるのは置き場所だけ。
  it("inserts the legend table with one add_entity command carrying only its position", () => {
    const command = panelLegendAddCommand(panelSheet(), { x: 300, y: 200 });
    expect(command).toMatchObject({
      type: "add_entity",
      sheet_id: "panel-1",
      entity: { kind: "panel_legend", at: { x: 300, y: 200 } },
    });
  });
});
