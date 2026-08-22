// デバイスナビゲータ (左パネル「デバイス」タブ) のツリー整形。
//
// ツリーの中身はRust側 (madake-core/src/search.rs の `device_tree`) が作る。ここは
// 「折りたたみを反映して行の並びに均す」「見出しの文言を組み立てる」「行クリックの
// ジャンプ先を求める」だけを持つ。文言はi18nカタログが持ち、ここが返すのはキーと引数。

import type { DeviceFunction, DeviceNode } from "../ipc";
import { functionLabel, type LabelSpec } from "./search";

/** ツリーに描く1行 (親=デバイス、子=機能)。 */
export type DeviceRow =
  | {
      type: "device";
      reference: string;
      device: DeviceNode;
      /** ▾ (展開) か ▸ (折りたたみ) か。機能を持たないデバイスは折りたたみ記号を出さない。 */
      expanded: boolean;
    }
  | {
      type: "function";
      reference: string;
      device: DeviceNode;
      fn: DeviceFunction;
      /** 同じデバイスの中での通し番号 (行のキーに使う)。 */
      index: number;
    };

/**
 * ツリーを描画用の行の並びへ均す。`collapsed`に入っている参照記号は子行を出さない。
 * デバイスの並び (参照記号順) と機能の並び (コイル→接点 / 読み順) はRust側が決めた順を保つ。
 */
export function deviceRows(devices: DeviceNode[], collapsed: Set<string>): DeviceRow[] {
  const rows: DeviceRow[] = [];
  for (const device of devices) {
    const expanded = !collapsed.has(device.reference);
    rows.push({ type: "device", reference: device.reference, device, expanded });
    if (!expanded) continue;
    device.functions.forEach((fn, index) => {
      rows.push({ type: "function", reference: device.reference, device, fn, index });
    });
  }
  return rows;
}

/**
 * デバイス行の見出し (デザイン: 「▾ K1  リレー MY2N」の後半部分)。
 *
 * リレーと端子台は種別で呼び (端子台は極数つき)、それ以外はシンボルの名前で呼ぶ。
 * 型番が分かっていれば末尾に添える。
 */
export function deviceHeadline(device: DeviceNode, japanese: boolean): LabelSpec {
  const value = device.value;
  switch (device.kind) {
    case "relay":
      return { key: value ? "devices.head.relayWithValue" : "devices.head.relay", params: { value } };
    case "terminal_block":
      return {
        key: "devices.head.terminalBlock",
        params: { poles: String(device.poles) },
      };
    case "other": {
      const name = (japanese ? device.symbol_name_ja : device.symbol_name) || device.symbol_id;
      return {
        key: value ? "devices.head.otherWithValue" : "devices.head.other",
        params: { name, value },
      };
    }
  }
}

/** 機能行の名前 (例 「接点 13-14」)。結果パネルと同じ文言を使う。 */
export function functionHeadline(fn: DeviceFunction): LabelSpec {
  return functionLabel(fn.kind, fn.terminals);
}

/** 機能行の所在バッジ (例 「/2.B3」)。 */
export function functionAddress(fn: DeviceFunction): string {
  return `/${fn.sheet_no}.${fn.zone}`;
}

/**
 * 機能行のクリック・「図面へジャンプ」のジャンプ先。
 * 呼び出し側はこのシートへ切り替えてから `controller.reveal(entityIds)` を呼ぶ。
 */
export function deviceJumpTarget(fn: DeviceFunction): { sheetId: string; entityIds: string[] } {
  return { sheetId: fn.sheet_id, entityIds: [fn.entity_id] };
}

/**
 * デバイスをまるごと削除するときに消すエンティティ (同じ参照記号の全機能)。
 * 端子台のように1つのシンボルが複数の機能を持つ場合も、重複なく1回ずつ返す。
 */
export function deviceDeleteIds(device: DeviceNode): string[] {
  return [...new Set(device.functions.map((f) => f.entity_id))];
}
