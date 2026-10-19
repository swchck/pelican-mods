import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { companion as api, type CompanionStatus } from "@/lib/api";

/** The live link to a running game through the companion mod. */
export const useCompanion = defineStore("companion", () => {
  const status = ref<CompanionStatus>({ connected: false, hello: null, state: null });
  const available = ref(false);
  /** Bumped when the game reports GMCM menus, so open settings can pick them up. */
  const gmcmSeen = ref<{ ids: string[]; at: number }>({ ids: [], at: 0 });
  /** Bumped when the game sends fresh perfection lists. */
  const catalogAt = ref(0);

  const connected = computed(() => status.value.connected && status.value.hello !== null);
  const loaded = (uniqueId: string) =>
    connected.value && (status.value.hello?.mods.some((m) => m.toLowerCase() === uniqueId.toLowerCase()) ?? false);

  async function init() {
    await Promise.all([
      api.onStatus((s) => (status.value = s)),
      api.onGmcm((ids) => (gmcmSeen.value = { ids, at: Date.now() })),
      api.onCatalog(() => (catalogAt.value = Date.now())),
    ]);
    try {
      [available.value, status.value] = await Promise.all([api.available(), api.status()]);
    } catch {
      // stays "not connected"; the next status event corrects it
    }
  }

  return { status, available, connected, gmcmSeen, catalogAt, loaded, init };
});
