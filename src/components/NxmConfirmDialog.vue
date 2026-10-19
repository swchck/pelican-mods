<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { DownloadIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { nexus, type NexusMod } from "@/lib/api";
import { errorText } from "@/i18n";
import { useNexus } from "@/stores/nexus";
import { useLibrary } from "@/stores/library";

const { t } = useI18n();
const nx = useNexus();
const lib = useLibrary();

const link = computed(() => nx.incoming[0] ?? null);
const mod = ref<NexusMod | null>(null);
const busy = ref(false);

const file = computed(() => mod.value?.files.find((f) => f.fileId === link.value?.fileId) ?? null);
/** Installed packages whose Nexus id matches, which this download would replace. */
const replaces = computed(() => {
  const id = link.value?.modId;
  if (!id) return [];
  return lib.packages.filter((p) =>
    p.mods.some((m) => m.manifest.updateKeys.some((k) => k.replace(/\s/g, "").toLowerCase() === `nexus:${id}`)),
  );
});

watch(
  link,
  async (l) => {
    mod.value = null;
    if (!l) return;
    try {
      const details = await nexus.mod(l.modId);
      if (link.value?.url === l.url) mod.value = details;
    } catch {
      // the dialog still works with ids alone; the user can open the page to check
    }
  },
  { immediate: true },
);

function next() {
  nx.incoming = nx.incoming.slice(1);
}

async function install() {
  const l = link.value;
  if (!l) return;
  busy.value = true;
  try {
    await nexus.openNxm(l.url);
    next();
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <Dialog :open="!!link" @update:open="(open) => !open && next()">
    <DialogContent v-if="link">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("nxmConfirm.title") }}</DialogTitle>
        <DialogDescription>{{ t("nxmConfirm.why") }}</DialogDescription>
      </DialogHeader>
      <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-sm">
        <dt class="text-muted-foreground font-bold">{{ t("nxmConfirm.mod") }}</dt>
        <dd class="font-extrabold break-words">
          {{ mod ? `${mod.name}${mod.author ? ` · ${mod.author}` : ""}` : `#${link.modId}` }}
        </dd>
        <dt class="text-muted-foreground font-bold">{{ t("nxmConfirm.file") }}</dt>
        <dd class="font-semibold break-words">
          {{ file ? `${file.name}${file.version ? ` ${file.version}` : ""}` : `#${link.fileId}` }}
        </dd>
        <template v-if="replaces.length">
          <dt class="text-muted-foreground font-bold">{{ t("nxmConfirm.replaces") }}</dt>
          <dd class="font-semibold">
            {{ replaces.map((p) => `${p.id} ${p.mods[0]?.manifest.version ?? ""}`).join(", ") }}
          </dd>
        </template>
      </dl>
      <p v-if="file?.category === 'OLD_VERSION'" class="text-destructive text-xs font-bold">
        {{ t("nxmConfirm.oldVersion") }}
      </p>
      <DialogFooter>
        <Button variant="outline" :disabled="busy" @click="next">{{ t("common.cancel") }}</Button>
        <Button :disabled="busy" @click="install">
          <Loader2Icon v-if="busy" class="animate-spin" /><DownloadIcon v-else />{{ t("nxmConfirm.install") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
