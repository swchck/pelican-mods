import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { router } from "./router";
import { i18n, setGameLanguage, setLocale } from "./i18n";
import { setWikiSource } from "./lib/wiki";
import { api } from "./lib/api";
import "./styles/globals.css";

if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  await import("./dev/browser-mock");
}

// a file dropped on the window would otherwise replace the whole app with it
for (const type of ["dragover", "drop"]) window.addEventListener(type, (e) => e.preventDefault());

const [settings, gameLanguage] = await Promise.all([api.settings(), api.gameLanguage().catch(() => null)]);
setGameLanguage(gameLanguage);
setLocale(settings.locale);
setWikiSource(settings.wiki);

createApp(App).use(createPinia()).use(router).use(i18n).mount("#app");
