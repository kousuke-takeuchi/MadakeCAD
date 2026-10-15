// 銘板の挿入ダイアログの状態 (M4 §8、.pen「M4残りデザイン⑤」`B2gfQZ`の銘板ダイアログ)。
//
// 盤レイアウトシートで銘板ツールを使い、フットプリントをクリックすると開く (何も無い所なら
// 単独の銘板)。挿入はadd_entity 1回 (Cmd+Zで戻る)。1行目の既定は回路図の説明 (機能ラベル)。

import { defineStore } from "pinia";
import type { Nameplate, Point } from "../ipc";
import { NAMEPLATE_DEFAULT, deviceDescriptions, nameplateAddCommand } from "../canvas/panelMarks";
import { useDocumentStore } from "./document";

interface NameplateState {
  open: boolean;
  /** 結び付けるフットプリント (nullなら単独)。 */
  footprintId: string | null;
  /** 単独の銘板の左上。 */
  at: Point;
  line1: string;
  line2: string;
  w: number;
  h: number;
  textMm: number;
  position: Nameplate["position"];
  follow: boolean;
  partNo: string;
  error: string | null;
}

export const useNameplatesStore = defineStore("nameplates", {
  state: (): NameplateState => ({
    open: false,
    footprintId: null,
    at: { x: 0, y: 0 },
    line1: "",
    line2: "",
    w: NAMEPLATE_DEFAULT.w,
    h: NAMEPLATE_DEFAULT.h,
    textMm: NAMEPLATE_DEFAULT.text_mm,
    position: "below",
    follow: true,
    partNo: "",
    error: null,
  }),

  getters: {
    /** 結び付ける部品の参照記号 (単独ならnull)。 */
    reference(state): string | null {
      if (!state.footprintId) return null;
      const fp = useDocumentStore().activeSheet?.entities[state.footprintId];
      return fp && fp.kind === "footprint" ? fp.reference.trim() || null : null;
    },
    /** 回路図の説明 (結び付けた部品の機能ラベル。無ければnull)。 */
    description(): string | null {
      const reference = this.reference;
      if (!reference) return null;
      return deviceDescriptions(useDocumentStore().project).get(reference) ?? null;
    },
    /** 挿入できるか (大きさ・文字高が正)。 */
    canInsert(state): boolean {
      return state.w > 0 && state.h > 0 && state.textMm > 0;
    },
  },

  actions: {
    /** ダイアログを開く。部品に結び付けるなら1行目を回路図の説明で始める。 */
    openFor(footprintId: string | null, at: Point) {
      this.$reset();
      this.footprintId = footprintId;
      this.at = at;
      this.follow = footprintId !== null;
      this.line1 = this.description ?? "";
      this.open = true;
    },

    cancel() {
      this.open = false;
    },

    /** 銘板を置く (add_entity 1回)。 */
    async insert(): Promise<boolean> {
      const doc = useDocumentStore();
      const sheet = doc.activeSheet;
      if (!sheet || !this.canInsert) return false;
      const lines = [this.line1.trim(), this.line2.trim()].filter((l, i) => l !== "" || i === 0);
      try {
        await doc.execute(
          nameplateAddCommand(sheet, {
            footprint: this.footprintId,
            at: this.at,
            w: this.w,
            h: this.h,
            lines: lines[0] === "" && lines.length === 1 ? [] : lines,
            text_mm: this.textMm,
            position: this.position,
            follow_description: this.footprintId !== null && this.follow,
            part_no: this.partNo.trim(),
          }),
        );
        this.open = false;
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      }
    },
  },
});
