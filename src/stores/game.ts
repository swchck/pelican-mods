import { computed, ref, shallowRef, watch } from "vue";
import { defineStore } from "pinia";
import { toast } from "vue-sonner";
import {
  api,
  events,
  isAppError,
  library,
  saves,
  type GameInstall,
  type Crash,
  type LogLine,
  type Preflight,
  type RunState,
  type SmapiProgress,
  type SmapiRelease,
} from "@/lib/api";
import { compareVersions } from "@/lib/version";
import { router } from "@/router";
import { useLibrary } from "@/stores/library";
import { errorText, i18n } from "@/i18n";

const LOG_LIMIT = 2000;

export const useGame = defineStore("game", () => {
  const phase = ref<"loading" | "setup" | "ready">("loading");
  const install = ref<GameInstall | null>(null);
  const detected = ref<GameInstall[]>([]);
  const latest = ref<SmapiRelease | null>(null);
  const progress = ref<SmapiProgress | null>(null);
  const run = ref<RunState>({ state: "idle" });
  // shallow: only ever replaced whole, and 2000 deep proxies cost memory for nothing
  const log = shallowRef<LogLine[]>([]);

  const running = computed(() => run.value.state === "running");
  const busy = computed(() => progress.value !== null);
  const promptDismissed = ref(false);
  /** What to offer in the startup dialog: SMAPI missing, outdated, or nothing. */
  const smapiPrompt = computed<"install" | "update" | null>(() => {
    const g = install.value;
    if (phase.value !== "ready" || !g || g.needsRosetta || promptDismissed.value || busy.value || running.value)
      return null;
    if (!g.smapiVersion) return "install";
    return smapiUpdate.value ? "update" : null;
  });
  const smapiUpdate = computed(() => {
    const cur = install.value?.smapiVersion;
    const next = latest.value?.version;
    return cur && next && compareVersions(next, cur) > 0 ? next : null;
  });

  async function init() {
    await Promise.all([
      events.log((lines) => {
        // one reactive write per batch, trimmed in the same step
        const next = log.value.concat(lines);
        log.value = next.length > LOG_LIMIT ? next.slice(next.length - LOG_LIMIT) : next;
      }),
      events.state((s) => {
        run.value = s;
        if (s.state === "exited") {
          refresh().catch((e) => toast.error(errorText(e)));
          // a stop from the app also ends with a non-zero code; only the game's own exit counts
          if (s.code !== 0 && !stopping) void explainCrash();
          if (launchedAt) void rememberProfile(launchedAt);
          launchedAt = 0;
          stopping = false;
          void warnFromLog();
          saves.mirror().catch((e) => toast.error(errorText(e)));
        }
      }),
      events.smapiProgress((p) => (progress.value = p)),
    ]);
    try {
      [install.value, run.value, log.value] = await Promise.all([api.currentGame(), api.runState(), api.log()]);
      // nothing saved yet, or the saved folder is gone: one install found means no question to ask
      if (!install.value) {
        detected.value = await api.detectGames();
        if (detected.value.length === 1) await choose(detected.value[0].path);
      }
    } catch (e) {
      // setup lets the user pick the game by hand; a spinner forever helps nobody
      toast.error(errorText(e));
    }
    phase.value = install.value ? "ready" : "setup";
    await checkSmapi();
  }

  /** The error the last session died on; cleared by the next launch. */
  const crash = ref<Crash | null>(null);
  let stopping = false;
  /** Unix seconds the modded session started; saves written after it ran with its profile. */
  let launchedAt = 0;

  async function rememberProfile(since: number) {
    const lib = useLibrary();
    lib.view = await library.rememberProfile(since).catch(() => lib.view);
  }

  async function explainCrash() {
    const c = await api.crash().catch(() => null);
    if (!c) return;
    crash.value = c;
    const { t } = i18n.global;
    toast.error(c.suspect ? t("crash.toastMod", { mod: c.suspect }) : t("crash.toast", { source: c.source }), {
      description: c.message.slice(0, 160),
      duration: 30000,
      action: { label: t("logReport.open"), onClick: () => void router.push("/console?tab=report") },
    });
  }

  /** Points at the log report when the session that just ended skipped mods or lost save data. */
  async function warnFromLog() {
    const r = await api.logReport().catch(() => null);
    if (!r || (!r.skipped.length && !r.lostLocations.length)) return;
    const { t } = i18n.global;
    const text = r.lostLocations.length
      ? t("logReport.toastLost", { n: r.lostLocations.length })
      : t("logReport.toastSkipped", { n: r.skipped.length });
    toast.warning(text, {
      duration: 20000,
      action: { label: t("logReport.open"), onClick: () => void router.push("/console?tab=report") },
    });
  }

  async function refresh() {
    install.value = await api.currentGame();
  }

  async function choose(path: string) {
    try {
      install.value = await api.setGamePath(path);
      phase.value = "ready";
    } catch (e) {
      toast.error(errorText(e));
    }
  }

  /** Makes another install the one the launcher plays, then reloads what depends on it. */
  async function switchTo(path: string) {
    await choose(path);
    await Promise.all([loadInstalls(), useLibrary().load()]);
  }

  async function loadInstalls() {
    detected.value = await api.detectGames().catch(() => detected.value);
  }

  async function forget(path: string) {
    try {
      await api.forgetGame(path);
    } catch (e) {
      toast.error(errorText(e));
    }
    await loadInstalls();
  }

  async function checkSmapi() {
    try {
      latest.value = await api.smapiLatest();
    } catch {
      // offline is normal for a launcher; the install button retries on click
      latest.value = null;
    }
  }

  async function smapi(action: "install" | "uninstall") {
    const { t } = i18n.global;
    promptDismissed.value = true;
    progress.value = { stage: "download", received: 0, total: 0 };
    try {
      install.value = await api.smapiRun(action);
      toast.success(
        action === "install" ? t("smapi.installed", { version: install.value.smapiVersion ?? "" }) : t("smapi.removed"),
      );
    } catch (e) {
      toast.error(errorText(e), { duration: 15000 });
    } finally {
      progress.value = null;
    }
  }

  async function installRosetta() {
    try {
      await api.installRosetta();
      await refresh();
      toast.success(i18n.global.t("rosetta.done"));
    } catch (e) {
      if (!(isAppError(e) && e.kind === "cancelled")) toast.error(errorText(e), { duration: 15000 });
    }
  }

  /** Set when the last played save uses mods that won't load; the play card asks first. */
  const preflight = ref<Preflight | null>(null);

  async function launch(modded: boolean, checked = false) {
    preflight.value = null;
    crash.value = null;
    if (modded && !checked) {
      const found = await api.preflight().catch(() => null);
      if (found) {
        preflight.value = found;
        return;
      }
    }
    try {
      log.value = [];
      run.value = await api.launch(modded);
      // a little early, so a save written in the first second still counts
      if (modded) launchedAt = Math.floor(Date.now() / 1000) - 5;
    } catch (e) {
      toast.error(errorText(e));
    }
  }

  /** True while the game runs only to be checked: launched, read, and closed again. */
  const checking = ref(false);
  /** SMAPI writes this once every mod has loaded; the check has seen enough then. */
  const READY = "Type 'help' for help";
  const CHECK_LIMIT_MS = 180_000;

  /**
   * Starts the game with the active profile, waits until SMAPI has loaded every mod (or
   * the game exits), and closes it; the SMAPI log then tells what the profile does.
   * Returns whether the game got far enough to be judged.
   */
  async function check(): Promise<boolean> {
    checking.value = true;
    try {
      await launch(true, true);
      if (run.value?.state !== "running") return false;
      const ready = await new Promise<boolean>((resolve) => {
        let stopWatching: (() => void) | null = null;
        const timer = setTimeout(() => finish(false), CHECK_LIMIT_MS);
        function finish(ok: boolean) {
          clearTimeout(timer);
          stopWatching?.();
          resolve(ok);
        }
        stopWatching = watch(
          [log, run],
          () => {
            if (log.value.some((l) => l.text.includes(READY))) finish(true);
            else if (run.value?.state === "exited") finish(false);
          },
          { immediate: true },
        );
      });
      // read afresh: the awaits above let the game's state move on
      const state = (): string | undefined => run.value?.state;
      if (state() === "running") await stop();
      return ready || state() === "exited";
    } finally {
      checking.value = false;
    }
  }

  async function stop() {
    stopping = true;
    try {
      await api.stop();
    } catch (e) {
      toast.error(errorText(e));
    }
  }

  return {
    crash,
    preflight,
    phase,
    install,
    detected,
    latest,
    progress,
    run,
    log,
    running,
    checking,
    check,
    busy,
    smapiUpdate,
    smapiPrompt,
    promptDismissed,
    init,
    refresh,
    choose,
    switchTo,
    loadInstalls,
    forget,
    checkSmapi,
    smapi,
    installRosetta,
    launch,
    stop,
  };
});
