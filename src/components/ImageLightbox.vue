<script setup lang="ts">
import { onUnmounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronLeftIcon, ChevronRightIcon, XIcon } from "@lucide/vue";

const props = defineProps<{ images: string[] }>();
const index = defineModel<number | null>({ required: true });
const { t } = useI18n();

function step(delta: number) {
  if (index.value === null || !props.images.length) return;
  index.value = (index.value + delta + props.images.length) % props.images.length;
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Escape") index.value = null;
  else if (e.key === "ArrowRight") step(1);
  else if (e.key === "ArrowLeft") step(-1);
  else return;
  e.preventDefault();
}

// listen only while open, so arrow keys keep working elsewhere in the app
watch(
  index,
  (v, prev) => {
    if (v !== null && prev === null) window.addEventListener("keydown", onKey);
    if (v === null) window.removeEventListener("keydown", onKey);
  },
  { immediate: true },
);
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <Teleport to="body">
    <div
      v-if="index !== null && images[index]"
      class="fixed inset-0 z-50 flex items-center justify-center bg-[#2b1a0e]/90 p-10"
      role="dialog"
      aria-modal="true"
      :aria-label="t('nexus.screenshot', { n: index + 1 })"
      @click.self="index = null"
    >
      <img
        :src="images[index]"
        alt=""
        class="max-h-full max-w-full rounded-md border-[3px] border-[#4e2c14] object-contain shadow-2xl"
      />
      <button
        type="button"
        class="bg-card border-wood-dark absolute top-5 right-5 grid size-10 place-items-center rounded-md border-2"
        :aria-label="t('gallery.close')"
        @click="index = null"
      >
        <XIcon class="size-5" />
      </button>
      <template v-if="images.length > 1">
        <button
          type="button"
          class="bg-card border-wood-dark absolute left-5 grid size-12 place-items-center rounded-md border-2"
          :aria-label="t('gallery.prev')"
          @click="step(-1)"
        >
          <ChevronLeftIcon class="size-6" />
        </button>
        <button
          type="button"
          class="bg-card border-wood-dark absolute right-5 grid size-12 place-items-center rounded-md border-2"
          :aria-label="t('gallery.next')"
          @click="step(1)"
        >
          <ChevronRightIcon class="size-6" />
        </button>
        <span
          class="bg-card border-wood-dark absolute bottom-5 rounded-md border-2 px-3 py-1 font-mono text-sm font-bold"
        >
          {{ index + 1 }} / {{ images.length }}
        </span>
      </template>
    </div>
  </Teleport>
</template>
