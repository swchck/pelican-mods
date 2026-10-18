<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { SmapiProgress } from "@/lib/api";

const props = defineProps<{ progress: SmapiProgress }>();
const { t } = useI18n();

const mb = (n: number) => `${(n / 1024 / 1024).toFixed(1)} MB`;

const view = computed(() => {
  const p = props.progress;
  if (p.stage === "download") {
    return {
      text: t("smapi.download", { received: mb(p.received), total: p.total ? mb(p.total) : "?" }),
      pct: p.total ? (p.received / p.total) * 100 : 0,
    };
  }
  return { text: t(`smapi.${p.stage}`), pct: 100 };
});
</script>

<template>
  <div class="flex flex-col gap-1.5" role="status">
    <span class="text-sm font-bold">{{ view.text }}</span>
    <div class="bg-muted h-3 overflow-hidden rounded-[3px] border-2 border-[#8c6e4a]">
      <div
        class="bg-success h-full transition-[width]"
        :class="{ 'animate-pulse': progress.stage !== 'download' }"
        :style="{ width: `${view.pct}%` }"
      />
    </div>
  </div>
</template>
