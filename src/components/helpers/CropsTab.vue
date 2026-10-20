<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import PixelToggle from "@/components/PixelToggle.vue";
import WikiName from "@/components/WikiName.vue";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, type GameCatalog } from "@/lib/api";
import { FERTILIZERS, plan, type Fertilizer, type Plan } from "@/lib/crops";
import { SEASONS, type Season } from "@/lib/game-clock";
import { useFarmDate } from "@/lib/farm-date";

const { t, n } = useI18n();
const date = useFarmDate();
const catalog = ref<GameCatalog | null>(null);
const titles = ref<Record<string, string>>({});

const season = ref<Season>("spring");
const day = ref(1);
const fertilizer = ref<Fertilizer>("none");
const agriculturist = ref(false);
const tiller = ref(false);
const anySeason = ref(false);

onMounted(async () => {
  const c = await api.gameCatalog();
  catalog.value = c;
  titles.value = await api.wikiTitles(c.crops.map((cr) => cr.id)).catch(() => ({}));
});
// starts on the farm's date; after that the player may plan ahead
watch(
  () => date.value,
  (d) => {
    if (!d) return;
    season.value = d.season;
    day.value = d.day;
  },
  { immediate: true, once: true },
);

// spring, summer, fall and winter seeds name one forage item but grow a random one of four
const MIXED_SEEDS = new Set(["495", "496", "497", "498"]);
// cactus lists every season, but its planting rules keep it indoors
const INDOORS_ONLY = new Set(["90"]);
const names = computed(() => new Map((catalog.value?.items ?? []).map((i) => [i.id, i.name])));
const FERTILIZER_ITEMS: Partial<Record<Fertilizer, string>> = { speedGro: "465", deluxe: "466", hyper: "918" };
const fertilizerName = (k: Fertilizer) => {
  const id = FERTILIZER_ITEMS[k];
  return (id && names.value.get(id)) || t(`cropsTab.fertilizers.${k}`);
};

const rows = computed(() => {
  const opts = {
    season: season.value,
    day: Math.min(28, Math.max(1, day.value || 1)),
    fertilizer: fertilizer.value,
    agriculturist: agriculturist.value,
    tiller: tiller.value,
    anySeason: anySeason.value,
  };
  const out: { id: string; name: string; regrows: boolean; trellis: boolean; seedPrice: number; plan: Plan }[] = [];
  for (const crop of catalog.value?.crops ?? []) {
    if (crop.seed && MIXED_SEEDS.has(crop.seed)) continue;
    if (!anySeason.value && INDOORS_ONLY.has(crop.id)) continue;
    const p = plan(crop, opts);
    // forage seeds and the like sell for nothing; there's no profit to rank
    if (!p || !p.harvests || !crop.price) continue;
    out.push({
      id: crop.id,
      name: names.value.get(crop.id) ?? crop.id,
      regrows: crop.regrow > 0,
      trellis: crop.trellis,
      seedPrice: crop.seedPrice,
      plan: p,
    });
  }
  return out.sort((a, b) => b.plan.profit - a.plan.profit);
});
const noData = computed(() => !!catalog.value?.crops.length && catalog.value.crops.every((c) => !c.phases.length));
</script>

<template>
  <div v-if="catalog" class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-3">
      <Select v-model="season" :disabled="anySeason">
        <SelectTrigger :aria-label="t('cropsTab.season')" class="bg-card border-input h-9 w-32 border-2 font-bold">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="s in SEASONS" :key="s" :value="s">{{ t(`calendar.season.${s}`) }}</SelectItem>
        </SelectContent>
      </Select>
      <label class="flex items-center gap-2 text-sm font-bold">
        {{ t("cropsTab.day") }}
        <input
          v-model.number="day"
          type="number"
          min="1"
          max="28"
          :disabled="anySeason"
          class="bg-card h-9 w-16 rounded-md border-2 border-[#a0703f] px-2 font-mono text-sm outline-none"
        />
      </label>
      <Select v-model="fertilizer">
        <SelectTrigger :aria-label="t('cropsTab.fertilizer')" class="bg-card border-input h-9 w-48 border-2 font-bold">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="(_, k) in FERTILIZERS" :key="k" :value="k">{{ fertilizerName(k) }}</SelectItem>
        </SelectContent>
      </Select>
      <label
        v-for="o in ['agriculturist', 'tiller', 'anySeason'] as const"
        :key="o"
        class="flex items-center gap-2 text-sm font-bold"
      >
        <PixelToggle
          :checked="o === 'agriculturist' ? agriculturist : o === 'tiller' ? tiller : anySeason"
          :label="t(`cropsTab.${o}`)"
          @toggle="
            o === 'agriculturist'
              ? (agriculturist = !agriculturist)
              : o === 'tiller'
                ? (tiller = !tiller)
                : (anySeason = !anySeason)
          "
        />
        {{ t(`cropsTab.${o}`) }}
      </label>
    </div>
    <p class="text-muted-foreground text-xs font-semibold">{{ t("cropsTab.hint") }}</p>
    <p v-if="noData" class="text-muted-foreground p-4 text-center text-sm font-semibold">{{ t("cropsTab.noData") }}</p>
    <p v-else-if="!rows.length" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ t("cropsTab.none") }}
    </p>
    <table v-else class="w-full text-sm">
      <thead class="text-muted-foreground text-left text-xs font-bold">
        <tr>
          <th class="py-1 pr-2">{{ t("cropsTab.crop") }}</th>
          <th class="px-2 text-right">{{ t("cropsTab.days") }}</th>
          <th class="px-2 text-right">{{ t("cropsTab.harvests") }}</th>
          <th class="px-2 text-right">{{ t("cropsTab.seedCost") }}</th>
          <th class="px-2 text-right">{{ t("cropsTab.profit") }}</th>
          <th class="pl-2 text-right">{{ t("cropsTab.perDay") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="r in rows" :key="r.id" class="border-border-soft border-t">
          <td class="py-1.5 pr-2">
            <WikiName :title="titles[r.id]" class="font-extrabold">{{ r.name }}</WikiName>
            <span v-if="r.regrows" class="text-muted-foreground ml-2 text-xs font-semibold">{{
              t("cropsTab.regrows")
            }}</span>
            <span v-if="r.trellis" class="text-muted-foreground ml-2 text-xs font-semibold">{{
              t("cropsTab.trellis")
            }}</span>
          </td>
          <td class="px-2 text-right font-mono">{{ r.plan.days }}</td>
          <td class="px-2 text-right font-mono">{{ r.plan.harvests }}</td>
          <td class="px-2 text-right font-mono">{{ r.seedPrice ? n(r.seedPrice * r.plan.seeds) : "—" }}</td>
          <td class="px-2 text-right font-mono font-bold" :class="{ 'text-destructive': r.plan.profit < 0 }">
            {{ n(Math.round(r.plan.profit)) }}
          </td>
          <td class="pl-2 text-right font-mono">{{ n(Math.round(r.plan.perDay * 10) / 10) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
