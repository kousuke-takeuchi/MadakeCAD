// 回路マクロのプレビュー/配置ジオメトリの仕様テスト
// (計画: docs/superpowers/plans/2026-08-22-m4-phase2-macros-xref-nav.md Task 2)。
//
// マクロの座標は基準点からの相対で保存されているので、ゴースト描画も確定も
// 「回転してからカーソル位置へ寄せる」同じ変換を通る (Rust macros::place_entity と同一規則)。
import { describe, expect, it } from "vitest";
import {
  macroCommands,
  macroEntities,
  macroName,
  macroSheet,
  macroVariantKeys,
  macroValueSetLabel,
  macroVariantLabel,
  placeMacroEntities,
  placePoint,
} from "./macroPreview";
import type { Command, Entity, Macro } from "../ipc";

const NIL = "00000000-0000-0000-0000-000000000000";

function addSymbol(id: string, reference: string, x: number, y: number, rotation = 0): Command {
  return {
    type: "add_entity",
    sheet_id: NIL,
    entity: {
      kind: "symbol",
      id,
      symbol_id: "relay_coil",
      at: { x, y },
      rotation,
      mirror: false,
      reference,
      value: "",
      attrs: {},
    },
  };
}

function addWire(id: string, points: { x: number; y: number }[]): Command {
  return {
    type: "add_entity",
    sheet_id: NIL,
    entity: {
      kind: "wire",
      id,
      points,
      color: "red",
      sq: 0.75,
      length_m: null,
      part_no: null,
      net: null,
    },
  };
}

function macro(): Macro {
  return {
    id: "motor_dol",
    name: "Motor starter (DOL)",
    name_ja: "モータ直入れ起動",
    description: "",
    description_ja: "",
    category: "motor",
    base_point: { x: 100, y: 130 },
    commands: [
      addSymbol("s1", "K1", 0, -10),
      addWire("w1", [
        { x: 0, y: 0 },
        { x: 20, y: 0 },
      ]),
    ],
    variants: [
      { key: "B", name: "Reversing", name_ja: "正逆転", commands: [addSymbol("s2", "K1", 0, -20)] },
      { key: "C", name: "Jog", name_ja: "寸動", commands: [addSymbol("s3", "K1", 0, -30)] },
    ],
    placeholders: [
      { key: "rating", label: "Rating", label_ja: "定格", targets: [{ entity: "s1", field: "value" }] },
    ],
    value_sets: [
      { id: "0_75_kw", label: "0.75 kW", label_ja: "0.75kW", values: { rating: "0.75kW" } },
      { id: "1_5_kw", label: "1.5 kW", label_ja: "", values: { rating: "1.5kW" } },
    ],
  };
}

describe("macroVariantKeys", () => {
  // ja: バリアントキーは既定の「A」が先頭で、そのあとにマクロが持つバリアントが並ぶ
  it("lists the default variant A first, followed by the macro's own variants", () => {
    expect(macroVariantKeys(macro())).toEqual(["A", "B", "C"]);
  });

  // ja: バリアントを持たないマクロのキーは「A」の1つだけ
  it("gives a macro without variants the single key A", () => {
    expect(macroVariantKeys({ ...macro(), variants: [] })).toEqual(["A"]);
  });
});

describe("macroVariantLabel", () => {
  // ja: バリアントの表示名は「キー+名前」で、UI言語が日本語なら日本語名を使う
  it("labels a variant with its key and name, in Japanese when the UI is Japanese", () => {
    expect(macroVariantLabel(macro(), "B", "ja")).toBe("B 正逆転");
    expect(macroVariantLabel(macro(), "B", "en")).toBe("B Reversing");
  });

  // ja: 名前の無いバリアント(既定のA)はキーだけを表示する
  it("shows just the key for a variant that has no name, such as the default A", () => {
    expect(macroVariantLabel(macro(), "A", "ja")).toBe("A");
  });
});

describe("macroValueSetLabel", () => {
  // ja: 値セットの表示名はUI言語に合わせ、日本語名が無ければ英語名を使う
  it("labels a value set in the UI language, falling back to the English name", () => {
    const [first, second] = macro().value_sets;
    expect(macroValueSetLabel(first, "ja")).toBe("0.75kW");
    expect(macroValueSetLabel(first, "en")).toBe("0.75 kW");
    expect(macroValueSetLabel(second, "ja")).toBe("1.5 kW");
  });

  // ja: 名前の無い値セットはidをそのまま表示する
  it("falls back to the id for a value set with no name at all", () => {
    expect(macroValueSetLabel({ id: "1_5_kw", label: "", label_ja: "", values: {} }, "ja")).toBe(
      "1_5_kw",
    );
  });
});

describe("macroCommands / macroEntities", () => {
  // ja: 「A」と未指定は既定のコマンド列、それ以外のキーはそのバリアントのコマンド列を読む
  it("reads the default commands for A (or no key) and the variant's own commands otherwise", () => {
    const m = macro();
    expect(macroCommands(m, "A")).toHaveLength(2);
    expect(macroCommands(m, null)).toHaveLength(2);
    expect(macroCommands(m, "B")).toHaveLength(1);
  });

  // ja: 知らないバリアントキーを指定したときは既定のコマンド列に戻る(空表示にしない)
  it("falls back to the default commands when the variant key is unknown", () => {
    expect(macroCommands(macro(), "Z")).toHaveLength(2);
  });

  // ja: マクロが置くエンティティだけを取り出す(add_entity以外のコマンドは描かない)
  it("extracts only the entities the macro adds", () => {
    const entities = macroEntities(macro(), "A");
    expect(entities.map((e) => e.kind)).toEqual(["symbol", "wire"]);
  });
});

describe("placePoint", () => {
  // ja: 回転0のときは基準点からの相対座標をそのまま挿入位置へ足す
  it("just shifts a point by the insertion position when the rotation is zero", () => {
    expect(placePoint({ x: 5, y: -10 }, 0, { x: 100, y: 200 })).toEqual({ x: 105, y: 190 });
  });

  // ja: 90度回転は用紙座標系(Y下向き)で (x,y) → (-y,x) に写る
  it("maps a point to (-y, x) for a quarter turn, in the paper coordinate system", () => {
    expect(placePoint({ x: 5, y: -10 }, 90, { x: 0, y: 0 })).toEqual({ x: 10, y: 5 });
    expect(placePoint({ x: 5, y: -10 }, 180, { x: 0, y: 0 })).toEqual({ x: -5, y: 10 });
    expect(placePoint({ x: 5, y: -10 }, 270, { x: 0, y: 0 })).toEqual({ x: -10, y: -5 });
  });
});

describe("placeMacroEntities", () => {
  // ja: 配置後のシンボルはカーソル位置へ移り、シンボル自身の向きにも回転が加わる
  it("moves each symbol to the cursor and adds the placement rotation to its own", () => {
    const placed = placeMacroEntities(macroEntities(macro(), "A"), { x: 100, y: 100 }, 90);
    const symbol = placed.find((e) => e.kind === "symbol") as Extract<Entity, { kind: "symbol" }>;
    expect(symbol.at).toEqual({ x: 110, y: 100 });
    expect(symbol.rotation).toBe(90);
  });

  // ja: 配線の頂点もシンボルと同じ変換で動くので、回転しても回路の形は崩れない
  it("moves wire points through the same transform, so a rotated macro keeps its shape", () => {
    const placed = placeMacroEntities(macroEntities(macro(), "A"), { x: 100, y: 100 }, 90);
    const wire = placed.find((e) => e.kind === "wire") as Extract<Entity, { kind: "wire" }>;
    expect(wire.points).toEqual([
      { x: 100, y: 100 },
      { x: 100, y: 120 },
    ]);
  });

  // ja: 配置は複製に対して行われ、元のマクロ定義は書き換えない(何度でも同じ形で置ける)
  it("works on copies, leaving the macro definition untouched so it can be placed again", () => {
    const entities = macroEntities(macro(), "A");
    placeMacroEntities(entities, { x: 300, y: 300 }, 180);
    const symbol = entities.find((e) => e.kind === "symbol") as Extract<Entity, { kind: "symbol" }>;
    expect(symbol.at).toEqual({ x: 0, y: -10 });
  });
});

describe("macroSheet", () => {
  // ja: プレビューは選んだバリアントのエンティティだけを載せた仮のシートを描く
  it("builds a throwaway sheet holding only the chosen variant's entities", () => {
    const sheet = macroSheet(macro(), "B");
    expect(Object.keys(sheet.entities)).toHaveLength(1);
    expect(sheet.id).toContain("motor_dol");
  });
});

describe("macroName", () => {
  // ja: マクロ名はUI言語が日本語なら日本語名、それ以外は英語名を出す
  it("shows the Japanese name when the UI is Japanese and the English name otherwise", () => {
    expect(macroName(macro(), "ja")).toBe("モータ直入れ起動");
    expect(macroName(macro(), "en")).toBe("Motor starter (DOL)");
  });

  // ja: 日本語名が空のマクロはどちらの言語でも英語名を出す(空欄にしない)
  it("falls back to the English name when a macro has no Japanese name", () => {
    expect(macroName({ ...macro(), name_ja: "" }, "ja")).toBe("Motor starter (DOL)");
  });
});
