<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { KeyRoundIcon, LinkIcon, LogOutIcon } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useNexus } from "@/stores/nexus";

const { t } = useI18n();
const nx = useNexus();
const nxm = ref("");

onMounted(() => {
  if (!nx.accountChecked) void nx.loadAccount();
});

async function paste() {
  if (await nx.openNxm(nxm.value)) nxm.value = "";
}
</script>

<template>
  <section class="wood-panel flex flex-col gap-3 p-5">
    <h2 class="font-pixel text-wood-dark text-xl font-bold">Nexus Mods</h2>

    <div v-if="nx.account" class="flex items-center gap-3">
      <span class="flex-1 text-sm font-bold">
        {{ t("nexus.account", { name: nx.account.name }) }}
        <span
          class="ml-1 rounded border-2 px-1.5 py-0.5 text-xs font-extrabold"
          :class="nx.account.isPremium ? 'bg-gold border-[#b57f17]' : 'border-[#cdba98] bg-[#ede3d0]'"
          >{{ nx.account.isPremium ? "Premium" : t("nexus.free") }}</span
        >
      </span>
      <Button variant="outline" size="sm" @click="nx.clearKey()"><LogOutIcon />{{ t("nexus.disconnect") }}</Button>
    </div>
    <div v-else class="flex items-center gap-3">
      <p class="flex-1 text-sm leading-relaxed">{{ t("nexus.keyAbout") }}</p>
      <Button @click="nx.keyDialog = true"><KeyRoundIcon />{{ t("nexus.connect") }}</Button>
    </div>

    <p v-if="nx.account && !nx.account.isPremium" class="text-muted-foreground text-xs font-semibold">
      {{ t("nexus.freeHint") }}
    </p>

    <form class="flex flex-col gap-1.5" @submit.prevent="paste">
      <Label for="nxm" class="font-bold">{{ t("nexus.pasteNxm") }}</Label>
      <div class="flex gap-2">
        <Input
          id="nxm"
          v-model="nxm"
          placeholder="nxm://stardewvalley/mods/…"
          class="bg-parchment border-input border-2 font-mono text-xs"
        />
        <Button type="submit" variant="outline" :disabled="!nxm.trim()"><LinkIcon />{{ t("nexus.go") }}</Button>
      </div>
    </form>
  </section>
</template>
