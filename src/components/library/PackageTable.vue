<script setup lang="ts">
import { computed, nextTick, watch } from "vue";
import { useI18n } from "vue-i18n";
import PixelToggle from "@/components/PixelToggle.vue";
import type { PackageView } from "@/lib/api";
import {
  packageAuthor,
  packageSource,
  packageStatus,
  packageTitle,
  packageUpdate,
  packageVersion,
} from "@/lib/packages";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const lib = useLibrary();

const rows = computed(() =>
  lib.visible.map((p) => ({
    pkg: p,
    title: packageTitle(p),
    author: packageAuthor(p),
    version: packageVersion(p),
    status: packageStatus(p, lib.report),
    update: packageUpdate(p, lib.report),
    source: packageSource(p),
    parts: p.mods.length,
  })),
);

const pill: Record<string, string> = {
  ok: "bg-success-soft border-[#9cc07a] text-[#2e5a1e]",
  off: "bg-[#ede3d0] border-[#cdba98] text-muted-foreground",
  issue: "bg-warning-soft border-[#d98b76] text-destructive",
  broken: "bg-warning-soft border-[#d98b76] text-destructive",
  update: "bg-gold border-[#b57f17] text-[#4e2c14]",
};

watch(
  () => lib.selectedId,
  async () => {
    await nextTick();
    document.querySelector('[role="option"][aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  },
);

function select(p: PackageView) {
  lib.selectedId = p.id;
}
</script>

<template>
  <section :aria-label="t('library.title')" class="wood-panel @container flex min-w-0 flex-1 flex-col overflow-hidden">
    <div
      class="text-muted-foreground border-border-soft grid grid-cols-[44px_minmax(0,1fr)_96px_112px] @xl:grid-cols-[52px_minmax(0,1fr)_112px_124px_84px] gap-2 border-b-2 border-dashed px-4 pt-3 pb-2.5 text-xs font-extrabold tracking-wider uppercase"
    >
      <span>{{ t("library.columns.on") }}</span>
      <span>{{ t("library.columns.mod") }}</span>
      <span>{{ t("library.columns.version") }}</span>
      <span>{{ t("library.columns.status") }}</span>
      <span class="hidden @xl:block">{{ t("library.columns.source") }}</span>
    </div>

    <p v-if="!lib.packages.length" class="text-muted-foreground p-6 text-sm font-semibold">{{ t("library.empty") }}</p>
    <p v-else-if="!rows.length" class="text-muted-foreground p-6 text-sm font-semibold">
      {{ t("library.nothingFound") }}
    </p>

    <div class="min-h-0 flex-1 overflow-auto" role="listbox" :aria-label="t('library.title')">
      <div
        v-for="r in rows"
        :key="r.pkg.id"
        role="option"
        :aria-selected="lib.selectedId === r.pkg.id"
        tabindex="0"
        class="grid cursor-default grid-cols-[44px_minmax(0,1fr)_96px_112px] @xl:grid-cols-[52px_minmax(0,1fr)_112px_124px_84px] items-center gap-2 border-b border-[#ebd3a8] px-4 py-2 outline-none focus-visible:bg-[#fbe3a8]/60"
        :class="lib.selectedId === r.pkg.id ? 'bg-accent' : 'hover:bg-[#fbefd4]'"
        @click="select(r.pkg)"
        @keydown.enter="select(r.pkg)"
      >
        <PixelToggle
          :checked="r.pkg.enabled"
          :disabled="r.pkg.system || lib.working"
          :label="r.title"
          @toggle="lib.toggle(r.pkg.id, !r.pkg.enabled)"
        />
        <div class="flex min-w-0 flex-col">
          <span class="truncate text-[15px] font-extrabold">
            {{ r.title }}
            <span v-if="r.parts > 1" class="text-muted-foreground text-xs font-bold">
              · {{ t("library.mods", r.parts) }}
            </span>
          </span>
          <span class="text-muted-foreground truncate text-xs">{{ r.author }}</span>
        </div>
        <span class="min-w-0 truncate font-mono text-[13px] font-bold" :title="r.version">{{ r.version }}</span>
        <span class="justify-self-start rounded border-2 px-2 py-0.5 text-xs font-extrabold" :class="pill[r.status]">
          {{
            r.status === "update" && r.update
              ? t("updates.pill", { version: r.update.version })
              : t(`library.status.${r.status}`)
          }}
        </span>
        <span class="text-muted-foreground hidden text-xs font-bold @xl:block">{{ r.source ?? "—" }}</span>
      </div>
    </div>
  </section>
</template>
