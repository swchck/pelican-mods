<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpenIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import AppLogo from "@/components/AppLogo.vue";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const game = useGame();

async function pick() {
  // .app bundles are packages, so the picker selects them as files
  const path = await open({
    title: t("setup.pickTitle"),
    defaultPath: "/Applications",
    filters: [{ name: "Stardew Valley", extensions: ["app"] }],
  });
  if (path) await game.choose(path);
}
</script>

<template>
  <div class="wood-planks grid h-full place-items-center p-8" data-tauri-drag-region>
    <div class="wood-panel flex w-full max-w-[560px] flex-col gap-5 p-8">
      <AppLogo class="[&_span:first-child]:text-wood-dark" />
      <div class="flex flex-col gap-1.5">
        <h1 class="font-pixel text-wood-dark text-3xl font-bold">{{ t("setup.title") }}</h1>
        <p class="text-muted-foreground font-semibold">{{ t("setup.subtitle") }}</p>
      </div>

      <div v-if="game.detected.length" class="flex flex-col gap-2.5">
        <span class="text-muted-foreground text-xs font-extrabold tracking-wider uppercase">{{
          t("setup.found")
        }}</span>
        <div
          v-for="g in game.detected"
          :key="g.path"
          class="bg-parchment border-border-soft flex items-center gap-3 rounded-md border-2 p-3"
        >
          <div class="flex min-w-0 flex-1 flex-col">
            <span class="font-extrabold">Stardew Valley {{ g.version ?? "" }} · {{ t(`store.${g.store}`) }}</span>
            <span class="text-muted-foreground truncate text-xs font-semibold" :title="g.path">{{ g.path }}</span>
          </div>
          <Button @click="game.choose(g.path)">{{ t("setup.use") }}</Button>
        </div>
      </div>
      <p v-else class="bg-warning-soft rounded-md border-2 border-[#d98b76] p-3 text-sm font-semibold">
        {{ t("setup.nothing") }}
      </p>

      <Button variant="outline" class="self-start" @click="pick"><FolderOpenIcon />{{ t("setup.pick") }}</Button>
    </div>
  </div>
</template>
