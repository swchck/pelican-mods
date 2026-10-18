<script setup lang="ts">
import { onMounted } from "vue";
import { Loader2Icon } from "@lucide/vue";
import { Toaster } from "@/components/ui/sonner";
import "vue-sonner/style.css";
import AppSidebar from "@/components/AppSidebar.vue";
import SetupView from "@/views/SetupView.vue";
import SmapiPrompt from "@/components/SmapiPrompt.vue";
import NexusKeyDialog from "@/components/NexusKeyDialog.vue";
import { useGame } from "@/stores/game";
import { useNexus } from "@/stores/nexus";

const game = useGame();

const nexus = useNexus();

onMounted(() => {
  void game.init();
  void nexus.init();
});
</script>

<template>
  <div v-if="game.phase === 'loading'" class="grid h-full place-items-center" data-tauri-drag-region>
    <Loader2Icon class="text-muted-foreground size-6 animate-spin" />
  </div>
  <SetupView v-else-if="game.phase === 'setup'" />
  <div v-else class="flex h-full">
    <AppSidebar />
    <main class="min-w-0 flex-1 overflow-auto px-6 pt-6 pb-5">
      <RouterView />
    </main>
  </div>
  <SmapiPrompt />
  <NexusKeyDialog />
  <Toaster position="bottom-right" rich-colors />
</template>
