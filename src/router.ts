import { createRouter, createWebHashHistory } from "vue-router";
import LibraryView from "@/views/LibraryView.vue";

// the library is the start page; the rest load on first visit, so the webview parses and
// keeps only what was opened
export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", component: LibraryView },
    { path: "/browse", component: () => import("@/views/BrowseView.vue") },
    { path: "/saves", component: () => import("@/views/SavesView.vue") },
    { path: "/progress", component: () => import("@/views/ProgressView.vue") },
    { path: "/helpers", component: () => import("@/views/HelpersView.vue") },
    { path: "/config", component: () => import("@/views/ConfigView.vue") },
    { path: "/game", component: () => import("@/views/HomeView.vue") },
    { path: "/console", component: () => import("@/views/ConsoleView.vue") },
    { path: "/settings", component: () => import("@/views/SettingsView.vue") },
  ],
});
