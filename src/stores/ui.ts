// エディタUIの共有状態: コマンドライン履歴、モーダル表示など。

import { defineStore } from "pinia";

export const useUiStore = defineStore("ui", {
  state: () => ({
    commandHistory: ["MadakeCAD コマンドライン (L=配線 E=削除 U=元に戻す)"] as string[],
    symbolPickerOpen: false,
  }),
  actions: {
    log(line: string) {
      this.commandHistory.push(line);
      if (this.commandHistory.length > 200) this.commandHistory.shift();
    },
  },
});
