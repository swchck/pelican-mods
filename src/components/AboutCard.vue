<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { HistoryIcon, KeyboardIcon, Loader2Icon, RefreshCwIcon } from "@lucide/vue";
import { hint, useShortcuts } from "@/stores/shortcuts";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import ReleaseNotes from "@/components/ReleaseNotes.vue";
import { bundledChangelog, useAppUpdate } from "@/stores/appUpdate";
import { compareVersions } from "@/lib/version";

const { t } = useI18n();
const app = useAppUpdate();
const history = ref(false);
const shortcuts = useShortcuts();
// the changelog in the repo can already describe the next version; show up to this one
const releases = computed(() =>
  history.value && app.current ? bundledChangelog().filter((r) => compareVersions(r.version, app.current!) <= 0) : [],
);
</script>

<template>
  <section class="wood-panel flex flex-col gap-3 p-5">
    <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("appUpdate.about") }}</h2>
    <div class="flex flex-wrap items-center gap-2">
      <span class="flex-1 text-sm font-bold">
        Pelican Mods {{ app.current }}
        <span
          v-if="app.update"
          class="bg-gold ml-1 rounded border-2 border-[#b57f17] px-1.5 py-0.5 text-xs font-extrabold"
        >
          {{ t("appUpdate.available", { version: app.update.version }) }}
        </span>
      </span>
      <Button v-if="app.update" @click="app.dialog = true">{{ t("appUpdate.details") }}</Button>
      <Button variant="outline" :disabled="app.checking" @click="app.check">
        <Loader2Icon v-if="app.checking" class="animate-spin" /><RefreshCwIcon v-else />{{ t("appUpdate.check") }}
      </Button>
      <Button variant="outline" @click="history = true"><HistoryIcon />{{ t("appUpdate.history") }}</Button>
      <Button variant="outline" :title="hint('help')" @click="shortcuts.help = true">
        <KeyboardIcon />{{ t("shortcuts.title") }}
      </Button>
    </div>
    <Dialog v-model:open="history">
      <DialogContent class="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle class="font-pixel text-wood-dark text-2xl">{{ t("appUpdate.history") }}</DialogTitle>
        </DialogHeader>
        <div class="bg-parchment border-border-soft max-h-[60vh] overflow-auto rounded-md border-2 p-3">
          <ReleaseNotes :releases="releases" />
        </div>
      </DialogContent>
    </Dialog>
  </section>
</template>
