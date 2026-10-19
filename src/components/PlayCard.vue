<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronUpIcon, PlayIcon, SquareIcon } from "@lucide/vue";
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
import { useLibrary } from "@/stores/library";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useGame } from "@/stores/game";
import { hint } from "@/stores/shortcuts";
import { useCompanion } from "@/stores/companion";
import { formatClock, seasonKey } from "@/lib/game-clock";

const { t } = useI18n();
const game = useGame();
const companion = useCompanion();
const lib = useLibrary();

const switchName = computed(() => {
  const id = game.preflight?.switchTo;
  const p = id ? lib.profiles?.profiles.find((x) => x.id === id) : null;
  return p ? (p.name ?? t("profiles.default")) : null;
});

async function switchAndPlay() {
  const id = game.preflight?.switchTo;
  if (id && (await lib.activateProfile(id))) await game.launch(true);
}

async function enableAndPlay() {
  const off = game.preflight?.missing.filter((m) => m.disabled) ?? [];
  for (const m of off) if (!(await lib.toggle(m.package, true))) return;
  await game.launch(true);
}

const live = computed(() => {
  const s = companion.status.state;
  if (!game.running || !s) return null;
  const season = seasonKey(s.season);
  return t("live.playing", {
    season: season ? t(`live.seasons.${season}`) : s.season,
    day: s.day,
    time: formatClock(s.time),
  });
});

const status = computed(() => {
  const r = game.run;
  if (r.state === "running") return { tone: "ok", text: t("play.running") };
  if (r.state === "exited")
    return {
      tone: r.code ? "bad" : "muted",
      text: r.code === null ? t("play.exitedNoCode") : t("play.exited", { code: r.code }),
    };
  if (game.install?.needsRosetta) return { tone: "bad", text: t("rosetta.status") };
  if (!game.install?.smapiVersion) return { tone: "bad", text: t("play.smapiMissing") };
  return null;
});
</script>

<template>
  <div
    data-test="play-card"
    class="bg-card border-wood-dark flex flex-col gap-2.5 rounded-lg border-[3px] p-3 shadow-[inset_0_0_0_2px_#e8c48a]"
  >
    <div class="text-muted-foreground flex justify-between text-xs font-bold">
      <span>Stardew {{ game.install?.version?.split(".").slice(0, 3).join(".") ?? "—" }}</span>
      <span v-if="game.install?.smapiVersion">SMAPI {{ game.install.smapiVersion }}</span>
    </div>
    <div
      v-if="status"
      class="flex items-center gap-1.5 text-xs font-bold"
      :class="{
        'text-success': status.tone === 'ok',
        'text-destructive': status.tone === 'bad',
        'text-muted-foreground': status.tone === 'muted',
      }"
    >
      <span class="size-2 shrink-0 rounded-[2px] bg-current" /><span class="shrink-0">{{ status.text }}</span>
      <span v-if="live" class="text-muted-foreground ml-auto truncate">{{ live }}</span>
    </div>

    <Button
      v-if="game.running"
      variant="destructive"
      size="xl"
      class="font-pixel w-full min-w-0 px-3 text-xl"
      :title="`${t('play.stop')} (${hint('game.stop')})`"
      @click="game.stop()"
    >
      <SquareIcon />{{ t("play.stop") }}
    </Button>
    <div v-else class="flex">
      <Button
        variant="play"
        size="xl"
        class="min-w-0 flex-1 rounded-r-none px-3 text-xl"
        :disabled="game.busy || !game.install?.smapiVersion || game.install.needsRosetta"
        :title="`${t('play.play')} (${hint('game.play')})`"
        @click="game.launch(true)"
      >
        <PlayIcon class="fill-current" />{{ t("play.play") }}
      </Button>
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <Button
            variant="play"
            size="xl"
            class="w-9 rounded-l-none border-l-0 px-0"
            :disabled="game.busy || game.install?.needsRosetta"
            :aria-label="t('play.playVanilla')"
          >
            <ChevronUpIcon />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="end">
          <DropdownMenuItem @select="game.launch(false)">{{ t("play.playVanilla") }}</DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>

    <Dialog :open="game.preflight !== null" @update:open="(v: boolean) => !v && (game.preflight = null)">
      <DialogContent v-if="game.preflight">
        <DialogHeader>
          <DialogTitle class="font-pixel text-xl">{{
            game.preflight.missing.length ? t("preflight.title") : t("preflight.otherProfile")
          }}</DialogTitle>
          <DialogDescription>
            <template v-if="game.preflight.missing.length">{{
              t("preflight.text", { save: `${game.preflight.farmer} · ${game.preflight.farmName}` })
            }}</template>
            <template v-if="switchName">
              {{
                t("preflight.lastProfile", {
                  save: `${game.preflight.farmer} · ${game.preflight.farmName}`,
                  profile: switchName,
                })
              }}
            </template>
          </DialogDescription>
        </DialogHeader>
        <ul class="flex flex-col gap-1 text-sm">
          <li v-for="m in game.preflight.missing" :key="m.package" class="flex flex-wrap items-baseline gap-x-2">
            <span class="font-bold">{{ m.name }}</span>
            <span class="text-muted-foreground text-xs font-semibold">
              {{ m.disabled ? t("preflight.disabled") : t("preflight.skipped") }}
              <template v-if="m.locationCount"> · {{ t("preflight.locations", { n: m.locationCount }) }}</template>
            </span>
          </li>
        </ul>
        <p v-if="game.preflight.missing.length" class="text-destructive text-xs font-bold">{{ t("preflight.risk") }}</p>
        <DialogFooter class="flex-wrap gap-2">
          <DialogClose as-child>
            <Button variant="outline">{{ t("common.cancel") }}</Button>
          </DialogClose>
          <Button variant="outline" @click="game.launch(true, true)">{{ t("preflight.anyway") }}</Button>
          <Button v-if="switchName" @click="switchAndPlay">{{ t("preflight.switch", { profile: switchName }) }}</Button>
          <Button v-else-if="game.preflight.missing.some((m) => m.disabled)" @click="enableAndPlay">
            {{ t("preflight.enable") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
