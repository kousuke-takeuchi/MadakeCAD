// シート間クロスリファレンス (IEC 61082-1)。Rust側 madake-core/src/xref.rs と同一ルール:
//   - 統合キー = ネットラベル名。同名ラベルは所属シートを問わず同じネットとして扱う
//   - 相手先 = 同名ラベルの他シートでの所在。自シート内の所在は相手ではないので除外する
//   - 住所は「/シート.ゾーン」(例 /2.B3)。シート番号は1始まりの表示順
//   - ゾーンは zone_cols/zone_rows の分割をラベル座標に当てたもの (行=英字を上から、列=数字を左から)

import type { NetLabel, Point, Project, Sheet } from "../ipc";

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
 */
export function xrefTextAt(label: Pick<NetLabel, "at" | "name">): Point {
  const width = NET_LABEL_FONT * CHAR_WIDTH_RATIO * [...label.name].length;
  return { x: label.at.x + width + XREF_GAP, y: label.at.y - NET_LABEL_RISE };
}
