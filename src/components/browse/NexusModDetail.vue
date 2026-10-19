<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  DownloadIcon,
  ExternalLinkIcon,
  HeartIcon,
  Loader2Icon,
} from "@lucide/vue";
import ImageLightbox from "@/components/ImageLightbox.vue";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { NexusFile, NexusMod } from "@/lib/api";
import { bbcodeImages, bbcodeToHtml } from "@/lib/bbcode";
import { useNexus } from "@/stores/nexus";

const props = defineProps<{ mod: NexusMod; installed: boolean; installedIds: Set<number> }>();
const { t, locale } = useI18n();
const nx = useNexus();

const gallery = computed(() => {
  const imgs = bbcodeImages(props.mod.description ?? "");
  return props.mod.pictureUrl ? [props.mod.pictureUrl, ...imgs.filter((i) => i !== props.mod.pictureUrl)] : imgs;
});
const shown = ref(0);
const lightbox = ref<number | null>(null);
const strip = ref<HTMLElement>();
watch(
  () => props.mod.modId,
  () => {
    shown.value = 0;
    lightbox.value = null;
  },
);
watch(shown, async (i) => {
  await nextTick();
  (strip.value?.children[i] as HTMLElement | undefined)?.scrollIntoView({ block: "nearest", inline: "nearest" });
});

function step(delta: number) {
  const n = gallery.value.length;
  if (n) shown.value = (shown.value + delta + n) % n;
}

const html = computed(() => bbcodeToHtml(props.mod.description ?? props.mod.summary ?? ""));
const visibleFiles = computed(() =>
  props.mod.files.filter((f) => f.category !== "ARCHIVED" && f.category !== "OLD_VERSION"),
);
const mainFile = computed(() => visibleFiles.value.find((f) => f.category === "MAIN") ?? visibleFiles.value[0]);
const busy = computed(() => nx.active.some((d) => d.nexusModId === props.mod.modId));

const num = (n: number) => new Intl.NumberFormat(locale.value, { notation: "compact" }).format(n);
const size = (f: NexusFile) => (f.sizeInBytes ? `${(f.sizeInBytes / 1024 / 1024).toFixed(1)} MB` : "");
const date = (unix: number) => new Date(unix * 1000).toLocaleDateString(locale.value);
const page = computed(() => `https://www.nexusmods.com/stardewvalley/mods/${props.mod.modId}`);

// description links must leave the app, not navigate the webview
function onClick(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest("a[data-external]");
  if (!a) return;
  e.preventDefault();
  void openUrl((a as HTMLAnchorElement).href);
}
</script>

<template>
  <article :aria-label="mod.name" class="wood-panel flex min-w-0 flex-1 flex-col gap-3.5 overflow-hidden p-5 pr-3">
    <header class="flex shrink-0 items-start gap-4">
      <div class="flex min-w-0 flex-1 flex-col gap-1">
        <h2 class="font-pixel text-wood-dark text-[28px] leading-tight font-bold break-words">{{ mod.name }}</h2>
        <span class="text-muted-foreground text-[13px] font-bold">
          {{ mod.author }}<template v-if="mod.version"> · {{ mod.version }}</template>
          <template v-if="mod.updatedAt"> · {{ new Date(mod.updatedAt).toLocaleDateString(locale) }}</template>
        </span>
        <div class="flex flex-wrap gap-1.5 pt-1">
          <span
            v-if="mod.category"
            class="bg-success-soft rounded border-2 border-[#9cc07a] px-2 py-0.5 text-xs font-bold text-[#2e5a1e]"
            >{{ mod.category }}</span
          >
          <span
            class="flex items-center gap-1 rounded border-2 border-[#e2b07a] bg-[#fbe9d0] px-2 py-0.5 text-xs font-bold text-[#7a3e12]"
          >
            <HeartIcon class="size-3" />{{ num(mod.endorsements) }}
          </span>
          <span
            class="flex items-center gap-1 rounded border-2 border-[#8db5d8] bg-[#dceaf6] px-2 py-0.5 text-xs font-bold text-[#1f4a6e]"
          >
            <DownloadIcon class="size-3" />{{ num(mod.downloads) }}
          </span>
        </div>
      </div>
      <div class="flex flex-col items-end gap-2">
        <Button
          v-if="mainFile"
          variant="play"
          size="xl"
          class="text-xl"
          :disabled="busy"
          @click="nx.download(mod.modId, mainFile.fileId)"
        >
          <Loader2Icon v-if="busy" class="animate-spin" />
          <CheckIcon v-else-if="installed" />
          <DownloadIcon v-else />
          {{ installed ? t("nexus.reinstall") : t("nexus.install") }}
        </Button>
        <Button variant="link" size="sm" @click="openUrl(page)"><ExternalLinkIcon />Nexus</Button>
      </div>
    </header>

    <div class="-mr-2 min-h-0 flex-1 overflow-y-auto pr-2">
      <div v-if="gallery.length" class="flex flex-col gap-2 pb-3.5">
        <div class="group relative aspect-[16/8] overflow-hidden rounded-md border-2 border-[#a0703f] bg-[#cfe6f5]">
          <button
            type="button"
            class="h-full w-full cursor-zoom-in"
            :aria-label="t('gallery.open')"
            @click="lightbox = shown"
          >
            <img :src="gallery[shown]" alt="" class="h-full w-full object-cover" />
          </button>
          <template v-if="gallery.length > 1">
            <button
              type="button"
              class="bg-card/90 border-wood-dark absolute top-1/2 left-2 grid size-9 -translate-y-1/2 place-items-center rounded-md border-2 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
              :aria-label="t('gallery.prev')"
              @click="step(-1)"
            >
              <ChevronLeftIcon class="size-5" />
            </button>
            <button
              type="button"
              class="bg-card/90 border-wood-dark absolute top-1/2 right-2 grid size-9 -translate-y-1/2 place-items-center rounded-md border-2 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
              :aria-label="t('gallery.next')"
              @click="step(1)"
            >
              <ChevronRightIcon class="size-5" />
            </button>
            <span
              class="bg-card/90 border-wood-dark absolute right-2 bottom-2 rounded border-2 px-1.5 font-mono text-xs font-bold"
              >{{ shown + 1 }} / {{ gallery.length }}</span
            >
          </template>
        </div>
        <div v-if="gallery.length > 1" ref="strip" class="no-scrollbar flex snap-x gap-2 overflow-x-auto">
          <button
            v-for="(src, i) in gallery"
            :key="src"
            type="button"
            :aria-label="t('nexus.screenshot', { n: i + 1 })"
            class="h-14 w-24 shrink-0 snap-start overflow-hidden rounded-md border-2"
            :class="i === shown ? 'border-wood-dark' : 'border-[#a0703f] opacity-70 hover:opacity-100'"
            @click="shown = i"
          >
            <img :src="src" alt="" class="h-full w-full object-cover" loading="lazy" />
          </button>
        </div>
      </div>

      <Tabs default-value="about" class="gap-3">
        <TabsList class="bg-card sticky top-0 z-10 w-full">
          <TabsTrigger value="about">{{ t("nexus.tabs.about") }}</TabsTrigger>
          <TabsTrigger value="files">{{ t("nexus.tabs.files", { n: visibleFiles.length }) }}</TabsTrigger>
          <TabsTrigger value="requirements">{{
            t("nexus.tabs.requirements", { n: mod.requirements.length })
          }}</TabsTrigger>
        </TabsList>

        <TabsContent value="about">
          <!-- eslint-disable-next-line vue/no-v-html -- bbcodeToHtml escapes the source and emits only whitelisted tags -->
          <div class="bbcode text-sm leading-relaxed select-text" @click="onClick" v-html="html" />
        </TabsContent>

        <TabsContent value="files">
          <div class="flex flex-col gap-2">
            <div
              v-for="f in visibleFiles"
              :key="f.fileId"
              class="bg-parchment border-border-soft flex items-center gap-3 rounded-md border-2 p-2.5"
            >
              <div class="flex min-w-0 flex-1 flex-col">
                <span class="truncate text-sm font-extrabold">{{ f.name }}</span>
                <span class="text-muted-foreground text-xs font-semibold">
                  {{ f.category ? t(`nexus.category.${f.category}`, f.category) : "" }} · {{ f.version }} ·
                  {{ size(f) }} ·
                  {{ date(f.date) }}
                </span>
              </div>
              <Button size="sm" variant="outline" :disabled="busy" @click="nx.download(mod.modId, f.fileId)">
                <DownloadIcon />{{ t("nexus.download") }}
              </Button>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="requirements">
          <p v-if="!mod.requirements.length" class="text-muted-foreground text-sm font-semibold">
            {{ t("nexus.noRequirements") }}
          </p>
          <div class="flex flex-col gap-2">
            <div
              v-for="(r, i) in mod.requirements"
              :key="i"
              class="bg-parchment border-border-soft flex items-center gap-3 rounded-md border-2 p-2.5"
            >
              <div class="flex min-w-0 flex-1 flex-col">
                <span class="text-sm font-extrabold">{{ r.modName ?? r.url }}</span>
                <span v-if="r.notes" class="text-muted-foreground text-xs">{{ r.notes }}</span>
              </div>
              <span
                v-if="r.modId && installedIds.has(r.modId)"
                class="bg-success-soft rounded border-2 border-[#9cc07a] px-2 py-0.5 text-xs font-extrabold text-[#2e5a1e]"
                >{{ t("nexus.installedBadge") }}</span
              >
              <Button
                v-else-if="r.modId && !r.externalRequirement"
                size="sm"
                variant="outline"
                @click="nx.select(r.modId)"
              >
                {{ t("nexus.open") }}
              </Button>
              <Button
                v-else-if="r.url"
                size="sm"
                variant="link"
                :aria-label="t('nexus.openLink', { name: r.modName ?? r.url })"
                @click="openUrl(r.url)"
                ><ExternalLinkIcon
              /></Button>
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
    <ImageLightbox v-model="lightbox" :images="gallery" />
  </article>
</template>

<style scoped>
.bbcode :deep(img) {
  max-width: 100%;
  border-radius: 6px;
  margin: 6px 0;
}
.bbcode :deep(a) {
  color: #7a3e12;
  text-decoration: underline;
}
.bbcode :deep(ul) {
  list-style: disc;
  padding-left: 1.25rem;
}
.bbcode :deep(ol) {
  list-style: decimal;
  padding-left: 1.25rem;
}
.bbcode :deep(blockquote) {
  border-left: 3px solid #d8b47c;
  padding-left: 0.75rem;
  color: #6b4a2e;
}
.bbcode :deep(code) {
  font-family: var(--font-mono);
  background: #fbefd4;
  padding: 0 4px;
  border-radius: 3px;
}
</style>
