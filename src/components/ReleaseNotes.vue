<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import { renderMarkdown, type Release } from "@/lib/changelog";

defineProps<{ releases: Release[] }>();

function onClick(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest("a[data-external]");
  if (!a) return;
  e.preventDefault();
  void openUrl((a as HTMLAnchorElement).href);
}
</script>

<template>
  <div class="flex flex-col gap-4" @click="onClick">
    <section v-for="r in releases" :key="r.version" class="flex flex-col gap-1">
      <h3 class="font-pixel text-wood-dark text-lg font-bold">
        {{ r.version
        }}<span v-if="r.date" class="text-muted-foreground ml-2 font-sans text-xs font-bold">{{ r.date }}</span>
      </h3>
      <!-- eslint-disable-next-line vue/no-v-html -- renderMarkdown escapes the text and emits only its own tags -->
      <div class="notes text-sm leading-relaxed select-text" v-html="renderMarkdown(r.body)" />
    </section>
  </div>
</template>

<style scoped>
.notes :deep(h4) {
  margin-top: 0.6rem;
  font-weight: 800;
  color: #7a3e12;
}
.notes :deep(ul) {
  list-style: disc;
  padding-left: 1.2rem;
}
.notes :deep(code) {
  font-family: var(--font-mono);
  background: #fbefd4;
  padding: 0 4px;
  border-radius: 3px;
}
.notes :deep(a) {
  color: #7a3e12;
  text-decoration: underline;
}
</style>
