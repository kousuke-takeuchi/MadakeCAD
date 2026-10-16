import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { i18n, setLocale } from "./i18n";
import { useSettingsStore } from "./stores/settings";
import { useUpdatesStore } from "./stores/updates";

const pinia = createPinia();
const app = createApp(App).use(pinia).use(i18n);

// UI言語は設定の`language`に従う(読込前は既定の英語で描画し、読込後に切替)。
// プロバイダの状態(APIキーが保存済みか)も一緒に読む: Anthropic APIを選んでいる
// ときの接続バッジは、claude CLIの検出ではなくこれを見る
const settings = useSettingsStore(pinia);
settings.load().then(() => {
  setLocale(settings.settings.language);
  // アプリ内アップデート: 起動して数秒後に1回だけ確認する (設定`check_updates`。失敗は黙る)
  const updates = useUpdatesStore(pinia);
  void updates.init();
  setTimeout(() => void updates.startupCheck(settings.settings.check_updates), 5000);
  return settings.loadProviderStatus();
});

app.mount("#app");
