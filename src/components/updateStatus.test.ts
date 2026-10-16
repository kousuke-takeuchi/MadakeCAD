// アップデートの状態行の仕様テスト (.pen「自動更新デザイン」`qyYGO`)。
import { describe, expect, it } from "vitest";
import { notesExcerpt, releaseDate, updateStatusLine, type StatusInput } from "./updateStatus";

const base: StatusInput = {
  support: "supported",
  status: "idle",
  available: null,
  downloaded: 0,
  total: null,
  lastChecked: null,
  error: null,
};

describe("update status line", () => {
  // ja: 最新のときは最終確認の時刻つきで「最新です」を緑で出す
  it("says up to date in green with the time of the last check", () => {
    const line = updateStatusLine({ ...base, status: "upToDate", lastChecked: new Date(2026, 9, 16, 10, 32) });
    expect(line).toEqual({ key: "updates.status.upToDate", params: { time: "10:32" }, tone: "ok" });
  });

  // ja: 新しい版があるときは版と公開日を出し、公開日が無ければ版だけを出す
  it("names the new version with its release date, or the version alone without a date", () => {
    const available = { version: "0.3.0", date: "2026-10-20T03:00:00Z", notes: "" };
    expect(updateStatusLine({ ...base, status: "available", available })).toEqual({
      key: "updates.status.availableOn",
      params: { version: "0.3.0", date: "2026-10-20" },
      tone: "info",
    });
    expect(updateStatusLine({ ...base, status: "available", available: { ...available, date: null } }).key).toBe(
      "updates.status.available",
    );
  });

  // ja: ダウンロード中はMB単位の進み具合を出し、全体が分からなければ受け取った量だけを出す
  it("shows the download in MB, or only what arrived when the total is unknown", () => {
    expect(updateStatusLine({ ...base, status: "downloading", downloaded: 12_400_000, total: 20_000_000 }).params).toEqual({
      done: "12.4",
      total: "20.0",
    });
    expect(updateStatusLine({ ...base, status: "downloading", downloaded: 1_000_000 }).key).toBe(
      "updates.status.downloadingUnknown",
    );
  });

  // ja: 失敗は理由つきで赤く出す
  it("shows a failure in red with its reason", () => {
    expect(updateStatusLine({ ...base, status: "error", error: "offline" })).toEqual({
      key: "updates.status.error",
      params: { error: "offline" },
      tone: "err",
    });
  });

  // ja: 開発ビルドとブラウザ検証モードでは、確認しないことを淡く出す
  it("says quietly that a development build or browser mode does not check", () => {
    expect(updateStatusLine({ ...base, support: "dev" }).key).toBe("updates.status.dev");
    expect(updateStatusLine({ ...base, support: "unsupported", status: "upToDate" }).key).toBe("updates.status.unsupported");
  });

  // ja: リリースノートは空行を除いた先頭3行を抜粋し、公開日は日付の部分だけを使う
  it("excerpts the first three non-empty note lines and keeps only the date part of the release date", () => {
    expect(notesExcerpt("・図枠エディタ\n\n・盤の銘板\r\n・寸法線\n・不具合修正")).toEqual(["・図枠エディタ", "・盤の銘板", "・寸法線"]);
    expect(releaseDate("2026-10-20T03:00:00Z")).toBe("2026-10-20");
    expect(releaseDate(null)).toBe("");
  });
});
