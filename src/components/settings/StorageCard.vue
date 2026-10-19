<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { Trash2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { api, type StorageUsage } from "@/lib/api";
import { errorText } from "@/i18n";

const { t, n } = useI18n();
const usage = ref<StorageUsage[]>([]);
const busy = ref<string | null>(null);

async function load() {
  usage.value = await api.storage().catch(() => []);
}
onMounted(load);

function size(bytes: number) {
  const units = ["B", "KB", "MB", "GB"];
  let v = bytes;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u++;
  }
  return `${n(v, { maximumFractionDigits: u ? 1 : 0 })} ${units[u]}`;
}

async function clean(kind: StorageUsage["kind"]) {
  if (!(await ask(t(`storage.confirm.${kind}`), { title: t("storage.title"), kind: "warning" }))) return;
  busy.value = kind;
  try {
    await api.cleanStorage(kind);
    await load();
    toast.success(t("storage.cleaned"));
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    busy.value = null;
  }
}
</script>

<template>
  <section class="wood-panel flex flex-col gap-2 p-5">
    <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("storage.title") }}</h2>
    <div v-for="u in usage" :key="u.kind" class="flex items-center gap-3 text-sm">
      <div class="flex min-w-0 flex-1 flex-col">
        <span class="font-extrabold">{{ t(`storage.kind.${u.kind}`) }}</span>
        <span class="text-muted-foreground text-xs font-semibold">{{ t(`storage.hint.${u.kind}`) }}</span>
      </div>
      <span class="text-muted-foreground shrink-0 font-mono text-xs">{{ size(u.bytes) }}</span>
      <Button size="sm" variant="outline" class="shrink-0" :disabled="!u.files || busy !== null" @click="clean(u.kind)">
        <Trash2Icon />{{ t("storage.clean") }}
      </Button>
    </div>
  </section>
</template>
