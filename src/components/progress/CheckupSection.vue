<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { CheckIcon, CircleIcon, HeartIcon, SearchIcon } from "@lucide/vue";
import { Input } from "@/components/ui/input";
import PixelToggle from "@/components/PixelToggle.vue";
import type { CheckupGroup, CheckupItem, CheckupSection } from "@/lib/api";

const props = defineProps<{ section: CheckupSection }>();
const { t, te, n } = useI18n();

const hideDone = ref(false);
const query = ref("");

function text(item: CheckupItem): string {
  if (item.name) return item.name;
  const key = `checkup.items.${item.label}`;
  const params = { need: n(item.need ?? 0), have: n(item.have ?? 0) };
  return te(key) ? t(key, params) : (item.label ?? item.id);
}

function title(group: CheckupGroup): string {
  if (group.title) return group.title;
  const key = `checkup.groups.${group.id}`;
  return te(key) ? t(key) : group.id;
}

/** Goals spell their target in the text; show the count only where it adds something. */
function progress(item: CheckupItem): string | null {
  // a labelled item without progress already says its amount ("2 500з")
  if (item.have === null || item.need === null)
    return item.name && item.need !== null && item.need > 1 ? `×${n(item.need)}` : null;
  if (item.label === "journalScrap" || item.label === "secretNote") return null;
  return `${n(item.have)} / ${n(item.need)}`;
}

const groups = computed(() => {
  const q = query.value.trim().toLowerCase();
  return props.section.groups
    .map((g) => ({
      group: g,
      items: g.items.filter((i) => (!hideDone.value || !i.done) && (!q || text(i).toLowerCase().includes(q))),
    }))
    .filter((g) => g.items.length || (!hideDone.value && !q));
});

const categoryOf = (g: CheckupGroup, i: number) =>
  g.category && g.category !== groups.value[i - 1]?.group.category ? g.category : null;
</script>

<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-3">
      <div class="relative max-w-72 flex-1">
        <SearchIcon class="text-muted-foreground absolute top-2.5 left-2.5 size-4" />
        <Input
          v-model="query"
          :placeholder="t('progress.filter')"
          class="bg-parchment border-input h-9 border-2 pl-8"
        />
      </div>
      <label class="flex items-center gap-2 text-sm font-bold">
        <PixelToggle :checked="hideDone" :label="t('checkup.hideDone')" @toggle="hideDone = !hideDone" />
        {{ t("checkup.hideDone") }}
      </label>
    </div>

    <template v-for="(entry, i) in groups" :key="entry.group.id">
      <h3
        v-if="categoryOf(entry.group, i)"
        class="font-pixel text-wood-dark border-border-soft mt-2 border-b-2 border-dashed pb-1 text-xl font-bold"
      >
        {{ t(`checkup.rooms.${entry.group.category}`) }}
      </h3>
      <section class="wood-panel flex flex-col gap-2 p-3.5">
        <div class="flex items-center gap-2">
          <span class="flex-1 font-extrabold">{{ title(entry.group) }}</span>
          <span class="text-muted-foreground font-mono text-xs font-bold"
            >{{ entry.group.done }} / {{ entry.group.total }}</span
          >
        </div>
        <div class="bg-parchment border-border-soft h-2 overflow-hidden rounded border-2">
          <div
            class="h-full"
            :class="entry.group.done >= entry.group.total ? 'bg-success' : 'bg-gold'"
            :style="{ width: `${entry.group.total ? (entry.group.done / entry.group.total) * 100 : 0}%` }"
          />
        </div>
        <ul class="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-x-4 gap-y-1">
          <li
            v-for="item in entry.items"
            :key="item.id"
            class="flex items-center gap-2 text-sm"
            :class="{ 'text-muted-foreground': item.done }"
          >
            <CheckIcon v-if="item.done" class="text-success size-4 shrink-0" />
            <CircleIcon v-else class="text-wood-light size-3.5 shrink-0" />
            <span class="truncate font-semibold" :title="text(item)">
              {{ text(item) }}
              <template v-if="item.label === 'journalScrap' || item.label === 'secretNote'"> #{{ item.have }}</template>
            </span>
            <span
              v-if="item.note"
              class="bg-parchment text-muted-foreground shrink-0 rounded px-1.5 text-[10px] font-bold"
              :class="{ 'bg-warning-soft text-destructive': item.note === 'foundNotDonated' }"
            >
              {{ t(`checkup.notes.${item.note}`) }}
            </span>
            <span
              v-if="progress(item)"
              class="text-muted-foreground ml-auto flex shrink-0 items-center gap-0.5 font-mono text-xs"
            >
              <HeartIcon v-if="entry.group.id === 'villagers'" class="size-3 fill-current text-[#d9534f]" />
              {{ progress(item) }}
            </span>
          </li>
        </ul>
        <p v-if="!entry.items.length" class="text-muted-foreground text-xs font-semibold">
          {{ entry.group.total ? t("checkup.allDone") : t("checkup.needsCatalog") }}
        </p>
      </section>
    </template>
    <p v-if="!groups.length" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ hideDone ? t("checkup.allDone") : t("checkup.nothingFound") }}
    </p>
  </div>
</template>
