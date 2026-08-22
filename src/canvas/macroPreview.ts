// 回路マクロのミニ描画と配置ジオメトリ (挿入ダイアログのタイル/プレビュー、
// キャンバスの配置ゴースト)。
//
// マクロは「挿入したら図面がこうなる」というCommand列なので、プレビューは開始
// テンプレートと同じくコマンド列から仮のシートを組み立てて通常のレンダラで描く。
// 座標は基準点 (`base_point`) からの相対で入っているため、ゴーストも確定も
// 「回転 → 挿入位置へ平行移動」の同じ変換を通る (Rust macros::place_entity と同一規則)。

import type { Command, Entity, Macro, MacroValueSet, Point, Sheet, SymbolDef } from "../ipc";
import { drawHarness, drawSymbol } from "./renderer";
import { theme, wireColorScreen } from "./theme";
import type { Viewport } from "./viewport";

/** 既定バリアントのキー。`variants`に無くても必ず選べる (本体の`commands`)。 */
export const DEFAULT_VARIANT = "A";

/** 使えるバリアントキーを並び順で返す (既定の"A" + マクロが持つバリアント)。Tabはこの順に巡回する。 */
export function macroVariantKeys(m: Macro): string[] {
  const keys = [DEFAULT_VARIANT];
  for (const v of m.variants) {
    if (!keys.some((k) => k.toUpperCase() === v.key.toUpperCase())) keys.push(v.key);
  }
  return keys;
}

/** バリアントの表示名「キー 名前」(名前が無ければキーだけ)。 */
export function macroVariantLabel(m: Macro, key: string, locale: string): string {
  const v = m.variants.find((x) => x.key.toUpperCase() === key.toUpperCase());
  const name = v ? (locale === "ja" && v.name_ja ? v.name_ja : v.name) : "";
  return name ? `${key} ${name}` : key;
}

/** UI言語に合わせた値セットの表示名 (日本語名が無ければ英語名、どちらも無ければid)。 */
export function macroValueSetLabel(v: MacroValueSet, locale: string): string {
  const label = locale === "ja" && v.label_ja ? v.label_ja : v.label;
  return label || v.id;
}

/** 指定バリアントのCommand列。"A"・未指定・知らないキーは既定の`commands`。 */
export function macroCommands(m: Macro, key?: string | null): Command[] {
  if (!key) return m.commands;
  const v = m.variants.find((x) => x.key.toUpperCase() === key.toUpperCase());
  return v ? v.commands : m.commands;
}

/** 指定バリアントが置くエンティティ (基準点からの相対座標のまま)。 */
export function macroEntities(m: Macro, key?: string | null): Entity[] {
  return macroCommands(m, key)
    .filter((c) => c.type === "add_entity")
    .map((c) => c.entity);
}

/** UI言語に合わせたマクロ名 (日本語名が無ければ英語名)。 */
export function macroName(m: Macro, locale: string): string {
  return locale === "ja" && m.name_ja ? m.name_ja : m.name;
}

/** UI言語に合わせたマクロの説明。 */
export function macroDescription(m: Macro, locale: string): string {
  return locale === "ja" && m.description_ja ? m.description_ja : m.description;
}

/**
 * 基準点からの相対座標を、回転(0/90/180/270)してから挿入位置へ移す。
 * 用紙座標系はY下向きなので、90度は (x,y) → (-y,x)。
 */
export function placePoint(p: Point, rotation: number, at: Point): Point {
  let rx = p.x;
  let ry = p.y;
  switch (((rotation % 360) + 360) % 360) {
    case 90:
      rx = -p.y;
      ry = p.x;
      break;
    case 180:
      rx = -p.x;
      ry = -p.y;
      break;
    case 270:
      rx = p.y;
      ry = -p.x;
      break;
  }
  return { x: at.x + rx, y: at.y + ry };
}

/**
 * マクロのエンティティ群を挿入位置・回転で配置した**複製**を返す (ゴースト描画用)。
 * 元の定義は書き換えないので、同じマクロを何度でも同じ形で置ける。
 */
export function placeMacroEntities(entities: Entity[], at: Point, rotation: number): Entity[] {
  const map = (p: Point) => placePoint(p, rotation, at);
  return entities.map((e): Entity => {
    switch (e.kind) {
      case "symbol":
        return { ...e, at: map(e.at), rotation: (e.rotation + rotation) % 360 };
      case "wire":
      case "harness":
        return { ...e, points: e.points.map(map) };
      case "net_label":
      case "text":
        return { ...e, at: map(e.at), rotation: (e.rotation + rotation) % 360 };
      case "junction":
        return { ...e, at: map(e.at) };
    }
  });
}

/** プレビュー用の仮シート (実際の図面には影響しない)。 */
export function macroSheet(m: Macro, key?: string | null): Sheet {
  const entities: Record<string, Entity> = {};
  for (const e of macroEntities(m, key)) entities[e.id] = e;
  return {
    id: `macro-preview-${m.id}-${key ?? DEFAULT_VARIANT}`,
    name: m.id,
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities,
  };
}

/**
 * 配置ゴースト: 確定後と同じ形の回路を半透明で描く。
 * 参照記号は挿入時にRust側で振り直されるので、ゴーストでは注記を描かない。
 */
export function drawMacroGhost(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  entities: Entity[],
  resolveSymbol: (id: string) => SymbolDef | undefined,
): void {
  ctx.globalAlpha = 0.6;
  for (const e of entities) {
    switch (e.kind) {
      case "symbol": {
        const def = resolveSymbol(e.symbol_id);
        if (def) drawSymbol(ctx, vp, e, def, false, theme.selection, false);
        break;
      }
      case "wire": {
        ctx.strokeStyle = wireColorScreen(e.color);
        ctx.lineWidth = Math.max(1.2, 0.35 * vp.scale);
        ctx.beginPath();
        e.points.forEach((p, i) => {
          const s = vp.toScreen(p);
          if (i === 0) ctx.moveTo(s.x, s.y);
          else ctx.lineTo(s.x, s.y);
        });
        ctx.stroke();
        break;
      }
      case "harness":
        drawHarness(ctx, vp, e.points, "", false);
        break;
      case "junction": {
        const s = vp.toScreen(e.at);
        ctx.fillStyle = theme.selection;
        ctx.beginPath();
        ctx.arc(s.x, s.y, Math.max(2, 0.8 * vp.scale), 0, Math.PI * 2);
        ctx.fill();
        break;
      }
      case "net_label":
      case "text": {
        const s = vp.toScreen(e.at);
        ctx.fillStyle = theme.selection;
        ctx.font = `${Math.max(9, 2.5 * vp.scale)}px monospace`;
        ctx.textAlign = "left";
        ctx.fillText(e.kind === "text" ? e.text : e.name, s.x, s.y);
        break;
      }
    }
  }
  ctx.globalAlpha = 1;
}
