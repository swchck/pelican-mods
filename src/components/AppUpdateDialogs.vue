<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2Icon, SparklesIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import ReleaseNotes from "@/components/ReleaseNotes.vue";
import { useAppUpdate } from "@/stores/appUpdate";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const app = useAppUpdate();
const game = useGame();

const whatsNewOpen = computed({
  get: () => app.whatsNew.length > 0,
  set: (v) => {
    if (!v) app.whatsNew = [];
  },
});
</script>

<template>
  <Dialog v-model:open="app.dialog">
    <DialogContent
      v-if="app.update"
      class="sm:max-w-lg"
      @interact-outside="app.installing && $event.preventDefault()"
      @escape-key-down="app.installing && $event.preventDefault()"
    >
      <DialogHeader>
        <DialogTitle class="font-pixel text-wood-dark text-2xl">
          {{ t("appUpdate.title", { version: app.update.version }) }}
        </DialogTitle>
        <DialogDescription>{{ t("appUpdate.current", { version: app.current }) }}</DialogDescription>
      </DialogHeader>
      <div class="bg-parchment border-border-soft max-h-80 overflow-auto rounded-md border-2 p-3">
        <ReleaseNotes v-if="app.notes.length" :releases="app.notes" />
        <p v-else class="text-muted-foreground text-sm">{{ t("appUpdate.noNotes") }}</p>
      </div>
      <p v-if="game.running" class="text-destructive text-sm font-bold">{{ t("appUpdate.gameRunning") }}</p>
      <div v-if="app.installing" class="flex flex-col gap-1.5" role="status">
        <div class="bg-muted h-3 overflow-hidden rounded-[3px] border-2 border-[#8c6e4a]">
          <div
            class="bg-success h-full transition-[width]"
            :class="{ 'w-1/3 animate-pulse': app.percent === null }"
            :style="app.percent === null ? undefined : { width: `${app.percent}%` }"
          />
        </div>
        <span class="text-muted-foreground text-xs font-bold">
          {{ app.percent === null || app.percent < 100 ? t("appUpdate.downloading") : t("appUpdate.restarting") }}
        </span>
      </div>
      <DialogFooter>
        <Button variant="ghost" :disabled="app.installing" @click="app.dialog = false">{{ t("prompt.later") }}</Button>
        <Button :disabled="app.installing || game.running" @click="app.install">
          <Loader2Icon v-if="app.installing" class="animate-spin" />{{ t("appUpdate.install") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog v-model:open="whatsNewOpen">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle class="font-pixel text-wood-dark flex items-center gap-2 text-2xl">
          <SparklesIcon class="text-gold size-6" />{{ t("appUpdate.whatsNew", { version: app.current }) }}
        </DialogTitle>
        <DialogDescription>{{ t("appUpdate.whatsNewText") }}</DialogDescription>
      </DialogHeader>
      <div class="bg-parchment border-border-soft max-h-96 overflow-auto rounded-md border-2 p-3">
        <ReleaseNotes :releases="app.whatsNew" />
      </div>
      <DialogFooter>
        <Button @click="whatsNewOpen = false">{{ t("appUpdate.great") }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
