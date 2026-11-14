// 図枠のロゴ画像 (data URI) の読み込みキャッシュ。
import { afterEach, describe, expect, it, vi } from "vitest";

import { frameImage, onFrameImageLoaded, resetFrameImages } from "./frameImages";

/** ブラウザのImageの代役。srcを入れたあと、テストが`onload`/`onerror`を呼んで読み込みを終わらせる。 */
class FakeImage {
  static made: FakeImage[] = [];
  src = "";
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  constructor() {
    FakeImage.made.push(this);
  }
}

afterEach(() => {
  vi.unstubAllGlobals();
  resetFrameImages();
  FakeImage.made = [];
});

describe("frame logo images", () => {
  // ja: 画像を作れない環境(テストやサーバー側)では、画像は無いものとして扱い、例外を出さない
  it("returns no image where images cannot be created", () => {
    vi.stubGlobal("Image", undefined);
    expect(frameImage("data:image/png;base64,AAAA")).toBeNull();
  });

  // ja: 初めての画像は読み込みを始めてnullを返し、読み込みが終わると再描画を頼み、次からはその画像を返す
  it("starts loading on first use, asks for a redraw when loaded, then returns the image", () => {
    vi.stubGlobal("Image", FakeImage);
    const redraw = vi.fn();
    onFrameImageLoaded(redraw);
    expect(frameImage("data:image/png;base64,AAAA")).toBeNull();
    expect(FakeImage.made).toHaveLength(1);
    expect(FakeImage.made[0].src).toBe("data:image/png;base64,AAAA");
    // 読み込み中にもう一度聞かれても、2つ目は作らない
    expect(frameImage("data:image/png;base64,AAAA")).toBeNull();
    expect(FakeImage.made).toHaveLength(1);
    FakeImage.made[0].onload!();
    expect(redraw).toHaveBeenCalledTimes(1);
    expect(frameImage("data:image/png;base64,AAAA")).toBe(FakeImage.made[0]);
  });

  // ja: 読み込みに失敗した画像は、その後もnullのままで、再描画も読み直しも起きない
  it("keeps returning no image for one that failed to load", () => {
    vi.stubGlobal("Image", FakeImage);
    const redraw = vi.fn();
    onFrameImageLoaded(redraw);
    expect(frameImage("data:image/png;base64,BAD")).toBeNull();
    FakeImage.made[0].onerror!();
    expect(frameImage("data:image/png;base64,BAD")).toBeNull();
    expect(FakeImage.made).toHaveLength(1);
    expect(redraw).not.toHaveBeenCalled();
  });
});
