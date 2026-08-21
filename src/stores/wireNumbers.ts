// 線番自動採番ダイアログの状態 (仕様: docs/internal/specs/m2-drawing-parity.md §2)。
// 選んだ内容は renumber_wires コマンド1回に変換して実行する (undo一発で戻る)。
// モデル(document ストアのミラー)を直接書き換えてはならない。

import { defineStore } from "pinia";
import type { Command, Patch, Sheet } from "../ipc";
import { useDocumentStore } from "./document";

/** 採番方式。ゾーン基準 (参照ベース) はM4予定でまだ選べない。 */
export type NumberingMethod = "sequential" | "zone";
/** 採番の対象範囲。 */
export type NumberingScope = "sheet" | "project";
/** 既存の線番の扱い。keep=追い番 (未採番のみ)、renumber=すべて振り直し。 */
export type ExistingPolicy = "keep" | "renumber";

interface WireNumbersState {
  open: boolean;
  sheetId: string | null;
  sheetName: string;
  method: NumberingMethod;
  /** 開始番号 (入力欄の生テキスト)。 */
  start: string;
  scope: NumberingScope;
  existing: ExistingPolicy;
  running: boolean;
  error: string | null;
}

/** 採番したネット数 = patchで書き換わったワイヤの線番の種類数。 */
function numberedNetCount(patch: Patch): number {
  const nets = new Set<string>();
  for (const op of patch.ops) {
    if (op.op !== "entity_upserted") continue;
    if (op.entity.kind !== "wire") continue;
    const net = (op.entity.net ?? "").trim();
    if (net) nets.add(net);
  }
  return nets.size;
}

export const useWireNumbersStore = defineStore("wireNumbers", {
  state: (): WireNumbersState => ({
    open: false,
    sheetId: null,
    sheetName: "",
    method: "sequential",
    start: "1",
    scope: "sheet",
    existing: "keep",
    running: false,
    error: null,
  }),

  getters: {
    /** ゾーン基準の採番が使えるか (M4で実装予定)。 */
    zoneAvailable(): boolean {
      return false;
    },
    /** 開始番号 (1以上の整数)。不正ならnull。 */
    startValue(state): number | null {
      const text = state.start.trim();
      if (!/^\d+$/.test(text)) return null;
      const value = Number(text);
      return value >= 1 ? value : null;
    },
    startValid(): boolean {
      return this.startValue !== null;
    },
    /** 実行するコマンド。入力が不正、またはシートが無ければnull。 */
    command(state): Command | null {
      const start = this.startValue;
      if (start === null || !state.sheetId) return null;
      return {
        type: "renumber_wires",
        // 「プロジェクト全体」はシート指定を省く (図面全体で一意に採番)
        sheet_id: state.scope === "sheet" ? state.sheetId : null,
        mode: state.existing === "keep" ? "append" : "renumber",
        start,
      };
    },
  },

  actions: {
    /** 対象シートを覚えて既定値でダイアログを開く。 */
    openFor(sheet: Sheet) {
      this.sheetId = sheet.id;
      this.sheetName = sheet.name;
      this.method = "sequential";
      this.start = "1";
      this.scope = "sheet";
      this.existing = "keep";
      this.error = null;
      this.open = true;
    },

    cancel() {
      this.open = false;
      this.error = null;
    },

    /**
     * renumber_wiresコマンドを1回実行して閉じる。
     * 戻り値=採番したネット数 (実行しなかった・失敗したときはnull)。
     */
    async run(): Promise<number | null> {
      const command = this.command;
      if (!command || this.running) return null;
      this.running = true;
      this.error = null;
      try {
        const patch = await useDocumentStore().execute(command);
        this.open = false;
        return numberedNetCount(patch);
      } catch (e) {
        this.error = String(e);
        return null;
      } finally {
        this.running = false;
      }
    },
  },
});
