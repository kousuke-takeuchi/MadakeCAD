// FreeCAD対応付け行の表示整形 (docs/internal/specs/m5-freecad.md M5-3)。
// FreeCADアドオン側の`format_placement` (freecad-addon/tests/test_link_routes.py) と
// 同じ文面になることを両側で固定する。
import { describe, expect, it } from "vitest";

import { formatMechNumber, formatPlacement, mechLinkText } from "./mechLink";

const link = (placement?: unknown) =>
  ({
    entity_id: "k1",
    fcstd_path: "/work/panel.FCStd",
    object_name: "Relay001",
    synced_at: "2026-09-27T00:00:00Z",
    placement: placement ?? null,
  }) as never;

describe("mechLink", () => {
  // ja: 配置は「(120, 45.5, 0) mm / 90°」の形で、FreeCADアドオンのパネルと同じ文面になる。
  it("prints a placement as (x, y, z) mm / angle°, the same text as the FreeCAD panel", () => {
    expect(formatPlacement({ x_mm: 120, y_mm: 45.5, z_mm: 0, rotation_deg: 90 })).toBe(
      "(120, 45.5, 0) mm / 90°",
    );
  });

  // ja: 計算で出た端数 (45.500000000000004 のような値) は有効6桁に丸めて表示する。
  it("rounds computed fractions to six significant digits instead of showing 45.500000000000004", () => {
    expect(formatMechNumber(45.500000000000004)).toBe("45.5");
    expect(formatMechNumber(0.1 + 0.2)).toBe("0.3");
    expect(formatMechNumber(1234.5678)).toBe("1234.57");
    expect(formatMechNumber(undefined)).toBe("0");
  });

  // ja: 未同期の配置は空文字になり、「3D対応付け」行はオブジェクト名だけを出す。
  it("shows only the object name until a placement has been synced", () => {
    expect(formatPlacement(null)).toBe("");
    expect(mechLinkText(link())).toBe("Relay001");
    expect(mechLinkText(link({ x_mm: 10, y_mm: 20, z_mm: 0, rotation_deg: 0 }))).toBe(
      "Relay001 @ (10, 20, 0) mm / 0°",
    );
    expect(mechLinkText(null)).toBe("");
  });
});
