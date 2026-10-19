<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { watchDebounced } from "@vueuse/core";
import { DownloadIcon, HeartIcon, Loader2Icon, SearchIcon, SlidersHorizontalIcon } from "@lucide/vue";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import NexusModDetail from "@/components/browse/NexusModDetail.vue";
import type { NexusSort } from "@/lib/api";
import { installedNexusIds } from "@/lib/nexus-ids";
import { useNexus } from "@/stores/nexus";
import { useLibrary } from "@/stores/library";

const { t, locale } = useI18n();
const nx = useNexus();
const lib = useLibrary();

const installed = computed(() => installedNexusIds(lib.packages));
const visible = computed(() =>
  nx.hideInstalled ? nx.results.filter((m) => !installed.value.has(m.modId)) : nx.results,
);
const showFilters = ref(nx.filtersActive);
const updatedOptions = ["any", "week", "month", "year"] as const;
const endorsementOptions = [0, 100, 1000, 10000];

// reka Select can't hold null; "__all" stands for no category
const categoryModel = computed({
  get: () => nx.category ?? "__all",
  set: (v: string) => {
    nx.category = v === "__all" ? null : v;
    void nx.search();
  },
});
const endorsementsModel = computed({
  get: () => String(nx.minEndorsements),
  set: (v: string) => {
    nx.minEndorsements = Number(v);
    void nx.search();
  },
});

function reset() {
  nx.resetFilters();
  void nx.search();
}
const sorts: NexusSort[] = ["endorsements", "downloads", "updated", "created"];
const num = (n: number) => new Intl.NumberFormat(locale.value, { notation: "compact" }).format(n);

watchDebounced(
  () => nx.query,
  () => void nx.search(),
  { debounce: 350 },
);

onMounted(() => {
  if (!nx.results.length) void nx.search();
});
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <header class="flex flex-wrap items-center gap-3">
      <h1 class="font-pixel text-wood-dark text-3xl font-bold">{{ t("nexus.title") }}</h1>
      <label
        class="bg-card border-wood-dark flex h-10 min-w-60 flex-1 items-center gap-2 rounded-md border-2 px-3 shadow-[inset_0_2px_0_#ebd3a8]"
      >
        <SearchIcon class="text-muted-foreground size-4.5" />
        <input
          v-model="nx.query"
          type="search"
          :placeholder="t('nexus.search')"
          :aria-label="t('nexus.search')"
          class="w-full bg-transparent text-[15px] font-bold outline-none"
        />
        <Loader2Icon v-if="nx.searching" class="text-muted-foreground size-4 animate-spin" />
      </label>
      <Button variant="outline" :aria-pressed="showFilters" @click="showFilters = !showFilters">
        <SlidersHorizontalIcon />{{ t("nexus.filters.title") }}
        <span v-if="nx.filtersActive" class="bg-gold border-wood-dark size-2 rounded-full border" />
      </Button>
      <Select v-model="nx.sort" @update:model-value="nx.search()">
        <SelectTrigger :aria-label="t('nexus.sortLabel')" class="bg-card border-input h-10 w-48 border-2 font-bold">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="s in sorts" :key="s" :value="s">{{ t(`nexus.sort.${s}`) }}</SelectItem>
        </SelectContent>
      </Select>
    </header>

    <div
      v-if="showFilters"
      class="bg-parchment border-border-soft flex flex-wrap items-center gap-2.5 rounded-md border-2 p-2.5"
    >
      <Select v-model="categoryModel">
        <SelectTrigger
          :aria-label="t('nexus.filters.category')"
          class="bg-card border-input h-9 w-60 border-2 font-bold"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="__all">{{ t("nexus.filters.allCategories") }}</SelectItem>
          <SelectItem v-for="[name, count] in nx.categories" :key="name" :value="name">
            {{ name }} <span class="text-muted-foreground">· {{ count }}</span>
          </SelectItem>
        </SelectContent>
      </Select>
      <Select v-model="nx.updated" @update:model-value="nx.search()">
        <SelectTrigger
          :aria-label="t('nexus.filters.updated')"
          class="bg-card border-input h-9 w-48 border-2 font-bold"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="u in updatedOptions" :key="u" :value="u">{{
            t(`nexus.filters.updatedOpt.${u}`)
          }}</SelectItem>
        </SelectContent>
      </Select>
      <Select v-model="endorsementsModel">
        <SelectTrigger
          :aria-label="t('nexus.filters.endorsements')"
          class="bg-card border-input h-9 w-48 border-2 font-bold"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="n in endorsementOptions" :key="n" :value="String(n)">
            {{ n ? t("nexus.filters.minEndorsements", { n: num(n) }) : t("nexus.filters.anyEndorsements") }}
          </SelectItem>
        </SelectContent>
      </Select>
      <div class="flex items-center gap-2 px-1">
        <Switch id="hide-installed" v-model="nx.hideInstalled" />
        <Label for="hide-installed" class="font-bold">{{ t("nexus.filters.hideInstalled") }}</Label>
      </div>
      <div class="flex-1" />
      <Button v-if="nx.filtersActive" variant="ghost" size="sm" @click="reset">{{ t("nexus.filters.reset") }}</Button>
    </div>

    <div class="flex min-h-0 flex-1 gap-4">
      <section
        :aria-label="t('nexus.results')"
        class="flex w-[360px] shrink-0 flex-col gap-2.5 overflow-auto pr-1 pb-1"
      >
        <p v-if="!nx.searching && !visible.length" class="text-muted-foreground p-4 text-sm font-semibold">
          {{ t("nexus.nothing") }}
        </p>
        <button
          v-for="m in visible"
          :key="m.modId"
          type="button"
          class="flex gap-3 rounded-lg border-[3px] p-3 text-left"
          :class="
            nx.selectedId === m.modId
              ? 'bg-accent border-wood-dark shadow-[0_3px_0_#8a5530]'
              : 'bg-card border-[#a0703f] shadow-[0_3px_0_#c9a06a] hover:bg-[#fbefd4]'
          "
          @click="nx.select(m.modId)"
        >
          <img
            v-if="m.thumbnailUrl"
            :src="m.thumbnailUrl"
            alt=""
            loading="lazy"
            class="size-16 shrink-0 rounded-md border-2 border-[#8c6e4a] bg-[#cfe6f5] object-cover"
          />
          <div v-else class="size-16 shrink-0 rounded-md border-2 border-[#8c6e4a] bg-[#cfe6f5]" />
          <div class="flex min-w-0 flex-col gap-0.5">
            <span class="flex items-center gap-1.5 text-[15px] font-extrabold">
              <span class="truncate">{{ m.name }}</span>
              <span
                v-if="installed.has(m.modId)"
                class="bg-success-soft shrink-0 rounded border-2 border-[#9cc07a] px-1 text-[10px] font-extrabold text-[#2e5a1e]"
                >{{ t("nexus.installedBadge") }}</span
              >
            </span>
            <span class="text-muted-foreground flex items-center gap-2 text-xs font-bold">
              <span class="truncate">{{ m.author }}</span>
              <span class="flex items-center gap-0.5"><HeartIcon class="size-3" />{{ num(m.endorsements) }}</span>
              <span class="flex items-center gap-0.5"><DownloadIcon class="size-3" />{{ num(m.downloads) }}</span>
            </span>
            <span class="line-clamp-2 text-[13px] leading-snug text-[#4a3020]">{{ m.summary }}</span>
          </div>
        </button>
        <Button
          v-if="nx.results.length && nx.results.length < nx.total"
          variant="outline"
          :disabled="nx.searching"
          @click="nx.search(true)"
        >
          {{ t("nexus.more") }}
        </Button>
      </section>

      <NexusModDetail
        v-if="nx.selected"
        :mod="nx.selected"
        :installed="installed.has(nx.selected.modId)"
        :installed-ids="installed"
      />
      <div
        v-else
        class="wood-panel text-muted-foreground grid flex-1 place-items-center p-6 text-center text-sm font-semibold"
      >
        <Loader2Icon v-if="nx.loadingDetails" class="size-6 animate-spin" />
        <span v-else>{{ t("nexus.select") }}</span>
      </div>
    </div>
  </div>
</template>
