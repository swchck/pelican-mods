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

/** "compare" frames the list as a co-op partner's, to match against rather than install. */
const props = defineProps<{ file: string; initial: ModListView; mode: "import" | "compare" }>();
const open = defineModel<boolean>("open", { required: true });
const { t } = useI18n();
const lib = useLibrary();

const list = ref(props.initial);
const installing = ref(false);

const state = (m: ModListView["mods"][number]) => {
  if (m.installed === null) return "missing";
  const c = compareVersions(m.installed, m.version);
  return c < 0 ? "old" : c > 0 ? "newer" : "ok";
};
const missing = computed(() => list.value.mods.filter((m) => state(m) === "missing" || state(m) === "old"));
const newer = computed(() => list.value.mods.filter((m) => state(m) === "newer").length);
const icon = { ok: CircleCheckIcon, old: CircleAlertIcon, newer: CircleAlertIcon, missing: CircleXIcon };

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
        <DialogTitle class="font-pixel text-xl">{{
          t(mode === "compare" ? "modList.compareTitle" : "modList.title", { name: list.name })
        }}</DialogTitle>
        <DialogDescription>
          {{ t("modList.summary", { n: list.mods.length, missing: missing.length })
          }}<template v-if="newer">, {{ t("modList.newer", { n: newer }) }}</template
          ><template v-if="list.extra.length">, {{ t("modList.extra", { n: list.extra.length }) }}</template
          ><template v-if="list.game"> · Stardew {{ list.game }}</template
          ><template v-if="list.smapi"> · SMAPI {{ list.smapi }}</template>
          <template v-if="mode === 'compare'"><br />{{ t("modList.compareHint") }}</template>
        </DialogDescription>
      </DialogHeader>
      <ul class="flex flex-col gap-1 text-sm">
        <li v-for="m in list.mods" :key="m.uniqueId" class="flex items-center gap-2">
          <component
            :is="icon[state(m)]"
            class="size-4 shrink-0"
            :class="{
              'text-success': state(m) === 'ok',
              'text-[#b57f17]': state(m) === 'old' || state(m) === 'newer',
              'text-destructive': state(m) === 'missing',
            }"
          />
          <span class="min-w-0 flex-1 truncate font-semibold">{{ m.name }}</span>
          <span class="text-muted-foreground shrink-0 font-mono text-xs">
            {{ m.installed && state(m) !== "ok" ? `${m.installed} / ` : "" }}{{ m.version }}
          </span>
        </li>
      </ul>
      <template v-if="list.extra.length">
        <h3 class="text-sm font-extrabold">{{ t("modList.extraTitle") }}</h3>
        <ul class="flex flex-col gap-1 text-sm">
          <li v-for="m in list.extra" :key="m.uniqueId" class="flex items-center gap-2">
            <CircleAlertIcon class="text-muted-foreground size-4 shrink-0" />
            <span class="min-w-0 flex-1 truncate font-semibold">{{ m.name }}</span>
            <span class="text-muted-foreground shrink-0 font-mono text-xs">{{ m.version }}</span>
          </li>
        </ul>
      </template>
      <DialogFooter class="flex-wrap gap-2">
        <DialogClose as-child>
          <Button variant="outline">{{ t("common.cancel") }}</Button>
        </DialogClose>
        <Button v-if="missing.length" variant="outline" :disabled="installing || lib.working" @click="installMissing">
          <Loader2Icon v-if="installing" class="animate-spin" /><DownloadIcon v-else />{{
            t("modList.installMissing", { n: missing.length })
          }}
        </Button>
        <Button v-if="mode === 'import'" :disabled="installing || lib.working" @click="makeProfile">
          <UsersIcon />{{ t("modList.makeProfile") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
