import { ref } from "vue";
import { defineStore } from "pinia";
import { api } from "@/lib/api";

/** Settings several screens read; the Settings screen still owns the rest. */
export const useSettings = defineStore("settings", () => {
  const spoilers = ref(false);
  let loaded: Promise<void> | null = null;

  function load() {
    loaded ??= api
      .settings()
      .then((s) => void (spoilers.value = s.spoilers ?? false))
      .catch(() => {});
    return loaded;
  }

  async function setSpoilers(on: boolean) {
    spoilers.value = (await api.setSpoilers(on)).spoilers ?? false;
  }

  return { spoilers, load, setSpoilers };
});
