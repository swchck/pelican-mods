<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RadioIcon } from "@lucide/vue";
import { formatClock, seasonKey } from "@/lib/game-clock";
import { useCompanion } from "@/stores/companion";

const { t, n } = useI18n();
const companion = useCompanion();

const state = computed(() => companion.status.state);
const date = computed(() => {
  const s = state.value;
  if (!s) return "";
  const season = seasonKey(s.season);
  return t("live.date", { season: season ? t(`live.seasons.${season}`) : s.season, day: s.day, year: s.year });
});
</script>

<template>
  <section
    v-if="companion.connected"
    class="wood-panel flex flex-col gap-2 p-4"
    :aria-label="t('live.title')"
    aria-live="polite"
  >
    <h2 class="font-pixel text-wood-dark flex items-center gap-2 text-xl font-bold">
      <RadioIcon class="text-success size-5" />{{ t("live.title") }}
    </h2>
    <p v-if="!state" class="text-muted-foreground text-sm font-semibold">{{ t("live.title_screen") }}</p>
    <dl v-else class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-1.5 text-sm">
      <dt class="text-muted-foreground font-bold">{{ t("live.farm") }}</dt>
      <dd class="truncate font-extrabold">{{ t("live.farmOf", { farm: state.farm, farmer: state.farmer }) }}</dd>
      <dt class="text-muted-foreground font-bold">{{ t("live.when") }}</dt>
      <dd class="font-extrabold">{{ date }} · {{ formatClock(state.time) }}</dd>
      <dt class="text-muted-foreground font-bold">{{ t("live.money") }}</dt>
      <dd class="font-mono font-bold">{{ t("live.gold", { amount: n(state.money) }) }}</dd>
      <template v-if="state.location">
        <dt class="text-muted-foreground font-bold">{{ t("live.location") }}</dt>
        <dd class="truncate font-bold">{{ state.location }}</dd>
      </template>
    </dl>
  </section>
</template>
