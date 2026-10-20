<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { Loader2Icon, RadarIcon, UsersIcon } from "@lucide/vue";
import PixelToggle from "@/components/PixelToggle.vue";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { lan, type LanHost, type ModListView } from "@/lib/api";
import { errorText } from "@/i18n";

const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ compare: [file: string, view: ModListView] }>();
const { t } = useI18n();

const sharing = ref(false);
const searching = ref(false);
const hosts = ref<LanHost[] | null>(null);
const fetching = ref<string | null>(null);

async function toggleShare() {
  try {
    if (sharing.value) await lan.shareStop();
    else await lan.shareStart();
    sharing.value = !sharing.value;
  } catch (e) {
    toast.error(errorText(e));
  }
}

async function search() {
  searching.value = true;
  try {
    hosts.value = await lan.discover();
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    searching.value = false;
  }
}

async function compare(h: LanHost) {
  fetching.value = `${h.addr}:${h.port}`;
  try {
    const got = await lan.fetch(h.addr, h.port);
    open.value = false;
    emit("compare", got.file, got.view);
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    fetching.value = null;
  }
}

// a share lives as long as the window, so nobody keeps answering by accident
watch(open, async (isOpen) => {
  if (!isOpen && sharing.value) {
    sharing.value = false;
    await lan.shareStop().catch(() => {});
  }
});
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-lg">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("coop.title") }}</DialogTitle>
        <DialogDescription>{{ t("coop.hint") }}</DialogDescription>
      </DialogHeader>
      <section class="wood-panel flex flex-col gap-2 p-3">
        <label class="flex items-center gap-2 text-sm font-extrabold">
          <PixelToggle :checked="sharing" :label="t('coop.share')" @toggle="toggleShare" />
          {{ t("coop.share") }}
        </label>
        <p class="text-muted-foreground text-xs font-semibold">
          {{ sharing ? t("coop.sharing") : t("coop.shareHint") }}
        </p>
      </section>
      <section class="wood-panel flex flex-col gap-2 p-3">
        <div class="flex items-center gap-2">
          <span class="flex-1 text-sm font-extrabold">{{ t("coop.find") }}</span>
          <Button size="sm" variant="outline" :disabled="searching" @click="search">
            <Loader2Icon v-if="searching" class="animate-spin" /><RadarIcon v-else />{{ t("coop.search") }}
          </Button>
        </div>
        <p v-if="hosts && !hosts.length" class="text-muted-foreground text-xs font-semibold">{{ t("coop.nobody") }}</p>
        <ul v-else-if="hosts" class="flex flex-col gap-1">
          <li v-for="h in hosts" :key="`${h.addr}:${h.port}`" class="flex items-center gap-2 text-sm">
            <span class="min-w-0 flex-1 truncate font-semibold">{{ h.name }}</span>
            <span class="text-muted-foreground font-mono text-xs">{{ h.addr }}</span>
            <Button size="xs" :disabled="fetching !== null" @click="compare(h)">
              <Loader2Icon v-if="fetching === `${h.addr}:${h.port}`" class="animate-spin" /><UsersIcon v-else />{{
                t("coop.compare")
              }}
            </Button>
          </li>
        </ul>
      </section>
    </DialogContent>
  </Dialog>
</template>
