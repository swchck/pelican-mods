<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { CakeIcon, ChevronLeftIcon, ChevronRightIcon, GiftIcon, PartyPopperIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import WikiName from "@/components/WikiName.vue";
import { api, type GameCatalog } from "@/lib/api";
import { SEASONS, type Season } from "@/lib/game-clock";
import { useFarmDate } from "@/lib/farm-date";
import { useLiveItems } from "@/lib/live-items";

const { t } = useI18n();
const date = useFarmDate();
const { live, byId: owned } = useLiveItems();
const placeText = (id: string) =>
  (owned.value.get(id) ?? []).map((p) => (p.where === "inventory" ? t("calendar.inventory") : p.where)).join(", ");

// the next week of birthdays, with the loved gifts the player already has
const upcoming = computed(() => {
  const d = date.value;
  const c = catalog.value;
  if (!d || !c) return [];
  const order = (season: string | null, day: number) => SEASONS.indexOf(season as Season) * 28 + day;
  const today = order(d.season, d.day);
  return c.villagers
    .filter((v) => v.birthSeason && v.birthDay)
    .map((v) => ({ v, inDays: (order(v.birthSeason, v.birthDay) - today + 112) % 112 }))
    .filter((x) => x.inDays <= 7)
    .sort((a, b) => a.inDays - b.inDays)
    .map(({ v, inDays }) => ({ v, inDays, have: v.loves.filter((id) => owned.value.has(id)) }));
});
const catalog = ref<GameCatalog | null>(null);
const titles = ref<Record<string, string>>({});
const season = ref<Season>("spring");
const picked = ref<number | null>(null);

onMounted(async () => {
  const c = await api.gameCatalog();
  catalog.value = c;
  const loves = [...new Set(c.villagers.flatMap((v) => v.loves))];
  titles.value = await api.wikiTitles(loves).catch(() => ({}));
});
watch(
  () => date.value?.season,
  (s) => {
    if (s) season.value = s;
  },
  { immediate: true },
);

const itemName = computed(() => new Map((catalog.value?.items ?? []).map((i) => [i.id, i.name])));
const days = computed(() =>
  Array.from({ length: 28 }, (_, i) => {
    const day = i + 1;
    const c = catalog.value;
    return {
      day,
      birthdays: (c?.villagers ?? []).filter((v) => v.birthSeason === season.value && v.birthDay === day),
      festivals: (c?.festivals ?? []).filter((f) => f.season === season.value && f.day <= day && day <= f.endDay),
    };
  }),
);
const today = computed(() => (date.value?.season === season.value ? date.value.day : null));
const detail = computed(() => days.value[(picked.value ?? today.value ?? 1) - 1]);

function shift(by: number) {
  season.value = SEASONS[(SEASONS.indexOf(season.value) + by + 4) % 4];
  picked.value = null;
}
</script>

<template>
  <div v-if="catalog" class="flex flex-col gap-3">
    <div class="flex items-center gap-2">
      <Button variant="outline" size="icon-sm" :aria-label="t('calendar.prev')" @click="shift(-1)"
        ><ChevronLeftIcon
      /></Button>
      <h2 class="font-pixel text-wood-dark min-w-28 text-center text-xl font-bold">
        {{ t(`calendar.season.${season}`) }}
      </h2>
      <Button variant="outline" size="icon-sm" :aria-label="t('calendar.next')" @click="shift(1)"
        ><ChevronRightIcon
      /></Button>
      <span v-if="date" class="text-muted-foreground ml-2 text-xs font-bold">
        {{ date.live ? t("calendar.fromGame") : t("calendar.fromSave") }}
      </span>
    </div>

    <section v-if="upcoming.length" class="wood-panel flex flex-col gap-1.5 p-3.5 text-sm">
      <h3 class="font-extrabold">{{ t("calendar.upcoming") }}</h3>
      <div v-for="u in upcoming" :key="u.v.key" class="flex flex-col">
        <span class="font-bold">
          <CakeIcon class="mr-1 inline size-4 text-[#d9534f]" />{{ u.v.name }}
          <span class="text-muted-foreground text-xs font-semibold">
            · {{ u.inDays === 0 ? t("calendar.today") : t("calendar.inDays", { n: u.inDays }) }}</span
          >
        </span>
        <span v-if="!live" class="text-muted-foreground text-xs font-semibold">{{ t("calendar.needGame") }}</span>
        <span v-else-if="!u.have.length" class="text-muted-foreground text-xs font-semibold">{{
          t("calendar.noneOwned")
        }}</span>
        <span v-for="id in u.have" v-else :key="id" class="text-xs font-semibold">
          <GiftIcon class="text-gold-shade mr-1 inline size-3.5" /><WikiName :title="titles[id]">{{
            itemName.get(id) ?? id
          }}</WikiName>
          <span class="text-muted-foreground"> — {{ placeText(id) }}</span>
        </span>
      </div>
    </section>

    <div class="grid grid-cols-7 gap-1.5">
      <button
        v-for="d in days"
        :key="d.day"
        type="button"
        class="bg-parchment border-border-soft flex min-h-20 flex-col items-start gap-0.5 rounded-md border-2 p-1.5 text-left text-[11px] hover:bg-[#fbe3a8]"
        :class="{
          'border-wood-dark shadow-[inset_0_0_0_1px_var(--wood-dark)]': d.day === today,
          'bg-[#fbe3a8]': d.day === (picked ?? today),
        }"
        @click="picked = d.day"
      >
        <span class="font-mono text-xs font-bold">{{ d.day }}</span>
        <!-- festival names run long in most languages ("Пустынный фестиваль"), so they wrap -->
        <span
          v-for="f in d.festivals"
          :key="f.name"
          class="text-gold-shade flex w-full min-w-0 items-start gap-1 font-bold"
          :title="f.name"
        >
          <PartyPopperIcon class="mt-px size-3 shrink-0" /><span
            class="line-clamp-3 min-w-0 leading-tight break-words hyphens-auto"
            >{{ f.name }}</span
          >
        </span>
        <span
          v-for="v in d.birthdays"
          :key="v.key"
          class="flex w-full min-w-0 items-center gap-1 font-semibold"
          :title="v.name"
        >
          <CakeIcon class="size-3 shrink-0 text-[#d9534f]" /><span class="min-w-0 truncate">{{ v.name }}</span>
        </span>
      </button>
    </div>

    <section v-if="detail" class="wood-panel flex flex-col gap-2 p-3.5 text-sm">
      <h3 class="font-extrabold">
        {{ t("calendar.day", { day: detail.day, season: t(`calendar.season.${season}`) }) }}
      </h3>
      <p v-if="!detail.birthdays.length && !detail.festivals.length" class="text-muted-foreground font-semibold">
        {{ t("calendar.quiet") }}
      </p>
      <p v-for="f in detail.festivals" :key="f.name" class="font-bold">
        <PartyPopperIcon class="text-gold-shade mr-1 inline size-4" />{{ f.name }}
        <span v-if="f.endDay > f.day" class="text-muted-foreground text-xs font-semibold">
          · {{ t("calendar.span", { from: f.day, to: f.endDay }) }}</span
        >
      </p>
      <div v-for="v in detail.birthdays" :key="v.key" class="flex flex-col gap-0.5">
        <span class="font-bold">
          <CakeIcon class="mr-1 inline size-4 text-[#d9534f]" /><WikiName :title="v.key">{{ v.name }}</WikiName>
        </span>
        <span v-if="v.loves.length" class="text-muted-foreground text-xs font-semibold">
          {{ t("calendar.loves") }}
          <template v-for="(id, i) in v.loves" :key="id"
            ><template v-if="i">, </template
            ><WikiName :title="titles[id]">{{ itemName.get(id) ?? id }}</WikiName></template
          >
        </span>
      </div>
    </section>
  </div>
</template>
