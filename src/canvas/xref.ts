// シート間クロスリファレンス (IEC 61082-1)。Rust側 madake-core/src/xref.rs と同一ルール:
//   - 統合キー = ネットラベル名。同名ラベルは所属シートを問わず同じネットとして扱う
//   - 相手先 = 同名ラベルの他シートでの所在。自シート内の所在は相手ではないので除外する
//   - 住所は「/シート.ゾーン」(例 /2.B3)。シート番号は1始まりの表示順
//   - ゾーンは zone_cols/zone_rows の分割をラベル座標に当てたもの (行=英字を上から、列=数字を左から)

import type { NetLabel, Point, Project, Sheet, SymbolInstance } from "../ipc";
import { annotatedInstances } from "./relayXref";
import { LABEL_FONT, REF_LABEL_DY } from "./textStyle";

/** XRefテキストの文字高さ (mm)。xref.rsのXREF_FONTと一致させること。 */
export const XREF_FONT = 2.0;
/** ネットラベル本文の文字高さ (mm)。xref.rsのNET_LABEL_FONTと一致させること。 */
export const NET_LABEL_FONT = 2.5;
/** ネットラベル本文とXRefテキストの間隔 (mm)。 */
export const XREF_GAP = 1.0;
/** ネットラベル本文のベースラインを座標から持ち上げる量 (mm)。 */
export const NET_LABEL_RISE = 1.0;
/** 文字幅の見積り係数 (文字高さに対する1文字の平均幅)。 */
const CHAR_WIDTH_RATIO = 0.6;
/** 複数の相手先を1行に並べるときの区切り。 */
export const XREF_SEPARATOR = " ";
/** 図枠の用紙端からのマージン (mm)。svg.rsのFRAME_MARGINと一致させること。 */
const FRAME_MARGIN = 10;

/** ネットラベルが置かれている場所 (どのシートのどのゾーンか)。 */
export interface NetSite {
  sheet_id: string;
  /** 1始まりのシート表示順。 */
  sheet_no: number;
  sheet_name: string;
  /** ゾーンアドレス (例 "B3")。 */
  zone: string;
  /** この所在を作っているネットラベルのentity id。 */
  label_id: string;
  name: string;
  /** 図面上の住所表記 (例 "/2.B3")。 */
  address: string;
}

function paperMm(sheet: Sheet): { w: number; h: number } {
  const dims: Record<string, [number, number]> = {
    A4: [297, 210],
    A3: [420, 297],
    A2: [594, 420],
    A1: [841, 594],
    A0: [1189, 841],
  };
  const [w, h] = dims[sheet.size] ?? [420, 297];
  return sheet.orientation === "Portrait" ? { w: h, h: w } : { w, h };
}

/**
 * 用紙座標のゾーンアドレス (例 "B3")。行=英字を上から、列=数字を左から数える。
 * 図枠の外の点は最も近いゾーンに丸める。
 */
export function zoneAt(sheet: Sheet, p: Point): string {
  const { w, h } = paperMm(sheet);
  const cols = Math.max(1, sheet.zone_cols);
  const rows = Math.max(1, sheet.zone_rows);
  const zw = (w - 2 * FRAME_MARGIN) / cols;
  const zh = (h - 2 * FRAME_MARGIN) / rows;
  const col = Math.min(cols - 1, Math.max(0, Math.floor((p.x - FRAME_MARGIN) / zw)));
  const row = Math.min(rows - 1, Math.max(0, Math.floor((p.y - FRAME_MARGIN) / zh)));
  return `${String.fromCharCode(65 + (row % 26))}${col + 1}`;
}

/**
 * プロジェクト内の全ネットラベルの所在を、ラベル名ごとにまとめて返す。
 * 並びはシート順→ゾーン順→ラベルid順で決定的。
 */
export function labelSites(project: Project): Map<string, NetSite[]> {
  const out = new Map<string, NetSite[]>();
  project.sheets.forEach((sheet, i) => {
    for (const entity of Object.values(sheet.entities)) {
      if (entity.kind !== "net_label") continue;
      const name = entity.name.trim();
      if (!name) continue;
      const zone = zoneAt(sheet, entity.at);
      const site: NetSite = {
        sheet_id: sheet.id,
        sheet_no: i + 1,
        sheet_name: sheet.name,
        zone,
        label_id: entity.id,
        name,
        address: `/${i + 1}.${zone}`,
      };
      const list = out.get(name);
      if (list) list.push(site);
      else out.set(name, [site]);
    }
  });
  for (const sites of out.values()) {
    sites.sort(
      (a, b) =>
        a.sheet_no - b.sheet_no ||
        (a.zone < b.zone ? -1 : a.zone > b.zone ? 1 : 0) ||
        (a.label_id < b.label_id ? -1 : a.label_id > b.label_id ? 1 : 0),
    );
  }
  return out;
}

/**
 * あるネットラベルの相手先 (同名ラベルが置かれている他シートの所在)。
 * 自分のシート内の所在は相手ではないので除外する。相手が無ければ空配列。
 */
export function xrefSites(project: Project, sheetId: string, labelName: string): NetSite[] {
  const name = labelName.trim();
  if (!name) return [];
  return (labelSites(project).get(name) ?? []).filter((s) => s.sheet_id !== sheetId);
}

/** 相手先の住所一覧 (例 ["/2.B3", "/3.A1"])。同じ住所は1つにまとめる。 */
export function xrefAddresses(project: Project, sheetId: string, labelName: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const site of xrefSites(project, sheetId, labelName)) {
    if (seen.has(site.address)) continue;
    seen.add(site.address);
    out.push(site.address);
  }
  return out;
}

/** ラベル脇に描くXRefテキスト (例 "/2.B3 /3.A1")。相手がいなければnull (=何も描かない)。 */
export function xrefText(project: Project, sheetId: string, labelName: string): string | null {
  const addresses = xrefAddresses(project, sheetId, labelName);
  return addresses.length ? addresses.join(XREF_SEPARATOR) : null;
}

/**
 * シート1枚分のXRefテキスト表 (ネットラベルのid → 表示テキスト)。
 * 相手のいないラベルは含まれない。キャンバス描画はこれを引く。
 */
export function sheetXrefs(project: Project | null, sheetId: string): Map<string, string> {
  const out = new Map<string, string>();
  const sheet = project?.sheets.find((s) => s.id === sheetId);
  if (!project || !sheet) return out;
  const sites = labelSites(project);
  for (const entity of Object.values(sheet.entities)) {
    if (entity.kind !== "net_label") continue;
    const name = entity.name.trim();
    if (!name) continue;
    const seen = new Set<string>();
    const addresses = (sites.get(name) ?? [])
      .filter((s) => s.sheet_id !== sheetId)
      .map((s) => s.address)
      .filter((a) => (seen.has(a) ? false : (seen.add(a), true)));
    if (addresses.length) out.set(entity.id, addresses.join(XREF_SEPARATOR));
  }
  return out;
}

/** デバイス (参照記号) が置かれている場所 (どのシートのどのゾーンか、G13)。 */
export interface DeviceSite {
  sheet_id: string;
  sheet_no: number;
  sheet_name: string;
  zone: string;
  entity_id: string;
  reference: string;
  address: string;
}

/**
 * プロジェクト内の全シンボルインスタンスの所在を、参照記号ごとにまとめて返す (G13)。
 * 参照記号が空のインスタンスは含まない。コイル⇔接点クロスリファレンス (relayXref.ts の
 * annotatedInstances) が既に自動で所在を注記するインスタンス**だけ**を対象から外す
 * (二重に描かないため)。接点を1つも持たないコイルのように、コイル⇔接点クロスリファレンスが
 * 何も描かないリレーシンボルはここで拾う。
 *
 * **整えバリアントの複製シート (`Sheet.variant_of`) は数えない。** 数えると元シートの全
 * シンボルが「複数シートに跨る参照記号」になり、比較しているあいだだけ元シートにも各案にも
 * 存在しない住所注記 (`/2.B3`) が生えてしまう (madake-coreの`xref::device_sites`と同じ扱い)。
 * シート番号は複製を飛ばしても変わらない (複製は末尾に並ぶ)。
 */
export function deviceSites(project: Project): Map<string, DeviceSite[]> {
  const out = new Map<string, DeviceSite[]>();
  const annotated = annotatedInstances(project);
  project.sheets.forEach((sheet, i) => {
    if (sheet.variant_of) return;
    for (const entity of Object.values(sheet.entities)) {
      if (entity.kind !== "symbol") continue;
      if (annotated.has(entity.id)) continue;
      const reference = entity.reference.trim();
      if (!reference) continue;
      const zone = zoneAt(sheet, entity.at);
      const site: DeviceSite = {
        sheet_id: sheet.id,
        sheet_no: i + 1,
        sheet_name: sheet.name,
        zone,
        entity_id: entity.id,
        reference,
        address: `/${i + 1}.${zone}`,
      };
      const list = out.get(reference);
      if (list) list.push(site);
      else out.set(reference, [site]);
    }
  });
  for (const sites of out.values()) {
    sites.sort(
      (a, b) =>
        a.sheet_no - b.sheet_no ||
        (a.zone < b.zone ? -1 : a.zone > b.zone ? 1 : 0) ||
        (a.entity_id < b.entity_id ? -1 : a.entity_id > b.entity_id ? 1 : 0),
    );
  }
  return out;
}

/** あるデバイスの相手先 (同じ参照記号を持つインスタンスが置かれている他シートの所在)。 */
export function deviceXrefSites(project: Project, sheetId: string, reference: string): DeviceSite[] {
  const name = reference.trim();
  if (!name) return [];
  // 案の複製 (Sheet.variant_of) では元シートを「自分の所在」として扱う
  const home = project.sheets.find((s) => s.id === sheetId)?.variant_of ?? sheetId;
  return (deviceSites(project).get(name) ?? []).filter((s) => s.sheet_id !== home);
}

/** 相手先の住所一覧 (例 ["/2.B3", "/3.A1"])。同じ住所は1つにまとめる。 */
export function deviceXrefAddresses(project: Project, sheetId: string, reference: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const site of deviceXrefSites(project, sheetId, reference)) {
    if (seen.has(site.address)) continue;
    seen.add(site.address);
    out.push(site.address);
  }
  return out;
}

/**
 * シート1枚分のデバイス単位XRefテキスト表 (シンボルのentity id → 表示テキスト、G13)。
 * 複数シートに跨る参照記号を持つインスタンスだけが入る。コイル⇔接点クロスリファレンスが
 * 所在を注記するコイル・接点は含まない (annotatedInstances)。
 */
export function sheetDeviceXrefs(project: Project | null, sheetId: string): Map<string, string> {
  const out = new Map<string, string>();
  const sheet = project?.sheets.find((s) => s.id === sheetId);
  if (!project || !sheet) return out;
  const sites = deviceSites(project);
  const annotated = annotatedInstances(project);
  // 案の複製は元シートの写しなので、元シートの所在を「相手先」として数えない
  // (数えると案を開いた瞬間に全シンボルへ住所注記が生える)
  const home = sheet.variant_of ?? sheetId;
  for (const entity of Object.values(sheet.entities)) {
    if (entity.kind !== "symbol") continue;
    if (annotated.has(entity.id)) continue;
    const reference = entity.reference.trim();
    if (!reference) continue;
    const seen = new Set<string>();
    const addresses = (sites.get(reference) ?? [])
      .filter((s) => s.sheet_id !== home)
      .map((s) => s.address)
      .filter((a) => (seen.has(a) ? false : (seen.add(a), true)));
    if (addresses.length) out.set(entity.id, addresses.join(XREF_SEPARATOR));
  }
  return out;
}

/**
 * 参照記号の脇に置くデバイスXRefテキストの基準点 (テキストは左揃え、G13)。
 * シンボル外形と**参照記号ラベル**の右端 (長いほう) からXREF_GAPだけ空け、高さは参照記号の
 * ラベルと同じ行に揃える (ネットラベル側のxrefTextAtがラベル本文の幅を測って逃がすのと同じ)。
 * boundsはシンボル外形の囲み矩形 (renderer.tsのsymbolBounds)、topYはその上端Y。
 */
export function deviceXrefAt(
  inst: Pick<SymbolInstance, "at" | "reference">,
  bounds: { min: Point; max: Point },
  topY: number,
): Point {
  // 参照記号は配置基準点を中心に描かれるので、右端は中心+幅の半分
  const labelRight = inst.at.x + (LABEL_FONT * CHAR_WIDTH_RATIO * [...inst.reference].length) / 2;
  return { x: Math.max(bounds.max.x, labelRight) + XREF_GAP, y: topY - REF_LABEL_DY };
}

/**
 * 相手先をクリックしたときのジャンプ先。プロパティパネルはこのシートへ切り替えてから
 * `controller.reveal(entityIds)` を呼ぶ (検証結果パネルの行クリックと同じ機構)。
 */
export function xrefJumpTarget(site: NetSite): { sheetId: string; entityIds: string[] } {
  return { sheetId: site.sheet_id, entityIds: [site.label_id] };
}

/**
 * ネットラベル本文の右脇に置くXRefテキストの基準点 (テキストは左揃え)。
 * ラベル本文の幅を文字数から見積り、その右に XREF_GAP だけ空ける。
 *
 * `font` はネットラベル本文の文字高さ (mm)。シートの注記文字高 (G20) で変わるので、
 * 逃がし量とベースラインの持ち上げも同じ比率で伸縮する。
 */
export function xrefTextAt(
  label: Pick<NetLabel, "at" | "name">,
  font: number = NET_LABEL_FONT,
): Point {
  const scale = font / NET_LABEL_FONT;
  const width = font * CHAR_WIDTH_RATIO * [...label.name].length;
  return { x: label.at.x + width + XREF_GAP * scale, y: label.at.y - NET_LABEL_RISE * scale };
}
