// 図枠のロゴ画像 (テンプレートの `logo.data_url`) の読み込みキャッシュ。
//
// レンダラ (renderer.ts) はctxとviewportだけを受け取る純関数なので、画像の読み込み
// (非同期・DOM依存) はここへ分ける。レンダラは `frameImage()` で「いま描ける画像」を
// 聞くだけで、まだ読めていなければ置き場所の枠を描く。読み込みが終わったら
// `onFrameImageLoaded()` に渡された関数 (キャンバスの再描画) を呼ぶ。

type Entry = { image: HTMLImageElement; ready: boolean };

const cache = new Map<string, Entry>();
let loaded: () => void = () => {};

/** 画像の読み込みが終わったときに呼ぶ関数 (キャンバスの再描画) を登録する。 */
export function onFrameImageLoaded(callback: () => void): void {
  loaded = callback;
}

/**
 * data URIの画像を返す。まだ読めていない・読めなかった・画像を作れない環境ではnull。
 * 初めて聞かれたURLは読み込みを始める (同じURLを2回読み込まない)。
 */
export function frameImage(dataUrl: string): HTMLImageElement | null {
  const hit = cache.get(dataUrl);
  if (hit) return hit.ready ? hit.image : null;
  if (typeof Image === "undefined") return null;
  const entry: Entry = { image: new Image(), ready: false };
  cache.set(dataUrl, entry);
  entry.image.onload = () => {
    entry.ready = true;
    loaded();
  };
  // 失敗した画像はキャッシュに残す (描くたびに読み直さない)。枠だけが描かれ続ける
  entry.image.onerror = () => {};
  entry.image.src = dataUrl;
  return null;
}

/** キャッシュと登録した関数を消す (テスト用)。 */
export function resetFrameImages(): void {
  cache.clear();
  loaded = () => {};
}
