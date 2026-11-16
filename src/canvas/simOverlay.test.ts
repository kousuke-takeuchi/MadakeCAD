
import { describe, expect, it } from "vitest";
import type { EntityLoad, Sheet, SimOpResult } from "../ipc";
import { buildOverlay, formatProbe, loadLevel, overLabel } from "./simOverlay";

const load = (id: string, amps: number, limit: number | null, kind: "wire" | "component" = "wire"): EntityLoad => ({
  entity_id: id,
  kind,
  amps,
  volts: [24, 23.5],
  limit,
  limit_kind: limit === null ? null : kind === "wire" ? "wire_ampacity" : "fuse_rating",
  ratio: limit === null ? null : amps / limit,
});
const result = (loads: EntityLoad[]): SimOpResult => ({ voltage: 24, nets: [], components: [], warnings: [], findings: [], loads });
const sheetWith = (ids: string[]): Sheet =>
  ({ id: "s1", entities: Object.fromEntries(ids.map((id) => [id, { kind: "wire", id, points: [] }])) }) as unknown as Sheet;

describe("simulation overlay", () => {
  // ja: 負荷率 0.8 未満と限度の無いものは色なし、0.8〜1.0 は注意 (橙)、1.0 を超えると超過 (赤)
  it("levels: below 0.8 or no limit is none, 0.8 to 1.0 is near, above 1.0 is over", () => {
    expect(loadLevel(null)).toBe("none");
    expect(loadLevel(0.79)).toBe("none");
    expect(loadLevel(0.8)).toBe("near");
    expect(loadLevel(1.0)).toBe("near");
    expect(loadLevel(1.01)).toBe("over");
  });

  // ja: 超過の札は「電流 (小数1桁) A / 限度 A」
  it("an over label reads current with one decimal over the limit", () => {
    expect(overLabel(load("w", 70.96, 50))).toBe("71.0A / 50A");
    expect(overLabel(load("w", 31, 30))).toBe("31.0A / 30A");
  });

  // ja: 重ねる表示には注意と超過だけが入り、超過にだけ札が付く
  it("the overlay holds only near and over items, and only over items get a label", () => {
    const m = buildOverlay(result([load("a", 1, 10), load("b", 9, 10), load("c", 12, 10), load("d", 5, null)]), sheetWith(["a", "b", "c", "d"]));
    expect([...m.keys()].sort()).toEqual(["b", "c"]);
    expect(m.get("b")).toEqual({ entityId: "b", kind: "wire", level: "near", label: null });
    expect(m.get("c")?.label).toBe("12.0A / 10A");
  });

  // ja: 計算の後で消された電線・部品は重ねる表示から外れる
  it("entities deleted after the run are left out", () => {
    const m = buildOverlay(result([load("gone", 12, 10)]), sheetWith([]));
    expect(m.size).toBe(0);
  });

  // ja: 古い結果 (loads の無い結果) でも空の表示になり、落ちない
  it("a result without loads gives an empty overlay", () => {
    const r = result([]);
    delete (r as { loads?: unknown }).loads;
    expect(buildOverlay(r, sheetWith(["a"])).size).toBe(0);
  });

  // ja: プローブは電流・電圧・限度・負荷率を読める文字にし、限度が無ければ限度と負荷率は出さない
  it("the probe text shows current, voltages, limit and ratio, without limit and ratio when there is none", () => {
    expect(formatProbe(load("w", 6, 12))).toEqual({ amps: "6.00 A", volts: "24.00 V → 23.50 V", limit: "12 A", ratio: "50%" });
    expect(formatProbe(load("w", 6, null))).toEqual({ amps: "6.00 A", volts: "24.00 V → 23.50 V", limit: null, ratio: null });
    expect(formatProbe({ ...load("p", 2, 5, "component"), volts: [24, 23.9, 0] }).volts).toBe("24.00 / 23.90 / 0.00 V");
  });
});
