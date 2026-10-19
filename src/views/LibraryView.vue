<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ArrowUpCircleIcon, DownloadIcon, HistoryIcon, PackageOpenIcon, RefreshCwIcon, SearchIcon } from "@lucide/vue";
import { useIntervalFn, useNow } from "@vueuse/core";
import { relativeTime } from "@/lib/time";
import { computed } from "vue";
import { Button } from "@/components/ui/button";
import PackageTable from "@/components/library/PackageTable.vue";
import PackageDetail from "@/components/library/PackageDetail.vue";
import ProfileSwitcher from "@/components/library/ProfileSwitcher.vue";
import ConflictBanner from "@/components/library/ConflictBanner.vue";
import DisableWarning from "@/components/library/DisableWarning.vue";
import HistoryDialog from "@/components/library/HistoryDialog.vue";
import { useLibrary } from "@/stores/library";
import { hint, useShortcut } from "@/stores/shortcuts";
import { matchesFilter } from "@/lib/packages";

const { t, locale } = useI18n();
const lib = useLibrary();
const historyOpen = ref(false);
const dragging = ref(false);
const filterInput = ref<HTMLInputElement>();

function move(delta: number) {
  const rows = lib.packages.filter((p) => matchesFilter(p, lib.filter));
  if (!rows.length) return;
  const i = rows.findIndex((p) => p.id === lib.selectedId);
  const next = i === -1 ? (delta > 0 ? 0 : rows.length - 1) : Math.min(rows.length - 1, Math.max(0, i + delta));
  lib.selectedId = rows[next].id;
}

useShortcut("page.search", () => filterInput.value?.focus());
useShortcut("page.install", () => void pick());
useShortcut("page.refresh", () => void lib.checkUpdates(true));
useShortcut("list.up", () => move(-1));
useShortcut("list.down", () => move(1));
useShortcut("list.toggle", () => {
  const p = lib.selected;
  if (p && !p.system && !lib.working) void lib.toggle(p.id, !p.enabled);
});
const checkedAt = computed(() => (lib.report?.checkedAt ? new Date(lib.report.checkedAt * 1000) : null));
const now = useNow({ scheduler: (cb) => useIntervalFn(cb, 30_000) });
const checkedTitle = computed(() =>
  checkedAt.value
    ? t("updates.checkedAt", { time: relativeTime(checkedAt.value, now.value, locale.value) })
    : t("updates.never"),
);
let unlisten: UnlistenFn | undefined;

async function install(paths: string[]) {
  if (!paths.length) return;
  if (await lib.install(paths)) toast.success(t("library.installed", { n: paths.length }));
}

async function pick() {
  const paths = await open({
    title: t("library.installTitle"),
    multiple: true,
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
  if (paths) await install(paths);
}

let unmounted = false;
onMounted(async () => {
  void lib.load();
  const off = await getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    if (p.type === "enter" || p.type === "over") dragging.value = true;
    else if (p.type === "leave") dragging.value = false;
    else if (p.type === "drop") {
      dragging.value = false;
      void install(p.paths);
    }
  });
  // left before the listener was in place: drop it now, or every drop installs twice
  if (unmounted) off();
  else unlisten = off;
});
onUnmounted(() => {
  unmounted = true;
  unlisten?.();
});
</script>

<template>
  <div class="relative flex h-full flex-col gap-4">
    <!-- one row at every window width: the filter shrinks first, then button labels go -->
    <header class="@container flex min-w-0 items-center gap-2.5">
      <h1 class="font-pixel text-wood-dark shrink-0 text-3xl font-bold">{{ t("library.title") }}</h1>
      <ProfileSwitcher class="shrink-0" />
      <label
        class="bg-card ml-auto flex h-9 w-60 min-w-28 shrink items-center gap-2 rounded-md border-2 border-[#a0703f] px-2.5 focus-within:border-[#4e2c14]"
      >
        <SearchIcon class="text-muted-foreground size-4 shrink-0" />
        <input
          ref="filterInput"
          v-model="lib.filter"
          type="search"
          :placeholder="t('library.filter')"
          :aria-label="t('library.filter')"
          class="w-full min-w-0 bg-transparent text-sm outline-none"
        />
      </label>
      <Button
        variant="outline"
        size="icon"
        class="shrink-0"
        :disabled="lib.checking"
        :title="`${t('updates.check')} (${hint('page.refresh')}) · ${checkedTitle}`"
        :aria-label="t('updates.check')"
        @click="lib.checkUpdates(true)"
      >
        <RefreshCwIcon :class="{ 'animate-spin': lib.checking }" />
      </Button>
      <Button
        v-if="lib.directUpdates.length"
        class="shrink-0"
        :disabled="lib.updating.size > 0"
        :title="t('updates.updateAll', { n: lib.directUpdates.length })"
        :aria-label="t('updates.updateAll', { n: lib.directUpdates.length })"
        @click="lib.updateAll()"
      >
        <ArrowUpCircleIcon />
        <span class="hidden @3xl:inline">{{ t("updates.updateAll", { n: lib.directUpdates.length }) }}</span>
        <span class="@3xl:hidden">{{ lib.directUpdates.length }}</span>
      </Button>
      <Button
        variant="outline"
        class="shrink-0"
        :disabled="lib.working"
        :title="`${t('library.installZip')} (${hint('page.install')})`"
        :aria-label="t('library.installZip')"
        @click="pick"
      >
        <DownloadIcon /><span class="hidden @2xl:inline">{{ t("library.installZip") }}</span>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="shrink-0"
        :title="t('history.title')"
        :aria-label="t('history.title')"
        @click="historyOpen = true"
      >
        <HistoryIcon />
      </Button>
    </header>

    <ConflictBanner />

    <div
      v-if="lib.view?.importable.length"
      class="bg-parchment border-border-soft flex items-center gap-3 rounded-md border-2 border-dashed p-3"
    >
      <PackageOpenIcon class="text-wood size-5 shrink-0" />
      <div class="flex flex-1 flex-col">
        <span class="text-sm font-extrabold">{{ t("library.importTitle", { n: lib.view.importable.length }) }}</span>
        <span class="text-muted-foreground text-xs font-semibold">{{ t("library.importText") }}</span>
      </div>
      <Button :disabled="lib.working" @click="lib.importGameMods(lib.view.importable)">{{
        t("library.import")
      }}</Button>
    </div>

    <div class="flex min-h-0 flex-1 gap-4">
      <PackageTable />
      <PackageDetail v-if="lib.selected" :key="lib.selected.id" :pkg="lib.selected" />
      <aside
        v-else
        class="wood-panel text-muted-foreground grid w-[clamp(260px,32%,330px)] shrink-0 place-items-center p-6 text-center text-sm font-semibold"
      >
        {{ t("library.select") }}
      </aside>
    </div>

    <div
      v-if="dragging"
      class="bg-background/80 border-wood-dark pointer-events-none absolute inset-0 grid place-items-center rounded-[10px] border-4 border-dashed"
    >
      <span class="font-pixel text-wood-dark text-3xl font-bold">{{ t("library.dropHint") }}</span>
    </div>
    <HistoryDialog v-model:open="historyOpen" />
    <DisableWarning />
  </div>
</template>
