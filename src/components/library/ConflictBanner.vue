<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { SwordsIcon } from "@lucide/vue";
import { library, type Conflict } from "@/lib/api";
import { packageTitle } from "@/lib/packages";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const lib = useLibrary();
const conflicts = ref<Conflict[]>([]);

// any install, removal or toggle can start or end a conflict
watch(
  () => JSON.stringify(lib.activeProfile?.enabled ?? []) + lib.packages.length,
  async () => {
    conflicts.value = await library.conflicts().catch(() => []);
  },
  { immediate: true },
);

const title = (id: string) => {
  const p = lib.packages.find((x) => x.id === id);
  return p ? packageTitle(p) : id;
};
const shown = computed(() => conflicts.value.slice(0, 5));
</script>

<template>
  <section
    v-if="conflicts.length"
    class="bg-warning-soft text-destructive flex flex-col gap-1.5 rounded-md border-2 border-[#d98b76] px-3.5 py-2.5 text-sm font-semibold"
  >
    <b class="flex items-center gap-2"
      ><SwordsIcon class="size-4" />{{ t("conflicts.title", { n: conflicts.length }) }}</b
    >
    <p v-for="c in shown" :key="c.target" class="leading-snug">
      <template v-for="(id, i) in c.packages" :key="id"
        ><template v-if="i">, </template
        ><button type="button" class="underline decoration-dotted underline-offset-2" @click="lib.selectedId = id">
          {{ title(id) }}
        </button></template
      >
      — {{ t(c.conditional ? "conflicts.maybe" : "conflicts.both", { target: c.target }) }}
    </p>
    <span class="text-xs">{{ t("conflicts.hint") }}</span>
  </section>
</template>
