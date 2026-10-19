<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArchiveRestoreIcon,
  DownloadIcon,
  FolderOpenIcon,
  Loader2Icon,
  SaveIcon,
  Trash2Icon,
  TrophyIcon,
  UploadIcon,
} from "@lucide/vue";
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
import { api, saves, type SaveProgress, type SaveSummary, type Snapshot } from "@/lib/api";
import { relativeTime } from "@/lib/time";
import { errorText } from "@/i18n";
import { useGame } from "@/stores/game";
import { useShortcut } from "@/stores/shortcuts";

const { t, locale } = useI18n();
const game = useGame();

const list = ref<SaveSummary[]>([]);
const loading = ref(true);
const selectedId = ref<string | null>(null);
const progress = ref<SaveProgress | null>(null);
const progressFor = ref<string | null>(null);
const snapshots = ref<Snapshot[]>([]);
const busy = ref(false);
const restoreTarget = ref<Snapshot | null>(null);
const deleteTarget = ref<SaveSummary | null>(null);

const selected = computed(() => list.value.find((s) => s.id === selectedId.value) ?? null);

// grass, autumn leaves, snow, and a summer sea; one per season like the in-game calendar
const banners = [
  "repeating-linear-gradient(90deg,#6fb04a 0 18px,#5e9e3e 18px 36px)",
  "repeating-linear-gradient(90deg,#e8c94a 0 18px,#d9b83a 18px 36px)",
  "repeating-linear-gradient(90deg,#e59a3c 0 18px,#d4882e 18px 36px)",
  "repeating-linear-gradient(90deg,#dde9f2 0 18px,#c9dae8 18px 36px)",
];
const skillNames = ["farming", "mining", "foraging", "fishing", "combat"] as const;

const money = (n: number) => `${new Intl.NumberFormat(locale.value).format(n)} ${t("saves.gold")}`;
const hours = (ms: number) => t("saves.hours", { n: Math.round(ms / 3_600_000) });
const date = (s: SaveSummary) => t("saves.date", { year: s.year, season: t(`saves.season.${s.season}`), day: s.day });

async function load() {
  loading.value = true;
  try {
    list.value = await saves.list();
    if (!selectedId.value || !list.value.some((s) => s.id === selectedId.value)) {
      selectedId.value = list.value[0]?.id ?? null;
    }
  } catch (e) {
    toast.error(errorText(e));
  } finally {
    loading.value = false;
  }
}

watch(selectedId, async (id) => {
  progress.value = null;
  snapshots.value = [];
  if (!id) return;
  progressFor.value = id;
  const [p, s] = await Promise.allSettled([saves.progress(id), saves.snapshots(id)]);
  if (progressFor.value !== id) return;
  if (p.status === "fulfilled") progress.value = p.value;
  else toast.error(errorText(p.reason));
  if (s.status === "fulfilled") snapshots.value = s.value;
});

async function run(op: () => Promise<unknown>, done?: string) {
  busy.value = true;
  try {
    await op();
    if (done) toast.success(done);
  } catch (e) {
    toast.error(errorText(e), { duration: 10000 });
  } finally {
    busy.value = false;
  }
}

async function backup() {
  const id = selectedId.value;
  if (!id) return;
  await run(async () => {
    await saves.backup(id);
    const list = await saves.snapshots(id);
    if (selectedId.value === id) snapshots.value = list;
  }, t("saves.backedUp"));
}

async function exportSave() {
  const s = selected.value;
  if (!s) return;
  const dest = await save({ defaultPath: `${s.id}.zip`, filters: [{ name: "Zip", extensions: ["zip"] }] });
  if (dest) await run(() => saves.export(s.id, dest), t("saves.exported"));
}

async function importSave() {
  const file = await open({ filters: [{ name: "Zip", extensions: ["zip"] }] });
  if (!file) return;
  await run(async () => {
    selectedId.value = await saves.import(file);
    await load();
  }, t("saves.imported"));
}

async function deleteSave() {
  const s = deleteTarget.value;
  deleteTarget.value = null;
  if (!s) return;
  await run(
    async () => {
      await saves.delete(s.id);
      await load();
    },
    t("saves.deleted", { name: s.farmName }),
  );
}

async function restore() {
  const snap = restoreTarget.value;
  restoreTarget.value = null;
  if (!snap) return;
  await run(async () => {
    await saves.restore(snap.file);
    progress.value = null;
    await load();
    const [list, p] = await Promise.all([saves.snapshots(snap.saveId), saves.progress(snap.saveId)]);
    if (selectedId.value === snap.saveId) [snapshots.value, progress.value] = [list, p];
  }, t("saves.restored"));
}

onMounted(load);
useShortcut("page.install", () => void importSave());
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <header class="flex flex-wrap items-center gap-3">
      <h1 class="font-pixel text-wood-dark flex-1 text-3xl font-bold">{{ t("saves.title") }}</h1>
      <Button variant="outline" :disabled="busy || game.running" @click="importSave">
        <UploadIcon />{{ t("saves.import") }}
      </Button>
      <Button variant="outline" @click="api.open('saves')"><FolderOpenIcon />{{ t("saves.folder") }}</Button>
    </header>

    <div v-if="loading" class="grid flex-1 place-items-center">
      <Loader2Icon class="text-muted-foreground size-6 animate-spin" />
    </div>
    <div v-else-if="!list.length" class="wood-panel grid flex-1 place-items-center p-8 text-center">
      <div class="flex max-w-md flex-col items-center gap-3">
        <SaveIcon class="text-wood size-10" />
        <p class="font-pixel text-wood-dark text-xl font-bold">{{ t("saves.emptyTitle") }}</p>
        <p class="text-muted-foreground text-sm font-semibold">{{ t("saves.emptyText") }}</p>
      </div>
    </div>

    <template v-else>
      <div class="grid shrink-0 grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-3.5">
        <button
          v-for="s in list"
          :key="s.id"
          type="button"
          class="overflow-hidden rounded-[10px] border-[3px] text-left"
          :class="
            selectedId === s.id
              ? 'border-wood-dark bg-accent shadow-[0_4px_0_#8a5530]'
              : 'bg-card border-[#a0703f] shadow-[0_4px_0_#c9a06a] hover:bg-[#fbefd4]'
          "
          @click="selectedId = s.id"
        >
          <div class="h-12 border-b-[3px] border-[#5b3a1e]" :style="{ background: banners[s.season] }" />
          <div class="flex flex-col gap-0.5 px-3.5 pt-2.5 pb-3">
            <span class="font-pixel text-wood-dark truncate text-xl font-bold">{{ s.farmName }}</span>
            <span class="text-muted-foreground truncate text-[13px] font-bold">{{ s.farmer }} · {{ date(s) }}</span>
            <span class="text-[13px] font-extrabold text-[#7a5a0e]">{{ money(s.money) }}</span>
          </div>
        </button>
      </div>

      <div v-if="selected" class="flex min-h-0 flex-1 gap-4">
        <section
          :aria-label="t('saves.progress')"
          class="wood-panel flex min-w-0 flex-1 flex-col gap-4 overflow-auto px-5 py-4"
        >
          <div class="flex items-center gap-2.5">
            <h2 class="font-pixel text-wood-dark flex-1 text-[22px] font-bold">
              {{ t("saves.progressOf", { farm: selected.farmName }) }}
            </h2>
            <span class="text-muted-foreground text-xs font-bold">{{ hours(selected.playTimeMs) }}</span>
            <Button variant="outline" size="sm" as-child>
              <RouterLink :to="{ path: '/progress', query: { save: selected.id } }">
                <TrophyIcon />{{ t("saves.perfection") }}
              </RouterLink>
            </Button>
          </div>

          <div v-if="!progress" class="text-muted-foreground flex items-center gap-2 text-sm font-semibold">
            <Loader2Icon class="size-4 animate-spin" />{{ t("saves.reading") }}
          </div>
          <template v-else>
            <div class="grid grid-cols-2 gap-x-7 gap-y-3">
              <div class="flex flex-col gap-1.5">
                <span class="flex justify-between text-sm font-extrabold">
                  <span>{{ progress.joja ? t("saves.joja") : t("saves.cc") }}</span>
                  <span class="font-mono">{{ progress.joja ? "✓" : `${progress.ccRooms.length} / 6` }}</span>
                </span>
                <div class="bg-muted h-3 overflow-hidden rounded-[3px] border-2 border-[#8c6e4a]">
                  <div
                    class="bg-success h-full"
                    :style="{ width: `${progress.joja ? 100 : (progress.ccRooms.length / 6) * 100}%` }"
                  />
                </div>
              </div>
              <div
                v-for="row in [
                  { label: t('saves.museum'), value: progress.museumItems },
                  { label: t('saves.fish'), value: progress.fishSpecies },
                  { label: t('saves.cooking'), value: `${progress.cookingCooked} / ${progress.cookingKnown}` },
                  { label: t('saves.crafting'), value: progress.craftingKnown },
                  { label: t('saves.earned'), value: money(selected.totalEarned) },
                ]"
                :key="row.label"
                class="border-border-soft flex items-center justify-between border-b border-dashed pb-1.5 text-sm"
              >
                <span class="font-extrabold">{{ row.label }}</span>
                <span class="font-mono font-bold">{{ row.value }}</span>
              </div>
            </div>

            <div class="flex flex-col gap-2">
              <span class="text-sm font-extrabold">{{ t("saves.skills") }}</span>
              <div class="grid grid-cols-5 gap-2.5">
                <div
                  v-for="k in skillNames"
                  :key="k"
                  class="bg-parchment border-border-soft flex flex-col items-center gap-1 rounded-md border-2 px-1 py-2"
                >
                  <span class="font-pixel text-wood-dark text-2xl font-bold">{{ selected.skills[k] }}</span>
                  <span class="text-muted-foreground text-xs font-extrabold">{{ t(`saves.skill.${k}`) }}</span>
                </div>
              </div>
            </div>

            <div v-if="progress.friendships.length" class="flex flex-col gap-2">
              <span class="text-sm font-extrabold">{{ t("saves.friends") }}</span>
              <div class="grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-x-4 gap-y-1">
                <div v-for="f in progress.friendships" :key="f.name" class="flex justify-between gap-2 text-[13px]">
                  <span class="truncate font-bold">{{ f.name }}</span>
                  <span class="font-mono text-[#c0392b]" :aria-label="t('saves.hearts', { n: f.hearts })">
                    {{ "♥".repeat(Math.min(f.hearts, 10)) || "·" }}
                  </span>
                </div>
              </div>
            </div>
          </template>
        </section>

        <aside
          :aria-label="t('saves.backups')"
          class="wood-panel flex w-[330px] shrink-0 flex-col gap-2.5 overflow-auto p-[18px]"
        >
          <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("saves.backups") }}</h2>
          <span class="text-muted-foreground text-xs font-bold">{{ t("saves.backupsAbout") }}</span>
          <p v-if="!snapshots.length" class="text-muted-foreground text-sm font-semibold">{{ t("saves.noBackups") }}</p>
          <div
            v-for="b in snapshots"
            :key="b.file"
            class="bg-parchment border-border-soft flex items-center gap-2.5 rounded-md border-2 px-2.5 py-2"
          >
            <div class="flex flex-1 flex-col">
              <span class="text-sm font-extrabold">{{ relativeTime(new Date(b.created), new Date(), locale) }}</span>
              <span class="text-muted-foreground text-xs font-bold">{{ t(`saves.reason.${b.reason}`) }}</span>
            </div>
            <Button size="sm" variant="outline" :disabled="busy || game.running" @click="restoreTarget = b">
              <ArchiveRestoreIcon />{{ t("saves.restore") }}
            </Button>
          </div>
          <div class="flex-1" />
          <div class="grid grid-cols-2 gap-2">
            <Button :disabled="busy" @click="backup"><SaveIcon />{{ t("saves.backup") }}</Button>
            <Button variant="outline" :disabled="busy" @click="exportSave"
              ><DownloadIcon />{{ t("saves.export") }}</Button
            >
            <Button
              variant="destructive"
              class="col-span-2"
              :disabled="busy || game.running || !selected"
              @click="deleteTarget = selected"
              ><Trash2Icon />{{ t("saves.delete") }}</Button
            >
          </div>
        </aside>
      </div>
    </template>

    <Dialog :open="restoreTarget !== null" @update:open="(v: boolean) => !v && (restoreTarget = null)">
      <DialogContent>
        <DialogHeader>
          <DialogTitle class="font-pixel text-xl">{{ t("saves.restoreTitle") }}</DialogTitle>
          <DialogDescription>{{ t("saves.restoreText") }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <DialogClose as-child>
            <Button variant="outline">{{ t("common.cancel") }}</Button>
          </DialogClose>
          <Button @click="restore">{{ t("saves.restore") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog :open="deleteTarget !== null" @update:open="(v: boolean) => !v && (deleteTarget = null)">
      <DialogContent>
        <DialogHeader>
          <DialogTitle class="font-pixel text-xl">{{
            t("saves.deleteTitle", { name: deleteTarget ? `${deleteTarget.farmer} · ${deleteTarget.farmName}` : "" })
          }}</DialogTitle>
          <DialogDescription>{{ t("saves.deleteText") }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <DialogClose as-child>
            <Button variant="outline">{{ t("common.cancel") }}</Button>
          </DialogClose>
          <Button variant="destructive" @click="deleteSave"><Trash2Icon />{{ t("saves.delete") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
