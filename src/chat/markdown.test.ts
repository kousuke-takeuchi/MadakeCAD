import { describe, expect, it } from "vitest";
import { renderMarkdown, safeExternalUrl } from "./markdown";

const md = (text: string) => renderMarkdown(text, { copyLabel: "コピー" });

describe("reply markdown", () => {
  // ja: 見出し・箇条書き・表・強調・インラインコード・引用・区切り線を描画する
  it("renders headings, lists, tables, emphasis, inline code, quotes and rules", () => {
    const html = md("## 変更\n\n- F2\n- W12\n\n| 電線 | 色 |\n|---|---|\n| W12 | 赤 |\n\n**太字** と `code`\n\n> 注意\n\n---");
    for (const tag of ["<h2>", "<ul>", "<table>", "<strong>", "<code>", "<blockquote>", "<hr>"]) {
      expect(html).toContain(tag);
    }
  });

  // ja: コードブロックには言語名とコピーボタンが付き、閉じていないコードブロックも届いた分までをコードとして描く
  it("gives code blocks a language label and a copy button, and draws an unclosed block up to what has arrived", () => {
    const html = md('```json\n{ "ref": "F2" }\n```');
    expect(html).toContain('<span class="md-lang">json</span>');
    expect(html).toContain("data-copy");
    expect(html).toContain("コピー");
    expect(md("```ts\nconst a = 1;")).toContain("const a = 1;");
  });

  // ja: 返答の生のHTMLは動かさず文字として表示する
  it("shows raw HTML in a reply as text instead of running it", () => {
    const html = md('<img src=x onerror="alert(1)"><script>alert(1)</script>');
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<script");
    expect(html).toContain("&lt;img");
  });

  // ja: リンクはhttp(s)だけをリンクにし、javascript:・file:・data:はただの文字にする
  it("keeps only http(s) links and turns javascript:, file: and data: links into plain text", () => {
    expect(md("[docs](https://example.com/a)")).toContain('href="https://example.com/a"');
    for (const bad of ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,x"]) {
      const html = md(`[x](${bad})`);
      expect(html).not.toContain("<a ");
      expect(html).not.toContain(`href="${bad}`);
    }
  });

  // ja: 画像は読み込まず、代替文字を表示したリンクにする
  it("does not load images; an image becomes a link labelled with its alt text", () => {
    const html = md("![回路図](https://example.com/track.png)");
    expect(html).not.toContain("<img");
    expect(html).toContain('href="https://example.com/track.png"');
    expect(html).toContain("回路図");
  });

  // ja: 外部で開いてよいURLはhttp(s)だけ
  it("treats only http(s) URLs as safe to open externally", () => {
    expect(safeExternalUrl("https://example.com")).toBe("https://example.com");
    expect(safeExternalUrl("HTTP://example.com")).toBe("HTTP://example.com");
    expect(safeExternalUrl("javascript:alert(1)")).toBeNull();
    expect(safeExternalUrl(null)).toBeNull();
  });
});
