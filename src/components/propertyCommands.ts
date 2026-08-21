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
