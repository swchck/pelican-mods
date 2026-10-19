<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  GamepadIcon,
  LibraryIcon,
  SaveIcon,
  SearchIcon,
  SettingsIcon,
  SparklesIcon,
  TerminalIcon,
  TrophyIcon,
} from "@lucide/vue";
import DownloadsCard from "@/components/DownloadsCard.vue";
import AppUpdateCard from "@/components/AppUpdateCard.vue";
import AppLogo from "@/components/AppLogo.vue";
import PlayCard from "@/components/PlayCard.vue";
import { onMounted } from "vue";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const lib = useLibrary();

// the badge needs the library even before its screen opens
onMounted(() => {
  if (!lib.view) void lib.load();
});

const links = [
  { to: "/", label: "nav.library", icon: LibraryIcon },
  { to: "/browse", label: "nav.browse", icon: SearchIcon },
  { to: "/saves", label: "nav.saves", icon: SaveIcon },
  { to: "/progress", label: "nav.progress", icon: TrophyIcon },
  { to: "/helpers", label: "nav.helpers", icon: SparklesIcon },
  { to: "/game", label: "nav.home", icon: GamepadIcon },
  { to: "/console", label: "nav.console", icon: TerminalIcon },
  { to: "/settings", label: "nav.settings", icon: SettingsIcon },
];
</script>

<template>
  <nav
    :aria-label="t('nav.home')"
    class="wood-planks border-wood-dark flex w-58 shrink-0 flex-col gap-1.5 border-r-4 px-3.5 pt-12 pb-4"
    data-tauri-drag-region
  >
    <AppLogo class="px-1.5 pb-4" />
    <RouterLink
      v-for="l in links"
      :key="l.to"
      :to="l.to"
      class="flex h-10 items-center gap-2.5 rounded-md border-2 border-transparent px-3 text-[15px] font-extrabold text-[#fff3d6] transition-colors hover:border-[#fff3d6]/25 hover:bg-[#fff3d6]/10"
      active-class="!bg-card !text-foreground !border-wood-dark shadow-[inset_0_-3px_0_#e2c08a]"
    >
      <component :is="l.icon" class="size-4" />
      <span class="flex-1">{{ t(l.label) }}</span>
      <span
        v-if="l.to === '/' && lib.updatable.length"
        class="bg-gold border-wood-dark rounded border-2 px-1.5 text-xs font-extrabold text-[#4e2c14]"
        :title="t('library.status.update')"
      >
        {{ lib.updatable.length }}
      </span>
    </RouterLink>
    <div class="flex-1" data-tauri-drag-region />
    <AppUpdateCard />
    <DownloadsCard />
    <PlayCard />
  </nav>
</template>
