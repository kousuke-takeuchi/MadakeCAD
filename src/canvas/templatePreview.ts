// 開始テンプレートのミニ描画 (テンプレート選択ダイアログのサムネイル・大プレビュー)。
//
// テンプレートは「適用したら図面がこうなる」というCommand列なので、プレビューは
// そのコマンド列から仮のシートを組み立て、通常の図面レンダラ (renderSheet) で描く。
// 専用の描画コードを持たないので、記法の変更が自動でプレビューにも効く。

import type { Command, Entity, Sheet, Template } from "../ipc";
import { Viewport } from "./viewport";

/** テンプレートのCommand列から、そのテンプレートが置くエンティティを取り出す。 */
export function templateEntities(commands: Command[]): Entity[] {
  return commands.filter((c) => c.type === "add_entity").map((c) => c.entity);
}

/** プレビュー用の仮シート (実際の図面には影響しない。idは固定でよい)。 */
export function templateSheet(template: Template): Sheet {
  const entities: Record<string, Entity> = {};
  for (const e of templateEntities(template.commands)) entities[e.id] = e;
  return {
    id: `preview-${template.id}`,
    name: template.id,
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities,
  };
}

/** 用紙mmの矩形。 */
export interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

/** エンティティ全体を囲む矩形 (mm)。空なら null。 */
export function entityBounds(entities: Entity[]): Bounds | null {
  let bounds: Bounds | null = null;
  const consider = (x: number, y: number) => {
    if (!bounds) bounds = { minX: x, minY: y, maxX: x, maxY: y };
    else {
      bounds.minX = Math.min(bounds.minX, x);
      bounds.minY = Math.min(bounds.minY, y);
      bounds.maxX = Math.max(bounds.maxX, x);
      bounds.maxY = Math.max(bounds.maxY, y);
    }
  };
  for (const e of entities) {
    if (e.kind === "wire" || e.kind === "harness") e.points.forEach((p) => consider(p.x, p.y));
    else consider(e.at.x, e.at.y);
  }
  return bounds;
}

/**
 * 描画範囲がキャンバス (px) に収まるビューポートを作る。
 * シンボルは基準点しか数えないので、はみ出さないよう周囲に余白 (mm) を足す。
 */
export function fitPreview(
  entities: Entity[],
  canvasW: number,
  canvasH: number,
  marginMm = 10,
): Viewport {
  const vp = new Viewport();
  const bounds = entityBounds(entities);
  if (!bounds) return vp;
  const w = bounds.maxX - bounds.minX + marginMm * 2;
  const h = bounds.maxY - bounds.minY + marginMm * 2;
  vp.scale = Math.min(canvasW / Math.max(w, 1), canvasH / Math.max(h, 1));
  vp.originX = canvasW / 2 - ((bounds.minX + bounds.maxX) / 2) * vp.scale;
  vp.originY = canvasH / 2 - ((bounds.minY + bounds.maxY) / 2) * vp.scale;
  return vp;
}
