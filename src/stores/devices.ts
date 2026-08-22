// デバイスナビゲータ (左パネル「デバイス」タブ, spec §10) の状態。
//
// ツリーの中身はRust側 (madake-core/src/search.rs の `device_tree`) が作る。ここは
// 「読み込み・折りたたみ・選択行・削除」だけを持つ。削除もCommandエンジン経由なので
// Cmd+Zで戻せる (アーキテクチャの絶対原則)。

import { defineStore } from "pinia";
import { deviceDeleteIds, deviceRows, type DeviceRow } from "../canvas/deviceTree";
import { ipc, type DeviceNode } from "../ipc";
import { useDocumentStore } from "./document";

interface DevicesState {
  devices: DeviceNode[];
  /** 折りたたんでいるデバイスの参照記号。 */
  collapsed: Set<string>;
  /** 選択中の行のキー (`参照記号` または `参照記号#機能の番号`)。 */
  selectedKey: string | null;
  loading: boolean;
  /** 最後に読み込んだときのドキュメントrevision (図面が変わったら読み直す)。 */
  revision: number;
}

/** ツリーの行のキー (選択状態の保持に使う)。 */
export function rowKey(row: DeviceRow): string {
  return row.type === "device" ? row.reference : `${row.reference}#${row.index}`;
}

export const useDevicesStore = defineStore("devices", {
  state: (): DevicesState => ({
    devices: [],
    collapsed: new Set<string>(),
    selectedKey: null,
    loading: false,
    revision: -1,
  }),

  getters: {
    /** 折りたたみを反映した描画用の行 (親=デバイス、子=機能)。 */
    rows(state): DeviceRow[] {
      return deviceRows(state.devices, state.collapsed);
    },
    count(state): number {
      return state.devices.length;
    },
  },

  actions: {
    /** ツリーを読み込む。図面が変わっていなければ何もしない (`force`で強制)。 */
    async load(force = false) {
      const revision = useDocumentStore().revision;
      if (!force && this.revision === revision) return;
      this.loading = true;
      try {
        this.devices = await ipc.getDeviceTree();
        this.revision = revision;
      } finally {
        this.loading = false;
      }
    },

    /** デバイス行の ▾ / ▸ の切替。 */
    toggle(reference: string) {
      if (this.collapsed.has(reference)) this.collapsed.delete(reference);
      else this.collapsed.add(reference);
    },

    select(key: string | null) {
      this.selectedKey = key;
    },

    /**
     * デバイスを図面から削除する (右クリックメニュー)。
     * シートを跨いでいる場合はシートごとに1コマンド。戻り値=消したエンティティ数。
     */
    async deleteDevice(device: DeviceNode): Promise<number> {
      const store = useDocumentStore();
      const ids = deviceDeleteIds(device);
      const bySheet = new Map<string, string[]>();
      for (const fn of device.functions) {
        if (!ids.includes(fn.entity_id)) continue;
        const list = bySheet.get(fn.sheet_id);
        if (list) {
          if (!list.includes(fn.entity_id)) list.push(fn.entity_id);
        } else {
          bySheet.set(fn.sheet_id, [fn.entity_id]);
        }
      }
      for (const [sheetId, sheetIds] of bySheet) {
        await store.execute({ type: "delete_entities", sheet_id: sheetId, ids: sheetIds });
      }
      if (this.selectedKey?.split("#")[0] === device.reference) this.selectedKey = null;
      await this.load(true);
      return ids.length;
    },
  },
});
