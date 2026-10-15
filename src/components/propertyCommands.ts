// プロパティパネルの編集 → Command 変換。モデルを直接書き換えず、必ずCommand経由で
// エンジンに渡す (undo/redoとpatch配信の一元化。CLAUDE.md「アーキテクチャの絶対原則」)。

import {
  DEFAULT_ANNOTATION_TEXT_MM,
  annotationTextMm,
  isValidAnnotationTextMm,
} from "../canvas/annotationText";
import { MATES_WITH_ATTR, isConnectorInstance } from "../canvas/mate";
import { DEVICE_BLOCK_ATTR } from "../canvas/relayXref";
import { FUNCTION_LABEL_ATTR } from "../canvas/textStyle";
import type { Command, Entity, Sheet, SymbolInstance } from "../ipc";

/**
 * プロパティパネルが確定するシンボルの`attrs` (G4 機能ラベル・G7 実機ブロック・G14 嵌合相手)。
 *
 * パネルが**持っている欄だけ**を書き換え、それ以外の属性 (ジャンパ・ブロック定義など
 * MCP・CLIが書いたものを含む) はそのまま残す。嵌合相手はコネクタ一族
 * (`connector_*`) のシンボルにしか欄が出ないので、それ以外のシンボルでは
 * この属性に一切触れない。空欄の欄はキーごと消して、旧い図面と同じ形で保存する。
 */
export function symbolAttrs(
  instance: SymbolInstance,
  edited: { functionLabel: string; deviceBlock: boolean; mate: string },
): Record<string, string> {
  const attrs = { ...(instance.attrs ?? {}) };
  const label = edited.functionLabel.trim();
  if (label) attrs[FUNCTION_LABEL_ATTR] = label;
  else delete attrs[FUNCTION_LABEL_ATTR];
  if (edited.deviceBlock) attrs[DEVICE_BLOCK_ATTR] = "true";
  else delete attrs[DEVICE_BLOCK_ATTR];
  if (isConnectorInstance(instance.symbol_id)) {
    const mate = edited.mate.trim();
    if (mate) attrs[MATES_WITH_ATTR] = mate;
    else delete attrs[MATES_WITH_ATTR];
  }
  return attrs;
}

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

/** プロパティパネルで編集するケーブルの欄 (入力欄の文字列のまま受け取る)。 */
export interface CablePatch {
  reference: string;
  part_no: string;
  /** 芯数 (1以上の整数)。 */
  core_count: string;
  /** 線径 sq (空欄=未設定)。 */
  sq: string;
  shield: boolean;
  grounded: boolean;
  /** 長さ m (空欄=未設定)。 */
  length_m: string;
}

/**
 * ケーブルのプロパティ編集コマンド (M4 §5)。芯線割当 (定義線が決めたもの) はそのまま残す。
 * 芯数が1以上の整数でない・線径や長さが負/数値でない入力はnull (図面を変えない)。
 * 何も変わっていなければnull。
 */
export function cableUpdateCommand(sheetId: string, entity: Entity, patch: CablePatch): Command | null {
  if (entity.kind !== "cable") return null;
  const cores = Number(patch.core_count);
  if (!Number.isInteger(cores) || cores < 1) return null;
  const optional = (text: string): number | null | undefined => {
    if (text.trim() === "") return null;
    const v = Number(text);
    return Number.isFinite(v) && v >= 0 ? v : undefined;
  };
  const sq = optional(patch.sq);
  const length = optional(patch.length_m);
  if (sq === undefined || length === undefined) return null;
  const next = {
    ...entity,
    reference: patch.reference.trim(),
    part_no: patch.part_no.trim(),
    core_count: cores,
    sq: sq ?? 0,
    shield: patch.shield ? { grounded: patch.grounded } : null,
    length_m: length,
  };
  const same =
    next.reference === entity.reference &&
    next.part_no === entity.part_no &&
    next.core_count === entity.core_count &&
    next.sq === entity.sq &&
    JSON.stringify(next.shield) === JSON.stringify(entity.shield ?? null) &&
    next.length_m === (entity.length_m ?? null);
  return same ? null : { type: "update_entity", sheet_id: sheetId, entity: next };
}

/**
 * シートの注記文字高の編集コマンド (G20)。配線注記・線番・ネットタグの大きさが変わる。
 *
 * 入力は数値入力欄の値 (数値または文字列)。空欄は「既定に戻す」(null) の意味。
 * 範囲 (1.0〜3.5mm) を外れた値・数値でない入力は
 * null を返して何も送らない (エンジンも同じ範囲で拒否する)。今と同じ高さならコマンドを
 * 作らない。
 */
export function annotationTextCommand(sheet: Sheet, input: string | number): Command | null {
  // 数値入力欄のv-modelは数値を返す (空欄は "")。どちらでも受けられるように文字列へ寄せる
  const text = String(input ?? "").trim();
  const height = text === "" ? null : Number(text);
  if (height !== null && !isValidAnnotationTextMm(height)) return null;
  const current = sheet.annotation_text_mm ?? null;
  if (height === current) return null;
  // 既定値そのものを入れたときは「設定なし」と同じ意味なので、旧い図面と同じ形で保存する
  const next = height === DEFAULT_ANNOTATION_TEXT_MM ? null : height;
  if (next === current && annotationTextMm(sheet) === DEFAULT_ANNOTATION_TEXT_MM) return null;
  return { type: "set_sheet_annotation_text", sheet_id: sheet.id, height_mm: next };
}
