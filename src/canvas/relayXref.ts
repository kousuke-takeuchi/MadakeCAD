// コイル⇔接点クロスリファレンス (M4 §4)。Rust側 madake-core/src/relay_xref.rs と同一ルール:
//   - 統合キー = 参照記号。同じ参照記号 (例 K1) のコイルと接点は1つのデバイス
//   - 接点マップ = コイルの下に置く表。1行 =「端子対 | 接点の所在 /シート.ゾーン」。
//     接点構成 (attrs.contact_config、例 "2NO+2NC") が分かるときは未使用の接点も
//     行として並べ、所在の代わりに「—」を書く
//   - コイル所在 = 接点の脇に置く「(/1.C2)」
//   - 端子対は IEC 60947-1: 先頭が接点の連番、末尾が a接点=3/4・b接点=1/2

import type { Point, Project, SymbolInstance } from "../ipc";
import { zoneAt } from "./xref";

/** 接点構成を持たせるシンボル属性のキー。部品DBのcontact_config列を配置時に写す。 */
export const CONTACT_CONFIG_ATTR = "contact_config";
/** 未使用の接点行に書く記号 (デザイン確定: 全角ダッシュ)。 */
export const CONTACT_UNUSED = "—";
/** リレーコイルのシンボルid。 */
export const COIL_SYMBOL_ID = "relay_coil";
/** a接点 (メーク接点) のシンボルid。 */
export const CONTACT_NO_SYMBOL_ID = "relay_contact_no";
/** b接点 (ブレーク接点) のシンボルid。 */
export const CONTACT_NC_SYMBOL_ID = "relay_contact_nc";

/** 接点マップの文字高さ (mm)。relay_xref.rsのCONTACT_MAP_FONTと一致させること。 */
export const CONTACT_MAP_FONT = 2.0;
/** 接点マップの行高さ (mm)。 */
export const CONTACT_MAP_ROW_H = 3.5;
/** 接点マップのセル内余白 (mm)。 */
export const CONTACT_MAP_PAD = 1.0;
/** 接点マップの列の最小幅 (mm)。 */
export const CONTACT_MAP_MIN_COL_W = 8.0;
/** シンボル外形の下端から接点マップ上端までの間隔 (mm)。 */
export const CONTACT_MAP_GAP = 2.5;
/** シンボル外形の右端からコイル所在テキストまでの間隔 (mm)。 */
export const COIL_LOCATION_GAP = 1.0;
/** 文字幅の見積り係数 (文字高さに対する1文字の平均幅)。xref.tsと共通。 */
const CHAR_WIDTH_RATIO = 0.6;

/** リレーデバイスの機能種別。 */
export type RelayRole = "coil" | "contact_no" | "contact_nc";

/** シンボルidからリレーの機能種別を判定する。リレー以外はnull。 */
export function relayRole(symbolId: string): RelayRole | null {
  switch (symbolId) {
    case COIL_SYMBOL_ID:
      return "coil";
    case CONTACT_NO_SYMBOL_ID:
      return "contact_no";
    case CONTACT_NC_SYMBOL_ID:
      return "contact_nc";
    default:
      return null;
  }
}

/** デバイスを構成する機能1つ (コイル1個・接点1個) の所在。 */
export interface RelayFunction {
  reference: string;
  role: RelayRole;
  entityId: string;
  sheetId: string;
  /** 1始まりのシート表示順。 */
  sheetNo: number;
  sheetName: string;
  /** ゾーンアドレス (例 "C2")。 */
  zone: string;
  /** 図面上の住所表記 (例 "/1.C2")。 */
  address: string;
}

/** 接点マップの1行。 */
export interface ContactMapRow {
  /** 端子対 (例 "13-14")。 */
  terminals: string;
  /** 所在「/シート.ゾーン」。未使用の接点は CONTACT_UNUSED。 */
  address: string;
  role: RelayRole;
  /** 図面に置かれている接点のentity id。未使用行はnull。 */
  entityId: string | null;
}

/** 接点構成 (部品DBの実装数)。 */
export interface ContactConfig {
  no: number;
  nc: number;
}

/** 参照記号1つ分のリレーデバイス (コイル+接点群)。 */
export interface RelayDevice {
  reference: string;
  coils: RelayFunction[];
  /** 接点 (図面の読み順: シート番号→ゾーン→id)。この並びが端子対の連番になる。 */
  contacts: RelayFunction[];
  /** 接点構成の生文字列 (未設定なら空)。 */
  contactConfigRaw: string;
  /** コイルの下に描く接点マップ。 */
  contactMap: ContactMapRow[];
  /** 接点の脇に描くコイル所在 (例 "(/1.C2)")。コイルが無ければnull。 */
  coilLocation: string | null;
}

/**
 * 接点構成の文字列を読む。例 "2NO+2NC" → a接点2・b接点2。
 * 区切りは + / , / 空白。大文字小文字は問わない。個数の無い "NO" や種別の無い "2"、
 * 未知の種別 ("2c" 等)、書きかけ ("2NO+") は読めないものとしてnullを返す (数を推測しない)。
 */
export function parseContactConfig(raw: string): ContactConfig | null {
  const text = raw.trim();
  if (!text || /^[+,]/.test(text) || /[+,]$/.test(text)) return null;
  const config: ContactConfig = { no: 0, nc: 0 };
  let terms = 0;
  for (const term of text.split(/[+,\s]+/).filter(Boolean)) {
    const m = /^(\d+)\s*([A-Za-z]+)$/.exec(term);
    if (!m) return null;
    const count = Number(m[1]);
    const kind = m[2].toUpperCase();
    if (kind === "NO") config.no += count;
    else if (kind === "NC") config.nc += count;
    else return null;
    terms += 1;
  }
  return terms > 0 ? config : null;
}

/**
 * 接点の端子対 (IEC 60947-1)。先頭の数字が接点の連番、末尾の2桁が機能を表し、
 * a接点は3/4、b接点は1/2になる。コイルは端子対ではなく固定の端子記号 A1-A2。
 */
export function terminalPair(position: number, role: RelayRole): string {
  if (role === "coil") return "A1-A2";
  return role === "contact_no" ? `${position}3-${position}4` : `${position}1-${position}2`;
}

/** デバイスの接点マップを組み立てる (使用中の接点 → 接点構成の残り)。 */
function buildContactMap(contacts: RelayFunction[], config: ContactConfig | null): ContactMapRow[] {
  const rows: ContactMapRow[] = contacts.map((c, i) => ({
    terminals: terminalPair(i + 1, c.role),
    address: c.address,
    role: c.role,
    entityId: c.entityId,
  }));
  if (!config) return rows;
  const used = (role: RelayRole) => contacts.filter((c) => c.role === role).length;
  const spare: [RelayRole, number][] = [
    ["contact_no", Math.max(0, config.no - used("contact_no"))],
    ["contact_nc", Math.max(0, config.nc - used("contact_nc"))],
  ];
  for (const [role, count] of spare) {
    for (let i = 0; i < count; i += 1) {
      rows.push({
        terminals: terminalPair(rows.length + 1, role),
        address: CONTACT_UNUSED,
        role,
        entityId: null,
      });
    }
  }
  return rows;
}

/**
 * プロジェクト内の全リレーデバイス (参照記号の昇順)。同じ参照記号のコイルと接点が
 * 1デバイスにまとまる。参照記号が空のシンボルはどのデバイスにも属さない。
 */
export function relayDevices(project: Project | null): RelayDevice[] {
  if (!project) return [];
  const byRef = new Map<string, { coils: RelayFunction[]; contacts: RelayFunction[]; raw: string }>();
  project.sheets.forEach((sheet, i) => {
    for (const entity of Object.values(sheet.entities)) {
      if (entity.kind !== "symbol") continue;
      const role = relayRole(entity.symbol_id);
      if (!role) continue;
      const reference = entity.reference.trim();
      if (!reference) continue;
      let device = byRef.get(reference);
      if (!device) {
        device = { coils: [], contacts: [], raw: "" };
        byRef.set(reference, device);
      }
      const zone = zoneAt(sheet, entity.at);
      const fn: RelayFunction = {
        reference,
        role,
        entityId: entity.id,
        sheetId: sheet.id,
        sheetNo: i + 1,
        sheetName: sheet.name,
        zone,
        address: `/${i + 1}.${zone}`,
      };
      if (role === "coil") device.coils.push(fn);
      else device.contacts.push(fn);
      // 接点構成はコイルに付いたものを優先し、無ければ接点のものを使う
      const raw = (entity.attrs?.[CONTACT_CONFIG_ATTR] ?? "").trim();
      if (raw && (!device.raw || role === "coil")) device.raw = raw;
    }
  });
  const byOrder = (a: RelayFunction, b: RelayFunction) =>
    a.sheetNo - b.sheetNo ||
    (a.zone < b.zone ? -1 : a.zone > b.zone ? 1 : 0) ||
    (a.entityId < b.entityId ? -1 : a.entityId > b.entityId ? 1 : 0);
  return [...byRef.entries()]
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([reference, d]) => {
      const coils = [...d.coils].sort(byOrder);
      const contacts = [...d.contacts].sort(byOrder);
      return {
        reference,
        coils,
        contacts,
        contactConfigRaw: d.raw,
        contactMap: buildContactMap(contacts, parseContactConfig(d.raw)),
        coilLocation: coils.length ? `(${coils[0].address})` : null,
      };
    });
}

/**
 * シート1枚分の接点マップ表 (コイルのentity id → 表の行)。キャンバス描画はこれを引く。
 * 接点を1つも持たないコイルは含まれない (描く表が無い)。
 */
export function sheetContactMaps(
  project: Project | null,
  sheetId: string,
): Map<string, ContactMapRow[]> {
  const out = new Map<string, ContactMapRow[]>();
  for (const device of relayDevices(project)) {
    if (!device.contactMap.length) continue;
    for (const coil of device.coils) {
      if (coil.sheetId === sheetId) out.set(coil.entityId, device.contactMap);
    }
  }
  return out;
}

/**
 * シート1枚分のコイル所在表 (接点のentity id → "(/1.C2)")。
 * コイルの見つからない接点は含まれない (何も描かない)。
 */
export function sheetCoilLocations(
  project: Project | null,
  sheetId: string,
): Map<string, string> {
  const out = new Map<string, string>();
  for (const device of relayDevices(project)) {
    if (!device.coilLocation) continue;
    for (const contact of device.contacts) {
      if (contact.sheetId === sheetId) out.set(contact.entityId, device.coilLocation);
    }
  }
  return out;
}

/** 配置後のシンボル外形の囲み矩形 (用紙座標mm)。 */
export interface SymbolBounds {
  min: Point;
  max: Point;
}

/** 接点マップの表の配置 (用紙座標mm)。行はY下方向に積む。 */
export interface ContactMapLayout {
  /** 表の左上X。 */
  x: number;
  /** 表の左上Y。 */
  y: number;
  /** 列幅 (端子対の列, 所在の列)。 */
  colW: [number, number];
  rowH: number;
  rows: number;
  width: number;
  height: number;
  /** i行目 (0始まり) の上端Y。 */
  rowTop(i: number): number;
  /** i行目の文字のベースラインY (行の中央に文字を置く)。 */
  baseline(i: number): number;
  /** col列目 (0または1) の左端X。 */
  colX(col: number): number;
  /** col列目の文字の左端X (セル内余白を空ける)。 */
  textX(col: number): number;
}

/**
 * 接点マップの表の配置を求める。centerXを中心に左右対称、topYから下へ伸びる。
 * 列幅は中身の文字数から見積り、CONTACT_MAP_MIN_COL_Wを下回らない。
 */
export function contactMapLayout(
  rows: ContactMapRow[],
  centerX: number,
  topY: number,
): ContactMapLayout {
  const widthOf = (pick: (r: ContactMapRow) => string) => {
    const chars = rows.reduce((max, r) => Math.max(max, [...pick(r)].length), 0);
    return Math.max(
      CONTACT_MAP_MIN_COL_W,
      CONTACT_MAP_FONT * CHAR_WIDTH_RATIO * chars + 2 * CONTACT_MAP_PAD,
    );
  };
  const colW: [number, number] = [widthOf((r) => r.terminals), widthOf((r) => r.address)];
  const width = colW[0] + colW[1];
  const x = centerX - width / 2;
  return {
    x,
    y: topY,
    colW,
    rowH: CONTACT_MAP_ROW_H,
    rows: rows.length,
    width,
    height: CONTACT_MAP_ROW_H * rows.length,
    rowTop: (i) => topY + CONTACT_MAP_ROW_H * i,
    baseline: (i) => topY + CONTACT_MAP_ROW_H * i + CONTACT_MAP_ROW_H / 2 + CONTACT_MAP_FONT * 0.35,
    colX: (col) => x + (col === 0 ? 0 : colW[0]),
    textX: (col) => x + (col === 0 ? 0 : colW[0]) + CONTACT_MAP_PAD,
  };
}

/**
 * コイルの接点マップの基準点 (表の中心X, 表の上端Y)。
 * シンボル外形の下端からCONTACT_MAP_GAPだけ空けてぶら下げる。
 */
export function contactMapOrigin(bounds: SymbolBounds, atX: number): Point {
  return { x: atX, y: bounds.max.y + CONTACT_MAP_GAP };
}

/**
 * 接点の脇に置くコイル所在テキストの基準点 (テキストは左揃え)。
 * シンボル外形の右端からCOIL_LOCATION_GAPだけ空ける。
 */
export function coilLocationAt(bounds: SymbolBounds, atY: number): Point {
  return { x: bounds.max.x + COIL_LOCATION_GAP, y: atY };
}

/** 配置インスタンスがリレー機能を持つか (キャンバス側の早期判定用)。 */
export function isRelaySymbol(inst: SymbolInstance): boolean {
  return relayRole(inst.symbol_id) !== null;
}
