<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { HouseIcon, SettingsIcon, TerminalIcon } from "@lucide/vue";
import AppLogo from "@/components/AppLogo.vue";
import PlayCard from "@/components/PlayCard.vue";

const { t } = useI18n();

const links = [
  { to: "/", label: "nav.home", icon: HouseIcon },
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
      <component :is="l.icon" class="size-4" />{{ t(l.label) }}
    </RouterLink>
    <div class="flex-1" data-tauri-drag-region />
    <PlayCard />
  </nav>
</template>
