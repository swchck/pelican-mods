<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { toast } from "vue-sonner";
import { BracesIcon, FolderOpenIcon, Loader2Icon, RotateCcwIcon, SaveIcon, SparklesIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import ConfigField from "@/components/config/ConfigField.vue";
import { library, modConfig, type ModConfig } from "@/lib/api";
import { getAt, inferSections, schemaSections, setAt, type Json } from "@/lib/config-form";
import { errorText } from "@/i18n";
import { useLibrary } from "@/stores/library";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const lib = useLibrary();
const game = useGame();

const cfg = ref<ModConfig | null>(null);
const draft = ref<Record<string, Json> | null>(null);
const raw = ref<string | null>(null);
const loading = ref(false);
const busy = ref(false);

/** Every mod in enabled store packages; mods with settings first. */
const entries = computed(() =>
  lib.packages
    .filter((p) => !p.system && p.enabled)
    .flatMap((p) => p.mods.map((m) => ({ pkg: p.id, rel: m.relPath, name: m.manifest.name, m })))
    .sort(
      (a, b) =>
        Number(b.m.hasConfig || b.m.hasSchema) - Number(a.m.hasConfig || a.m.hasSchema) || a.name.localeCompare(b.name),
    ),
);
const current = computed(() => {
  const pkg = String(route.query.pkg ?? "");
  const rel = String(route.query.rel ?? "");
  return entries.value.find((e) => e.pkg === pkg && e.rel === rel) ?? null;
});
const sections = computed(() => {
  if (!cfg.value) return [];
  if (cfg.value.schema?.length) return schemaSections(cfg.value.schema);
  return draft.value ? inferSections(draft.value) : [];
});
// config files are plain JSON; structuredClone chokes on Vue's reactive proxies
const cloneJson = <T,>(v: T): T => JSON.parse(JSON.stringify(v)) as T;
const dirty = computed(() => JSON.stringify(draft.value) !== JSON.stringify(cfg.value?.values ?? null));

async function load() {
  const c = current.value;
  cfg.value = null;
  draft.value = null;
  raw.value = null;
  if (!c) return;
  loading.value = true;
  try {
    cfg.value = await modConfig.get(c.pkg, c.rel);
    draft.value = cfg.value.values ? cloneJson(cfg.value.values) : null;
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    loading.value = false;
  }
}

function select(pkg: string, rel: string) {
  if (dirty.value && !window.confirm(t("config.discard"))) return;
  void router.replace({ path: "/config", query: { pkg, rel } });
}

function change(path: string[], value: Json) {
  if (draft.value) draft.value = setAt(draft.value, path, value);
}

async function run(op: () => Promise<ModConfig | void>, done: string) {
  const c = current.value;
  if (!c) return;
  busy.value = true;
  try {
    const next = await op();
    if (next) {
      cfg.value = next;
      draft.value = next.values ? cloneJson(next.values) : null;
    } else {
      await load();
    }
    raw.value = null;
    toast.success(done);
  } catch (e) {
    toast.error(errorText(e), { duration: 10000 });
  } finally {
    busy.value = false;
  }
}

function save() {
  const c = current.value;
  if (!c) return;
  let values: unknown = draft.value;
  if (raw.value !== null) {
    try {
      values = JSON.parse(raw.value);
    } catch (e) {
      toast.error(t("config.badJson", { error: String(e) }));
      return;
    }
  }
  void run(() => modConfig.save(c.pkg, c.rel, values), t("config.saved"));
}

function createDefaults() {
  const c = current.value;
  if (c) void run(() => modConfig.defaults(c.pkg, c.rel), t("config.created"));
}

function generate() {
  void run(async () => {
    await modConfig.generate();
    await lib.load();
  }, t("config.generated"));
}

function toggleRaw() {
  raw.value = raw.value === null ? JSON.stringify(draft.value ?? {}, null, 2) : null;
}

watch(current, load);
onMounted(async () => {
  if (!lib.view) await lib.load();
  if (!current.value && entries.value[0]) select(entries.value[0].pkg, entries.value[0].rel);
  else await load();
});
</script>

<template>
  <div class="flex h-full gap-4">
    <nav :aria-label="t('config.mods')" class="wood-panel flex w-60 shrink-0 flex-col gap-1 overflow-auto p-2.5">
      <span class="font-pixel text-wood-dark px-2 pt-1 pb-2 text-lg font-bold">{{ t("config.mods") }}</span>
      <button
        v-for="e in entries"
        :key="`${e.pkg}/${e.rel}`"
        type="button"
        class="flex items-center justify-between gap-2 rounded-md border-2 px-2.5 py-1.5 text-left text-sm font-extrabold"
        :class="
          current === e
            ? 'bg-gold border-wood-dark shadow-[inset_0_-3px_0_var(--gold-shade)]'
            : 'border-transparent hover:bg-[#fbefd4]'
        "
        @click="select(e.pkg, e.rel)"
      >
        <span class="truncate">{{ e.name }}</span>
        <span v-if="!e.m.hasConfig && !e.m.hasSchema" class="text-muted-foreground shrink-0 text-[10px] font-bold">
          {{ t("config.noFile") }}
        </span>
      </button>
      <p v-if="!entries.length" class="text-muted-foreground p-2 text-sm font-semibold">{{ t("config.empty") }}</p>
    </nav>

    <section v-if="current" class="flex min-w-0 flex-1 flex-col gap-3">
      <header class="flex items-start gap-3">
        <div class="flex min-w-0 flex-1 flex-col">
          <h1 class="font-pixel text-wood-dark truncate text-3xl font-bold">{{ current.name }}</h1>
          <span class="text-muted-foreground text-[13px] font-bold">
            {{ cfg?.schema?.length ? t("config.fromSchema") : cfg?.values ? t("config.fromFile") : "" }}
          </span>
        </div>
        <Button v-if="cfg" variant="ghost" size="sm" @click="library.open(cfg.path.replace(/\/config\.json$/, ''))">
          <FolderOpenIcon />{{ t("library.openFolder") }}
        </Button>
      </header>

      <div v-if="loading" class="grid flex-1 place-items-center">
        <Loader2Icon class="text-muted-foreground size-6 animate-spin" />
      </div>

      <div v-else-if="cfg && !draft" class="wood-panel grid flex-1 place-items-center p-8 text-center">
        <div class="flex max-w-md flex-col items-center gap-3">
          <SparklesIcon class="text-wood size-9" />
          <p class="font-pixel text-wood-dark text-xl font-bold">{{ t("config.missingTitle") }}</p>
          <template v-if="cfg.schema?.length">
            <p class="text-sm leading-relaxed">{{ t("config.missingSchema") }}</p>
            <Button :disabled="busy" @click="createDefaults"><SparklesIcon />{{ t("config.createDefaults") }}</Button>
          </template>
          <template v-else>
            <p class="text-sm leading-relaxed">{{ t("config.missingCode") }}</p>
            <Button :disabled="busy || game.running || !game.install?.smapiVersion" @click="generate">
              <Loader2Icon v-if="busy" class="animate-spin" /><SparklesIcon v-else />{{ t("config.generate") }}
            </Button>
            <span class="text-muted-foreground text-xs">{{ t("config.generateHint") }}</span>
          </template>
        </div>
      </div>

      <div v-else-if="draft" class="wood-panel min-h-0 flex-1 overflow-auto px-6 py-4">
        <textarea
          v-if="raw !== null"
          v-model="raw"
          spellcheck="false"
          :aria-label="t('config.json')"
          class="bg-parchment border-input h-full min-h-80 w-full rounded-md border-2 p-3 font-mono text-xs"
        />
        <template v-else>
          <div v-for="(s, i) in sections" :key="s.title ?? i" class="mb-2">
            <h2
              v-if="s.title"
              class="font-pixel border-border-soft mt-2 mb-1 border-b-2 border-dashed pb-1.5 text-xl font-bold text-[#7a3e12]"
            >
              {{ s.title }}
            </h2>
            <div class="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-x-8">
              <ConfigField
                v-for="f in s.fields"
                :key="f.path.join('.')"
                :field="f"
                :value="getAt(draft, f.path)"
                @change="(v) => change(f.path, v)"
              />
            </div>
          </div>
        </template>
      </div>

      <footer v-if="draft" class="flex items-center gap-2.5">
        <span class="text-muted-foreground flex-1 text-[13px] font-bold">
          {{ dirty || raw !== null ? t("config.unsaved") : "" }}
        </span>
        <Button variant="ghost" :disabled="busy || (!dirty && raw === null)" @click="load">
          <RotateCcwIcon />{{ t("config.revert") }}
        </Button>
        <Button v-if="cfg?.schema?.length" variant="outline" :disabled="busy" @click="createDefaults">
          {{ t("config.resetDefaults") }}
        </Button>
        <Button variant="outline" :aria-pressed="raw !== null" @click="toggleRaw"><BracesIcon />JSON</Button>
        <Button
          variant="play"
          size="lg"
          class="font-sans text-[15px]"
          :disabled="busy || (!dirty && raw === null)"
          @click="save"
        >
          <SaveIcon />{{ t("config.save") }}
        </Button>
      </footer>
    </section>
    <div v-else class="wood-panel text-muted-foreground grid flex-1 place-items-center p-6 text-sm font-semibold">
      {{ t("config.select") }}
    </div>
  </div>
</template>
