<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CheckIcon, CircleIcon, CloudRainIcon, SunIcon } from "@lucide/vue";
import PixelToggle from "@/components/PixelToggle.vue";
import WikiName from "@/components/WikiName.vue";
import { api, saves, type GameCatalog } from "@/lib/api";
import { formatClock } from "@/lib/game-clock";
import { useFarmDate } from "@/lib/farm-date";

const { t, te } = useI18n();
const date = useFarmDate();
const catalog = ref<GameCatalog | null>(null);
const titles = ref<Record<string, string>>({});
/** Fish ids still to catch on the save; null when no save is known. */
const uncaught = ref<Set<string> | null>(null);
const scope = ref<"now" | "season" | "year">("now");
const hideCaught = ref(false);

onMounted(async () => {
  const c = await api.gameCatalog();
  catalog.value = c;
  titles.value = await api.wikiTitles(c.fishInfo.map((f) => f.id)).catch(() => ({}));
});
watch(
  () => date.value?.saveId,
  async (id) => {
    if (!id) return (uncaught.value = null);
    const p = await saves.perfection(id).catch(() => null);
    const fish = p?.parts.find((x) => x.id === "fish");
    uncaught.value = fish ? new Set(fish.missing.map((m) => m.key)) : null;
  },
  { immediate: true },
);

// snow is not rain to a fish; storms are
const rainy = computed(() => date.value?.weather === "rain" || date.value?.weather === "storm");
function bitesNow(f: GameCatalog["fishInfo"][number]): boolean {
  const d = date.value;
  if (!d) return true;
  if (scope.value === "year") return true;
  if (!f.trap && !f.seasons.includes(d.season)) return false;
  if (scope.value === "season" || !d.live) return true;
  if (f.weather === "sunny" && rainy.value) return false;
  if (f.weather === "rainy" && !rainy.value) return false;
  const time = d.time ?? 0;
  return f.trap || f.times.some(([from, to]) => from <= time && time < to);
}
const caught = (id: string) => (uncaught.value ? !uncaught.value.has(id) : false);
const place = (name: string) => (te(`fishPlaces.${name}`) ? t(`fishPlaces.${name}`) : name);
// the fishing minigame and a scratch location the game builds maps in, not places to fish
const HIDDEN_PLACES = new Set(["fishingGame", "Temp"]);
const places = (names: string[]) => names.filter((n) => !HIDDEN_PLACES.has(n)).map(place);

const shown = computed(() =>
  (catalog.value?.fishInfo ?? [])
    .filter(bitesNow)
    .filter((f) => !hideCaught.value || !caught(f.id))
    .sort((a, b) => Number(caught(a.id)) - Number(caught(b.id)) || a.name.localeCompare(b.name)),
);
</script>

<template>
  <div v-if="catalog" class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-3">
      <div class="bg-parchment border-border-soft flex rounded-md border-2 p-0.5">
        <button
          v-for="s in ['now', 'season', 'year'] as const"
          :key="s"
          type="button"
          class="rounded px-3 py-1 text-sm font-bold"
          :class="scope === s ? 'bg-gold' : 'hover:bg-[#fbefd4]'"
          @click="scope = s"
        >
          {{ t(`fish.scope.${s}`) }}
        </button>
      </div>
      <label class="flex items-center gap-2 text-sm font-bold">
        <PixelToggle :checked="hideCaught" :label="t('fish.hideCaught')" @toggle="hideCaught = !hideCaught" />
        {{ t("fish.hideCaught") }}
      </label>
      <span class="text-muted-foreground text-xs font-bold">
        {{
          !date
            ? ""
            : date.live
              ? t("fish.nowLive", {
                  time: formatClock(date.time ?? 600),
                  weather: t(`fish.weatherNow.${date.weather ?? "sun"}`),
                })
              : t("fish.nowSave")
        }}
      </span>
    </div>

    <p v-if="!shown.length" class="text-muted-foreground p-4 text-center text-sm font-semibold">{{ t("fish.none") }}</p>
    <ul class="grid grid-cols-[repeat(auto-fill,minmax(min(320px,100%),1fr))] gap-2">
      <li
        v-for="f in shown"
        :key="f.id"
        class="wood-panel flex flex-col gap-1 p-3 text-sm"
        :class="{ 'opacity-60': caught(f.id) }"
      >
        <div class="flex items-center gap-2">
          <CheckIcon v-if="caught(f.id)" class="text-success size-4 shrink-0" />
          <CircleIcon v-else class="text-wood-light size-3.5 shrink-0" />
          <WikiName :title="titles[f.id]" class="font-extrabold">{{ f.name }}</WikiName>
          <SunIcon v-if="f.weather === 'sunny'" class="text-gold-shade ml-auto size-4" :aria-label="t('fish.sunny')" />
          <CloudRainIcon
            v-else-if="f.weather === 'rainy'"
            class="ml-auto size-4 text-[#4f7fb0]"
            :aria-label="t('fish.rainy')"
          />
        </div>
        <span class="text-muted-foreground text-xs font-semibold">
          <template v-if="f.trap">{{ t("fish.trap") }}</template>
          <template v-else>
            {{ f.times.map(([a, b]) => `${formatClock(a)}–${formatClock(b)}`).join(", ") }}
            · {{ f.seasons.map((s) => t(`calendar.season.${s}`)).join(", ") }}
          </template>
        </span>
        <span v-if="places(f.locations).length" class="text-xs leading-snug">{{ places(f.locations).join(", ") }}</span>
      </li>
    </ul>
  </div>
</template>
