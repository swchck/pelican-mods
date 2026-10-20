<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { TagIcon } from "@lucide/vue";
import { matchesStatus, packageStatus, type StatusFilter } from "@/lib/packages";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const lib = useLibrary();

const statuses: StatusFilter[] = ["all", "problems", "updates", "off"];
const counts = computed(() => {
  const all = lib.packages.map((p) => packageStatus(p, lib.report));
  return Object.fromEntries(statuses.map((s) => [s, all.filter((st) => matchesStatus(st, s)).length]));
});

const chip =
  "flex h-7 shrink-0 items-center gap-1.5 rounded-md border-2 px-2.5 text-xs font-extrabold transition-colors";
const on = "border-[#4e2c14] bg-wood-dark text-[#fdf3dc]";
const off = "border-border-soft bg-card text-muted-foreground hover:border-[#a0703f]";
</script>

<template>
  <div class="flex min-w-0 flex-wrap items-center gap-1.5" role="toolbar" :aria-label="t('library.filters')">
    <template v-for="s in statuses" :key="s">
      <button
        v-if="s === 'all' || counts[s] > 0 || lib.statusFilter === s"
        type="button"
        :class="[chip, lib.statusFilter === s ? on : off]"
        :aria-pressed="lib.statusFilter === s"
        @click="lib.statusFilter = s"
      >
        {{ t(`library.show.${s}`) }}<span class="font-mono opacity-70">{{ counts[s] }}</span>
      </button>
    </template>
    <span v-if="lib.tags.length" class="bg-border-soft mx-1 h-5 w-0.5 shrink-0" aria-hidden="true" />
    <button
      v-for="[tag, n] in lib.tags"
      :key="tag"
      type="button"
      :class="[chip, lib.tagFilter === tag ? on : off]"
      :aria-pressed="lib.tagFilter === tag"
      @click="lib.tagFilter = lib.tagFilter === tag ? null : tag"
    >
      <TagIcon class="size-3" />{{ tag }}<span class="font-mono opacity-70">{{ n }}</span>
    </button>
  </div>
</template>
