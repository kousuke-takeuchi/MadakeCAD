// アップデートの状態行 (.pen「自動更新デザイン」`qyYGO`の「状態」欄と通知カード) の文言を決める。
// 文言そのものはUIカタログ (`updates.status.*`) にあり、ここはキー・差し込み値・色調だけを返す。

import type { UpdateSupport } from "../ipc";
import type { AvailableUpdate, UpdateStatus } from "../stores/updates";

export type StatusTone = "muted" | "ok" | "info" | "err";

export interface StatusLine {
  key: string;
  params: Record<string, string>;
  tone: StatusTone;
}

export interface StatusInput {
  support: UpdateSupport | null;
  status: UpdateStatus;
  available: AvailableUpdate | null;
  downloaded: number;
  total: number | null;
  lastChecked: Date | null;
  error: string | null;
}

const mb = (bytes: number) => (bytes / 1_000_000).toFixed(1);
const hhmm = (d: Date) => `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;

/** 公開日の表示 (latest.jsonの`pub_date`の日付部分。無ければ空)。 */
export function releaseDate(date: string | null | undefined): string {
  return date ? date.slice(0, 10) : "";
}

/** 状態行のキー・差し込み値・色調。 */
export function updateStatusLine(s: StatusInput): StatusLine {
  if (s.support === "dev") return { key: "updates.status.dev", params: {}, tone: "muted" };
  if (s.support === "unsupported") return { key: "updates.status.unsupported", params: {}, tone: "muted" };
  switch (s.status) {
    case "checking":
      return { key: "updates.status.checking", params: {}, tone: "muted" };
    case "upToDate":
      return { key: "updates.status.upToDate", params: { time: s.lastChecked ? hhmm(s.lastChecked) : "" }, tone: "ok" };
    case "available": {
      const date = releaseDate(s.available?.date);
      return {
        key: date ? "updates.status.availableOn" : "updates.status.available",
        params: { version: s.available?.version ?? "", date },
        tone: "info",
      };
    }
    case "downloading":
      return s.total
        ? { key: "updates.status.downloading", params: { done: mb(s.downloaded), total: mb(s.total) }, tone: "info" }
        : { key: "updates.status.downloadingUnknown", params: { done: mb(s.downloaded) }, tone: "info" };
    case "installing":
      return { key: "updates.status.installing", params: {}, tone: "info" };
    case "error":
      return { key: "updates.status.error", params: { error: s.error ?? "" }, tone: "err" };
    default:
      return { key: "updates.status.idle", params: {}, tone: "muted" };
  }
}

/** リリースノートの抜粋 (先頭の空でない行を最大`lines`行)。 */
export function notesExcerpt(notes: string, lines = 3): string[] {
  return notes
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l !== "")
    .slice(0, lines);
}
