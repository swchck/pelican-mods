<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { EllipsisIcon, FileDownIcon, FileUpIcon, PencilIcon, PlusIcon, Trash2Icon, UsersIcon } from "@lucide/vue";
import { open as openFile, save } from "@tauri-apps/plugin-dialog";
import { toast } from "vue-sonner";
import ModListDialog from "@/components/library/ModListDialog.vue";
import { library } from "@/lib/api";
import { errorText } from "@/i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { ModListView, Profile } from "@/lib/api";
import { useLibrary } from "@/stores/library";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const lib = useLibrary();
const game = useGame();

const dialog = ref<"create" | "rename" | "delete" | null>(null);
const name = ref("");
const copy = ref(true);

const nameOf = (p: Profile) => p.name ?? t("profiles.default");

const imported = ref<{ file: string; view: ModListView; mode: "import" | "compare" } | null>(null);
const importOpen = ref(false);

async function exportList() {
  const base = lib.activeProfile ? nameOf(lib.activeProfile) : "mods";
  const dest = await save({ defaultPath: `${base}.json`, filters: [{ name: "JSON", extensions: ["json"] }] });
  if (!dest) return;
  try {
    await library.exportList(dest);
    toast.success(t("modList.exported"));
  } catch (e) {
    toast.error(errorText(e));
  }
}

async function importList(mode: "import" | "compare" = "import") {
  const file = await openFile({ filters: [{ name: "JSON", extensions: ["json"] }] });
  if (!file) return;
  try {
    imported.value = { file, view: await library.readList(file), mode };
    importOpen.value = true;
  } catch (e) {
    toast.error(errorText(e));
  }
}
const active = computed({
  get: () => lib.profiles?.active ?? "",
  set: (id: string) => void lib.activateProfile(id),
});
const canDelete = computed(() => (lib.profiles?.profiles.length ?? 0) > 1);

function open(kind: "create" | "rename" | "delete") {
  name.value = kind === "rename" && lib.activeProfile ? nameOf(lib.activeProfile) : "";
  copy.value = true;
  dialog.value = kind;
}

async function submit() {
  const id = lib.activeProfile?.id;
  const ok =
    dialog.value === "create"
      ? await lib.createProfile(name.value, copy.value ? (id ?? null) : null)
      : dialog.value === "rename" && id
        ? await lib.renameProfile(id, name.value)
        : dialog.value === "delete" && id
          ? await lib.deleteProfile(id)
          : false;
  if (ok) dialog.value = null;
}
</script>

<template>
  <div class="flex items-center gap-1">
    <Select v-model="active" :disabled="game.running || lib.working">
      <SelectTrigger
        :aria-label="t('profiles.label')"
        class="bg-card border-input h-9 min-w-32 border-2 font-extrabold"
      >
        <span class="text-muted-foreground hidden text-xs font-bold @4xl:inline">{{ t("profiles.label") }}</span>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem v-for="p in lib.profiles?.profiles ?? []" :key="p.id" :value="p.id">{{ nameOf(p) }}</SelectItem>
      </SelectContent>
    </Select>
    <DropdownMenu>
      <DropdownMenuTrigger as-child>
        <Button variant="ghost" size="icon" :aria-label="t('profiles.label')" :disabled="game.running">
          <EllipsisIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuItem @select="open('create')"><PlusIcon />{{ t("profiles.create") }}</DropdownMenuItem>
        <DropdownMenuItem @select="open('rename')"><PencilIcon />{{ t("profiles.rename") }}</DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem @select="exportList"><FileUpIcon />{{ t("modList.export") }}</DropdownMenuItem>
        <DropdownMenuItem @select="importList()"><FileDownIcon />{{ t("modList.import") }}</DropdownMenuItem>
        <DropdownMenuItem @select="importList('compare')"><UsersIcon />{{ t("modList.compare") }}</DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem :disabled="!canDelete" class="text-destructive" @select="open('delete')">
          <Trash2Icon />{{ t("profiles.delete") }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>

    <Dialog :open="dialog !== null" @update:open="(v: boolean) => !v && (dialog = null)">
      <DialogContent>
        <form class="flex flex-col gap-4" @submit.prevent="submit">
          <DialogHeader>
            <DialogTitle class="font-pixel text-xl">
              {{
                dialog === "create"
                  ? t("profiles.createTitle")
                  : dialog === "rename"
                    ? t("profiles.renameTitle")
                    : t("profiles.delete")
              }}
            </DialogTitle>
            <DialogDescription v-if="dialog === 'delete' && lib.activeProfile">
              {{ t("profiles.deleteConfirm", { name: nameOf(lib.activeProfile) }) }}
            </DialogDescription>
          </DialogHeader>
          <template v-if="dialog !== 'delete'">
            <div class="flex flex-col gap-1.5">
              <Label for="profile-name" class="font-bold">{{ t("profiles.name") }}</Label>
              <Input id="profile-name" v-model="name" class="bg-parchment border-input border-2" autofocus />
            </div>
            <div v-if="dialog === 'create'" class="flex items-center gap-2">
              <Switch id="profile-copy" v-model="copy" />
              <Label for="profile-copy" class="font-semibold">{{ t("profiles.copy") }}</Label>
            </div>
          </template>
          <DialogFooter>
            <DialogClose as-child>
              <Button type="button" variant="outline">{{ t("common.cancel") }}</Button>
            </DialogClose>
            <Button
              type="submit"
              :variant="dialog === 'delete' ? 'destructive' : 'default'"
              :disabled="lib.working || (dialog !== 'delete' && !name.trim())"
            >
              {{ dialog === "delete" ? t("profiles.delete") : t("profiles.save") }}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
    <ModListDialog
      v-if="imported"
      :key="imported.file"
      v-model:open="importOpen"
      :file="imported.file"
      :initial="imported.view"
      :mode="imported.mode"
    />
  </div>
</template>
