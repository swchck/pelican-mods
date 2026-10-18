<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { Download } from "@/lib/api";
import { useNexus } from "@/stores/nexus";

const { t } = useI18n();
const nx = useNexus();

function pct(d: Download): number {
  if (d.state === "downloading") return d.total ? Math.round((d.received / d.total) * 100) : 0;
  return d.state === "installing" ? 100 : 0;
}
</script>

<template>
  <div
    v-if="nx.active.length"
    class="bg-card border-wood-dark flex flex-col gap-2 rounded-lg border-[3px] p-3 shadow-[inset_0_0_0_2px_#e8c48a]"
    role="status"
  >
    <span class="text-muted-foreground text-xs font-extrabold">{{ t("nexus.downloads") }}</span>
    <div v-for="d in nx.active" :key="d.id" class="flex flex-col gap-1">
      <div class="flex justify-between gap-2 text-[13px] font-bold">
        <span class="truncate">{{ d.name }}</span>
        <span class="shrink-0">{{
          d.state === "installing" ? t("nexus.installing") : d.state === "resolving" ? "…" : `${pct(d)}%`
        }}</span>
      </div>
      <div class="bg-muted h-2.5 overflow-hidden rounded-[3px] border-2 border-[#8c6e4a]">
        <div
          class="bg-success h-full transition-[width]"
          :class="{ 'animate-pulse': d.state !== 'downloading' }"
          :style="{ width: `${d.state === 'resolving' ? 100 : pct(d)}%` }"
        />
      </div>
    </div>
  </div>
</template>
