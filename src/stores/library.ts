import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { toast } from "vue-sonner";
import { library as api, type LibraryView } from "@/lib/api";
import { errorText } from "@/i18n";

export const useLibrary = defineStore("library", () => {
  const view = ref<LibraryView | null>(null);
  const selectedId = ref<string | null>(null);
  const filter = ref("");
  const working = ref(false);

  const packages = computed(() => view.value?.packages ?? []);
  const selected = computed(() => packages.value.find((p) => p.id === selectedId.value) ?? null);
  const profiles = computed(() => view.value?.profiles ?? null);
  const activeProfile = computed(() => profiles.value?.profiles.find((p) => p.id === profiles.value?.active) ?? null);

  /** Runs a library command, swapping in the view it returns; failures become toasts. */
  async function run(op: () => Promise<LibraryView>): Promise<boolean> {
    working.value = true;
    try {
      view.value = await op();
      if (selectedId.value && !packages.value.some((p) => p.id === selectedId.value)) selectedId.value = null;
      return true;
    } catch (e) {
      toast.error(errorText(e), { duration: 10000 });
      return false;
    } finally {
      working.value = false;
    }
  }

  return {
    view,
    selectedId,
    filter,
    working,
    packages,
    selected,
    profiles,
    activeProfile,
    load: () => run(api.get),
    install: (paths: string[]) => run(() => api.install(paths)),
    importGameMods: (names: string[]) => run(() => api.import(names)),
    remove: (id: string) => run(() => api.remove(id)),
    toggle: (id: string, on: boolean) => run(() => api.toggle(id, on)),
    createProfile: (name: string, copyFrom: string | null) => run(() => api.createProfile(name, copyFrom)),
    renameProfile: (id: string, name: string) => run(() => api.renameProfile(id, name)),
    deleteProfile: (id: string) => run(() => api.deleteProfile(id)),
    activateProfile: (id: string) => run(() => api.activateProfile(id)),
  };
});
