import { createRouter, createWebHashHistory } from "vue-router";
import HomeView from "@/views/HomeView.vue";
import LibraryView from "@/views/LibraryView.vue";
import BrowseView from "@/views/BrowseView.vue";
import SavesView from "@/views/SavesView.vue";
import ProgressView from "@/views/ProgressView.vue";
import HelpersView from "@/views/HelpersView.vue";
import ConfigView from "@/views/ConfigView.vue";
import ConsoleView from "@/views/ConsoleView.vue";
import SettingsView from "@/views/SettingsView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", component: LibraryView },
    { path: "/browse", component: BrowseView },
    { path: "/saves", component: SavesView },
    { path: "/progress", component: ProgressView },
    { path: "/helpers", component: HelpersView },
    { path: "/config", component: ConfigView },
    { path: "/game", component: HomeView },
    { path: "/console", component: ConsoleView },
    { path: "/settings", component: SettingsView },
  ],
});
