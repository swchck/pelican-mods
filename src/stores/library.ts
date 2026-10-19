import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { toast } from "vue-sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { library as api, type LibraryView, type PackageView, type UpdateOutcome, type UpdateReport } from "@/lib/api";
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

  // results of changes applied so far; a reload that saw fewer is stale
  let changes = 0;
  let inFlight = 0;

  /**
   * Runs a library command, swapping in the view it returns; failures become toasts.
   * `read` marks a plain reload: its view is dropped if a change landed meanwhile, since
   * the reload may have read the store before that change committed.
   */
  async function run(op: () => Promise<LibraryView>, read = false): Promise<boolean> {
    const seen = changes;
    inFlight++;
    working.value = true;
    try {
      const next = await op();
      if (!read) {
        changes++;
        view.value = next;
      } else if (seen === changes) {
        view.value = next;
      }
      if (selectedId.value && !packages.value.some((p) => p.id === selectedId.value)) selectedId.value = null;
      return true;
    } catch (e) {
      toast.error(errorText(e), { duration: 10000 });
      return false;
    } finally {
      working.value = --inFlight > 0;
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
    const ok = await run(api.get, true);
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
      await settle(await api.update(p.id), "updates", { name: p.id, version: u.version });
    } catch (e) {
      toast.error(errorText(e), { duration: 10000 });
    } finally {
      const next = new Set(updating.value);
      next.delete(p.id);
      updating.value = next;
    }
  }

  async function settle(outcome: UpdateOutcome, texts: "updates" | "deps", vars: Record<string, string>) {
    if (outcome.kind === "installed") {
      changes++;
      view.value = outcome.view;
      toast.success(i18n.global.t(`${texts}.done`, vars));
    } else if (outcome.kind === "queued") {
      toast.info(i18n.global.t(`${texts}.queued`, vars));
    } else {
      await openUrl(outcome.url);
    }
  }

  /** Finds the mod another one needs and installs it the way an update would. */
  async function installDependency(uniqueId: string, name: string) {
    updating.value = new Set(updating.value).add(uniqueId);
    try {
      await settle(await api.installDependency(uniqueId), "deps", { name });
    } catch (e) {
      toast.error(errorText(e), { duration: 10000 });
    } finally {
      const next = new Set(updating.value);
      next.delete(uniqueId);
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
    installDependency,
    rollback: (uniqueId: string) => run(() => api.rollback(uniqueId)),
    /** Attaches or (with null) detaches a GitHub repo, then asks smapi.io again with it. */
    async setGithub(uniqueId: string, repo: string | null) {
      if (await run(() => api.setGithub(uniqueId, repo))) await checkUpdates(true);
    },
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
