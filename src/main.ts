import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { router } from "./router";
import { i18n, setLocale } from "./i18n";
import { api } from "./lib/api";
import "./styles/globals.css";

if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
  await import("./dev/browser-mock");
}

// a file dropped on the window would otherwise replace the whole app with it
for (const type of ["dragover", "drop"]) window.addEventListener(type, (e) => e.preventDefault());

setLocale((await api.settings()).locale);

createApp(App).use(createPinia()).use(router).use(i18n).mount("#app");
