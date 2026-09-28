// ファイル操作のショートカット判定の仕様テスト。
import { describe, expect, it } from "vitest";
import { fileShortcutOf, type ShortcutKeyEvent } from "./editorShortcuts";

/** キーイベントを組み立てる (既定は⌘だけを押した状態)。 */
function key(code: string, extra: Partial<ShortcutKeyEvent> = {}): ShortcutKeyEvent {
  return { metaKey: true, ctrlKey: false, shiftKey: false, altKey: false, code, ...extra };
}

describe("editor shortcuts: ファイル操作", () => {
  // ja: ⌘N=新規 / ⌘O=開く / ⌘S=保存 / ⇧⌘S=名前を付けて保存
  it("Cmd+N is new, Cmd+O is open, Cmd+S is save and Shift+Cmd+S is save-as", () => {
    expect(fileShortcutOf(key("KeyN"), false)).toBe("new");
    expect(fileShortcutOf(key("KeyO"), false)).toBe("open");
    expect(fileShortcutOf(key("KeyS"), false)).toBe("save");
    expect(fileShortcutOf(key("KeyS", { shiftKey: true }), false)).toBe("saveAs");
  });

  // ja: WindowsとLinuxではCtrlでも同じショートカットが効く
  it("Ctrl works the same as Cmd on Windows and Linux", () => {
    expect(fileShortcutOf(key("KeyS", { metaKey: false, ctrlKey: true }), false)).toBe("save");
  });

  // ja: 修飾キー無しのN/O/Sはファイル操作にならない (図面のショートカットのまま)
  it("plain N, O and S are not file shortcuts", () => {
    expect(fileShortcutOf(key("KeyN", { metaKey: false }), false)).toBeNull();
    expect(fileShortcutOf(key("KeyS", { metaKey: false }), false)).toBeNull();
  });

  // ja: モーダル (シンボルエディタ等) を開いている間はファイル操作のショートカットが効かない
  it("file shortcuts do nothing while a modal is open", () => {
    expect(fileShortcutOf(key("KeyN"), true)).toBeNull();
    expect(fileShortcutOf(key("KeyO"), true)).toBeNull();
    expect(fileShortcutOf(key("KeyS"), true)).toBeNull();
    expect(fileShortcutOf(key("KeyS", { shiftKey: true }), true)).toBeNull();
  });

  // ja: ⌥を足した組み合わせ (⌥⌘Sなど) はファイル操作にしない
  it("adding Option/Alt makes it a different shortcut, not a file action", () => {
    expect(fileShortcutOf(key("KeyS", { altKey: true }), false)).toBeNull();
    expect(fileShortcutOf(key("KeyN", { altKey: true }), false)).toBeNull();
  });

  // ja: ⇧⌘Nと⇧⌘Oはファイル操作に割り当てない (別のショートカットに残す)
  it("Shift+Cmd+N and Shift+Cmd+O are not file actions", () => {
    expect(fileShortcutOf(key("KeyN", { shiftKey: true }), false)).toBeNull();
    expect(fileShortcutOf(key("KeyO", { shiftKey: true }), false)).toBeNull();
  });

  // ja: 判定は物理キー (`code`) で行うので、非ラテン配列でも同じキーで効く
  it("the decision is made on the physical key, so non-Latin layouts still work", () => {
    // ロシア語配列で⌘Sを押したイベント: `key`は"ы"だが`code`は"KeyS"
    expect(fileShortcutOf(key("KeyS"), false)).toBe("save");
    expect(fileShortcutOf(key("Digit1"), false)).toBeNull();
  });
});
