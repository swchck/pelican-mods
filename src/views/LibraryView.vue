<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { DownloadIcon, PackageOpenIcon, SearchIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import PackageTable from "@/components/library/PackageTable.vue";
import PackageDetail from "@/components/library/PackageDetail.vue";
import ProfileSwitcher from "@/components/library/ProfileSwitcher.vue";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const lib = useLibrary();
const dragging = ref(false);
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

onMounted(async () => {
  void lib.load();
  unlisten = await getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    if (p.type === "enter" || p.type === "over") dragging.value = true;
    else if (p.type === "leave") dragging.value = false;
    else if (p.type === "drop") {
      dragging.value = false;
      void install(p.paths);
    }
  });
});
onUnmounted(() => unlisten?.());
</script>

<template>
  <div class="relative flex h-full flex-col gap-4">
    <header class="flex flex-wrap items-center gap-3">
      <h1 class="font-pixel text-wood-dark text-3xl font-bold">{{ t("library.title") }}</h1>
      <ProfileSwitcher />
      <div class="flex-1" />
      <label
        class="bg-card flex h-9 w-60 items-center gap-2 rounded-md border-2 border-[#a0703f] px-2.5 focus-within:border-[#4e2c14]"
      >
        <SearchIcon class="text-muted-foreground size-4" />
        <input
          v-model="lib.filter"
          type="search"
          :placeholder="t('library.filter')"
          :aria-label="t('library.filter')"
          class="w-full bg-transparent text-sm outline-none"
        />
      </label>
      <Button variant="outline" :disabled="lib.working" @click="pick"
        ><DownloadIcon />{{ t("library.installZip") }}</Button
      >
    </header>

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
        class="wood-panel text-muted-foreground grid w-[330px] shrink-0 place-items-center p-6 text-center text-sm font-semibold"
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
  </div>
</template>
