<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { formatCombo } from "@/lib/shortcuts";
import { SHORTCUTS, isMac, useShortcuts } from "@/stores/shortcuts";

const { t } = useI18n();
const store = useShortcuts();
const groups = ["nav", "game", "page", "list", "help"] as const;
</script>

<template>
  <Dialog v-model:open="store.help">
    <DialogContent class="sm:max-w-xl">
      <DialogHeader>
        <DialogTitle class="font-pixel text-wood-dark text-2xl">{{ t("shortcuts.title") }}</DialogTitle>
        <DialogDescription>{{ t("shortcuts.about") }}</DialogDescription>
      </DialogHeader>
      <div class="grid max-h-[60vh] grid-cols-2 gap-x-8 gap-y-4 overflow-auto">
        <section v-for="g in groups" :key="g" class="flex flex-col gap-1.5">
          <h3 class="text-sm font-extrabold text-[#7a3e12]">{{ t(`shortcuts.group.${g}`) }}</h3>
          <div
            v-for="s in SHORTCUTS.filter((x) => x.group === g)"
            :key="s.id"
            class="flex items-center justify-between gap-3 text-[13px]"
          >
            <span>{{ t(`shortcuts.action.${s.id.replace(".", "_")}`) }}</span>
            <span class="flex gap-1">
              <kbd
                v-for="c in s.combos"
                :key="c"
                class="bg-parchment border-wood-dark rounded border-2 px-1.5 font-mono text-xs font-bold shadow-[inset_0_-2px_0_#e2c08a]"
                >{{ formatCombo(c, isMac) }}</kbd
              >
            </span>
          </div>
        </section>
      </div>
    </DialogContent>
  </Dialog>
</template>
