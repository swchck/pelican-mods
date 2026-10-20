<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { translations, type TranslationLine } from "@/lib/api";
import { errorText } from "@/i18n";
import { useLibrary } from "@/stores/library";

const props = defineProps<{ pkg: string; relPath: string; title: string; language: string }>();
const open = defineModel<boolean>("open", { required: true });
const { t, locale } = useI18n();
const lib = useLibrary();
const lines = ref<TranslationLine[] | null>(null);
const languageName = (code: string, in_: string) => new Intl.DisplayNames([in_], { type: "language" }).of(code) ?? code;
const query = ref("");

watch(open, async (on) => {
  if (!on) return;
  lines.value = null;
  try {
    lines.value = await translations.get(props.pkg, props.relPath, props.language);
  } catch (e) {
    toast.error(errorText(e));
    open.value = false;
  }
});

const done = computed(() => lines.value?.filter((l) => l.text.trim()).length ?? 0);
const shown = computed(() => {
  const q = query.value.trim().toLowerCase();
  return (lines.value ?? []).filter((l) => !q || l.source.toLowerCase().includes(q) || l.key.includes(q));
});

async function save() {
  const texts = Object.fromEntries((lines.value ?? []).map((l) => [l.key, l.text]));
  if (await lib.saveTranslation(props.pkg, props.relPath, props.language, texts)) {
    toast.success(t("translate.saved"));
    open.value = false;
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="flex max-h-[85vh] flex-col sm:max-w-3xl">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">
          {{ t("translate.title", { mod: title, language: languageName(language, locale) }) }}
        </DialogTitle>
        <DialogDescription>{{ t("translate.hint") }}</DialogDescription>
      </DialogHeader>
      <p v-if="!lines" class="text-muted-foreground flex items-center gap-2 text-sm">
        <Loader2Icon class="size-4 animate-spin" />
      </p>
      <template v-else>
        <div class="flex items-center gap-3 text-sm">
          <input
            v-model="query"
            type="search"
            :placeholder="t('translate.search')"
            :aria-label="t('translate.search')"
            class="bg-card h-8 w-64 rounded-md border-2 border-[#a0703f] px-2 outline-none"
          />
          <span class="text-muted-foreground font-mono text-xs font-bold">{{ done }} / {{ lines.length }}</span>
        </div>
        <ul class="flex min-h-0 flex-1 flex-col gap-2 overflow-auto pr-1 text-sm">
          <li v-for="l in shown" :key="l.key" class="grid grid-cols-2 gap-2">
            <span class="flex flex-col">
              <span class="whitespace-pre-line">{{ l.source }}</span>
              <code class="text-muted-foreground text-[11px]">{{ l.key }}</code>
            </span>
            <textarea
              v-model="l.text"
              rows="2"
              :aria-label="l.key"
              class="bg-card rounded-md border-2 border-[#cdba98] px-2 py-1 outline-none focus:border-[#a0703f]"
            />
          </li>
        </ul>
      </template>
      <DialogFooter>
        <Button :disabled="!lines || lib.working" @click="save">{{ t("translate.save") }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
