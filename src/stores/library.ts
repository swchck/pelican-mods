import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { toast } from "vue-sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  library as api,
  type BisectStep,
  type LibraryView,
  type PackageView,
  type SaveUsage,
  type UpdateOutcome,
  type UpdateReport,
} from "@/lib/api";
import {
  matchesFilter,
  matchesStatus,
  packageNexusId,
  packageStatus,
  packageTitle,
  packageUpdate,
  type StatusFilter,
} from "@/lib/packages";
import { nexus as nexusApi, translations as translationsApi } from "@/lib/api";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
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
  const statusFilter = ref<StatusFilter>("all");
  /** Shows only packages with this tag; null shows every one. */
  const tagFilter = ref<string | null>(null);
  /** The packages the table lists, after the text, status and tag filters. */
  const visible = computed(() =>
    packages.value.filter(
      (p) =>
        matchesFilter(p, filter.value) &&
        matchesStatus(packageStatus(p, report.value), statusFilter.value) &&
        (tagFilter.value === null || p.tags.includes(tagFilter.value)),
    ),
  );
  /** Every tag in use, with how many packages carry it, by name. */
  const tags = computed(() => {
    const counts = new Map<string, number>();
    for (const p of packages.value) for (const t of p.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts].sort(([a], [b]) => a.localeCompare(b));
  });
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
      // a fresh install may bring a Nexus mod the pictures don't cover yet
      void loadPictures();
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

  /** Nexus thumbnails by mod id, for packages that come from Nexus. */
  const pictures = ref<Record<number, string>>({});
  async function loadPictures() {
    const ids = [...new Set(packages.value.map(packageNexusId).filter((id): id is number => id !== null))];
    const missing = ids.filter((id) => !(id in pictures.value));
    if (!missing.length) return;
    // offline, or Nexus is down: the library just goes without pictures
    const found = await nexusApi.thumbnails(missing).catch(() => ({}));
    pictures.value = { ...pictures.value, ...found };
  }
  const pictureOf = (p: PackageView) => {
    const id = packageNexusId(p);
    return id === null ? null : (pictures.value[id] ?? null);
  };

  async function load() {
    const ok = await run(api.get, true);
    if (ok) {
      void loadPictures();
      void checkUpdates(false);
      backgroundChecks();
    }
    return ok;
  }

  // hourly while the app stays open; the backend answers from its cache until it's 6 hours
  // old, so this asks smapi.io about four times a day at most
  const RECHECK_MS = 60 * 60 * 1000;
  let rechecking: ReturnType<typeof setInterval> | null = null;

  function backgroundChecks() {
    if (rechecking) return;
    rechecking = setInterval(async () => {
      const before = new Set(updatable.value.map((p) => p.id));
      await checkUpdates(false);
      const fresh = updatable.value.filter((p) => !before.has(p.id));
      if (fresh.length) await notifyUpdates(fresh.map(packageTitle));
    }, RECHECK_MS);
  }

  async function notifyUpdates(names: string[]) {
    try {
      let allowed = await isPermissionGranted();
      if (!allowed) allowed = (await requestPermission()) === "granted";
      if (!allowed) return;
      const { t } = i18n.global;
      sendNotification({
        title: t("updates.notifyTitle", { n: names.length }),
        body: names.slice(0, 4).join(", ") + (names.length > 4 ? "…" : ""),
      });
    } catch {
      // a notification is a nicety; the library's badge says the same
    }
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

  /** Finds a mod by UniqueID (helped by a list's keys and repo) and installs it like an update. */
  async function installDependency(uniqueId: string, name: string, keys?: string[], github?: string | null) {
    updating.value = new Set(updating.value).add(uniqueId);
    try {
      await settle(await api.installDependency(uniqueId, keys, github), "deps", { name });
    } catch (e) {
      toast.error(errorText(e), { duration: 10000 });
    } finally {
      const next = new Set(updating.value);
      next.delete(uniqueId);
      updating.value = next;
    }
  }

  const bisectResult = ref<BisectStep | null>(null);

  /** A mod about to be turned off that saves still use; the library asks before it goes. */
  const pendingDisable = ref<{ id: string; usage: SaveUsage[] } | null>(null);

  async function toggle(id: string, on: boolean): Promise<boolean> {
    if (!on) {
      const usage = await api.saveUsage(id).catch(() => []);
      if (usage.length) {
        pendingDisable.value = { id, usage };
        return false;
      }
    }
    return run(() => api.toggle(id, on));
  }

  async function confirmDisable() {
    const p = pendingDisable.value;
    pendingDisable.value = null;
    if (p) await run(() => api.toggle(p.id, false));
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
    profileFromList: (name: string, uniqueIds: string[]) => run(() => api.profileFromList(name, uniqueIds)),
    rollback: (uniqueId: string) => run(() => api.rollback(uniqueId)),
    undo: (at: number) => run(() => api.undo(at)),
    saveTranslation: (pkg: string, relPath: string, language: string, texts: Record<string, string>) =>
      run(() => translationsApi.save(pkg, relPath, language, texts)),
    bisectStart: (name: string) => run(() => api.bisectStart(name)),
    bisectCancel: () => run(() => api.bisectCancel()),
    async bisectPartner(name: string, pkg: string, candidates: string[]) {
      if (await run(() => api.bisectPartner(name, pkg, candidates))) bisectResult.value = null;
    },
    /** The last finished search's answer, until the player acts on it. */
    bisectResult,
    async bisectAnswer(problem: boolean) {
      let step: BisectStep | null = null;
      await run(async () => {
        const out = await api.bisectAnswer(problem);
        step = out.step;
        return out.view;
      });
      if (step && (step as BisectStep).kind !== "next") bisectResult.value = step;
    },
    /** Attaches or (with null) detaches a GitHub repo, then asks smapi.io again with it. */
    async setGithub(uniqueId: string, repo: string | null) {
      if (await run(() => api.setGithub(uniqueId, repo))) await checkUpdates(true);
    },
    selectedId,
    pictureOf,
    filter,
    statusFilter,
    tagFilter,
    visible,
    tags,
    async setTags(id: string, next: string[]) {
      await run(() => api.setTags(id, next));
      // a tag nobody carries any more can't keep filtering the table down to nothing
      if (tagFilter.value !== null && !tags.value.some(([t]) => t === tagFilter.value)) tagFilter.value = null;
    },
    working,
    packages,
    selected,
    profiles,
    activeProfile,
    load,
    install: (paths: string[]) => run(() => api.install(paths)),
    importGameMods: (names: string[]) => run(() => api.import(names)),
    remove: (id: string) => run(() => api.remove(id)),
    toggle,
    pendingDisable,
    confirmDisable,
    createProfile: (name: string, copyFrom: string | null) => run(() => api.createProfile(name, copyFrom)),
    renameProfile: (id: string, name: string) => run(() => api.renameProfile(id, name)),
    deleteProfile: (id: string) => run(() => api.deleteProfile(id)),
    activateProfile: (id: string) => run(() => api.activateProfile(id)),
  };
});
