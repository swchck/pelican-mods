<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { toast } from "vue-sonner";
import { ChevronDownIcon, HeartIcon, Loader2Icon, RefreshCwIcon, SearchIcon, TrophyIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import CheckupSection from "@/components/progress/CheckupSection.vue";
import WikiName from "@/components/WikiName.vue";
import {
  saves,
  type Checkup,
  type Perfection,
  type PerfectionMissing,
  type PerfectionPart,
  type SaveSummary,
} from "@/lib/api";
import { errorText } from "@/i18n";
import { useCompanion } from "@/stores/companion";
import { useGame } from "@/stores/game";
import { useShortcut } from "@/stores/shortcuts";

const { t, te, n } = useI18n();
const route = useRoute();
const router = useRouter();
const companion = useCompanion();
const game = useGame();

const list = ref<SaveSummary[]>([]);
const result = ref<Perfection | null>(null);
const checkup = ref<Checkup | null>(null);
const loading = ref(false);
const open = ref<string | null>(null);
const filter = ref("");
const search = ref<InstanceType<typeof Input> | null>(null);

const saveId = computed(() => String(route.query.save ?? "") || list.value[0]?.id || "");
const pct = (v: number) => n(v, { maximumFractionDigits: 1 });

let generation = 0;

async function load() {
  const id = saveId.value;
  const gen = ++generation;
  if (!id) return;
  loading.value = true;
  try {
    // one save pass serves both; the backend caches the facts by file time
    const [next, report] = await Promise.all([saves.perfection(id), saves.checkup(id)]);
    if (gen === generation) [result.value, checkup.value] = [next, report];
  } catch (e) {
    if (gen !== generation) return;
    result.value = null;
    toast.error(errorText(e));
  } finally {
    if (gen === generation) loading.value = false;
  }
}

function pick(id: unknown) {
  void router.replace({ path: "/progress", query: { ...route.query, save: String(id) } });
}

/** The section shown: "perfection" or a checkup section id. */
const active = computed(() => String(route.query.section ?? "perfection"));
const activeSection = computed(() => checkup.value?.sections.find((s) => s.id === active.value) ?? null);

function show(id: string) {
  void router.replace({ path: "/progress", query: { ...route.query, section: id } });
}

function toggle(id: string) {
  open.value = open.value === id ? null : id;
  filter.value = "";
}

/** Parts the save can still gain on, biggest gap first; finished ones last. */
const parts = computed(() => [...(result.value?.parts ?? [])].sort((a, b) => gap(b) - gap(a) || b.weight - a.weight));
const gap = (p: PerfectionPart) => (p.score === null ? p.weight / 2 : p.weight - p.score);
const ratio = (p: PerfectionPart) => (p.total ? p.done / p.total : 0);

function missingName(p: PerfectionPart, m: PerfectionMissing): string {
  const key = `progress.items.${p.id}.${m.key}`;
  return te(key) ? t(key) : m.name;
}

const shown = computed(() => {
  const p = result.value?.parts.find((x) => x.id === open.value);
  if (!p) return [];
  const q = filter.value.trim().toLowerCase();
  return q ? p.missing.filter((m) => missingName(p, m).toLowerCase().includes(q)) : p.missing;
});

useShortcut("page.refresh", () => void load());
useShortcut("page.search", () => search.value?.$el?.focus());

watch(saveId, load);
// fresh lists from the game, or a session that just ended and saved, change the numbers
watch(() => companion.catalogAt, load);
watch(
  () => game.running,
  (running, was) => {
    if (was && !running) void load();
  },
);
onMounted(async () => {
  const before = saveId.value;
  try {
    list.value = await saves.list();
  } catch (e) {
    toast.error(errorText(e));
  }
  // with no save in the URL the list just picked one, and watch(saveId) loads it
  if (saveId.value === before) await load();
});
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <header class="flex flex-wrap items-center gap-3">
      <h1 class="font-pixel text-wood-dark flex-1 text-3xl font-bold">{{ t("progress.title") }}</h1>
      <Select v-if="list.length" :model-value="saveId" @update:model-value="pick">
        <SelectTrigger class="bg-parchment border-input w-64 border-2 font-bold">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="s in list" :key="s.id" :value="s.id">{{ s.farmer }} · {{ s.farmName }}</SelectItem>
        </SelectContent>
      </Select>
      <Button variant="outline" :disabled="loading || !saveId" @click="load">
        <Loader2Icon v-if="loading" class="animate-spin" /><RefreshCwIcon v-else />{{ t("progress.refresh") }}
      </Button>
    </header>

    <div v-if="!list.length && !loading" class="wood-panel text-muted-foreground grid flex-1 place-items-center p-8">
      {{ t("progress.noSaves") }}
    </div>

    <div v-else-if="result" class="flex min-h-0 flex-1 gap-4">
      <nav
        :aria-label="t('progress.sections')"
        class="wood-panel flex w-64 shrink-0 flex-col gap-1 overflow-auto p-2.5"
      >
        <button
          v-for="entry in [
            { id: 'perfection', done: null, total: null },
            ...(checkup?.sections ?? []).map((s) => ({ id: s.id, done: s.done, total: s.total })),
          ]"
          :key="entry.id"
          type="button"
          class="flex items-center justify-between gap-2 rounded-md border-2 px-2.5 py-1.5 text-left text-sm font-extrabold"
          :class="
            active === entry.id
              ? 'bg-gold border-wood-dark shadow-[inset_0_-3px_0_var(--gold-shade)]'
              : 'border-transparent hover:bg-[#fbefd4]'
          "
          :aria-current="active === entry.id ? 'page' : undefined"
          @click="show(entry.id)"
        >
          <span class="leading-tight">{{ t(`checkup.sections.${entry.id}`) }}</span>
          <span class="shrink-0 font-mono text-[11px] font-bold opacity-70">
            {{
              entry.total === null
                ? result.percent === null
                  ? ""
                  : `${pct(result.percent)}%`
                : `${entry.done}/${entry.total}`
            }}
          </span>
        </button>
        <p
          v-if="checkup && (!checkup.catalog || checkup.catalog.builtin)"
          class="text-muted-foreground px-2 pt-2 text-xs leading-snug font-semibold"
        >
          {{ checkup.catalog ? t("checkup.builtinCatalog") : t("checkup.needsCatalog") }}
        </p>
      </nav>

      <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-auto pb-2">
        <template v-if="active === 'perfection'">
          <section class="wood-panel flex items-center gap-5 p-5" :aria-label="t('progress.total')">
            <TrophyIcon class="text-gold-shade size-12 shrink-0" />
            <div class="flex min-w-0 flex-1 flex-col gap-2">
              <div class="flex items-baseline gap-3">
                <span class="font-pixel text-wood-dark text-4xl font-bold">
                  {{ result.percent === null ? "—" : `${pct(result.percent)}%` }}
                </span>
                <span class="text-muted-foreground text-sm font-bold">{{ t("progress.total") }}</span>
              </div>
              <div class="bg-parchment border-wood-dark h-4 overflow-hidden rounded border-2">
                <div class="bg-success h-full transition-[width]" :style="{ width: `${result.percent ?? 0}%` }" />
              </div>
              <p v-if="!result.catalog" class="text-muted-foreground text-xs font-semibold">
                {{ t("progress.noCatalog") }}
              </p>
              <p v-else class="text-muted-foreground text-xs font-semibold">
                {{
                  t(result.catalog.builtin ? "progress.builtinCatalog" : "progress.catalogFrom", {
                    game: result.catalog.game,
                  })
                }}
              </p>
            </div>
          </section>

          <div class="grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] content-start gap-3">
            <section
              v-for="p in parts"
              :key="p.id"
              class="wood-panel flex flex-col gap-2 p-3.5"
              :class="{ 'col-span-full': open === p.id }"
            >
              <button
                type="button"
                class="flex items-center gap-2 text-left"
                :aria-expanded="open === p.id"
                :disabled="!p.missing.length"
                @click="toggle(p.id)"
              >
                <span class="flex-1 font-extrabold">{{ t(`progress.parts.${p.id}`) }}</span>
                <span class="text-muted-foreground font-mono text-xs font-bold">
                  {{ p.total === null ? p.done : `${p.done} / ${p.total}` }}
                </span>
                <ChevronDownIcon
                  v-if="p.missing.length"
                  class="text-muted-foreground size-4 transition-transform"
                  :class="{ 'rotate-180': open === p.id }"
                />
              </button>
              <div class="bg-parchment border-border-soft h-2.5 overflow-hidden rounded border-2">
                <div
                  class="h-full"
                  :class="p.score === p.weight ? 'bg-success' : 'bg-gold'"
                  :style="{ width: `${ratio(p) * 100}%` }"
                />
              </div>
              <span class="text-muted-foreground text-xs font-semibold">
                {{
                  p.score === null
                    ? t("progress.needsCatalog", { weight: p.weight })
                    : t("progress.score", { score: pct(p.score), weight: p.weight })
                }}
              </span>

              <div v-if="open === p.id" class="flex flex-col gap-2 pt-1">
                <div v-if="p.missing.length > 12" class="relative max-w-72">
                  <SearchIcon class="text-muted-foreground absolute top-2.5 left-2.5 size-4" />
                  <Input
                    ref="search"
                    v-model="filter"
                    :placeholder="t('progress.filter')"
                    class="bg-parchment border-input h-9 border-2 pl-8"
                  />
                </div>
                <ul class="grid grid-cols-[repeat(auto-fill,minmax(200px,1fr))] gap-x-4 gap-y-1">
                  <li v-for="m in shown" :key="m.key" class="flex items-center gap-2 text-sm">
                    <WikiName :title="m.wiki" class="truncate font-semibold">{{ missingName(p, m) }}</WikiName>
                    <span
                      v-if="m.known === false"
                      class="bg-warning-soft text-destructive shrink-0 rounded px-1.5 text-[10px] font-bold"
                    >
                      {{ t("progress.notLearned") }}
                    </span>
                    <span
                      v-if="m.have !== null && m.need !== null"
                      class="text-muted-foreground ml-auto flex shrink-0 items-center gap-0.5 font-mono text-xs"
                    >
                      <HeartIcon v-if="p.id === 'friends'" class="size-3 fill-current text-[#d9534f]" />
                      {{ p.id === "slayer" ? `${n(m.have)} / ${n(m.need)}` : `${m.have} / ${m.need}` }}
                    </span>
                  </li>
                </ul>
              </div>
            </section>
          </div>
        </template>
        <CheckupSection v-else-if="activeSection" :key="active" :section="activeSection" />
      </div>
    </div>

    <div v-else-if="loading" class="grid flex-1 place-items-center">
      <Loader2Icon class="text-muted-foreground size-6 animate-spin" />
    </div>
  </div>
</template>
