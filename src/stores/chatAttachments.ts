// 入力欄に並べる送信前の添付 (.pen「添付デザイン - エージェントへ画像・PDFを添付」`s8nJcQ`)。
//
// 添付の入口は3つ: 「+」→「画像/ファイルを追加...」(OSのファイル選択)・チャット欄への
// ドラッグ&ドロップ・⌘V (画像の貼り付け)。浮きカードと左ドックの入力欄で共有する
// (下書き本文と同じく、どちらから送っても同じ添付が付く)。
//
// 規則はRust (`madake_agent::attachments`) と同じ: 画像 (PNG/JPEG/GIF/WebP)・PDF・Excel (.xlsx)・CSV、1件20MB、
// 1回5件まで。外れたものはチップを赤くして理由を出し、送信ボタンを淡色にする
// (最終判断は送るときにRustがもう一度行う)。OpenAI互換のモデルはPDFを読めないので、
// その組み合わせも送れない状態として扱う (表計算はRustがテキストにして渡すので、どのモデルにも送れる)。
//
// アプリ (Tauri) ではファイルのパスを持ち、中身はRustが読む。貼り付け・ブラウザ検証モードでは
// 中身をbase64で持つ。

import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { inTauri } from "../ipc";
import type { AttachmentInput, AttachmentMeta, OutgoingAttachments } from "./chat";
import { useSettingsStore } from "./settings";

export const IMAGE_MIMES = ["image/png", "image/jpeg", "image/gif", "image/webp"] as const;
export const PDF_MIME = "application/pdf";
export const XLSX_MIME = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
export const CSV_MIME = "text/csv";
/** 1件の上限 (Rust `MAX_ATTACHMENT_BYTES`)。 */
export const MAX_ATTACHMENT_BYTES = 20 * 1024 * 1024;
/** 1回の上限 (Rust `MAX_ATTACHMENTS`)。 */
export const MAX_ATTACHMENTS = 5;
/** ファイル選択の絞り込み。 */
export const ATTACHMENT_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "webp", "pdf", "xlsx", "csv"];

/** 文言のキーと差し込み値 (UIカタログ`chat.attach.*`)。 */
export interface AttachmentProblem {
  key: string;
  params: Record<string, string | number>;
}

export interface PendingAttachment {
  id: string;
  name: string;
  mime: string;
  size: number;
  /** アプリではファイルのパス (中身はRustが読む)。 */
  path?: string;
  /** 貼り付け・ブラウザ検証モードではbase64の中身。 */
  data?: string;
  /** 添付できない理由 (種類・大きさ・件数・読めない)。 */
  problem: AttachmentProblem | null;
}

/** ファイル名からMIMEを推す (Rust `mime_from_name`と同じ)。分からなければ空。 */
export function mimeFromName(name: string): string {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (ext === "png") return "image/png";
  if (ext === "jpg" || ext === "jpeg") return "image/jpeg";
  if (ext === "gif") return "image/gif";
  if (ext === "webp") return "image/webp";
  if (ext === "pdf") return PDF_MIME;
  if (ext === "xlsx") return XLSX_MIME;
  if (ext === "csv") return CSV_MIME;
  return "";
}

export function isPdf(mime: string): boolean {
  return mime === PDF_MIME;
}

/** Excel (.xlsx) かCSV (Rustがテキストにして渡す)。 */
export function isSpreadsheet(mime: string): boolean {
  return mime === XLSX_MIME || mime === CSV_MIME;
}

export function isAttachableMime(mime: string): boolean {
  return (IMAGE_MIMES as readonly string[]).includes(mime) || mime === PDF_MIME || isSpreadsheet(mime);
}

/** 大きさの表示 (1.2 MB / 340 KB)。 */
export function formatSize(bytes: number): string {
  return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

/** 1件の確かめ (種類・大きさ)。 */
export function attachmentProblem(mime: string, size: number): AttachmentProblem | null {
  if (!isAttachableMime(mime)) return { key: "chat.attach.unsupported", params: {} };
  if (size > MAX_ATTACHMENT_BYTES) {
    return { key: "chat.attach.tooLarge", params: { size: formatSize(size), max: MAX_ATTACHMENT_BYTES / 1024 / 1024 } };
  }
  return null;
}

/** 選んだモデルで送れない組み合わせ (OpenAI互換+PDF)。送れるならnull。 */
export function providerProblem(items: readonly PendingAttachment[], provider: string): AttachmentProblem | null {
  if (provider === "openai_compat" && items.some((a) => isPdf(a.mime))) {
    return { key: "chat.attach.pdfUnsupported", params: {} };
  }
  return null;
}

let seq = 0;
const nextId = () => `att-${++seq}`;

/** Fileをbase64で読む (`data:...;base64,`の後ろだけ)。 */
async function fileToBase64(file: Blob): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(binary);
}

interface AttachmentInfo {
  path: string;
  name: string;
  mime: string;
  size: number;
  error: string | null;
}

interface ChatAttachmentsState {
  items: PendingAttachment[];
  /** ファイルをチャット欄の上に持ってきている (入力欄を青枠にする)。 */
  dragOver: boolean;
  /** 貼り付け画像の通し番号 (「貼り付け画像 1.png」)。 */
  pasted: number;
}

export const useChatAttachmentsStore = defineStore("chatAttachments", {
  state: (): ChatAttachmentsState => ({ items: [], dragOver: false, pasted: 0 }),

  getters: {
    /** 選んだモデルで送れない理由 (注意帯)。 */
    providerProblem(state): AttachmentProblem | null {
      return providerProblem(state.items, useSettingsStore().settings.provider);
    },
    /** 送れない添付があるか (送信ボタンを淡色にする)。 */
    blocking(): boolean {
      return this.items.some((a) => a.problem !== null) || this.providerProblem !== null;
    },
    /** 送信する添付 (バックエンドへの指定と、発言に出す情報)。 */
    outgoing(state): OutgoingAttachments {
      const ok = state.items.filter((a) => a.problem === null);
      const inputs: AttachmentInput[] = ok.map((a) =>
        a.path ? { name: a.name, mime: a.mime, path: a.path } : { name: a.name, mime: a.mime, data: a.data ?? "" },
      );
      const metas: AttachmentMeta[] = ok.map((a) => ({ name: a.name, mime: a.mime, size: a.size }));
      return { inputs, metas };
    },
  },

  actions: {
    /** 並びに足す。6件目以降は件数オーバーとして赤く出す。 */
    push(item: Omit<PendingAttachment, "id">) {
      const tooMany = this.items.length >= MAX_ATTACHMENTS;
      this.items.push({
        id: nextId(),
        ...item,
        problem: item.problem ?? (tooMany ? { key: "chat.attach.tooMany", params: { max: MAX_ATTACHMENTS } } : null),
      });
    },

    /** ファイルのパスで足す (アプリのファイル選択・ドラッグ&ドロップ)。中身はRustが読む。 */
    async addPaths(paths: string[]) {
      if (!paths.length) return;
      const infos = await invoke<AttachmentInfo[]>("agent_inspect_attachments", { paths });
      for (const info of infos) {
        this.push({
          name: info.name,
          mime: info.mime,
          size: info.size,
          path: info.path,
          problem: info.error ? { key: "chat.attach.cannotRead", params: { reason: info.error } } : attachmentProblem(info.mime, info.size),
        });
      }
    },

    /**
     * Fileで足す (ブラウザ検証モードのファイル選択・ドロップ、⌘Vの貼り付け)。中身はbase64で持つ。
     * `pasted`なら名前を「貼り付け画像 N.png」にする (クリップボードには名前が無いため)。
     */
    async addFiles(files: File[], pastedLabel?: string) {
      for (const file of files) {
        // ファイルは知っている拡張子を優先する (WindowsはCSVを`application/vnd.ms-excel`と名乗る)。
        // 貼り付けの名前は仮のもの (「image.png」) なので、クリップボードの種類を優先する
        const mime = pastedLabel ? file.type || mimeFromName(file.name) : mimeFromName(file.name) || file.type;
        let name = file.name;
        if (pastedLabel) {
          this.pasted += 1;
          const ext = mime.split("/")[1]?.replace("jpeg", "jpg") ?? "png";
          name = `${pastedLabel} ${this.pasted}.${ext}`;
        }
        const problem = attachmentProblem(mime, file.size);
        this.push({ name, mime, size: file.size, data: problem ? undefined : await fileToBase64(file), problem });
      }
    },

    remove(id: string) {
      this.items = this.items.filter((a) => a.id !== id);
      // 消したことで5件以内に収まったものは件数オーバーを外す
      this.items.forEach((a, i) => {
        if (a.problem?.key === "chat.attach.tooMany" && i < MAX_ATTACHMENTS) a.problem = null;
      });
    },

    clear() {
      this.items = [];
    },
  },
});

/** アプリ (Tauri) で動いているか。ファイルのパスで扱えるかの判断に使う。 */
export const pathsAvailable = inTauri;
