<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { DownloadIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const game = useGame();

// the release lookup may fail offline; without a version there is nothing concrete to offer
const open = computed({
  get: () => game.smapiPrompt !== null && game.latest !== null,
  set: (v) => {
    if (!v) game.promptDismissed = true;
  },
});

const text = computed(() =>
  game.smapiPrompt === "update"
    ? t("prompt.updateText", { current: game.install?.smapiVersion, next: game.latest?.version })
    : t("prompt.installText", { version: game.latest?.version }),
);
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent>
      <DialogHeader>
        <DialogTitle class="font-pixel text-wood-dark text-2xl">
          {{ game.smapiPrompt === "update" ? t("prompt.updateTitle") : t("prompt.installTitle") }}
        </DialogTitle>
        <DialogDescription class="text-foreground text-sm leading-relaxed">{{ text }}</DialogDescription>
      </DialogHeader>
      <DialogFooter>
        <Button variant="outline" @click="open = false">{{ t("prompt.later") }}</Button>
        <Button @click="game.smapi('install')">
          <DownloadIcon />{{ game.smapiPrompt === "update" ? t("prompt.update") : t("prompt.install") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
