<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { BackpackIcon, BoxIcon, RefrigeratorIcon, SearchIcon, TractorIcon } from "@lucide/vue";
import { saves, type StoredItem } from "@/lib/api";
import { useFarmDate } from "@/lib/farm-date";

const { t, te, n } = useI18n();
const date = useFarmDate();
const items = ref<StoredItem[] | null>(null);
const query = ref("");

watch(
  () => date.value?.saveId,
  async (id) => {
    items.value = id ? await saves.items(id).catch(() => null) : null;
  },
  { immediate: true },
);

// the game names most places itself; the farm and the player's own rooms it doesn't
function placeName(i: StoredItem): string {
  if (i.place.kind === "inventory") return t("calendar.inventory");
  const loc = i.place.location;
  if (i.locationName) return i.locationName;
  for (const key of [`chestsTab.places.${loc}`, `fishPlaces.${loc}`]) if (te(key)) return t(key);
  return loc.replace(/^Custom_/, "").replace(/([a-z])([A-Z])/g, "$1 $2");
}

const icons = { chest: BoxIcon, fridge: RefrigeratorIcon, grabber: TractorIcon, inventory: BackpackIcon };

const groups = computed(() => {
  const q = query.value.trim().toLowerCase();
  const byId = new Map<string, { id: string; name: string; total: number; stacks: StoredItem[] }>();
  for (const i of items.value ?? []) {
    if (q && !i.display.toLowerCase().includes(q) && !i.name.toLowerCase().includes(q)) continue;
    const g = byId.get(i.id) ?? { id: i.id, name: i.display, total: 0, stacks: [] };
    g.total += i.stack;
    g.stacks.push(i);
    byId.set(i.id, g);
  }
  return [...byId.values()].sort((a, b) => (q ? a.name.localeCompare(b.name) : b.total - a.total));
});

// one line per container: the same chest often holds several qualities of one thing
function places(stacks: StoredItem[]) {
  const out = new Map<string, { label: string; kind: StoredItem["place"]["kind"]; count: number }>();
  for (const s of stacks) {
    const tile = s.place.tile ? ` (${s.place.tile[0]}, ${s.place.tile[1]})` : "";
    const label = placeName(s) + tile;
    const key = `${s.place.kind}|${label}`;
    const p = out.get(key) ?? { label, kind: s.place.kind, count: 0 };
    p.count += s.stack;
    out.set(key, p);
  }
  return [...out.values()].sort((a, b) => b.count - a.count);
}

const SHOWN = 60;
</script>

<template>
  <div class="flex flex-col gap-3">
    <p v-if="!date?.saveId" class="text-muted-foreground p-4 text-center text-sm font-semibold">
      {{ t("museumTab.noSave") }}
    </p>
    <template v-else>
      <div class="flex flex-wrap items-center gap-3">
        <label
          class="bg-card flex h-9 w-72 max-w-full items-center gap-2 rounded-md border-2 border-[#a0703f] px-2.5 focus-within:border-[#4e2c14]"
        >
          <SearchIcon class="text-muted-foreground size-4 shrink-0" />
          <input
            v-model="query"
            type="search"
            :placeholder="t('chestsTab.search')"
            :aria-label="t('chestsTab.search')"
            class="w-full min-w-0 bg-transparent text-sm outline-none"
          />
        </label>
        <span class="text-muted-foreground text-xs font-semibold">{{ t("chestsTab.asOfSave") }}</span>
      </div>
      <p v-if="items && !groups.length" class="text-muted-foreground text-sm font-semibold">
        {{ t("chestsTab.nothing") }}
      </p>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3">
        <section v-for="g in groups.slice(0, SHOWN)" :key="g.id" class="wood-panel flex flex-col gap-1 p-3 text-sm">
          <div class="flex items-baseline gap-2">
            <span class="min-w-0 flex-1 truncate font-extrabold">{{ g.name }}</span>
            <span class="font-mono text-xs font-bold">×{{ n(g.total) }}</span>
          </div>
          <span
            v-for="p in places(g.stacks)"
            :key="p.kind + p.label"
            class="text-muted-foreground flex items-center gap-1.5 text-xs font-semibold"
          >
            <component :is="icons[p.kind]" class="size-3.5 shrink-0" />
            <span class="min-w-0 flex-1 truncate">{{ p.label }}</span>
            <span class="font-mono">{{ n(p.count) }}</span>
          </span>
        </section>
      </div>
      <p v-if="groups.length > SHOWN" class="text-muted-foreground text-xs font-semibold">
        {{ t("chestsTab.more", { n: groups.length - SHOWN }) }}
      </p>
    </template>
  </div>
</template>
