import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { i18n, setLocale } from "./i18n";
import { useSettingsStore } from "./stores/settings";

const pinia = createPinia();
const app = createApp(App).use(pinia).use(i18n);

// UI言語は設定の`language`に従う(読込前は既定の英語で描画し、読込後に切替)。
// プロバイダの状態(APIキーが保存済みか)も一緒に読む: Anthropic APIを選んでいる
// ときの接続バッジは、claude CLIの検出ではなくこれを見る
const settings = useSettingsStore(pinia);
settings.load().then(() => {
  setLocale(settings.settings.language);
  return settings.loadProviderStatus();
});

app.mount("#app");
