<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { CpuIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { useGame } from "@/stores/game";

const { t } = useI18n();
const game = useGame();
const working = ref(false);

async function install() {
  working.value = true;
  try {
    await game.installRosetta();
  } finally {
    working.value = false;
  }
}
</script>

<template>
  <section
    class="bg-warning-soft flex items-start gap-4 rounded-[10px] border-[3px] border-[#8e2f1f] p-5 shadow-[0_4px_0_#d98b76]"
    role="alert"
  >
    <CpuIcon class="text-destructive mt-1 size-6 shrink-0" />
    <div class="flex flex-1 flex-col gap-2">
      <h2 class="font-pixel text-destructive text-xl font-bold">{{ t("rosetta.title") }}</h2>
      <p class="text-sm leading-relaxed">{{ t("rosetta.about") }}</p>
      <p class="text-muted-foreground text-xs font-semibold">{{ t("rosetta.license") }}</p>
      <Button class="self-start" :disabled="working" @click="install">
        <Loader2Icon v-if="working" class="animate-spin" />{{ t("rosetta.install") }}
      </Button>
    </div>
  </section>
</template>
