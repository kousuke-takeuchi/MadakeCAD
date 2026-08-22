// シートの種別 (回路図 / 2D盤レイアウト、M4仕様 §8) の扱い。
//
// 種別は`Sheet.kind`が正で、種別を書いていない古いシートは回路図として開く
// (Rust側 `madake_core::model::Sheet::is_panel` と同じ判断)。
// シートの追加も必ず`add_sheet`コマンド経由 (CLAUDE.md「アーキテクチャの絶対原則」)。

import type { Command, Project, Sheet, SheetKind } from "../ipc";

/** 新しいシートの名前の接頭辞 (種別ごとに数える)。 */
const NAME_PREFIX: Record<SheetKind, string> = {
  schematic: "Sheet",
  panel: "Panel",
};

/** 盤レイアウトシートか。種別を持たない古いシートは回路図として扱う。 */
export function isPanelSheet(sheet: Sheet | null | undefined): boolean {
  return sheet?.kind === "panel";
}

/** シートの種別 (既定=回路図)。 */
export function sheetKind(sheet: Sheet | null | undefined): SheetKind {
  return isPanelSheet(sheet) ? "panel" : "schematic";
}

/** 図面タブ行に出す「シート種別」の文言のi18nキー。 */
export function sheetKindLabelKey(sheet: Sheet | null | undefined): string {
  return `sheets.kind.${sheetKind(sheet)}`;
}

/**
 * 新しいシートの名前。種別ごとに数える (回路図が3枚あっても最初の盤は`Panel1`)。
 * 同じ名前のシートが残っているときは空いている番号まで進める。
 */
export function nextSheetName(project: Project | null, kind: SheetKind): string {
  const sheets = project?.sheets ?? [];
  const prefix = NAME_PREFIX[kind];
  const used = new Set(sheets.map((s) => s.name));
  let n = sheets.filter((s) => sheetKind(s) === kind).length + 1;
  while (used.has(`${prefix}${n}`)) n += 1;
  return `${prefix}${n}`;
}

/**
 * シート追加コマンド。用紙は表示中のシート (無ければ最初のシート) に合わせるので、
 * 盤レイアウトシートだけ用紙が変わってしまうことがない。
 */
export function addSheetCommand(
  project: Project | null,
  kind: SheetKind,
  activeSheetId?: string | null,
): Command {
  const sheets = project?.sheets ?? [];
  const model = sheets.find((s) => s.id === activeSheetId) ?? sheets[0];
  return {
    type: "add_sheet",
    name: nextSheetName(project, kind),
    size: model?.size ?? "A3",
    orientation: model?.orientation ?? "Landscape",
    kind,
  };
}
