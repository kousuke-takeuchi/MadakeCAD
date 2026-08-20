<!--
  仮のデバッグ画面。本UIはPencil(pen.dev)でのデザイン確定後に実装する。
  バックエンド(Commandエンジン+MCPサーバー)の動作確認用:
  MCP経由の編集がdoc:patchイベントでリアルタイムに反映されることを確認できる。
-->
<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface ProjectSnapshot {
  revision: number;
  project: {
    name: string;
    sheets: Array<{
      id: string;
      name: string;
      size: string;
      orientation: string;
      entities: Record<string, unknown>;
    }>;
  };
  can_undo: boolean;
  can_redo: boolean;
}

const snapshot = ref<ProjectSnapshot | null>(null);
const patchLog = ref<string[]>([]);
let unlisten: UnlistenFn | null = null;

async function refresh() {
  snapshot.value = await invoke<ProjectSnapshot>("get_project");
}

onMounted(async () => {
  await refresh();
  unlisten = await listen("doc:patch", (event) => {
    const patch = event.payload as { revision: number; ops: Array<{ op: string }> };
    patchLog.value.unshift(
      `rev ${patch.revision}: ${patch.ops.map((o) => o.op).join(", ")}`
    );
    if (patchLog.value.length > 50) patchLog.value.pop();
    refresh();
  });
});

onUnmounted(() => {
  unlisten?.();
});
</script>

<template>
  <main class="debug">
    <h1>MadakeCAD</h1>
    <p class="note">
      バックエンド動作確認画面(仮)。UIデザインはPencilで作成後に実装します。
    </p>
    <section v-if="snapshot">
      <h2>{{ snapshot.project.name }}</h2>
      <p>revision: {{ snapshot.revision }} / undo: {{ snapshot.can_undo }} / redo: {{ snapshot.can_redo }}</p>
      <ul>
        <li v-for="sheet in snapshot.project.sheets" :key="sheet.id">
          {{ sheet.name }} ({{ sheet.size }} {{ sheet.orientation }}) -
          エンティティ {{ Object.keys(sheet.entities).length }} 件
        </li>
      </ul>
    </section>
    <section>
      <h3>MCP接続</h3>
      <p><code>http://127.0.0.1:9310/mcp</code> (Claude Codeからは .mcp.json の "madakecad")</p>
      <h3>patchログ</h3>
      <ol class="log">
        <li v-for="(line, i) in patchLog" :key="i">{{ line }}</li>
      </ol>
    </section>
  </main>
</template>

<style>
body {
  margin: 0;
  font-family: "Hiragino Kaku Gothic ProN", "Noto Sans JP", sans-serif;
  background: #1e1e1e;
  color: #ddd;
}
.debug {
  padding: 2rem;
  max-width: 720px;
}
.note {
  color: #999;
}
.log {
  font-family: monospace;
  font-size: 12px;
  color: #8c8;
}
code {
  background: #333;
  padding: 2px 6px;
  border-radius: 3px;
}
</style>
