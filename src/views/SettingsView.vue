<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpenIcon, RefreshCwIcon, Trash2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import SmapiProgress from "@/components/SmapiProgress.vue";
import NexusAccountCard from "@/components/NexusAccountCard.vue";
import AboutCard from "@/components/AboutCard.vue";
import PixelToggle from "@/components/PixelToggle.vue";
import { api, companion as companionApi, saves } from "@/lib/api";
import { errorText, setLocale } from "@/i18n";
import { setWikiSource, wikiSource } from "@/lib/wiki";
import StorageCard from "@/components/settings/StorageCard.vue";
import { useGame } from "@/stores/game";
import GameInstalls from "@/components/settings/GameInstalls.vue";
import { useCompanion } from "@/stores/companion";

const { t } = useI18n();
const game = useGame();
const locale = ref("system");
const confirmUninstall = ref(false);
const companion = useCompanion();
const companionOn = ref(true);

onMounted(async () => {
  try {
    const s = await api.settings();
    locale.value = s.locale ?? "system";
    companionOn.value = !s.companionDisabled;
    backupFolder.value = s.backupFolder;
  } catch (e) {
    toast.error(errorText(e));
  }
});

async function toggleCompanion() {
  try {
    companionOn.value = !(await companionApi.setEnabled(!companionOn.value)).companionDisabled;
  } catch (e) {
    toast.error(errorText(e));
  }
}

async function changeLocale(value: unknown) {
  const next = value === "system" ? null : String(value);
  try {
    await api.setLocale(next);
    setLocale(next);
  } catch (e) {
    toast.error(errorText(e));
  }
}

const backupFolder = ref<string | null>(null);

async function pickBackupFolder() {
  const folder = await open({ directory: true, title: t("settings.backupPick") });
  if (!folder) return;
  await setBackupFolder(folder);
}

async function setBackupFolder(folder: string | null) {
  try {
    backupFolder.value = (await api.setBackupFolder(folder)).backupFolder;
    // copy right away instead of waiting for the next session
    if (folder) {
      const n = await saves.mirror();
      toast.success(t("settings.backupDone", { n }));
    }
  } catch (e) {
    toast.error(errorText(e));
  }
}

async function changeWiki(value: unknown) {
  try {
    setWikiSource((await api.setWiki(String(value))).wiki);
  } catch (e) {
    toast.error(errorText(e));
  }
}

async function uninstall() {
  confirmUninstall.value = false;
  await game.smapi("uninstall");
}
</script>

<template>
  <div class="flex max-w-[760px] flex-col gap-4">
    <h1 class="font-pixel text-wood-dark text-3xl font-bold">{{ t("settings.title") }}</h1>

    <GameInstalls />

    <section class="wood-panel flex flex-col gap-3 p-5">
      <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("settings.smapi") }}</h2>
      <SmapiProgress v-if="game.progress" :progress="game.progress" />
      <div v-else class="flex flex-wrap items-center gap-2">
        <span class="flex-1 text-sm font-bold">
          {{
            game.install?.smapiVersion
              ? t("home.smapiInstalled", { version: game.install.smapiVersion })
              : t("play.smapiMissing")
          }}
        </span>
        <Button variant="outline" :disabled="game.running || game.install?.needsRosetta" @click="game.smapi('install')">
          <RefreshCwIcon />{{ game.install?.smapiVersion ? t("settings.reinstall") : t("home.install") }}
        </Button>
        <Button
          v-if="game.install?.smapiVersion"
          variant="destructive"
          :disabled="game.running"
          @click="confirmUninstall = true"
        >
          <Trash2Icon />{{ t("settings.uninstall") }}
        </Button>
      </div>
    </section>

    <NexusAccountCard />

    <section v-if="companion.available" class="wood-panel flex flex-col gap-2 p-5">
      <div class="flex items-center gap-3">
        <h2 class="font-pixel text-wood-dark flex-1 text-xl font-bold">{{ t("settings.companion") }}</h2>
        <PixelToggle :checked="companionOn" :label="t('settings.companion')" @toggle="toggleCompanion" />
      </div>
      <p class="text-muted-foreground text-sm leading-relaxed font-semibold">{{ t("settings.companionHint") }}</p>
      <p v-if="game.running && companionOn" class="text-muted-foreground text-xs font-bold">
        {{ companion.connected ? t("settings.companionConnected") : t("settings.companionWaiting") }}
      </p>
    </section>

    <section class="wood-panel flex items-center gap-3 p-5">
      <h2 class="font-pixel text-wood-dark flex-1 text-xl font-bold">{{ t("settings.language") }}</h2>
      <Select v-model="locale" @update:model-value="changeLocale">
        <SelectTrigger class="bg-parchment border-input w-48 border-2 font-bold">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="system">{{ t("settings.languageSystem") }}</SelectItem>
          <SelectItem value="ru">Русский</SelectItem>
          <SelectItem value="en">English</SelectItem>
          <SelectItem value="de">Deutsch</SelectItem>
          <SelectItem value="es">Español</SelectItem>
          <SelectItem value="fr">Français</SelectItem>
          <SelectItem value="pt">Português</SelectItem>
          <SelectItem value="it">Italiano</SelectItem>
          <SelectItem value="ja">日本語</SelectItem>
          <SelectItem value="ko">한국어</SelectItem>
          <SelectItem value="zh">中文</SelectItem>
          <SelectItem value="tr">Türkçe</SelectItem>
          <SelectItem value="hu">Magyar</SelectItem>
        </SelectContent>
      </Select>
    </section>

    <section class="wood-panel flex flex-col gap-2 p-5">
      <div class="flex flex-wrap items-center gap-3">
        <h2 class="font-pixel text-wood-dark flex-1 text-xl font-bold">{{ t("settings.backupFolder") }}</h2>
        <Button variant="outline" @click="pickBackupFolder"><FolderOpenIcon />{{ t("settings.backupPick") }}</Button>
        <Button v-if="backupFolder" variant="ghost" @click="setBackupFolder(null)">{{
          t("settings.backupOff")
        }}</Button>
      </div>
      <p class="text-muted-foreground text-sm leading-relaxed font-semibold">
        {{ backupFolder ? t("settings.backupOn", { folder: backupFolder }) : t("settings.backupHint") }}
      </p>
    </section>

    <section class="wood-panel flex flex-col gap-2 p-5">
      <div class="flex items-center gap-3">
        <h2 class="font-pixel text-wood-dark flex-1 text-xl font-bold">{{ t("settings.wiki") }}</h2>
        <Select :model-value="wikiSource" @update:model-value="changeWiki">
          <SelectTrigger class="bg-parchment border-input w-48 border-2 font-bold">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="official">{{ t("settings.wikiOfficial") }}</SelectItem>
            <SelectItem value="fandom">Fandom</SelectItem>
            <SelectItem value="off">{{ t("settings.wikiOff") }}</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <p class="text-muted-foreground text-sm leading-relaxed font-semibold">{{ t("settings.wikiHint") }}</p>
    </section>

    <StorageCard />

    <AboutCard />

    <Dialog v-model:open="confirmUninstall">
      <DialogContent>
        <DialogHeader>
          <DialogTitle class="font-pixel text-xl">{{ t("settings.uninstall") }}</DialogTitle>
          <DialogDescription>{{ t("settings.uninstallConfirm") }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <DialogClose as-child>
            <Button variant="outline">{{ t("common.cancel") }}</Button>
          </DialogClose>
          <Button variant="destructive" @click="uninstall">{{ t("settings.uninstall") }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
