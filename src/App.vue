<script setup lang="ts">
import { onBeforeUnmount, onMounted } from "vue";
import { useRouter } from "vue-router";
import { Loader2Icon } from "@lucide/vue";
import { Toaster } from "@/components/ui/sonner";
import "vue-sonner/style.css";
import AppSidebar from "@/components/AppSidebar.vue";
import SetupView from "@/views/SetupView.vue";
import SmapiPrompt from "@/components/SmapiPrompt.vue";
import NexusKeyDialog from "@/components/NexusKeyDialog.vue";
import NxmConfirmDialog from "@/components/NxmConfirmDialog.vue";
import AppUpdateDialogs from "@/components/AppUpdateDialogs.vue";
import ShortcutsDialog from "@/components/ShortcutsDialog.vue";
import { useShortcut, useShortcuts } from "@/stores/shortcuts";
import { useAppUpdate } from "@/stores/appUpdate";
import { useCompanion } from "@/stores/companion";
import { useGame } from "@/stores/game";
import { useNexus } from "@/stores/nexus";

const game = useGame();

const nexus = useNexus();
const router = useRouter();
const shortcuts = useShortcuts();
const uninstall = shortcuts.install();
onBeforeUnmount(uninstall);

const go = (path: string) => () => void router.push(path);
useShortcut("nav.library", go("/"));
useShortcut("nav.browse", go("/browse"));
useShortcut("nav.saves", go("/saves"));
useShortcut("nav.progress", go("/progress"));
useShortcut("nav.helpers", go("/helpers"));
useShortcut("nav.game", go("/game"));
useShortcut("nav.console", go("/console"));
useShortcut("nav.settings", go("/settings"));
useShortcut("help", () => (shortcuts.help = !shortcuts.help));
// the same guards as the Play card's buttons
const canPlay = () => game.phase === "ready" && !game.running && !game.busy && !game.install?.needsRosetta;
useShortcut("game.play", () => canPlay() && game.install?.smapiVersion && void game.launch(true));
useShortcut("game.vanilla", () => canPlay() && void game.launch(false));
useShortcut("game.stop", () => game.running && void game.stop());

onMounted(() => {
  void game.init();
  void nexus.init();
  void useAppUpdate().init();
  void useCompanion().init();
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
  <NxmConfirmDialog />
  <AppUpdateDialogs />
  <ShortcutsDialog />
  <Toaster position="bottom-right" rich-colors />
</template>
