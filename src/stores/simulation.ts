// DC動作点シミュレーション結果ストア (spec §3.6)。

import { defineStore } from "pinia";
import { ipc, type SimOpResult } from "../ipc";

export const useSimulationStore = defineStore("simulation", {
  state: () => ({
    result: null as SimOpResult | null,
    error: null as string | null,
    panelOpen: false,
    running: false,
    /** 開路扱いにするスイッチ/接点の参照記号 (what-if)。 */
    openSwitches: [] as string[],
  }),
  actions: {
    async run(sheetId: string | null) {
      this.running = true;
      try {
        this.result = await ipc.simulateOp(sheetId, this.openSwitches);
        this.error = null;
      } catch (e) {
        this.result = null;
        this.error = e instanceof Error ? e.message : String(e);
      } finally {
        this.running = false;
        this.panelOpen = true;
      }
    },
    toggleOpen(reference: string) {
      const i = this.openSwitches.indexOf(reference);
      if (i >= 0) this.openSwitches.splice(i, 1);
      else this.openSwitches.push(reference);
    },
    close() {
      this.panelOpen = false;
    },
  },
});
