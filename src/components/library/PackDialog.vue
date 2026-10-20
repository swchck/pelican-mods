<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { ImagePlusIcon, Loader2Icon, PackagePlusIcon, XIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { api, saves } from "@/lib/api";
import { useLibrary } from "@/stores/library";

const open = defineModel<boolean>("open", { required: true });
const { t } = useI18n();
const lib = useLibrary();

const name = ref("");
const author = ref("");
const rows = ref<{ file: string; target: string }[]>([]);
const villagers = ref<string[]>([]);
const busy = ref(false);

onMounted(async () => {
  villagers.value = (await api.gameCatalog().catch(() => null))?.villagers.map((v) => v.key) ?? [];
  author.value = (await saves.list().catch(() => []))[0]?.farmer ?? "";
});

// the sheets people retexture most; the field takes any asset name
const COMMON = [
  "LooseSprites/Cursors",
  "Maps/springobjects",
  "TileSheets/tools",
  "TileSheets/crops",
  "Animals/cat",
  "Animals/dog",
];
const targets = computed(() => [...villagers.value.flatMap((v) => [`Portraits/${v}`, `Characters/${v}`]), ...COMMON]);

// "Abigail.png" most likely means her portrait
function guess(file: string): string {
  const stem =
    file
      .split(/[\\/]/)
      .pop()
      ?.replace(/\.png$/i, "") ?? "";
  const v = villagers.value.find((k) => k.toLowerCase() === stem.toLowerCase());
  return v ? `Portraits/${v}` : "";
}

async function add() {
  const picked = await openFile({ multiple: true, filters: [{ name: "PNG", extensions: ["png"] }] });
  for (const file of picked ?? []) rows.value.push({ file, target: guess(file) });
}

const ready = computed(() => name.value.trim() && rows.value.length && rows.value.every((r) => r.target.trim()));

async function create() {
  busy.value = true;
  const ok = await lib.makePack(
    name.value.trim(),
    author.value.trim(),
    rows.value.map((r) => ({ file: r.file, target: r.target.trim() })),
  );
  busy.value = false;
  if (ok) {
    toast.success(t("pack.made", { name: name.value.trim() }));
    open.value = false;
    rows.value = [];
    name.value = "";
  }
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="flex max-h-[85vh] flex-col sm:max-w-2xl">
      <DialogHeader>
        <DialogTitle class="font-pixel text-xl">{{ t("pack.title") }}</DialogTitle>
        <DialogDescription>{{ t("pack.hint") }}</DialogDescription>
      </DialogHeader>
      <div class="grid grid-cols-2 gap-2">
        <input
          v-model="name"
          :placeholder="t('pack.name')"
          :aria-label="t('pack.name')"
          class="bg-card h-9 rounded-md border-2 border-[#a0703f] px-2.5 text-sm outline-none"
        />
        <input
          v-model="author"
          :placeholder="t('pack.author')"
          :aria-label="t('pack.author')"
          class="bg-card h-9 rounded-md border-2 border-[#a0703f] px-2.5 text-sm outline-none"
        />
      </div>
      <datalist id="pack-targets">
        <option v-for="o in targets" :key="o" :value="o" />
      </datalist>
      <ul class="flex min-h-0 flex-1 flex-col gap-1.5 overflow-auto">
        <li v-for="(r, i) in rows" :key="i" class="flex items-center gap-2 text-sm">
          <span class="min-w-0 flex-1 truncate font-semibold" :title="r.file">{{ r.file.split(/[\\/]/).pop() }}</span>
          <span class="text-muted-foreground">→</span>
          <input
            v-model="r.target"
            list="pack-targets"
            :placeholder="t('pack.target')"
            :aria-label="t('pack.targetFor', { file: r.file.split(/[\\/]/).pop() })"
            class="bg-card h-8 w-64 rounded-md border-2 border-[#a0703f] px-2 font-mono text-xs outline-none"
          />
          <Button size="icon-sm" variant="ghost" :aria-label="t('pack.remove')" @click="rows.splice(i, 1)">
            <XIcon />
          </Button>
        </li>
      </ul>
      <p v-if="!rows.length" class="text-muted-foreground text-sm font-semibold">{{ t("pack.empty") }}</p>
      <p class="text-muted-foreground text-xs font-semibold">{{ t("pack.needsCp") }}</p>
      <DialogFooter class="gap-2">
        <Button variant="outline" @click="add"><ImagePlusIcon />{{ t("pack.add") }}</Button>
        <Button :disabled="!ready || busy || lib.working" @click="create">
          <Loader2Icon v-if="busy" class="animate-spin" /><PackagePlusIcon v-else />{{ t("pack.create") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
