<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import {
  CalendarDaysIcon,
  CloudSunIcon,
  FishIcon,
  GemIcon,
  SproutIcon,
  NutIcon,
  BoxIcon,
  LandmarkIcon,
  Loader2Icon,
  RefreshCwIcon,
  StoreIcon,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import TreasureTable from "@/components/helpers/TreasureTable.vue";
import CalendarTab from "@/components/helpers/CalendarTab.vue";
import FishTab from "@/components/helpers/FishTab.vue";
import ChestsTab from "@/components/helpers/ChestsTab.vue";
import MuseumTab from "@/components/helpers/MuseumTab.vue";
import WalnutsTab from "@/components/helpers/WalnutsTab.vue";
import CropsTab from "@/components/helpers/CropsTab.vue";
import WikiName from "@/components/WikiName.vue";
import { api as appApi, companion as api, isAppError, type Forecast } from "@/lib/api";
import { bestGrange, grangeScore, itemPoints, place, SLOTS, type GrangeItem } from "@/lib/grange";
import { errorText } from "@/i18n";
import { useCompanion } from "@/stores/companion";
import { useSettings } from "@/stores/settings";
import { useShortcut } from "@/stores/shortcuts";

const { t, n, te } = useI18n();
const companion = useCompanion();
const settings = useSettings();
void settings.load();

const tab = ref<"forecast" | "fair" | "calendar" | "fish" | "museum" | "chests" | "crops" | "walnuts">("forecast");
/** Forecast and fair come from the running game; calendar and fish work from the lists. */
const needsGame = computed(() => tab.value === "forecast" || tab.value === "fair");
const forecast = ref<Forecast | null>(null);
/** English wiki titles of the forecast's base game items, by item id. */
const titles = ref<Record<string, string>>({});
const items = ref<GrangeItem[] | null>(null);
/** Indexes into `pool` the player put on the display; null follows the suggestion. */
const chosen = ref<number[] | null>(null);
const loading = ref(false);
const noWorld = ref(false);

const ready = computed(() => companion.connected && companion.status.state !== null);

async function load() {
  if (!ready.value || !needsGame.value) return;
  loading.value = true;
  noWorld.value = false;
  try {
    if (tab.value === "forecast") {
      const f = await api.forecast();
      forecast.value = f;
      const ids = [...f.geodes, ...f.mysteryBoxes].flatMap((g) => g.next.map((x) => x.id));
      ids.push(...(f.cart ?? []).map((c) => c.id));
      titles.value = await appApi.wikiTitles([...new Set(ids)]).catch(() => ({}));
    } else {
      items.value = await api.items();
      chosen.value = null;
    }
  } catch (e) {
    if (isAppError(e) && e.kind === "no_world") noWorld.value = true;
    else toast.error(errorText(e));
  } finally {
    loading.value = false;
  }
}

const luck = computed(() => {
  const l = forecast.value?.luck ?? 0;
  if (l === 0) return "neutral";
  if (l < -0.07) return "awful";
  if (l < -0.02) return "bad";
  if (l > 0.07) return "great";
  if (l > 0.02) return "good";
  return l < 0 ? "meh" : "fine";
});
const SEASONS = ["spring", "summer", "fall", "winter"] as const;
const dateText = (d: { day: number; season: number }) =>
  t("helpers.date", { day: d.day, season: t(`live.seasons.${SEASONS[d.season]}`) });
const weather = (w: string | null) => (w && te(`helpers.weather.${w}`) ? t(`helpers.weather.${w}`) : (w ?? "—"));

/** Candidate items for the grange, best first; one row per stack. */
const pool = computed(() =>
  (items.value ?? [])
    .filter((i) => itemPoints(i) > 1 || i.price > 0)
    .sort((a, b) => itemPoints(b) - itemPoints(a) || b.price - a.price),
);
const suggested = computed(() => {
  const best = bestGrange(pool.value);
  return best.map((b) => pool.value.indexOf(b));
});
const display = computed(() => (chosen.value ?? suggested.value).map((i) => pool.value[i]));
const score = computed(() => grangeScore(display.value));
const counts = computed(() => {
  const m = new Map<number, number>();
  for (const i of chosen.value ?? suggested.value) m.set(i, (m.get(i) ?? 0) + 1);
  return m;
});

function add(i: number) {
  const cur = [...(chosen.value ?? suggested.value)];
  if (cur.length >= SLOTS || (counts.value.get(i) ?? 0) >= pool.value[i].stack) return;
  chosen.value = [...cur, i];
}

function remove(slot: number) {
  const cur = [...(chosen.value ?? suggested.value)];
  cur.splice(slot, 1);
  chosen.value = cur;
}

const where = (w: string) => (w === "inventory" ? t("helpers.fair.inventory") : w);
const quality = ["", "silver", "gold", "", "iridium"];

watch(
  () => settings.spoilers,
  (on) => {
    if (!on && tab.value === "walnuts") tab.value = "forecast";
  },
);

useShortcut("page.refresh", () => void load());
watch([tab, ready], load, { immediate: true });
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <header class="flex items-center gap-3">
      <h1 class="font-pixel text-wood-dark flex-1 text-3xl font-bold">{{ t("helpers.title") }}</h1>
      <Button v-if="needsGame" variant="outline" :disabled="loading || !ready" @click="load">
        <Loader2Icon v-if="loading" class="animate-spin" /><RefreshCwIcon v-else />{{ t("helpers.refresh") }}
      </Button>
    </header>

    <Tabs v-model="tab" class="flex min-h-0 flex-1 flex-col">
      <TabsList>
        <TabsTrigger value="forecast"><CloudSunIcon />{{ t("helpers.tabs.forecast") }}</TabsTrigger>
        <TabsTrigger value="fair"><StoreIcon />{{ t("helpers.tabs.fair") }}</TabsTrigger>
        <TabsTrigger value="calendar"><CalendarDaysIcon />{{ t("helpers.tabs.calendar") }}</TabsTrigger>
        <TabsTrigger value="crops"><SproutIcon />{{ t("helpers.tabs.crops") }}</TabsTrigger>
        <TabsTrigger value="fish"><FishIcon />{{ t("helpers.tabs.fish") }}</TabsTrigger>
        <TabsTrigger value="museum"><LandmarkIcon />{{ t("helpers.tabs.museum") }}</TabsTrigger>
        <TabsTrigger value="chests"><BoxIcon />{{ t("helpers.tabs.chests") }}</TabsTrigger>
        <TabsTrigger v-if="settings.spoilers" value="walnuts"><NutIcon />{{ t("helpers.tabs.walnuts") }}</TabsTrigger>
      </TabsList>

      <div
        v-if="needsGame && (!ready || noWorld)"
        class="wood-panel mt-2 grid flex-1 place-items-center p-8 text-center"
      >
        <div class="flex max-w-md flex-col items-center gap-3">
          <GemIcon class="text-wood size-9" />
          <p class="font-pixel text-wood-dark text-xl font-bold">{{ t("helpers.needGame") }}</p>
          <p class="text-sm leading-relaxed">
            {{ companion.connected ? t("helpers.needFarm") : t("helpers.needGameHint") }}
          </p>
        </div>
      </div>

      <TabsContent v-if="ready && !noWorld" value="forecast" class="min-h-0 flex-1 overflow-auto">
        <div v-if="forecast" class="flex flex-col gap-3">
          <div class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3">
            <section class="wood-panel flex flex-col gap-1 p-4">
              <span class="text-muted-foreground text-xs font-bold">{{ t("helpers.luck") }}</span>
              <span class="font-pixel text-wood-dark text-xl font-bold">{{ t(`helpers.luckLevels.${luck}`) }}</span>
              <span class="text-muted-foreground font-mono text-xs">{{
                n(forecast.luck, { maximumFractionDigits: 3 })
              }}</span>
            </section>
            <section class="wood-panel flex flex-col gap-1 p-4">
              <span class="text-muted-foreground text-xs font-bold">{{ t("helpers.tomorrow") }}</span>
              <span class="font-pixel text-wood-dark text-xl font-bold">{{ weather(forecast.weatherTomorrow) }}</span>
              <span v-if="forecast.islandWeatherTomorrow" class="text-muted-foreground text-xs font-semibold">
                {{ t("helpers.island", { weather: weather(forecast.islandWeatherTomorrow) }) }}
              </span>
            </section>
          </div>
          <div class="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3">
            <section v-if="settings.spoilers" class="wood-panel flex flex-col gap-1.5 p-4">
              <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("helpers.nights") }}</h2>
              <p class="text-muted-foreground text-xs font-semibold">{{ t("helpers.nightsHint") }}</p>
              <ul v-if="forecast.nights.length" class="flex flex-col gap-1 text-sm">
                <li
                  v-for="night in forecast.nights"
                  :key="`${night.year}-${night.season}-${night.day}`"
                  class="flex gap-2"
                >
                  <span class="text-muted-foreground w-28 shrink-0 font-semibold">{{ dateText(night) }}</span>
                  <span class="font-extrabold">{{ t(`helpers.nightEvents.${night.event}`) }}</span>
                </li>
              </ul>
              <p v-else class="text-sm font-semibold">{{ t("helpers.quietNights") }}</p>
            </section>
            <section v-if="forecast.bookseller?.length" class="wood-panel flex flex-col gap-1 p-4">
              <span class="text-muted-foreground text-xs font-bold">{{ t("helpers.bookseller") }}</span>
              <span class="font-pixel text-wood-dark text-xl font-bold">
                {{ forecast.bookseller.map((d) => t("helpers.dayOf", { day: d })).join(", ") }}
              </span>
            </section>
            <section v-if="settings.spoilers && forecast.cart" class="wood-panel flex flex-col gap-1.5 p-4">
              <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("helpers.cart") }}</h2>
              <ul class="flex flex-col gap-0.5 text-sm">
                <li v-for="c in forecast.cart" :key="c.id" class="flex gap-2">
                  <WikiName :title="titles[c.id]" class="truncate font-semibold">{{ c.name }}</WikiName>
                  <span class="text-muted-foreground ml-auto shrink-0 font-mono text-xs">
                    {{ t("live.gold", { amount: n(c.price) }) }}
                  </span>
                </li>
              </ul>
            </section>
          </div>
          <p
            v-if="!settings.spoilers"
            class="text-muted-foreground bg-parchment rounded-md border-2 border-dashed border-[#cdba98] p-3 text-sm font-semibold"
          >
            {{ t("helpers.spoilersHidden") }}
          </p>
          <TreasureTable
            v-if="settings.spoilers"
            :title="t('helpers.geodes')"
            :hint="t('helpers.geodesHint', { n: forecast.geodesCracked })"
            :opened="forecast.geodesCracked"
            :series="forecast.geodes"
            :titles="titles"
          />
          <TreasureTable
            v-if="settings.spoilers && forecast.mysteryBoxes.length"
            :title="t('helpers.mysteryBoxes')"
            :hint="t('helpers.mysteryBoxesHint', { n: forecast.mysteryBoxesOpened })"
            :opened="forecast.mysteryBoxesOpened"
            :series="forecast.mysteryBoxes"
            :titles="titles"
          />
        </div>
      </TabsContent>

      <TabsContent v-if="ready && !noWorld" value="fair" class="min-h-0 flex-1 overflow-auto">
        <div v-if="items" class="flex flex-col gap-3">
          <section class="wood-panel flex flex-wrap items-center gap-4 p-4">
            <div class="flex flex-col">
              <span class="text-muted-foreground text-xs font-bold">{{ t("helpers.fair.score") }}</span>
              <span class="font-pixel text-wood-dark text-3xl font-bold">{{ score }}</span>
            </div>
            <span
              class="rounded-full px-3 py-1 text-sm font-extrabold"
              :class="place(score) === 1 ? 'bg-gold' : 'bg-parchment'"
            >
              {{ t(`helpers.fair.place.${place(score)}`) }}
            </span>
            <span class="text-muted-foreground flex-1 text-xs font-semibold">{{ t("helpers.fair.rules") }}</span>
            <Button v-if="chosen" variant="ghost" size="sm" @click="chosen = null">{{
              t("helpers.fair.reset")
            }}</Button>
          </section>

          <section class="wood-panel flex flex-col gap-2 p-4">
            <h2 class="font-pixel text-wood-dark text-xl font-bold">
              {{ t("helpers.fair.display", { n: display.length, slots: SLOTS }) }}
            </h2>
            <p v-if="!display.length" class="text-muted-foreground text-sm">{{ t("helpers.fair.empty") }}</p>
            <ul class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-2">
              <li
                v-for="(it, slot) in display"
                :key="slot"
                class="bg-parchment border-border-soft flex items-center gap-2 rounded-md border-2 px-2.5 py-1.5 text-sm"
              >
                <span class="flex min-w-0 flex-1 flex-col">
                  <span class="truncate font-extrabold">
                    {{ it.name }}
                    <span v-if="quality[it.quality]" class="text-muted-foreground text-xs">
                      · {{ t(`helpers.fair.quality.${quality[it.quality]}`) }}
                    </span>
                  </span>
                  <span class="text-muted-foreground truncate text-xs">{{ where(it.where) }}</span>
                </span>
                <span class="font-mono text-xs font-bold">+{{ itemPoints(it) }}</span>
                <Button variant="ghost" size="sm" class="px-1.5" :aria-label="t('common.remove')" @click="remove(slot)">
                  ×
                </Button>
              </li>
            </ul>
          </section>

          <section class="wood-panel flex flex-col gap-2 p-4">
            <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("helpers.fair.candidates") }}</h2>
            <p v-if="!pool.length" class="text-muted-foreground text-sm">{{ t("helpers.fair.noItems") }}</p>
            <ul class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-x-4 gap-y-1">
              <li v-for="(it, i) in pool" :key="i">
                <button
                  type="button"
                  class="hover:bg-parchment flex w-full items-center gap-2 rounded px-1.5 py-0.5 text-left text-sm disabled:opacity-50"
                  :disabled="display.length >= SLOTS || (counts.get(i) ?? 0) >= it.stack"
                  @click="add(i)"
                >
                  <span class="truncate font-semibold">{{ it.name }}</span>
                  <span v-if="quality[it.quality]" class="text-muted-foreground shrink-0 text-xs">
                    {{ t(`helpers.fair.quality.${quality[it.quality]}`) }}
                  </span>
                  <span class="text-muted-foreground ml-auto shrink-0 font-mono text-xs">+{{ itemPoints(it) }}</span>
                </button>
              </li>
            </ul>
          </section>
        </div>
      </TabsContent>
      <TabsContent value="calendar" class="min-h-0 flex-1 overflow-auto"><CalendarTab /></TabsContent>
      <TabsContent value="crops" class="min-h-0 flex-1 overflow-auto"><CropsTab /></TabsContent>
      <TabsContent value="fish" class="min-h-0 flex-1 overflow-auto"><FishTab /></TabsContent>
      <TabsContent value="museum" class="min-h-0 flex-1 overflow-auto"><MuseumTab /></TabsContent>
      <TabsContent value="chests" class="min-h-0 flex-1 overflow-auto"><ChestsTab /></TabsContent>
      <TabsContent v-if="settings.spoilers" value="walnuts" class="min-h-0 flex-1 overflow-auto"
        ><WalnutsTab
      /></TabsContent>
    </Tabs>
  </div>
</template>
