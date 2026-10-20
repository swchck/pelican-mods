<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { TagIcon, XIcon } from "@lucide/vue";
import type { PackageView } from "@/lib/api";
import { useLibrary } from "@/stores/library";

const props = defineProps<{ pkg: PackageView }>();
const { t } = useI18n();
const lib = useLibrary();
const draft = ref("");
const listId = computed(() => `tags-${props.pkg.id}`);
const suggestions = computed(() => lib.tags.map(([tag]) => tag).filter((tag) => !props.pkg.tags.includes(tag)));

function add() {
  const tag = draft.value.replace(/,/g, " ").trim();
  draft.value = "";
  if (tag) void lib.setTags(props.pkg.id, [...props.pkg.tags, tag]);
}

function remove(tag: string) {
  void lib.setTags(
    props.pkg.id,
    props.pkg.tags.filter((x) => x !== tag),
  );
}
</script>

<template>
  <div class="flex flex-wrap items-center gap-1.5 text-xs">
    <TagIcon class="text-muted-foreground size-3.5" :aria-label="t('library.tags')" />
    <span
      v-for="tag in pkg.tags"
      :key="tag"
      class="bg-parchment flex items-center gap-1 rounded border-2 border-[#cdba98] py-0.5 pr-1 pl-2 font-bold"
    >
      {{ tag }}
      <button
        type="button"
        class="text-muted-foreground hover:text-destructive"
        :aria-label="t('library.tagRemove', { tag })"
        :disabled="lib.working"
        @click="remove(tag)"
      >
        <XIcon class="size-3" />
      </button>
    </span>
    <input
      v-model="draft"
      :list="listId"
      :placeholder="t('library.tagAdd')"
      :aria-label="t('library.tagAdd')"
      maxlength="32"
      class="border-border-soft bg-card h-6 w-24 min-w-0 rounded border-2 border-dashed px-1.5 font-semibold outline-none focus:border-[#a0703f]"
      @keydown="(e: KeyboardEvent) => (e.key === 'Enter' || e.key === ',') && (e.preventDefault(), add())"
      @blur="add"
    />
    <datalist :id="listId">
      <option v-for="s in suggestions" :key="s" :value="s" />
    </datalist>
  </div>
</template>
