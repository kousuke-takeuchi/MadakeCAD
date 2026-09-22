// コイル⇔接点クロスリファレンス (M4 §4)。Rust側 madake-core/src/relay_xref.rs と同一ルール:
//   - 統合キー = 参照記号。同じ参照記号 (例 K1) のコイルと接点は1つのデバイス
//   - 接点マップ = コイルの下に置く表。1行 =「端子対 | 接点の所在 /シート.ゾーン」。
//     接点構成 (attrs.contact_config、例 "2NO+2NC") が分かるときは未使用の接点も
//     行として並べ、所在の代わりに「—」を書く
//   - コイル所在 = 接点の脇に置く「(/1.C2)」
//   - 端子対は IEC 60947-1: 先頭が接点の連番、末尾が a接点=3/4・b接点=1/2。
//     c接点 (切換接点) は共通=1・b側=2・a側=4 の3端子で「11-12-14」と書く

import type { Point, Project, SymbolInstance } from "../ipc";
import { LABEL_FONT, REF_LABEL_DY, VALUE_LABEL_DY } from "./textStyle";
import { zoneAt } from "./xref";
import type { WireNoteLabel } from "./wireNotes";

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
/** c接点 (切換接点) のシンボルid。1極でa接点とb接点を兼ねる。 */
export const CONTACT_CO_SYMBOL_ID = "relay_contact_co";

/** 接点マップの文字高さ (mm)。relay_xref.rsのCONTACT_MAP_FONTと一致させること。 */
export const CONTACT_MAP_FONT = 2.0;
/** 行の中央に文字を置くときの、行の中央高さからベースラインまでの下向きのずれ (mm)。 */
export const CONTACT_MAP_BASELINE_SHIFT = CONTACT_MAP_FONT * 0.35;
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
/** シンボル外形の上端・下端からコイル所在テキストまでの間隔 (mm、G22の上下への逃がし)。 */
export const COIL_LOCATION_VGAP = 1.0;
/** 文字幅の見積り係数 (文字高さに対する1文字の平均幅)。xref.tsと共通。 */
const CHAR_WIDTH_RATIO = 0.6;
/** 座標一致・食い込みの許容誤差 (mm)。Rust側 tidy::EPS = netlist::CONNECT_EPS と同じ。 */
const RECT_EPS = 0.01;
/** コイルを実機ブロックとして描かせるインスタンス属性のキー (G7)。値"true"で有効。 */
export const DEVICE_BLOCK_ATTR = "device_block";
/** コイルのピン間隔 (mm)。relay_coilシンボルのA1/A2ピンはこの半分(±7.5mm)にある。
 * 実機ブロックの外形はこの幅を下回らない(ピンの位置を変えずに済むように)。 */
export const DEVICE_BLOCK_MIN_WIDTH = 15.0;

/** リレーデバイスの機能種別。 */
export type RelayRole = "coil" | "contact_no" | "contact_nc" | "contact_co";

/** シンボルidからリレーの機能種別を判定する。リレー以外はnull。 */
export function relayRole(symbolId: string): RelayRole | null {
  switch (symbolId) {
    case COIL_SYMBOL_ID:
      return "coil";
    case CONTACT_NO_SYMBOL_ID:
      return "contact_no";
    case CONTACT_NC_SYMBOL_ID:
      return "contact_nc";
    case CONTACT_CO_SYMBOL_ID:
      return "contact_co";
    default:
      return null;
  }
}

/**
 * コイルが実機ブロック表示 (G7) を使うか。attrs.device_blockがちょうど"true"のときだけ有効
 * (分散配置が既定)。コイル以外のシンボルに付いていても効果は無い。
 */
export function isDeviceBlock(inst: Pick<SymbolInstance, "attrs">): boolean {
  return (inst.attrs?.[DEVICE_BLOCK_ATTR] ?? "") === "true";
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

/** 接点構成 (部品DBの実装数)。使用中の接点の数を数えるときにも使う。 */
export interface ContactConfig {
  no: number;
  nc: number;
  /** c接点 (切換接点) の実装数。 */
  co: number;
}

/**
 * 使用中の接点を接点構成へ割り当てた結果 (Rust側 allocate_contacts と同じ規則)。
 * c接点1極はa接点にもb接点にもなるので、専用極が足りない分はc接点の空き極から充当する。
 * 逆にc接点の図記号を描いたときは、c接点の極でしか賄えない。
 */
export interface ContactAllocation {
  /** まだ使っていない極数 (未使用接点の行になる)。 */
  spare: ContactConfig;
  /** 実装数では賄えなかった数 (0でなければ接点数超過)。 */
  shortage: ContactConfig;
}

/** 使用中の接点数を接点構成の極へ割り当てる。 */
export function allocateContacts(config: ContactConfig, used: ContactConfig): ContactAllocation {
  let coLeft = Math.max(0, config.co - used.co);
  const shortage: ContactConfig = { no: 0, nc: 0, co: Math.max(0, used.co - config.co) };
  const borrow = (usedCount: number, available: number) => {
    let deficit = Math.max(0, usedCount - available);
    const taken = Math.min(deficit, coLeft);
    coLeft -= taken;
    deficit -= taken;
    return deficit;
  };
  shortage.no = borrow(used.no, config.no);
  shortage.nc = borrow(used.nc, config.nc);
  return {
    spare: {
      no: Math.max(0, config.no - used.no),
      nc: Math.max(0, config.nc - used.nc),
      co: coLeft,
    },
    shortage,
  };
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
  const config: ContactConfig = { no: 0, nc: 0, co: 0 };
  let terms = 0;
  for (const term of text.split(/[+,\s]+/).filter(Boolean)) {
    const m = /^(\d+)\s*([A-Za-z]+)$/.exec(term);
    if (!m) return null;
    const count = Number(m[1]);
    const kind = m[2].toUpperCase();
    if (kind === "NO") config.no += count;
    else if (kind === "NC") config.nc += count;
    else if (kind === "CO") config.co += count;
    else return null;
    terms += 1;
  }
  return terms > 0 ? config : null;
}

/**
 * 接点の端子対 (IEC 60947-1 / IEC 60947-5-1)。先頭の数字が接点の連番、末尾の1桁が機能を
 * 表し、a接点は3/4、b接点は1/2になる。c接点は共通=1・b側=2・a側=4 の3端子を1極で持つので
 * 「11-12-14」と3つ並べて書く。コイルは端子対ではなく固定の端子記号 A1-A2。
 */
export function terminalPair(position: number, role: RelayRole): string {
  if (role === "coil") return "A1-A2";
  if (role === "contact_no") return `${position}3-${position}4`;
  if (role === "contact_co") return `${position}1-${position}2-${position}4`;
  return `${position}1-${position}2`;
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
  const count = (role: RelayRole) => contacts.filter((c) => c.role === role).length;
  const { spare: left } = allocateContacts(config, {
    no: count("contact_no"),
    nc: count("contact_nc"),
    co: count("contact_co"),
  });
  const spare: [RelayRole, number][] = [
    ["contact_no", left.no],
    ["contact_nc", left.nc],
    ["contact_co", left.co],
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
 * 接点を1つも持たないコイルは含まれない (描く表が無い)。実機ブロック表示 (G7、
 * attrs.device_block) のコイルも含まれない: 接点の並びはブロックの中に描かれるので、
 * 別表として二重に出さない。
 */
export function sheetContactMaps(
  project: Project | null,
  sheetId: string,
): Map<string, ContactMapRow[]> {
  const out = new Map<string, ContactMapRow[]>();
  const sheet = project?.sheets.find((s) => s.id === sheetId);
  for (const device of relayDevices(project)) {
    if (!device.contactMap.length) continue;
    for (const coil of device.coils) {
      if (coil.sheetId !== sheetId) continue;
      const inst = sheet?.entities[coil.entityId];
      if (inst && inst.kind === "symbol" && isDeviceBlock(inst)) continue;
      out.set(coil.entityId, device.contactMap);
    }
  }
  return out;
}

/** 実機ブロック表示 (G7) の中身: コイル+同じ参照記号を持つ接点を1つの外形にまとめて描く。 */
export interface DeviceBlock {
  reference: string;
  /** 型番 (空なら型番の行を描かない)。 */
  partNo: string;
  /** 接点の「機器イメージ」: 端子対+所在を接点マップと同じ並びで持つ。 */
  contacts: ContactMapRow[];
}

/** デバイスの実機ブロック表示の中身を組み立てる。partNoはコイルインスタンスのvalue。 */
export function buildDeviceBlock(device: RelayDevice, partNo: string): DeviceBlock {
  return { reference: device.reference, partNo: partNo.trim(), contacts: device.contactMap };
}

/**
 * シート1枚分の実機ブロック表示 (コイルのentity id → ブロックの中身)。device_block属性の
 * 付いたコイルだけが対象。キャンバス描画はこれを引く。
 */
export function sheetDeviceBlocks(
  project: Project | null,
  sheetId: string,
): Map<string, DeviceBlock> {
  const out = new Map<string, DeviceBlock>();
  const sheet = project?.sheets.find((s) => s.id === sheetId);
  if (!project || !sheet) return out;
  const devices = relayDevices(project);
  for (const entity of Object.values(sheet.entities)) {
    if (entity.kind !== "symbol" || entity.symbol_id !== COIL_SYMBOL_ID) continue;
    if (!isDeviceBlock(entity)) continue;
    const reference = entity.reference.trim();
    if (!reference) continue;
    const device = devices.find((d) => d.reference === reference);
    if (!device) continue;
    out.set(entity.id, buildDeviceBlock(device, entity.value));
  }
  return out;
}

/**
 * コイル⇔接点クロスリファレンスが所在を自動で注記するインスタンス (プロジェクト全体)。
 * sheetContactMaps・sheetDeviceBlocks・sheetCoilLocations が拾うものと同じ顔ぶれ。
 * デバイス単位のシート間クロスリファレンス (G13、xref.ts) は、この顔ぶれだけを避ける:
 * 接点を1つも持たないコイルのように、コイル⇔接点クロスリファレンスが何も描かないリレー
 * シンボルは避けない (何の所在も出ないままになってしまうので)。
 * Rust側 relay_xref::annotated_instances と同一ルール。
 */
export function annotatedInstances(project: Project | null): Set<string> {
  const out = new Set<string>();
  if (!project) return out;
  const blocks = new Set<string>();
  for (const sheet of project.sheets) {
    for (const entity of Object.values(sheet.entities)) {
      if (entity.kind === "symbol" && entity.symbol_id === COIL_SYMBOL_ID && isDeviceBlock(entity)) {
        blocks.add(entity.id);
      }
    }
  }
  for (const device of relayDevices(project)) {
    const hasRows = device.contactMap.length > 0;
    for (const coil of device.coils) {
      if (hasRows || blocks.has(coil.entityId)) out.add(coil.entityId);
    }
    if (device.coilLocation) for (const c of device.contacts) out.add(c.entityId);
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
    baseline: (i) =>
      topY + CONTACT_MAP_ROW_H * i + CONTACT_MAP_ROW_H / 2 + CONTACT_MAP_BASELINE_SHIFT,
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

/** 軸に平行な外接矩形の重なり判定 (Rust側 tidy::Rect::overlaps と同じ許容誤差)。 */
function rectsOverlap(
  a: { x0: number; y0: number; x1: number; y1: number },
  b: { x0: number; y0: number; x1: number; y1: number },
): boolean {
  return (
    a.x0 < b.x1 - RECT_EPS && b.x0 < a.x1 - RECT_EPS && a.y0 < b.y1 - RECT_EPS && b.y0 < a.y1 - RECT_EPS
  );
}

/** ベースライン位置・幅・寄せ方から、CONTACT_MAP_FONTの高さで見積もる文字の外接矩形。 */
function textRect(at: Point, width: number, height: number, align: "left" | "center") {
  const x0 = align === "left" ? at.x : at.x - width / 2;
  return { x0, y0: at.y - height, x1: x0 + width, y1: at.y };
}

/** 文字列をCONTACT_MAP_FONTで描いたときの見積り幅 (mm)。 */
function textWidth(text: string): number {
  return textWidthAt(text, CONTACT_MAP_FONT);
}

/** 文字列を文字高fontで描いたときの見積り幅 (mm)。 */
function textWidthAt(text: string, font: number): number {
  return font * CHAR_WIDTH_RATIO * [...text].length;
}

/**
 * インスタンス自身の参照記号・型番のラベルが紙面で占める矩形 (空の欄は含まない)。
 * 位置・文字高はシンボルを描くときと同じ (外形の上に中央揃えで併記する)。
 */
function instanceLabelRects(
  inst: Pick<SymbolInstance, "at" | "reference" | "value">,
  bounds: SymbolBounds,
) {
  return (
    [
      [inst.reference, REF_LABEL_DY],
      [inst.value, VALUE_LABEL_DY],
    ] as const
  )
    .filter(([text]) => !!text)
    .map(([text, dy]) =>
      textRect(
        { x: inst.at.x, y: bounds.min.y - dy },
        textWidthAt(text, LABEL_FONT),
        LABEL_FONT,
        "center",
      ),
    );
}

/**
 * 接点の脇に置くコイル所在テキストの基準点 (テキストは左揃え、G22)。
 *
 * 配線注記 (wireNotes.ts) と同じ「反対側へ逃がす」考え方で、まず右 (外形右端+
 * COIL_LOCATION_GAP、シンボル中央高さ) を試し、そこが配線注記と重なるなら上→下の順に
 * 外形の外を試す。切換接点は右2本のピンがシンボル中央高さを1グリッドピッチで挟むので、
 * 横引きの配線注記がちょうどこの右側の位置に来て重なることがある。
 *
 * textは実際に描くコイル所在の文字列 (重なり判定の幅の見積りに使う)。notesはそのシートの
 * 配線注記 (wireNoteLabels)、noteFontはそのシートで配線注記を描く文字高 (wireNoteFont、
 * 注記文字高 (G20) で伸縮する)。逃がし先はそのインスタンス自身の参照記号・型番のラベル
 * (外形の上に併記される) も避ける。全ての候補が重なっていれば、最後の逃がし先 (下) を返す。
 *
 * コイル所在そのものの文字高はCONTACT_MAP_FONT固定 (リレーのクロスリファレンスは注記文字高
 * で伸縮しない)。伸縮するのは相手の配線注記の側だけ。
 */
export function coilLocationAt(
  inst: Pick<SymbolInstance, "at" | "reference" | "value">,
  bounds: SymbolBounds,
  text: string,
  notes: readonly WireNoteLabel[],
  noteFont: number,
): Point {
  const font = CONTACT_MAP_FONT;
  const width = textWidth(text);
  const candidates: Point[] = [
    { x: bounds.max.x + COIL_LOCATION_GAP, y: inst.at.y },
    { x: bounds.min.x, y: bounds.min.y - COIL_LOCATION_VGAP },
    { x: bounds.min.x, y: bounds.max.y + COIL_LOCATION_VGAP + font },
  ];
  const occupied = [
    ...notes.map((note) =>
      textRect(note.at, textWidthAt(note.text, noteFont), noteFont, note.align),
    ),
    ...instanceLabelRects(inst, bounds),
  ];
  for (const at of candidates) {
    const probe = textRect(at, width, font, "left");
    if (!occupied.some((r) => rectsOverlap(probe, r))) return at;
  }
  return candidates[candidates.length - 1];
}

/** 配置インスタンスがリレー機能を持つか (キャンバス側の早期判定用)。 */
export function isRelaySymbol(inst: SymbolInstance): boolean {
  return relayRole(inst.symbol_id) !== null;
}

/**
 * 実機ブロック表示 (G7) の外形の配置 (**シンボルのローカル座標**mm、配置基準点が原点)。
 *
 * 表は**常に正立**で描く: インスタンスの回転・ミラーは外形にも行にも効かせない
 * (deviceBlockBoundsは配置基準点へ平行移動するだけ)。回すのはピンの位置だけで、引き出し線が
 * 回った先のピンから外形のいちばん近い点まで伸びる (deviceBlockLeadEnd)。表を回すと複数行の
 * 文字が互いに重なって読めなくなるため。ピンの高さ (ローカルY=0) はヘッダ1行目の中央に来る
 * (仕切り線が引き出し線の高さに重ならないように)。Rust側 device_block_layout と同一ルール。
 */
export interface DeviceBlockLayout {
  x: number;
  y: number;
  width: number;
  rowH: number;
  /** ヘッダ行数 (参照記号のみなら1、型番もあれば2)。 */
  headerRows: number;
  contactRows: number;
  /** 接点の列幅 (端子対の列, 所在の列)。contactMapLayoutと同じ求め方。 */
  colW: [number, number];
  height: number;
  /** i行目 (0始まり) のヘッダ行のセル (外形の全幅)。ローカル座標の (左上, 右下)。 */
  headerCell(i: number): SymbolBounds;
  /** i行目の接点行の上端Y。 */
  contactRowTop(i: number): number;
  /** i行目の接点行の、col列目 (0または1) のセル。ローカル座標の (左上, 右下)。 */
  contactCell(i: number, col: number): SymbolBounds;
  /** col列目 (0または1) の左端X。 */
  colX(col: number): number;
  /** 回していないコイルのピンから外形の左右端までの引き出し線の半分の長さ (最小幅なら0)。 */
  leadHalfLen: number;
}

/**
 * 実機ブロック表示の外形の配置をシンボルのローカル座標で求める (原点=配置基準点)。
 * 回していないコイルのピンは外形の左右端の、ちょうどピンの高さ(ローカルY=0)を通る位置に
 * つながるので、外形をどれだけ広げてもコイルの配線は繋がったまま (幅が
 * DEVICE_BLOCK_MIN_WIDTHを超えた分だけ引き出し線が伸びる)。
 */
export function deviceBlockLayout(block: DeviceBlock): DeviceBlockLayout {
  const [centerX, atY] = [0, 0];
  const headerRows = block.partNo ? 2 : 1;
  const contactLayout = contactMapLayout(block.contacts, centerX, 0);
  const headerW =
    Math.max(textWidth(block.reference), headerRows === 2 ? textWidth(block.partNo) : 0) +
    2 * CONTACT_MAP_PAD;
  // 接点が無ければcontactMapLayoutの列幅(最小幅ぶん)は外形の広さに数えない
  const contactsW = block.contacts.length ? contactLayout.width : 0;
  const width = Math.max(contactsW, headerW, DEVICE_BLOCK_MIN_WIDTH);
  const rowH = CONTACT_MAP_ROW_H;
  const x = centerX - width / 2;
  // ピンの高さはヘッダ**1行目**の中央。ヘッダ行の仕切り線がピンの高さに重ならない
  const y = atY - rowH / 2;
  const contactsX = x + (width - (contactLayout.colW[0] + contactLayout.colW[1])) / 2;
  return {
    x,
    y,
    width,
    rowH,
    headerRows,
    contactRows: block.contacts.length,
    colW: contactLayout.colW,
    height: rowH * (headerRows + block.contacts.length),
    headerCell: (i) => ({
      min: { x, y: y + rowH * i },
      max: { x: x + width, y: y + rowH * (i + 1) },
    }),
    contactRowTop: (i) => y + rowH * headerRows + rowH * i,
    contactCell(i, col) {
      const left = contactsX + (col === 0 ? 0 : contactLayout.colW[0]);
      const top = this.contactRowTop(i);
      return { min: { x: left, y: top }, max: { x: left + contactLayout.colW[col], y: top + rowH } };
    },
    colX: (col) => contactsX + (col === 0 ? 0 : contactLayout.colW[0]),
    leadHalfLen: Math.max(0, (width - DEVICE_BLOCK_MIN_WIDTH) / 2),
  };
}

/**
 * 実機ブロックの外形を用紙座標で見た囲み矩形。表は常に正立なので、配置の回転・ミラーでは
 * 向きも大きさも変わらない (配置基準点へ平行移動するだけ)。描画・クリックで掴む当たり判定・
 * 外形の下に置く機能ラベル (G4) はこれを引く。Rust側 device_block_bounds と同一。
 */
export function deviceBlockBounds(
  inst: Pick<SymbolInstance, "at">,
  block: DeviceBlock,
): SymbolBounds {
  const layout = deviceBlockLayout(block);
  return {
    min: { x: inst.at.x + layout.x, y: inst.at.y + layout.y },
    max: { x: inst.at.x + layout.x + layout.width, y: inst.at.y + layout.y + layout.height },
  };
}

/**
 * ピンのローカル座標から引き出し線の軸 (**ローカル座標**の外向き単位ベクトル) を求める。
 * シンボルのピンは配置基準点から外へ伸びているので、大きいほうの成分がピンの向き。
 * 用紙座標の軸は、これを配置変換した向き (回転0/180なら水平、90/270なら垂直)。
 * Rust側 device_block_lead_axis と同一。
 */
export function deviceBlockLeadAxis(pinAt: Point): Point {
  return Math.abs(pinAt.x) >= Math.abs(pinAt.y)
    ? { x: Math.sign(pinAt.x), y: 0 }
    : { x: 0, y: Math.sign(pinAt.y) };
}

/**
 * ピン (用紙座標、回転・ミラー済み) から実機ブロックの外形へ引く引き出し線の終点。
 *
 * 引き出し線は**ピン自身の軸**に沿って出る (axis = 配置変換後の外向き単位ベクトル、
 * deviceBlockLeadAxis)。そこに繋がる配線と同じ向きなので、回転0/180なら水平に左右の辺へ、
 * 90/270なら垂直に上下の辺へ届く。外形が内容で広がってピンが中に入ったときは、同じ軸を
 * 中心と反対向きに伸ばして縁で止める (接点の行の文字を横切らない)。ピンがちょうど縁の上に
 * ある・軸の延長が外形に当たらないときはnull。Rust側 device_block_lead_end と同一。
 */
export function deviceBlockLeadEnd(pin: Point, axis: Point, bounds: SymbolBounds): Point | null {
  const { min, max } = bounds;
  const inside = pin.x > min.x && pin.x < max.x && pin.y > min.y && pin.y < max.y;
  const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);
  let end: Point;
  if (Math.abs(axis.x) > Math.abs(axis.y)) {
    // 水平の軸: ピンの高さが外形の高さの範囲に無ければ、軸を伸ばしても外形に当たらない
    if (pin.y < min.y - 1e-9 || pin.y > max.y + 1e-9) return null;
    const x = inside ? (axis.x > 0 ? max.x : min.x) : clamp(pin.x, min.x, max.x);
    end = { x, y: pin.y };
  } else {
    if (pin.x < min.x - 1e-9 || pin.x > max.x + 1e-9) return null;
    const y = inside ? (axis.y > 0 ? max.y : min.y) : clamp(pin.y, min.y, max.y);
    end = { x: pin.x, y };
  }
  return Math.abs(end.x - pin.x) > 1e-9 || Math.abs(end.y - pin.y) > 1e-9 ? end : null;
}

/** 実機ブロックに描く文字1つ分。表は常に正立で、文字はセルの中に水平に書く。文字高はCONTACT_MAP_FONT。 */
export interface DeviceBlockText {
  text: string;
  /** ベースラインの基準点 (用紙座標)。 */
  at: Point;
  /** 寄せ方。ヘッダ (参照記号・型番) は中央、接点の行は左。 */
  align: "center" | "left";
  /** この文字を収めるセル。 */
  cell: SymbolBounds;
}

/**
 * 実機ブロックに描く文字の一覧 (用紙座標)。上からヘッダ行 → 接点の行の順。
 * キャンバス描画・SVG出力はこれをそのまま描く。Rust側 device_block_texts と同一。
 */
export function deviceBlockTexts(
  inst: Pick<SymbolInstance, "at">,
  block: DeviceBlock,
): DeviceBlockText[] {
  const layout = deviceBlockLayout(block);
  const cellAt = (c: SymbolBounds): SymbolBounds => ({
    min: { x: inst.at.x + c.min.x, y: inst.at.y + c.min.y },
    max: { x: inst.at.x + c.max.x, y: inst.at.y + c.max.y },
  });
  const baseline = (cell: SymbolBounds) =>
    (cell.min.y + cell.max.y) / 2 + CONTACT_MAP_BASELINE_SHIFT;
  const out: DeviceBlockText[] = [];
  [block.reference, block.partNo].forEach((text, i) => {
    if (i >= layout.headerRows) return;
    const cell = cellAt(layout.headerCell(i));
    out.push({
      text,
      at: { x: (cell.min.x + cell.max.x) / 2, y: baseline(cell) },
      align: "center",
      cell,
    });
  });
  block.contacts.forEach((row, i) => {
    [row.terminals, row.address].forEach((text, col) => {
      const cell = cellAt(layout.contactCell(i, col));
      out.push({
        text,
        at: { x: cell.min.x + CONTACT_MAP_PAD, y: baseline(cell) },
        align: "left",
        cell,
      });
    });
  });
  return out;
}
