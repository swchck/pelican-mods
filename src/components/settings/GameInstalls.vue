<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { CheckIcon, FolderPlusIcon, XIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const game = useGame();

onMounted(() => void game.loadInstalls());

async function add() {
  const path = await open({
    title: t("setup.pickTitle"),
    defaultPath: "/Applications",
    filters: [{ name: "Stardew Valley", extensions: ["app"] }],
  });
  if (path) await game.switchTo(path);
}
</script>

<template>
  <section class="wood-panel flex flex-col gap-3 p-5">
    <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("settings.game") }}</h2>
    <ul class="flex flex-col gap-2">
      <li
        v-for="g in game.detected"
        :key="g.path"
        class="bg-parchment flex items-center gap-3 rounded-md border-2 px-3 py-2"
        :class="g.path === game.install?.path ? 'border-[#4e2c14]' : 'border-border-soft'"
      >
        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
          <span class="flex flex-wrap items-baseline gap-x-2 text-sm font-extrabold">
            {{ t(`store.${g.store}`) }}
            <span class="text-muted-foreground font-mono text-xs">
              Stardew {{ g.version ?? "?" }} · SMAPI {{ g.smapiVersion ?? "—" }}
            </span>
          </span>
          <code class="text-muted-foreground truncate text-xs" :title="g.path">{{ g.path }}</code>
        </div>
        <span
          v-if="g.path === game.install?.path"
          class="text-success flex shrink-0 items-center gap-1 text-xs font-extrabold"
        >
          <CheckIcon class="size-3.5" />{{ t("settings.gameActive") }}
        </span>
        <template v-else>
          <Button size="sm" variant="outline" :disabled="game.running || game.busy" @click="game.switchTo(g.path)">
            {{ t("settings.gameUse") }}
          </Button>
          <Button
            size="icon-sm"
            variant="ghost"
            :aria-label="t('settings.gameForget')"
            :title="t('settings.gameForget')"
            @click="game.forget(g.path)"
          >
            <XIcon />
          </Button>
        </template>
      </li>
    </ul>
    <div class="flex flex-wrap items-center gap-3">
      <Button variant="outline" :disabled="game.running || game.busy" @click="add">
        <FolderPlusIcon />{{ t("settings.gameAdd") }}
      </Button>
      <span class="text-muted-foreground min-w-0 flex-1 text-xs font-semibold">{{ t("settings.gameHint") }}</span>
    </div>
  </section>
</template>
