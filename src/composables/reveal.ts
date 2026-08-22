// reveal (シート切替+選択+ズーム) の共通経路。
//
// 検証結果パネル・検索結果パネル・デバイスナビゲータ・参照サーフィンは、どれも
// 「どのシートのどのエンティティか」を求めてからこれを呼ぶ。ジャンプ先の求め方だけが
// それぞれ違い、表示のさせ方は1本にまとめてある。

import { inject } from "vue";
import { useDocumentStore } from "../stores/document";
import type { EditorController } from "../tools/controller";

/** ジャンプ先 (`xrefJumpTarget` / `searchJumpTarget` / `deviceJumpTarget` の戻り値)。 */
export interface RevealTarget {
  sheetId: string;
  entityIds: string[];
}

export function useReveal(): (target: RevealTarget) => void {
  const store = useDocumentStore();
  const controller = inject<EditorController>("controller")!;
  return ({ sheetId, entityIds }) => {
    if (store.activeSheetId !== sheetId) store.activeSheetId = sheetId;
    controller.reveal(entityIds);
  };
}
