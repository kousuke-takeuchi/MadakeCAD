// ピン数可変シンボル(connector_{n}p / terminal_block_{n}p)のTS側生成。
// Rust側 madake-core/src/symbol.rs の dynamic_symbol と同一座標を返すこと
// (両者は dynamicSymbol.test.ts と Rust側テストで代表値を突き合わせている)。

import type { Point, Primitive, PinDef, SymbolDef } from "../ipc";

/** 動的シンボルの最大極数(Rust側 DYNAMIC_PIN_MAX と一致)。 */
export const DYNAMIC_PIN_MAX = 50;

function parsePinCount(rest: string): number | null {
  const m = /^([0-9]+)p$/.exec(rest);
  if (!m) return null;
  const n = Number(m[1]);
  return n >= 1 && n <= DYNAMIC_PIN_MAX ? n : null;
}

const p = (x: number, y: number): Point => ({ x, y });
const offset = (i: number, n: number) => (i - (n - 1) / 2) * 5.0;

export function dynamicSymbol(id: string): SymbolDef | null {
  if (id.startsWith("connector_")) {
    const n = parsePinCount(id.slice("connector_".length));
    if (n === null) return null;
    const primitives: Primitive[] = [
      { type: "rect", p1: p(-4, offset(0, n) - 2.5), p2: p(4, offset(n - 1, n) + 2.5), filled: false },
    ];
    const pins: PinDef[] = [];
    for (let i = 0; i < n; i++) {
      const y = offset(i, n);
      primitives.push({ type: "text", at: p(-2, y), text: String(i + 1), height: 2 });
      primitives.push({ type: "line", pts: [p(4, y), p(7.5, y)] });
      pins.push({ number: String(i + 1), name: "", at: p(7.5, y) });
    }
    return {
      id,
      name: `Connector ${n}P`,
      name_ja: `コネクタ(${n}極)`,
      category: "connector",
      ref_prefix: "J",
      primitives,
      pins,
    };
  }
  if (id.startsWith("terminal_block_")) {
    const n = parsePinCount(id.slice("terminal_block_".length));
    if (n === null) return null;
    const primitives: Primitive[] = [
      { type: "rect", p1: p(-2.5, offset(0, n) - 2.5), p2: p(2.5, offset(n - 1, n) + 2.5), filled: false },
    ];
    const pins: PinDef[] = [];
    for (let i = 0; i < n; i++) {
      const y = offset(i, n);
      const no = String(i + 1);
      primitives.push({ type: "circle", center: p(0, y), r: 1.8, filled: false });
      primitives.push({ type: "text", at: p(0, y - 1), text: no, height: 2 });
      primitives.push({ type: "line", pts: [p(-2.5, y), p(-1.8, y)] });
      primitives.push({ type: "line", pts: [p(1.8, y), p(2.5, y)] });
      // 貫通端子: 左右2接続点に同一ピン番号(ネットリストで内部短絡)
      pins.push({ number: no, name: "", at: p(-2.5, y) });
      pins.push({ number: no, name: "", at: p(2.5, y) });
    }
    return {
      id,
      name: `Terminal block ${n}P`,
      name_ja: `端子台(${n}極)`,
      category: "connector",
      ref_prefix: "TB",
      primitives,
      pins,
    };
  }
  return null;
}

/** 静的ライブラリ優先でsymbol_idを解決し、無ければ動的生成にフォールバックする。 */
export function resolveSymbolDef(
  id: string,
  defs: ReadonlyMap<string, SymbolDef>,
): SymbolDef | undefined {
  return defs.get(id) ?? dynamicSymbol(id) ?? undefined;
}
