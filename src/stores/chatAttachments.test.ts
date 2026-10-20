// 入力欄の送信前の添付 (.pen「添付デザイン - エージェントへ画像・PDFを添付」`s8nJcQ`) の仕様テスト。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { invoke } from "@tauri-apps/api/core";
import { pastedImages, draggingFiles } from "../composables/chatDrop";
import {
  CSV_MIME,
  MAX_ATTACHMENTS,
  XLSX_MIME,
  attachmentProblem,
  formatSize,
  mimeFromName,
  providerProblem,
  useChatAttachmentsStore,
} from "./chatAttachments";
import { useSettingsStore } from "./settings";

vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const file = (name: string, type: string, bytes = [1, 2, 3]) => new File([new Uint8Array(bytes)], name, { type });

describe("pending chat attachments", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  // ja: 画像 (PNG/JPEG/GIF/WebP)・PDF・Excel (.xlsx)・CSVだけを添付でき、種類は拡張子でも判断する (大文字小文字は問わない)
  it("accepts images, PDFs, Excel workbooks and CSV files only, judging the kind by extension too", () => {
    expect(mimeFromName("回路写真.JPG")).toBe("image/jpeg");
    expect(mimeFromName("sheet.pdf")).toBe("application/pdf");
    expect(mimeFromName("AOGU_COCO_BOM.XLSX")).toBe(XLSX_MIME);
    expect(mimeFromName("部品.csv")).toBe(CSV_MIME);
    expect(mimeFromName("notes.txt")).toBe("");
    expect(mimeFromName("old.xls")).toBe("");
    expect(attachmentProblem("image/png", 10)).toBeNull();
    expect(attachmentProblem(XLSX_MIME, 10)).toBeNull();
    expect(attachmentProblem(CSV_MIME, 10)).toBeNull();
    expect(attachmentProblem("text/plain", 10)?.key).toBe("chat.attach.unsupported");
  });

  // ja: ブラウザが別の種類を名乗っても、知っている拡張子なら拡張子で決める (WindowsはCSVをExcelの種類と名乗る)
  it("trusts a known extension over the type the browser reports, since Windows labels CSV files as Excel", async () => {
    const store = useChatAttachmentsStore();
    await store.addFiles([file("部品.csv", "application/vnd.ms-excel", [0x61])]);
    expect(store.items[0]).toMatchObject({ mime: CSV_MIME, problem: null, data: "YQ==" });
  });

  // ja: 表計算はPDFを読めないOpenAI互換のモデルにも送れる (テキストにして渡すため)
  it("lets spreadsheets go to OpenAI-compatible models, which cannot read PDFs, because they are sent as text", () => {
    const sheet = { id: "1", name: "BOM.xlsx", mime: XLSX_MIME, size: 1, problem: null };
    expect(providerProblem([sheet], "openai_compat")).toBeNull();
    expect(providerProblem([{ ...sheet, mime: "application/pdf" }], "openai_compat")?.key).toBe("chat.attach.pdfUnsupported");
  });

  // ja: 20MBを超えるファイルは大きさつきで「大きすぎます」になり、大きさはMB/KBで表示する
  it("marks files over 20 MB as too large and shows sizes in MB or KB", () => {
    const problem = attachmentProblem("application/pdf", 25 * 1024 * 1024);
    expect(problem).toEqual({ key: "chat.attach.tooLarge", params: { size: "25.0 MB", max: 20 } });
    expect(formatSize(1_258_291)).toBe("1.2 MB");
    expect(formatSize(340 * 1024)).toBe("340 KB");
  });

  // ja: 貼り付け・ブラウザのファイルは中身をbase64で持ち、送信する添付と発言に出す情報になる
  it("keeps pasted or browser files as base64 and turns them into outgoing attachments", async () => {
    const store = useChatAttachmentsStore();
    await store.addFiles([file("a.png", "image/png")]);
    expect(store.outgoing).toEqual({
      inputs: [{ name: "a.png", mime: "image/png", data: "AQID" }],
      metas: [{ name: "a.png", mime: "image/png", size: 3 }],
    });
  });

  // ja: ⌘Vの画像には「貼り付け画像 N.png」の名前を順に付ける (クリップボードに名前が無いため)
  it("names pasted images in order since the clipboard has no file name", async () => {
    const store = useChatAttachmentsStore();
    await store.addFiles([file("image.png", "image/png")], "貼り付け画像");
    await store.addFiles([file("image.png", "image/jpeg")], "貼り付け画像");
    expect(store.items.map((a) => a.name)).toEqual(["貼り付け画像 1.png", "貼り付け画像 2.jpg"]);
  });

  // ja: アプリで選んだ・落としたファイルはパスで持ち、名前・種類・大きさと読めない理由はRustの下見から取る
  it("keeps picked or dropped files as paths with the preview from Rust", async () => {
    vi.mocked(invoke).mockResolvedValue([
      { path: "/tmp/sheet.pdf", name: "sheet.pdf", mime: "application/pdf", size: 2048, error: null },
      { path: "/tmp/memo.txt", name: "memo.txt", mime: "", size: 1, error: "memo.txt: 画像かPDFだけ" },
    ]);
    const store = useChatAttachmentsStore();
    await store.addPaths(["/tmp/sheet.pdf", "/tmp/memo.txt"]);
    expect(invoke).toHaveBeenCalledWith("agent_inspect_attachments", { paths: ["/tmp/sheet.pdf", "/tmp/memo.txt"] });
    expect(store.items[0]).toMatchObject({ path: "/tmp/sheet.pdf", problem: null });
    expect(store.items[1].problem?.key).toBe("chat.attach.cannotRead");
    expect(store.outgoing.inputs).toEqual([{ name: "sheet.pdf", mime: "application/pdf", path: "/tmp/sheet.pdf" }]);
  });

  // ja: 6件目以降は件数オーバーで送れない状態になり、1件外すと5件以内に戻ったものは送れるようになる
  it("blocks the sixth attachment and unblocks it once one is removed", async () => {
    const store = useChatAttachmentsStore();
    await store.addFiles(Array.from({ length: MAX_ATTACHMENTS + 1 }, (_, i) => file(`${i}.png`, "image/png")));
    expect(store.items[MAX_ATTACHMENTS].problem?.key).toBe("chat.attach.tooMany");
    expect(store.blocking).toBe(true);
    store.remove(store.items[0].id);
    expect(store.items.every((a) => a.problem === null)).toBe(true);
    expect(store.blocking).toBe(false);
  });

  // ja: OpenAI互換のモデルを選んでいるとPDFは送れない状態になり (注意帯)、他のプロバイダでは送れる
  it("blocks a PDF for an OpenAI-compatible model but not for other providers", async () => {
    const store = useChatAttachmentsStore();
    await store.addFiles([file("sheet.pdf", "application/pdf")]);
    expect(store.blocking).toBe(false);
    useSettingsStore().settings.provider = "openai_compat";
    expect(store.providerProblem?.key).toBe("chat.attach.pdfUnsupported");
    expect(store.blocking).toBe(true);
  });

  // ja: 送れない添付は送信する添付に入れない
  it("leaves attachments with a problem out of what is sent", async () => {
    const store = useChatAttachmentsStore();
    await store.addFiles([file("ok.png", "image/png"), file("x.zip", "application/zip")]);
    expect(store.outgoing.metas.map((m) => m.name)).toEqual(["ok.png"]);
  });

  // ja: 貼り付けで添付するのは画像だけで、ドラッグはファイルを含むときだけ受ける
  it("takes only images from a paste and reacts only to drags that carry files", () => {
    expect(pastedImages([file("a.png", "image/png"), file("b.pdf", "application/pdf")]).map((f) => f.name)).toEqual(["a.png"]);
    expect(draggingFiles(["Files"])).toBe(true);
    expect(draggingFiles(["text/plain"])).toBe(false);
    expect(draggingFiles(undefined)).toBe(false);
  });
});
