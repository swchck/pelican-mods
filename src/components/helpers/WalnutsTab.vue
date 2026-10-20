<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CircleCheckIcon, CircleIcon } from "@lucide/vue";
import { saves, type WalnutSource, type Walnuts } from "@/lib/api";
import { useFarmDate } from "@/lib/farm-date";

const { t, te } = useI18n();
const date = useFarmDate();
const walnuts = ref<Walnuts | null>(null);
const onlyMissing = ref(true);

watch(
  () => date.value?.saveId,
  async (id) => {
    walnuts.value = id ? await saves.walnuts(id).catch(() => null) : null;
  },
  { immediate: true },
);

const areaName = (a: string) => (te(`walnutsTab.areas.${a}`) ? t(`walnutsTab.areas.${a}`) : a);

function label(s: WalnutSource): string {
  if (s.tile) return t(`walnutsTab.${s.kind}`, { x: s.tile[0], y: s.tile[1] });
  return te(`walnutsTab.sources.${s.key}`) ? t(`walnutsTab.sources.${s.key}`) : s.key;
}

const areas = computed(() => {
  const byArea = new Map<string, { area: string; have: number; max: number; sources: WalnutSource[] }>();
  for (const s of walnuts.value?.sources ?? []) {
    const a = byArea.get(s.area) ?? { area: s.area, have: 0, max: 0, sources: [] };
    a.have += s.have;
    a.max += s.max;
    if (!onlyMissing.value || s.have < s.max) a.sources.push(s);
    byArea.set(s.area, a);
  }
  return [...byArea.values()].filter((a) => a.sources.length);
});
</script>

<template>
  <div class="flex flex-col gap-3">
    <p v-if="!date?.saveId" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ t("museumTab.noSave") }}
    </p>
    <template v-else-if="walnuts">
      <div class="flex flex-wrap items-center gap-3">
        <span class="font-extrabold">{{
          t("walnutsTab.summary", { found: walnuts.found, total: walnuts.total })
        }}</span>
        <div class="bg-muted h-2.5 w-48 overflow-hidden rounded-full border border-[#a0703f]">
          <div class="h-full bg-[#d9a520]" :style="{ width: `${(100 * walnuts.found) / walnuts.total}%` }" />
        </div>
        <label class="flex items-center gap-2 text-xs font-semibold">
          <input v-model="onlyMissing" type="checkbox" />{{ t("walnutsTab.onlyMissing") }}
        </label>
      </div>
      <p class="text-muted-foreground text-xs font-semibold">{{ t("walnutsTab.note") }}</p>
      <p v-if="!areas.length" class="text-muted-foreground text-sm font-semibold">{{ t("walnutsTab.allFound") }}</p>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3">
        <section v-for="a in areas" :key="a.area" class="wood-panel flex flex-col gap-1 p-3 text-sm">
          <div class="flex items-baseline gap-2">
            <span class="min-w-0 flex-1 truncate font-extrabold">{{ areaName(a.area) }}</span>
            <span class="font-mono text-xs font-bold">{{ a.have }}/{{ a.max }}</span>
          </div>
          <span
            v-for="s in a.sources"
            :key="s.key"
            class="flex items-center gap-1.5 text-xs font-semibold"
            :class="{ 'text-muted-foreground': s.have >= s.max }"
          >
            <CircleCheckIcon v-if="s.have >= s.max" class="text-success size-3.5 shrink-0" />
            <CircleIcon v-else class="size-3.5 shrink-0" />
            <span class="min-w-0 flex-1 truncate">{{ label(s) }}</span>
            <span v-if="s.max > 1" class="font-mono">{{ s.have }}/{{ s.max }}</span>
          </span>
        </section>
      </div>
    </template>
  </div>
</template>
