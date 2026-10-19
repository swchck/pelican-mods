import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { toast } from "vue-sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { library as api, type LibraryView, type PackageView, type UpdateReport } from "@/lib/api";
import { packageUpdate } from "@/lib/packages";
import { errorText, i18n } from "@/i18n";
import { useNexus } from "./nexus";

export const useLibrary = defineStore("library", () => {
  const view = ref<LibraryView | null>(null);
  const selectedId = ref<string | null>(null);
  const filter = ref("");
  const working = ref(false);
  const report = ref<UpdateReport | null>(null);
  const checking = ref(false);
  const updating = ref(new Set<string>());

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

  const updatable = computed(() => packages.value.filter((p) => packageUpdate(p, report.value)));
  // what "Update all" can do without a browser: GitHub and ModDrop always, Nexus only with Premium
  const directUpdates = computed(() => {
    const premium = useNexus().account?.isPremium ?? false;
    return updatable.value.filter((p) => {
      const via = packageUpdate(p, report.value)?.via;
      return via === "github" || via === "moddrop" || (via === "nexus" && premium);
    });
  });

  async function checkUpdates(force: boolean) {
    checking.value = true;
    try {
      report.value = await api.checkUpdates(force);
    } catch (e) {
      // the automatic check runs at startup, often offline; only a manual one reports
      if (force) toast.error(errorText(e));
    } finally {
      checking.value = false;
    }
  }

  async function load() {
    const ok = await run(api.get);
    if (ok) void checkUpdates(false);
    return ok;
  }

  /**
   * Updates in place from GitHub, queues a Nexus download, or opens the page where
   * only a browser can get the file.
   */
  async function update(p: PackageView) {
    const u = packageUpdate(p, report.value);
    if (!u) return;
    if (u.via === "page") {
      await openUrl(u.url);
      return;
    }
    updating.value = new Set(updating.value).add(p.id);
    try {
      const outcome = await api.update(p.id);
      const vars = { name: p.id, version: u.version };
      if (outcome.kind === "installed") {
        view.value = outcome.view;
        toast.success(i18n.global.t("updates.done", vars));
      } else if (outcome.kind === "queued") {
        toast.info(i18n.global.t("updates.queued", vars));
      } else {
        await openUrl(outcome.url);
      }
    } catch (e) {
      toast.error(errorText(e), { duration: 10000 });
    } finally {
      const next = new Set(updating.value);
      next.delete(p.id);
      updating.value = next;
    }
  }

  async function updateAll() {
    for (const p of directUpdates.value) await update(p);
  }

  return {
    view,
    report,
    checking,
    updating,
    updatable,
    directUpdates,
    checkUpdates,
    update,
    updateAll,
    rollback: (uniqueId: string) => run(() => api.rollback(uniqueId)),
    selectedId,
    filter,
    working,
    packages,
    selected,
    profiles,
    activeProfile,
    load,
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
