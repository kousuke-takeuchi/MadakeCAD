// プロパティパネルの編集 → Command 変換。モデルを直接書き換えず、必ずCommand経由で
// エンジンに渡す (undo/redoとpatch配信の一元化。CLAUDE.md「アーキテクチャの絶対原則」)。

import type { Command, Entity } from "../ipc";

/**
 * 線番の個別編集コマンド (docs/internal/specs/m2-drawing-parity.md §2)。
 * 前後の空白は落とし、空欄なら線番を消す。変わっていなければnull (コマンドを送らない)。
 * 配線以外のエンティティにはnullを返す。
 */
export function wireNumberCommand(
  sheetId: string,
  entity: Entity,
  input: string,
): Command | null {
  if (entity.kind !== "wire") return null;
  const next = input.trim();
  const current = (entity.net ?? "").trim();
  if (next === current) return null;
  return {
    type: "set_wire_numbers",
    sheet_id: sheetId,
    numbers: [{ wire_id: entity.id, number: next === "" ? null : next }],
  };
}

/** 配線プロパティの編集結果。 */
export interface WireEdit {
  command: Command | null;
  /** FreeCADで計測した長さを手入力で上書きしたか (呼び出し側が警告を出す)。 */
  overwroteMeasured: boolean;
}

/**
 * 配線の色・線径・電線品番・長さの編集コマンド (M5-2: 長さの出所つき)。
 * 何も変わっていなければコマンド無し。長さを手で変えると出所は「手入力」に戻り、
 * それがFreeCAD計測値の上書きなら `overwroteMeasured` を立てる (暗黙の上書きを警告するため)。
 * 配線以外にはコマンド無し。
 */
export function wireUpdateCommand(
  sheetId: string,
  entity: Entity,
  edit: { color: string; sq: number; part_no: string; length_m: string },
): WireEdit {
  if (entity.kind !== "wire") return { command: null, overwroteMeasured: false };
  const length = edit.length_m.trim() === "" ? null : Number(edit.length_m);
  const lengthChanged = length !== entity.length_m;
  const next: Entity = {
    ...entity,
    color: edit.color,
    sq: edit.sq,
    part_no: edit.part_no || null,
    length_m: length,
    length_source: lengthChanged ? "manual" : entity.length_source ?? "manual",
  };
  const changed =
    next.color !== entity.color ||
    next.sq !== entity.sq ||
    next.part_no !== entity.part_no ||
    lengthChanged;
  if (!changed) return { command: null, overwroteMeasured: false };
  return {
    command: { type: "update_entity", sheet_id: sheetId, entity: next },
    overwroteMeasured: lengthChanged && entity.length_source === "freecad",
  };
}

/**
 * ハーネス境界の名前・備考の編集コマンド (docs/internal/specs/m2-drawing-parity.md §3)。
 * 名前は前後の空白を落とす。名前も備考も変わっていなければnull (コマンドを送らない)。
 * ハーネス以外のエンティティにはnullを返す。
 */
export function harnessUpdateCommand(
  sheetId: string,
  entity: Entity,
  name: string,
  note: string,
): Command | null {
  if (entity.kind !== "harness") return null;
  const nextName = name.trim();
  const nextNote = note.trim();
  if (nextName === (entity.name ?? "") && nextNote === (entity.note ?? "")) return null;
  return {
    type: "update_entity",
    sheet_id: sheetId,
    entity: { ...entity, name: nextName, note: nextNote },
  };
}
