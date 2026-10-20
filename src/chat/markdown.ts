// 返答のMarkdownを安全なHTMLにする (設計書 4. Markdown)。
// 画面にはCSPが無いので、モデルの出力をHTMLとして動かさない: 生HTMLは文字、リンクはhttp(s)だけ、
// 画像は読み込まずリンクにする。リンクを押したときの扱い (OSのブラウザで開く) は部品側。
import MarkdownIt from "markdown-it";

/** 外部 (OSのブラウザ) で開いてよいURLか。http(s)だけ通す。 */
export function safeExternalUrl(href: string | null): string | null {
  return href && /^https?:\/\//i.test(href.trim()) ? href : null;
}

// breaks: 今までの pre-wrap 表示と同じく、1つの改行を改行として出す
const md = new MarkdownIt({ html: false, linkify: false, breaks: true });

// http(s)以外のリンクはリンクとして認めない (markdown-itは認めないリンクを文字のまま残す)
md.validateLink = (url: string) => safeExternalUrl(url) !== null;

// 画像は読み込まず、altを表示したリンクにする
md.renderer.rules.image = (tokens, idx) => {
  const token = tokens[idx];
  const src = String(token.attrGet("src") ?? "");
  const alt = md.utils.escapeHtml(token.content || src);
  return `<a href="${md.utils.escapeHtml(src)}">${alt}</a>`;
};

let copyLabel = "Copy";

// コードブロック: 言語名とコピーボタンの見出しを付ける (中身はここでエスケープする)
md.renderer.rules.fence = (tokens, idx) => {
  const token = tokens[idx];
  const lang = md.utils.escapeHtml(token.info.trim().split(/\s+/)[0] ?? "");
  const code = md.utils.escapeHtml(token.content);
  return (
    `<div class="md-code"><div class="md-code-head"><span class="md-lang">${lang}</span>` +
    `<button type="button" class="md-copy" data-copy>${md.utils.escapeHtml(copyLabel)}</button></div>` +
    `<pre><code>${code}</code></pre></div>\n`
  );
};

/** 返答の本文を安全なHTMLにする。`copyLabel`はコードブロックのコピーボタンの文字 (画面の言語に合わせる)。 */
export function renderMarkdown(text: string, options: { copyLabel: string }): string {
  copyLabel = options.copyLabel;
  return md.render(text);
}
