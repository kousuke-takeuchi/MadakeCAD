// ポップアップ(モデルピッカー / 会話履歴 / コンテキスト追加)の共通開閉ロジック。
//
// 閉じる操作は2経路:
//  - 外側クリック: 呼び出し側が `<div class="backdrop" @click="popover.close()" />` を
//    トリガーの手前に描いて拾う。documentのclick購読にするとトリガー自身のクリックが
//    伝播して「開いた直後に閉じる」ため、あえてbackdrop方式で統一している。
//  - Esc: keydownの購読はこのcomposableが持つ(開いている間だけ張る)。

import { onBeforeUnmount, ref, watch } from "vue";

export interface PopoverOptions {
  /** 開いた直後に呼ばれる(検索欄へのフォーカス等)。 */
  onOpen?: () => void | Promise<void>;
  /** 閉じた直後に呼ばれる(検索語のクリア等)。 */
  onClose?: () => void;
}

export function usePopover(options: PopoverOptions = {}) {
  const open = ref(false);

  function close() {
    open.value = false;
  }

  function toggle() {
    open.value = !open.value;
  }

  function onWindowKeydown(ev: KeyboardEvent) {
    if (ev.key !== "Escape") return;
    // Escはポップアップを閉じるだけで消費する。バブリングまで流すと
    // EditorLayoutのwindowリスナ経由でキャンバスツールのEsc処理
    // (作画キャンセル/ツールリセット)まで同時に発火してしまう
    ev.preventDefault();
    ev.stopPropagation();
    close();
  }

  watch(open, (isOpen) => {
    if (isOpen) {
      window.addEventListener("keydown", onWindowKeydown, { capture: true });
      void options.onOpen?.();
    } else {
      window.removeEventListener("keydown", onWindowKeydown, { capture: true });
      options.onClose?.();
    }
  });

  // 開いたままアンマウントされてもリスナを残さない
  onBeforeUnmount(() => window.removeEventListener("keydown", onWindowKeydown, { capture: true }));

  return { open, toggle, close };
}
