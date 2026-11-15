// 参照記号・型番の置き場所 (Rust 側 caption.rs と同じ規則)
import { describe, expect, it } from "vitest";

import { captionLayoutOf, parseCaptionLayout } from "./caption";

describe("caption layout", () => {
  // ja: caption_layout の無い (または空の) シンボルには置き場所の指定が無い
  it("has no layout without the attribute", () => {
    expect(parseCaptionLayout({})).toBeNull();
    expect(parseCaptionLayout({ caption_layout: "  " })).toBeNull();
    expect(parseCaptionLayout(null)).toBeNull();
  });

  // ja: 行の指定は、配置点からのずれ・文字サイズ・寄せ・非表示を持つ。省いたものは既定になる (ずれ0、いつもの大きさ、中央寄せ、表示)
  it("reads offset, size, alignment and hide, with defaults for what is left out", () => {
    const layout = parseCaptionLayout({
      caption_layout: '{"ref":{"x":-3,"y":-4.5,"h":2.0,"anchor":"start"},"value":{"hide":true}}',
    });
    expect(layout?.ref).toEqual({ x: -3, y: -4.5, h: 2, anchor: "start", hide: false });
    expect(layout?.value).toEqual({ x: 0, y: 0, h: null, anchor: "middle", hide: true });
    expect(parseCaptionLayout({ caption_layout: '{"value":{"y":6}}' })?.ref).toBeNull();
  });

  // ja: 壊れた JSON・知らないキー・0.5〜20mm の外の文字サイズ・数でないずれは断り、メッセージに属性名を出す。描画は壊れた属性を無いものとして扱う
  it("refuses bad layouts with the attribute name in the message", () => {
    for (const bad of [
      "{",
      '{"ref":{"x":1,"size":2}}',
      '{"title":{"x":1}}',
      '{"ref":{"h":0.2}}',
      '{"value":{"h":25}}',
      '{"ref":{"anchor":"left"}}',
      '{"ref":{"x":"a"}}',
    ]) {
      expect(() => parseCaptionLayout({ caption_layout: bad }), bad).toThrow(/caption_layout/);
      expect(captionLayoutOf({ caption_layout: bad }), bad).toBeNull();
    }
  });
});
