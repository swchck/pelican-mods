<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { DownloadIcon, ExternalLinkIcon } from "@lucide/vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "@/components/ui/button";
import SmapiProgress from "@/components/SmapiProgress.vue";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const game = useGame();
</script>

<template>
  <section class="wood-panel flex flex-col gap-3 p-5" :aria-label="t('home.smapi')">
    <div class="flex items-center gap-3">
      <h2 class="font-pixel text-wood-dark flex-1 text-xl font-bold">{{ t("home.smapi") }}</h2>
      <span
        v-if="game.install?.smapiVersion"
        class="bg-success-soft rounded border-2 border-[#9cc07a] px-2 py-0.5 text-xs font-extrabold text-[#2e5a1e]"
      >
        {{ t("home.smapiInstalled", { version: game.install.smapiVersion }) }}
      </span>
      <span
        v-if="game.smapiUpdate"
        class="bg-gold rounded border-2 border-[#b57f17] px-2 py-0.5 text-xs font-extrabold text-[#4e2c14]"
      >
        {{ t("home.smapiUpdate", { version: game.smapiUpdate }) }}
      </span>
    </div>

    <p v-if="!game.install?.smapiVersion" class="text-sm leading-relaxed">{{ t("home.smapiAbout") }}</p>

    <SmapiProgress v-if="game.progress" :progress="game.progress" />
    <div v-else-if="!game.install?.smapiVersion || game.smapiUpdate" class="flex items-center gap-3">
      <Button size="lg" :disabled="game.running || game.install?.needsRosetta" @click="game.smapi('install')">
        <DownloadIcon />
        {{ game.smapiUpdate ? t("home.update", { version: game.smapiUpdate }) : t("home.install") }}
      </Button>
      <span v-if="game.latest && !game.smapiUpdate" class="text-muted-foreground text-sm font-semibold">
        {{ t("home.smapiLatest") }}: {{ game.latest.version }}
      </span>
      <Button v-if="game.latest" variant="link" size="sm" @click="openUrl(game.latest.notesUrl)">
        <ExternalLinkIcon />GitHub
      </Button>
    </div>
  </section>
</template>
