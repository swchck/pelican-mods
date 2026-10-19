<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { Undo2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { library, type HistoryEntry } from "@/lib/api";
import { relativeTime } from "@/lib/time";
import { errorText } from "@/i18n";
import { useGame } from "@/stores/game";
import { useLibrary } from "@/stores/library";

const open = defineModel<boolean>("open", { required: true });
const { t, locale } = useI18n();
const lib = useLibrary();
const game = useGame();
const entries = ref<HistoryEntry[]>([]);
const busy = ref(false);

watch(open, async (on) => {
  if (on) entries.value = await library.history().catch(() => []);
});

async function undo(e: HistoryEntry) {
  busy.value = true;
  try {
    lib.view = await library.undo(e.at);
    entries.value = await library.history();
    toast.success(t("history.undone", { name: e.name }));
  } catch (err) {
    toast.error(errorText(err));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-h-[85vh] max-w-xl overflow-auto">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("history.title") }}</DialogTitle>
        <DialogDescription>{{ t("history.hint") }}</DialogDescription>
      </DialogHeader>
      <p v-if="!entries.length" class="text-muted-foreground text-sm font-semibold">{{ t("history.empty") }}</p>
      <ul class="flex flex-col gap-1.5">
        <li
          v-for="e in entries"
          :key="e.at"
          class="bg-parchment border-border-soft flex items-center gap-2.5 rounded-md border-2 px-2.5 py-2 text-sm"
          :class="{ 'opacity-60': e.undone }"
        >
          <div class="flex min-w-0 flex-1 flex-col">
            <span class="truncate font-extrabold">{{ e.name }}</span>
            <span class="text-muted-foreground text-xs font-semibold">
              {{ t(`history.kind.${e.kind}`, { version: e.version, previous: e.previous ?? "?" }) }} ·
              {{ relativeTime(new Date(e.at), new Date(), locale) }}
            </span>
          </div>
          <span v-if="e.undone" class="text-muted-foreground text-xs font-bold">{{ t("history.wasUndone") }}</span>
          <Button v-else size="sm" variant="outline" :disabled="busy || game.running" @click="undo(e)">
            <Undo2Icon />{{ t("history.undo") }}
          </Button>
        </li>
      </ul>
    </DialogContent>
  </Dialog>
</template>
