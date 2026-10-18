<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronUpIcon, PlayIcon, SquareIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const game = useGame();

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
      <span class="size-2 rounded-[2px] bg-current" />{{ status.text }}
    </div>

    <Button v-if="game.running" variant="destructive" size="xl" class="w-full" @click="game.stop()">
      <SquareIcon />{{ t("play.stop") }}
    </Button>
    <div v-else class="flex">
      <Button
        variant="play"
        size="xl"
        class="flex-1 rounded-r-none"
        :disabled="game.busy || !game.install?.smapiVersion || game.install.needsRosetta"
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
  </div>
</template>
