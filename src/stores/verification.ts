// 検証結果ストア (spec §3.5)。検証はRust側の純関数で、ここは結果の保持とパネル状態のみ。

import { defineStore } from "pinia";
import { ipc, type Diagnostic } from "../ipc";

interface VerificationState {
  diagnostics: Diagnostic[];
  panelOpen: boolean;
  running: boolean;
}

export const useVerificationStore = defineStore("verification", {
  state: (): VerificationState => ({
    diagnostics: [],
    panelOpen: false,
    running: false,
  }),

  getters: {
    counts(state): { error: number; warning: number; info: number } {
      const c = { error: 0, warning: 0, info: 0 };
      for (const d of state.diagnostics) c[d.severity]++;
      return c;
    },
  },

  actions: {
    /** 検証を実行して結果パネルを開く。sheetId=nullで全シート。 */
    async run(sheetId: string | null) {
      this.running = true;
      try {
        this.diagnostics = await ipc.verify(sheetId);
        this.panelOpen = true;
      } finally {
        this.running = false;
      }
    },
    close() {
      this.panelOpen = false;
    },
  },
});
