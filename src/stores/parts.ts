// 部品DB (グローバル共有マスタ) の検索結果ストア。部品挿入ダイアログの部品DBセクション用。

import { defineStore } from "pinia";
import { ipc, type Part } from "../ipc";

export const usePartsStore = defineStore("parts", {
  state: () => ({
    results: [] as Part[],
    loading: false,
  }),
  actions: {
    /** 部品DBを検索する。失敗時(アプリ未接続等)は空扱い。 */
    async search(query: string) {
      this.loading = true;
      try {
        this.results = await ipc.searchParts(query);
      } catch {
        this.results = [];
      } finally {
        this.loading = false;
      }
    },
  },
});
