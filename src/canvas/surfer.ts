// 参照サーフィン (Surfer, spec §4)。図面上の要素をAlt(Option)+クリックすると、
// 「同じものが図面のどこに出てくるか」の一覧をポップアップで出し、↑↓+Enterで巡回する。
//
// 対象は3つ:
//  - **参照記号のあるシンボル** → 同じ参照記号のデバイスの全機能 (コイル・接点・端子・本体)
//  - **ネットラベル** → 同名ラベルの全所在 (自シートも含む。相手先だけを出すXRefとは違う)
//  - **線番のついたワイヤ** → 同じ線番の全ワイヤ
//
// ジャンプは検索・デバイスナビゲータと同じreveal機構 (シート切替+選択+ズーム)。
// 文言はi18nカタログが持ち、ここが返すのは種別と所在だけ。

import type { DeviceFunctionKind, DeviceNode, Project } from "../ipc";
import { zoneAt } from "./xref";

/** 巡回先1件。 */
export interface SurferSite {
  /** デバイスの機能 (ネット・線番の所在ではnull)。 */
  function: DeviceFunctionKind | null;
  /** 機能の端子の呼び名 (例 "13-14")。無ければ空。 */
  terminals: string;
  /** 図面上の住所 (例 "/2.B3")。 */
  address: string;
  sheetId: string;
  sheetNo: number;
  sheetName: string;
  zone: string;
  entityId: string;
}

/** ポップアップ1つ分 (見出し+巡回先)。 */
export interface SurferTarget {
  /** 何をサーフィンしているか (見出しの文言の選択に使う)。 */
  kind: "device" | "net" | "wire_no";
  /** 見出しに出す名前 (参照記号・ネット名・線番)。 */
  title: string;
  sites: SurferSite[];
}

function address(sheetNo: number, zone: string): string {
  return `/${sheetNo}.${zone}`;
}

/**
 * Alt+クリックされたエンティティのサーフィン対象。
 *
 * 巡回先が1件も無い (参照記号も線番もネット名も付いていない) 要素ではnullを返し、
 * 呼び出し側はポップアップを出さない。
 */
export function surferTargetFor(
  project: Project | null,
  devices: DeviceNode[],
  sheetId: string,
  entityId: string,
): SurferTarget | null {
  if (!project) return null;
  const sheet = project.sheets.find((s) => s.id === sheetId);
  const entity = sheet?.entities[entityId];
  if (!entity) return null;

  if (entity.kind === "symbol") {
    const reference = entity.reference.trim();
    if (!reference) return null;
    const device = devices.find((d) => d.reference === reference);
    if (!device || device.functions.length === 0) return null;
    return {
      kind: "device",
      title: reference,
      sites: device.functions.map((f) => ({
        function: f.kind,
        terminals: f.terminals,
        address: address(f.sheet_no, f.zone),
        sheetId: f.sheet_id,
        sheetNo: f.sheet_no,
        sheetName: f.sheet_name,
        zone: f.zone,
        entityId: f.entity_id,
      })),
    };
  }

  if (entity.kind === "net_label") {
    const name = entity.name.trim();
    if (!name) return null;
    return {
      kind: "net",
      title: name,
      sites: collect(project, (e) => (e.kind === "net_label" && e.name.trim() === name ? e.at : null)),
    };
  }

  if (entity.kind === "wire") {
    const no = (entity.net ?? "").trim();
    if (!no) return null;
    return {
      kind: "wire_no",
      title: no,
      sites: collect(project, (e) =>
        e.kind === "wire" && (e.net ?? "").trim() === no ? wireAnchor(e.points) : null,
      ),
    };
  }
  return null;
}

/** ワイヤの代表点 (最も上、同じ高さなら最も左)。Rust側の線番の採番と同じ基準。 */
function wireAnchor(points: { x: number; y: number }[]): { x: number; y: number } {
  return points.reduce(
    (best, p) => (p.y < best.y || (p.y === best.y && p.x < best.x) ? p : best),
    points[0] ?? { x: 0, y: 0 },
  );
}

/**
 * プロジェクト全体から条件に合うエンティティの所在を集める。
 * 並びはシート順→ゾーン→entity idで決定的 (Rust側のNetSiteと同じ規則)。
 */
function collect(
  project: Project,
  at: (entity: Project["sheets"][number]["entities"][string]) => { x: number; y: number } | null,
): SurferSite[] {
  const sites: SurferSite[] = [];
  project.sheets.forEach((sheet, i) => {
    for (const entity of Object.values(sheet.entities)) {
      const point = at(entity);
      if (!point) continue;
      const zone = zoneAt(sheet, point);
      sites.push({
        function: null,
        terminals: "",
        address: address(i + 1, zone),
        sheetId: sheet.id,
        sheetNo: i + 1,
        sheetName: sheet.name,
        zone,
        entityId: entity.id,
      });
    }
  });
  sites.sort(
    (a, b) =>
      a.sheetNo - b.sheetNo ||
      (a.zone < b.zone ? -1 : a.zone > b.zone ? 1 : 0) ||
      (a.entityId < b.entityId ? -1 : a.entityId > b.entityId ? 1 : 0),
  );
  return sites;
}

/** 巡回先のジャンプ先 (シート切替+選択+ズーム)。検索・ナビゲータと同じ機構。 */
export function surferJumpTarget(site: SurferSite): { sheetId: string; entityIds: string[] } {
  return { sheetId: site.sheetId, entityIds: [site.entityId] };
}
