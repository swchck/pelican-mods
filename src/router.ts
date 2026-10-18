import { createRouter, createWebHashHistory } from "vue-router";
import HomeView from "@/views/HomeView.vue";
import LibraryView from "@/views/LibraryView.vue";
import BrowseView from "@/views/BrowseView.vue";
import ConsoleView from "@/views/ConsoleView.vue";
import SettingsView from "@/views/SettingsView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", component: LibraryView },
    { path: "/browse", component: BrowseView },
    { path: "/game", component: HomeView },
    { path: "/console", component: ConsoleView },
    { path: "/settings", component: SettingsView },
  ],
});
