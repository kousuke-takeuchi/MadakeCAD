// エージェントへの添付の入口 (.pen「添付デザイン」`s8nJcQ`): ファイル選択・ドラッグ&ドロップ・⌘V。
//
// アプリ (Tauri) ではOSからのファイルドロップをwebviewが受け取り (HTML5のdropにはファイルが来ない)、
// 落とした位置がチャット欄 (`data-chat-drop`の付いた要素) の上ならパスで添付する。ブラウザ検証
// モードではHTML5のdrag/dropでFileを受け取る。

import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { inTauri } from "../ipc";
import { ATTACHMENT_EXTENSIONS, useChatAttachmentsStore } from "../stores/chatAttachments";

/** 貼り付けられたファイルのうち画像だけ (⌘Vで添付できるのは画像だけ)。 */
export function pastedImages(files: ArrayLike<File>): File[] {
  return Array.from(files).filter((f) => f.type.startsWith("image/"));
}

/** ドラッグにファイルが含まれているか (図面内のドラッグ等は相手にしない)。 */
export function draggingFiles(types: ReadonlyArray<string> | DOMStringList | undefined): boolean {
  return !!types && Array.from(types as ArrayLike<string>).includes("Files");
}

/** 画面上の位置がチャット欄 (`data-chat-drop`) の上か。 */
function overChat(x: number, y: number): boolean {
  return !!document.elementFromPoint(x, y)?.closest("[data-chat-drop]");
}

let nativeInstalled = false;

/** アプリでのOSからのファイルドロップを受ける (起動時に1回)。 */
export async function installNativeChatDrop(): Promise<void> {
  if (!inTauri || nativeInstalled) return;
  nativeInstalled = true;
  const attachments = useChatAttachmentsStore();
  await getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === "leave") {
      attachments.dragOver = false;
      return;
    }
    const at = p.position.toLogical(window.devicePixelRatio || 1);
    const inside = overChat(at.x, at.y);
    if (p.type === "drop") {
      attachments.dragOver = false;
      if (inside) void attachments.addPaths(p.paths);
    } else {
      attachments.dragOver = inside;
    }
  });
}

/** チャット欄に付けるHTML5のドラッグ&ドロップと貼り付け (ブラウザ検証モード・⌘V)。 */
export function useChatDropHandlers(pastedLabel: () => string) {
  const attachments = useChatAttachmentsStore();
  return {
    onDragOver(ev: DragEvent) {
      if (inTauri || !draggingFiles(ev.dataTransfer?.types)) return;
      ev.preventDefault();
      attachments.dragOver = true;
    },
    onDragLeave(ev: DragEvent) {
      if (inTauri) return;
      const to = ev.relatedTarget as Node | null;
      if (!to || !(ev.currentTarget as Node).contains(to)) attachments.dragOver = false;
    },
    onDrop(ev: DragEvent) {
      if (inTauri || !draggingFiles(ev.dataTransfer?.types)) return;
      ev.preventDefault();
      attachments.dragOver = false;
      void attachments.addFiles(Array.from(ev.dataTransfer?.files ?? []));
    },
    onPaste(ev: ClipboardEvent) {
      const images = pastedImages(ev.clipboardData?.files ?? []);
      if (!images.length) return;
      ev.preventDefault();
      void attachments.addFiles(images, pastedLabel());
    },
  };
}

/** 「+」→「画像/ファイルを追加...」: OSのファイル選択 (画像・PDFで絞り込み、複数選択可)。 */
export async function pickAttachments(filterName: string): Promise<void> {
  const attachments = useChatAttachmentsStore();
  if (inTauri) {
    const picked = await openDialog({ multiple: true, filters: [{ name: filterName, extensions: ATTACHMENT_EXTENSIONS }] });
    const paths = picked == null ? [] : Array.isArray(picked) ? picked : [picked];
    await attachments.addPaths(paths);
    return;
  }
  const input = document.createElement("input");
  input.type = "file";
  input.multiple = true;
  input.accept = ATTACHMENT_EXTENSIONS.map((e) => `.${e}`).join(",");
  input.onchange = () => void attachments.addFiles(Array.from(input.files ?? []));
  input.click();
}
