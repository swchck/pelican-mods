import { computed, ref, shallowRef } from "vue";
import { defineStore } from "pinia";
import { toast } from "vue-sonner";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { parseChangelog, releasesSince, type Release } from "@/lib/changelog";
import changelogEn from "../../CHANGELOG.md?raw";
import changelogRu from "../../CHANGELOG.ru.md?raw";
import { errorText, i18n } from "@/i18n";

const FIRST_CHECK_MS = 5_000;
const RECHECK_MS = 6 * 60 * 60 * 1000;
const SEEN_KEY = "lastSeenVersion";

/** The changelog shipped with this build, in the UI language. */
export function bundledChangelog(): Release[] {
  return parseChangelog(i18n.global.locale.value === "ru" ? changelogRu : changelogEn);
}

function readSeen(): string | null {
  try {
    return localStorage.getItem(SEEN_KEY);
  } catch {
    return null;
  }
}

function writeSeen(v: string) {
  try {
    localStorage.setItem(SEEN_KEY, v);
  } catch {
    // private storage off: "what's new" just shows again next launch
  }
}

export const useAppUpdate = defineStore("appUpdate", () => {
  const current = ref("");
  const update = shallowRef<Update | null>(null);
  const notes = ref<Release[]>([]);
  const dialog = ref(false);
  const whatsNew = ref<Release[]>([]);
  const checking = ref(false);
  const installing = ref(false);
  const total = ref(0);
  const done = ref(0);
  const lastChecked = ref<Date | null>(null);
  const percent = computed(() => (total.value ? Math.min(100, Math.round((done.value / total.value) * 100)) : null));
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function loadNotes(u: Update) {
    // the new version's changelog lives with its release; this build only knows its own
    try {
      const text = await invoke<string>("app_changelog", { version: u.version, locale: i18n.global.locale.value });
      notes.value = releasesSince(parseChangelog(text), current.value, u.version);
    } catch {
      notes.value = u.body ? [{ version: u.version, date: null, body: u.body }] : [];
    }
  }

  async function lookForUpdate(manual = false) {
    if (installing.value || checking.value) return;
    checking.value = true;
    try {
      const found = await check();
      lastChecked.value = new Date();
      if (found && found.version !== update.value?.version) {
        update.value = found;
        await loadNotes(found);
        toast(i18n.global.t("appUpdate.available", { version: found.version }), {
          duration: Infinity,
          action: { label: i18n.global.t("appUpdate.details"), onClick: () => (dialog.value = true) },
        });
      } else if (!found && manual) {
        toast.success(i18n.global.t("appUpdate.upToDate"));
      }
    } catch (e) {
      // automatic checks fail quietly when offline; a manual one says why
      if (manual) toast.error(errorText(e));
    } finally {
      checking.value = false;
    }
  }

  async function install() {
    const u = update.value;
    if (!u) return;
    installing.value = true;
    done.value = 0;
    try {
      await u.downloadAndInstall((e) => {
        if (e.event === "Started") total.value = e.data.contentLength ?? 0;
        else if (e.event === "Progress") done.value += e.data.chunkLength;
      });
      await relaunch();
    } catch (e) {
      installing.value = false;
      toast.error(i18n.global.t("appUpdate.failed"), { description: errorText(e) });
    }
  }

  async function init() {
    current.value = await getVersion();
    const seen = readSeen();
    // a fresh install has nothing to catch up on; an upgrade shows what changed since
    if (seen && seen !== current.value) whatsNew.value = releasesSince(bundledChangelog(), seen, current.value);
    writeSeen(current.value);
    if (import.meta.env.DEV) return;
    const schedule = (ms: number) => {
      timer = setTimeout(async () => {
        await lookForUpdate();
        schedule(RECHECK_MS);
      }, ms);
    };
    clearTimeout(timer);
    schedule(FIRST_CHECK_MS);
  }

  return {
    current,
    update,
    notes,
    dialog,
    whatsNew,
    checking,
    installing,
    percent,
    lastChecked,
    init,
    check: () => lookForUpdate(true),
    install,
  };
});
