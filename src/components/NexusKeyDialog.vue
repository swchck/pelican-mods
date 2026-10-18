<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { ClipboardCheckIcon, ExternalLinkIcon, KeyRoundIcon, Loader2Icon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { looksLikeNexusKey } from "@/lib/nexus-key";
import { useNexus } from "@/stores/nexus";

const KEYS_PAGE = "https://www.nexusmods.com/settings/api-keys";

const { t } = useI18n();
const nx = useNexus();
const key = ref("");
const fromClipboard = ref(false);
const saving = ref(false);
let unlisten: UnlistenFn | undefined;

// read only while this dialog is open: the user started the flow and expects the paste
async function tryClipboard() {
  if (!nx.keyDialog || key.value.trim()) return;
  try {
    const text = await readText();
    if (looksLikeNexusKey(text)) {
      key.value = text.trim();
      fromClipboard.value = true;
    }
  } catch {
    // empty or non-text clipboard
  }
}

watch(
  () => nx.keyDialog,
  (open) => {
    if (open) {
      key.value = "";
      fromClipboard.value = false;
      void tryClipboard();
    } else {
      nx.pending = null;
    }
  },
);

onMounted(async () => {
  unlisten = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (focused) void tryClipboard();
  });
});
onUnmounted(() => unlisten?.());

async function submit() {
  saving.value = true;
  await nx.setKey(key.value);
  saving.value = false;
}
</script>

<template>
  <Dialog v-model:open="nx.keyDialog">
    <DialogContent>
      <form class="flex flex-col gap-4" @submit.prevent="submit">
        <DialogHeader>
          <DialogTitle class="font-pixel text-wood-dark text-2xl">{{ t("nexusKey.title") }}</DialogTitle>
          <DialogDescription class="text-foreground text-sm leading-relaxed">
            {{ nx.pending ? t("nexusKey.forDownload") : t("nexusKey.about") }}
          </DialogDescription>
        </DialogHeader>

        <ol class="flex list-decimal flex-col gap-1.5 pl-5 text-sm">
          <li>
            <Button type="button" variant="link" size="sm" class="h-auto px-0" @click="openUrl(KEYS_PAGE)">
              <ExternalLinkIcon />{{ t("nexusKey.step1") }}
            </Button>
          </li>
          <li>{{ t("nexusKey.step2") }}</li>
          <li>{{ t("nexusKey.step3") }}</li>
        </ol>

        <div class="flex flex-col gap-1.5">
          <Label for="nexus-key-input" class="font-bold">{{ t("nexus.key") }}</Label>
          <Input
            id="nexus-key-input"
            v-model="key"
            type="password"
            autocomplete="off"
            class="bg-parchment border-input border-2 font-mono"
            @input="fromClipboard = false"
          />
          <span v-if="fromClipboard" class="text-success flex items-center gap-1 text-xs font-bold">
            <ClipboardCheckIcon class="size-3.5" />{{ t("nexusKey.pasted") }}
          </span>
          <span class="text-muted-foreground text-xs">{{ t("nexusKey.keychain") }}</span>
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" @click="nx.keyDialog = false">{{ t("common.cancel") }}</Button>
          <Button type="submit" :disabled="!key.trim() || saving">
            <Loader2Icon v-if="saving" class="animate-spin" /><KeyRoundIcon v-else />{{ t("nexus.connect") }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
