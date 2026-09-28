// ファイル操作のキーボードショートカットの判定。
//
// ⌘N/⌘O/⌘S/⇧⌘S はモーダル (シンボルエディタ・割付表・帳票ダイアログ等) を開いている
// 間は効かせない。効かせてしまうと、開いたままのモーダルの下でドキュメントが丸ごと
// 差し替わり、モーダルはもう存在しないエンティティを指すことになる。
// 文字は`ev.key`ではなく`ev.code`で見る (非ラテン配列でも同じ物理キーで効く)。

/** ファイル操作のショートカットの種類。 */
export type FileShortcut = "new" | "open" | "save" | "saveAs";

/** 判定に使うキーイベントの部分。 */
export interface ShortcutKeyEvent {
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  code: string;
}

/**
 * ⌘N=新規 / ⌘O=開く / ⌘S=保存 / ⇧⌘S=名前を付けて保存。
 * 該当しない・⌥が押されている・モーダルが開いているときはnull。
 */
export function fileShortcutOf(event: ShortcutKeyEvent, modalOpen: boolean): FileShortcut | null {
  if (modalOpen) return null;
  if (!(event.metaKey || event.ctrlKey)) return null;
  if (event.altKey) return null;
  switch (event.code) {
    case "KeyN":
      return event.shiftKey ? null : "new";
    case "KeyO":
      return event.shiftKey ? null : "open";
    case "KeyS":
      return event.shiftKey ? "saveAs" : "save";
    default:
      return null;
  }
}
