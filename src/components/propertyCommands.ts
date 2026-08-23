// プロパティパネルの編集 → Command 変換。モデルを直接書き換えず、必ずCommand経由で
// エンジンに渡す (undo/redoとpatch配信の一元化。CLAUDE.md「アーキテクチャの絶対原則」)。

import {
  DEFAULT_ANNOTATION_TEXT_MM,
  annotationTextMm,
  isValidAnnotationTextMm,
} from "../canvas/annotationText";
import type { Command, Entity, Sheet } from "../ipc";

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
