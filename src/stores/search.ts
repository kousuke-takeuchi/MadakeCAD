// プロジェクト内検索 (⌘F, spec §10) の状態。
//
// 検索そのものはRust側の純関数 (madake-core/src/search.rs) が行い、ここは
// 「浮き検索バーの開閉・入力・フィルタチップ・結果と巡回位置」だけを持つ。
// 結果パネル (下部ドック) は検証結果パネルと同じ様式で、行クリック=reveal。

import { defineStore } from "pinia";
import { cycleIndex, kindsForFilter, type SearchFilter } from "../canvas/search";
import { ipc, type SearchHit } from "../ipc";

interface SearchState {
  /** 作図領域右上に浮く検索バーを出しているか。 */
  barOpen: boolean;
  query: string;
  filter: SearchFilter;
  hits: SearchHit[];
  /** Enter巡回で今どのヒットを見ているか (-1 = まだ選んでいない)。 */
  activeIndex: number;
  /** 下部ドックの結果パネルを出しているか。 */
  panelOpen: boolean;
  running: boolean;
}

export const useSearchStore = defineStore("search", {
  state: (): SearchState => ({
    barOpen: false,
    query: "",
    filter: "all",
    hits: [],
    activeIndex: -1,
    panelOpen: false,
    running: false,
  }),

  getters: {
    count(state): number {
      return state.hits.length;
    },
    /** Enter巡回で今見ているヒット (未選択ならnull)。 */
    activeHit(state): SearchHit | null {
      return state.hits[state.activeIndex] ?? null;
    },
  },

  actions: {
    /** ⌘F: 検索バーを開く (すでに開いていれば入力を選び直すだけ)。 */
    openBar() {
      this.barOpen = true;
    },

    /** Esc: 検索バーも結果パネルも閉じる。検索語は次の⌘Fのために残す。 */
    close() {
      this.barOpen = false;
      this.panelOpen = false;
    },

    /** 結果パネルだけ閉じる (パネルの✕)。 */
    closePanel() {
      this.panelOpen = false;
    },

    setQuery(query: string) {
      this.query = query;
    },

    /** フィルタチップの切替。対象が変わるので検索し直す。 */
    async setFilter(filter: SearchFilter) {
      this.filter = filter;
      await this.run();
    },

    /**
     * 現在の検索語とフィルタで検索し、結果パネルを開く。
     * 検索語が空なら結果を捨ててパネルを閉じる (図面全体を並べたりしない)。
     */
    async run() {
      const query = this.query.trim();
      this.activeIndex = -1;
      if (!query) {
        this.hits = [];
        this.panelOpen = false;
        return;
      }
      this.running = true;
      try {
        this.hits = await ipc.searchProject(query, kindsForFilter(this.filter));
        this.panelOpen = true;
      } finally {
        this.running = false;
      }
    },

    /**
     * Enter=次へ / Shift+Enter=前へ。巡回位置を進めて、そのヒットを返す
     * (呼び出し側がrevealする)。結果が空ならnull。
     */
    step(step: number): SearchHit | null {
      this.activeIndex = cycleIndex(this.hits.length, this.activeIndex, step);
      return this.activeHit;
    },

    /** 結果パネルの行クリック。巡回位置をその行に合わせる。 */
    select(index: number): SearchHit | null {
      if (index < 0 || index >= this.hits.length) return null;
      this.activeIndex = index;
      return this.hits[index];
    },
  },
});
