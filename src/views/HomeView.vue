<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { FileTextIcon, FolderIcon, PackageIcon, SaveIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import SmapiCard from "@/components/SmapiCard.vue";
import RosettaBanner from "@/components/RosettaBanner.vue";
import { api, type Place } from "@/lib/api";
import { errorText } from "@/i18n";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const game = useGame();

const places: { place: Place; label: string; icon: typeof FolderIcon }[] = [
  { place: "game", label: "home.openGame", icon: FolderIcon },
  { place: "mods", label: "home.openMods", icon: PackageIcon },
  { place: "saves", label: "home.openSaves", icon: SaveIcon },
  { place: "smapiLog", label: "home.openLog", icon: FileTextIcon },
];

async function openPlace(place: Place) {
  try {
    await api.open(place);
  } catch (e) {
    toast.error(errorText(e));
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <h1 class="font-pixel text-wood-dark text-3xl font-bold">{{ t("home.title") }}</h1>

    <section class="wood-panel overflow-hidden" :aria-label="t('home.game')">
      <div class="h-24 border-b-[3px] border-[#5b3a1e] bg-[#cfe6f5]">
        <div class="grass-strip mt-14 h-10 border-t-4 border-[#3f7f2a]" />
      </div>
      <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-1.5 p-5 text-sm">
        <dt class="text-muted-foreground font-bold">{{ t("home.game") }}</dt>
        <dd class="font-extrabold">Stardew Valley · {{ t(`store.${game.install?.store ?? "other"}`) }}</dd>
        <dt class="text-muted-foreground font-bold">{{ t("home.version") }}</dt>
        <dd class="font-mono font-bold">{{ game.install?.version ?? "—" }}</dd>
        <dt class="text-muted-foreground font-bold">{{ t("home.path") }}</dt>
        <dd class="truncate font-mono text-xs leading-5 select-text" :title="game.install?.path">
          {{ game.install?.path }}
        </dd>
      </dl>
    </section>

    <RosettaBanner v-if="game.install?.needsRosetta" />
    <SmapiCard />

    <section class="flex flex-col gap-2.5" :aria-label="t('home.places')">
      <h2 class="font-pixel text-wood-dark text-xl font-bold">{{ t("home.places") }}</h2>
      <div class="flex flex-wrap gap-2">
        <Button v-for="p in places" :key="p.place" variant="outline" @click="openPlace(p.place)">
          <component :is="p.icon" />{{ t(p.label) }}
        </Button>
      </div>
    </section>
  </div>
</template>
