// ピン数可変シンボル(connector_{n}p / terminal_block_{n}p / plc_di_{n}p / plc_do_{n}p)のTS側生成。
// Rust側 madake-core/src/symbol.rs の dynamic_symbol と同一座標を返すこと
// (両者は dynamicSymbol.test.ts と Rust側テストで代表値を突き合わせている)。

import type { Point, Primitive, PinDef, SymbolDef } from "../ipc";

/** 動的シンボルの最大極数(Rust側 DYNAMIC_PIN_MAX と一致)。 */
export const DYNAMIC_PIN_MAX = 50;
/** PLCモジュールの最大点数(Rust側 PLC_POINT_MAX と一致)。 */
export const PLC_POINT_MAX = 64;
/** PLCモジュールのI/O点の縦ピッチ mm(Rust側 PLC_POINT_PITCH_MM と一致)。 */
export const PLC_POINT_PITCH_MM = 5.0;

function parseCount(rest: string, max: number): number | null {
  const m = /^([0-9]+)p$/.exec(rest);
  if (!m) return null;
  const n = Number(m[1]);
  return n >= 1 && n <= max ? n : null;
}

function parsePinCount(rest: string): number | null {
  return parseCount(rest, DYNAMIC_PIN_MAX);
}

/**
 * `plc_di_{n}p` / `plc_do_{n}p` からPLC I/Oモジュールのシンボルを作る
 * (Rust側 `plc_module_symbol` と同一の座標)。縦長の箱の左側に点数ぶんの
 * 接続点を5mmピッチで並べ、箱の中に点番号と種別 (DI/DO) を書く。
 */
function plcModuleSymbol(id: string): SymbolDef | null {
  const input = id.startsWith("plc_di_");
  const output = id.startsWith("plc_do_");
  if (!input && !output) return null;
  const n = parseCount(id.slice("plc_di_".length), PLC_POINT_MAX);
  if (n === null) return null;
  const pitch = PLC_POINT_PITCH_MM;
  const at = (i: number) => (i - (n - 1) / 2) * pitch;
  const top = at(0) - pitch;
  const bottom = at(n - 1) + pitch;
  const primitives: Primitive[] = [
    { type: "rect", p1: p(-5, top), p2: p(5, bottom), filled: false },
    { type: "text", at: p(0, top + 3.5), text: input ? "DI" : "DO", height: 3 },
  ];
  const pins: PinDef[] = [];
  for (let i = 0; i < n; i++) {
    const y = at(i);
    primitives.push({ type: "text", at: p(-2.5, y + 1), text: String(i + 1), height: 2 });
    primitives.push({ type: "line", pts: [p(-7.5, y), p(-5, y)] });
    pins.push({ number: String(i + 1), name: "", at: p(-7.5, y), dir: "left" });
  }
  return {
    id,
    name: input ? `PLC input module ${n} points` : `PLC output module ${n} points`,
    name_ja: input ? `PLC入力モジュール(${n}点)` : `PLC出力モジュール(${n}点)`,
    category: "plc",
    ref_prefix: "PLC",
    primitives,
    pins,
  };
}

const p = (x: number, y: number): Point => ({ x, y });
const offset = (i: number, n: number) => (i - (n - 1) / 2) * 5.0;

export function dynamicSymbol(id: string): SymbolDef | null {
  if (id.startsWith("plc_di_") || id.startsWith("plc_do_")) return plcModuleSymbol(id);
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
      pins.push({ number: String(i + 1), name: "", at: p(7.5, y), dir: "right" });
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
      pins.push({ number: no, name: "", at: p(-2.5, y), dir: "left" });
      pins.push({ number: no, name: "", at: p(2.5, y), dir: "right" });
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
