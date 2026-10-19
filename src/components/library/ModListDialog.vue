<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { CircleAlertIcon, CircleCheckIcon, CircleXIcon, DownloadIcon, Loader2Icon, UsersIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { library, type ModListView } from "@/lib/api";
import { compareVersions } from "@/lib/version";
import { errorText } from "@/i18n";
import { useLibrary } from "@/stores/library";

const props = defineProps<{ file: string; initial: ModListView }>();
const open = defineModel<boolean>("open", { required: true });
const { t } = useI18n();
const lib = useLibrary();

const list = ref(props.initial);
const installing = ref(false);

const state = (m: ModListView["mods"][number]) =>
  m.installed === null ? "missing" : compareVersions(m.installed, m.version) < 0 ? "old" : "ok";
const missing = computed(() => list.value.mods.filter((m) => state(m) !== "ok"));
const icon = { ok: CircleCheckIcon, old: CircleAlertIcon, missing: CircleXIcon };

async function installMissing() {
  installing.value = true;
  try {
    // one at a time: each may need the user on a Nexus page
    for (const m of missing.value) await lib.installDependency(m.uniqueId, m.name, m.updateKeys, m.github);
    list.value = await library.readList(props.file);
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    installing.value = false;
  }
}

async function makeProfile() {
  await lib.profileFromList(
    list.value.name,
    list.value.mods.map((m) => m.uniqueId),
  );
  open.value = false;
  toast.success(t("modList.profileMade", { name: list.value.name }));
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-h-[85vh] max-w-xl overflow-auto">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("modList.title", { name: list.name }) }}</DialogTitle>
        <DialogDescription>
          {{ t("modList.summary", { n: list.mods.length, missing: missing.length })
          }}<template v-if="list.game"> · Stardew {{ list.game }}</template
          ><template v-if="list.smapi"> · SMAPI {{ list.smapi }}</template>
        </DialogDescription>
      </DialogHeader>
      <ul class="flex flex-col gap-1 text-sm">
        <li v-for="m in list.mods" :key="m.uniqueId" class="flex items-center gap-2">
          <component
            :is="icon[state(m)]"
            class="size-4 shrink-0"
            :class="{
              'text-success': state(m) === 'ok',
              'text-[#b57f17]': state(m) === 'old',
              'text-destructive': state(m) === 'missing',
            }"
          />
          <span class="min-w-0 flex-1 truncate font-semibold">{{ m.name }}</span>
          <span class="text-muted-foreground shrink-0 font-mono text-xs">
            {{ m.installed && state(m) === "old" ? `${m.installed} → ` : "" }}{{ m.version }}
          </span>
        </li>
      </ul>
      <DialogFooter class="flex-wrap gap-2">
        <DialogClose as-child>
          <Button variant="outline">{{ t("common.cancel") }}</Button>
        </DialogClose>
        <Button v-if="missing.length" variant="outline" :disabled="installing || lib.working" @click="installMissing">
          <Loader2Icon v-if="installing" class="animate-spin" /><DownloadIcon v-else />{{
            t("modList.installMissing", { n: missing.length })
          }}
        </Button>
        <Button :disabled="installing || lib.working" @click="makeProfile">
          <UsersIcon />{{ t("modList.makeProfile") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
