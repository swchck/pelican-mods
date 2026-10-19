<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { LandmarkIcon, MapPinIcon } from "@lucide/vue";
import WikiName from "@/components/WikiName.vue";
import { saves, type CheckupItem } from "@/lib/api";
import { useFarmDate } from "@/lib/farm-date";
import { useLiveItems } from "@/lib/live-items";

const { t } = useI18n();
const date = useFarmDate();
const { live, byId } = useLiveItems();
/** Museum pieces the save hasn't donated, artifacts and minerals together. */
const missing = ref<CheckupItem[] | null>(null);

watch(
  () => date.value?.saveId,
  async (id) => {
    if (!id) return (missing.value = null);
    const report = await saves.checkup(id).catch(() => null);
    const museum = report?.sections.find((s) => s.id === "museum");
    missing.value =
      museum?.groups
        .filter((g) => g.id === "artifacts" || g.id === "minerals")
        .flatMap((g) => g.items.filter((i) => !i.done)) ?? null;
  },
  { immediate: true },
);

const where = (id: string) =>
  (byId.value.get(id) ?? []).map((p) => (p.where === "inventory" ? t("calendar.inventory") : p.where)).join(", ");
// what you can carry to Gunther today first, then what's found but sitting somewhere unknown
const rows = computed(() =>
  (missing.value ?? [])
    .map((i) => ({ i, at: where(i.id), found: i.note === "foundNotDonated" }))
    .sort((a, b) => Number(!!b.at) - Number(!!a.at) || Number(b.found) - Number(a.found)),
);
const ready = computed(() => rows.value.filter((r) => r.at).length);
</script>

<template>
  <div class="flex flex-col gap-3">
    <p v-if="missing === null" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ t("museumTab.noSave") }}
    </p>
    <template v-else>
      <p class="text-muted-foreground text-sm font-semibold">
        <LandmarkIcon class="mr-1 inline size-4" />
        {{ t("museumTab.summary", { missing: rows.length, ready }) }}
        <template v-if="!live"> {{ t("museumTab.needGame") }}</template>
      </p>
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(min(280px,100%),1fr))] gap-2">
        <li
          v-for="r in rows"
          :key="r.i.id"
          class="wood-panel flex flex-col gap-0.5 p-3 text-sm"
          :class="{ 'opacity-60': !r.at && !r.found }"
        >
          <WikiName :title="r.i.wiki" class="font-extrabold">{{ r.i.name }}</WikiName>
          <span v-if="r.at" class="text-success flex items-center gap-1 text-xs font-bold">
            <MapPinIcon class="size-3.5 shrink-0" />{{ r.at }}
          </span>
          <span v-else-if="r.found" class="text-muted-foreground text-xs font-semibold">{{
            t("museumTab.foundElsewhere")
          }}</span>
          <span v-else class="text-muted-foreground text-xs font-semibold">{{ t("museumTab.notFound") }}</span>
        </li>
      </ul>
    </template>
  </div>
</template>
