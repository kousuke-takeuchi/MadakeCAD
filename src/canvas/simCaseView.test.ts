import { describe, expect, it } from "vitest";
import type { SimCase, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { faultPoints, roleAttributes } from "./simCaseView";

const def = (role?: string) => ({ id: "d", pins: [{ number: "1", at: { x: 0, y: 0 } }, { number: "2", at: { x: 10, y: 0 } }], sim: role ? { role } : undefined }) as unknown as SymbolDef;
const sym = { kind: "symbol", id: "f1", symbol_id: "d", at: { x: 0, y: 10 }, rotation: 0, mirror: false, reference: "F1", value: "", attrs: {} } as unknown as SymbolInstance;
const sheet = {
  id: "s",
  entities: {
    w1: { kind: "wire", id: "w1", points: [{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 20 }] },
    f1: sym,
  },
} as unknown as Sheet;
const caseWith = (faults: SimCase["faults"]): SimCase => ({ id: "c", name: "c", open_switches: [], overrides: {}, faults });

describe("simulation case view helpers", () => {
  // ja: 選んだ部品で編集できる値は役割で決まり、どの部品でもピン定格を最後に足す
  it("the editable attributes follow the part's role, always ending with the pin rating", () => {
    expect(roleAttributes(def("source"))).toEqual(["voltage", "r_int_ohm", "i_max_a", "pin_rating_a"]);
    expect(roleAttributes(def("load"))).toEqual(["current_a", "v_min", "pin_rating_a"]);
    expect(roleAttributes(def("fuse"))).toEqual(["rating_a", "pin_rating_a"]);
    expect(roleAttributes(def("converter"))).toEqual(["v_out", "eta", "i_out_a", "i_limit_a", "v_in_min", "pin_rating_a"]);
    expect(roleAttributes(def())).toEqual(["pin_rating_a"]);
    expect(roleAttributes(undefined)).toEqual(["pin_rating_a"]);
  });

  // ja: 短絡の印は電線の始点かピンの点、断線の印は電線の長さの半分の所に置き、消えた対象は飛ばす
  it("places short marks at the wire start or the pin and break marks halfway along the wire, skipping missing targets", () => {
    const pts = faultPoints(
      caseWith([
        { kind: "short", target: { wire_id: "w1" } },
        { kind: "short", target: { entity_id: "f1", pin: "2" } },
        { kind: "open", wire_id: "w1" },
        { kind: "open", wire_id: "gone" },
        { kind: "short", target: { entity_id: "f1", pin: "9" } },
      ]),
      sheet,
      () => def(),
    );
    expect(pts).toEqual([
      { kind: "short", at: { x: 0, y: 0 } },
      { kind: "short", at: { x: 10, y: 10 } },
      { kind: "open", at: { x: 10, y: 5 } },
    ]);
    expect(faultPoints(null, sheet, () => def())).toEqual([]);
  });
});
