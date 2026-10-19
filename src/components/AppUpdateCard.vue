<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { SparklesIcon } from "@lucide/vue";
import { useAppUpdate } from "@/stores/appUpdate";

const { t } = useI18n();
const app = useAppUpdate();
</script>

<template>
  <button
    v-if="app.update"
    type="button"
    class="bg-gold border-wood-dark flex items-center gap-2 rounded-lg border-[3px] p-2.5 text-left shadow-[inset_0_-3px_0_var(--gold-shade)]"
    @click="app.dialog = true"
  >
    <SparklesIcon class="size-4 shrink-0" />
    <span class="flex flex-col leading-tight">
      <span class="text-[13px] font-extrabold">{{ t("appUpdate.available", { version: app.update.version }) }}</span>
      <span class="text-xs font-bold text-[#6b4a2e]">{{ t("appUpdate.details") }}</span>
    </span>
  </button>
</template>
