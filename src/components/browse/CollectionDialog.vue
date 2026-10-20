<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CircleCheckIcon, DownloadIcon, ExternalLinkIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { nexus as api, type CollectionView } from "@/lib/api";
import { errorText } from "@/i18n";
import { useNexus } from "@/stores/nexus";

const open = defineModel<boolean>("open", { required: true });
const { t } = useI18n();
const nx = useNexus();

const input = ref("");
const view = ref<CollectionView | null>(null);
const loading = ref(false);
const withOptional = ref(false);
const opened = ref(new Set<number>());
const queued = ref(false);

async function load() {
  loading.value = true;
  try {
    view.value = await api.collection(input.value);
    opened.value = new Set();
    queued.value = false;
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    loading.value = false;
  }
}

const installed = computed(() => new Set(view.value?.installed ?? []));
const missing = computed(
  () =>
    view.value?.collection.files.filter((f) => !installed.value.has(f.modId) && (withOptional.value || !f.optional)) ??
    [],
);
const premium = computed(() => nx.account?.isPremium ?? false);

async function downloadAll() {
  queued.value = true;
  for (const f of missing.value) {
    try {
      await api.download(f.modId, f.fileId);
    } catch (e) {
      toast.error(errorText(e));
      return;
    }
  }
  toast.success(t("collection.queued", { n: missing.value.length }));
}

// without Premium only Nexus' own button hands the file over; the nxm link it sends
// comes back to the app and installs by itself
async function openPage(modId: number, fileId: number) {
  opened.value = new Set(opened.value).add(fileId);
  await openUrl(await api.filePage(modId, fileId));
}
const next = computed(() => missing.value.find((f) => !opened.value.has(f.fileId)) ?? null);
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="flex max-h-[85vh] flex-col sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("collection.title") }}</DialogTitle>
        <DialogDescription>{{ t("collection.hint") }}</DialogDescription>
      </DialogHeader>
      <form class="flex gap-2" @submit.prevent="load">
        <input
          v-model="input"
          :placeholder="t('collection.placeholder')"
          :aria-label="t('collection.placeholder')"
          class="bg-card h-9 min-w-0 flex-1 rounded-md border-2 border-[#a0703f] px-2.5 text-sm outline-none"
        />
        <Button type="submit" :disabled="loading || !input.trim()">
          <Loader2Icon v-if="loading" class="animate-spin" />{{ t("collection.open") }}
        </Button>
      </form>

      <template v-if="view">
        <div class="flex flex-wrap items-baseline gap-x-2 text-sm">
          <b class="font-extrabold">{{ view.collection.name }}</b>
          <span v-if="view.collection.author" class="text-muted-foreground">{{ view.collection.author }}</span>
          <span class="text-muted-foreground font-mono text-xs">rev {{ view.collection.revision }}</span>
        </div>
        <p class="text-sm font-semibold">
          {{
            t("collection.summary", {
              total: view.collection.files.length,
              have: view.collection.files.filter((f) => installed.has(f.modId)).length,
              missing: missing.length,
            })
          }}
        </p>
        <label class="flex items-center gap-2 text-xs font-semibold">
          <input v-model="withOptional" type="checkbox" />{{ t("collection.optional") }}
        </label>
        <div class="flex flex-wrap gap-2">
          <Button v-if="premium" :disabled="!missing.length || queued" @click="downloadAll">
            <DownloadIcon />{{ t("collection.downloadAll", { n: missing.length }) }}
          </Button>
          <Button v-else-if="next" @click="openPage(next.modId, next.fileId)">
            <ExternalLinkIcon />{{ t("collection.openNext", { name: next.modName }) }}
          </Button>
        </div>
        <p v-if="!premium" class="text-muted-foreground text-xs font-semibold">{{ t("collection.noPremium") }}</p>
        <ul class="flex min-h-0 flex-1 flex-col gap-1 overflow-auto text-sm">
          <li v-for="f in view.collection.files" :key="f.fileId" class="flex items-center gap-2">
            <CircleCheckIcon v-if="installed.has(f.modId)" class="text-success size-4 shrink-0" />
            <span v-else class="size-4 shrink-0" />
            <span class="min-w-0 flex-1 truncate font-semibold" :class="{ 'text-muted-foreground': f.optional }">
              {{ f.modName }}
            </span>
            <span class="text-muted-foreground font-mono text-xs">{{ f.version }}</span>
            <Button
              v-if="!installed.has(f.modId) && !premium"
              size="xs"
              variant="ghost"
              :aria-label="t('collection.openPage', { name: f.modName })"
              @click="openPage(f.modId, f.fileId)"
            >
              <ExternalLinkIcon :class="{ 'opacity-40': opened.has(f.fileId) }" />
            </Button>
          </li>
        </ul>
      </template>
    </DialogContent>
  </Dialog>
</template>
