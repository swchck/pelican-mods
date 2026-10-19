import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { toast } from "vue-sonner";
import {
  isAppError,
  nexus,
  nexusEvents,
  type Download,
  type NexusAccount,
  type NexusCard,
  type NexusFilters,
  type NexusMod,
  type NexusSort,
  type NxmLink,
} from "@/lib/api";
import { errorText, i18n } from "@/i18n";
import { useLibrary } from "@/stores/library";

const DETAILS_KEPT = 20;

export const useNexus = defineStore("nexus", () => {
  const query = ref("");
  const sort = ref<NexusSort>("endorsements");
  const category = ref<string | null>(null);
  const updated = ref<"any" | "week" | "month" | "year">("any");
  const minEndorsements = ref(0);
  const hideInstalled = ref(false);
  const categories = ref<[string, number][]>([]);
  const filtersActive = computed(
    () => category.value !== null || updated.value !== "any" || minEndorsements.value > 0 || hideInstalled.value,
  );
  const results = ref<NexusCard[]>([]);
  const total = ref(0);
  const searching = ref(false);
  const selectedId = ref<number | null>(null);
  const details = ref<Record<number, NexusMod>>({});
  const loadingDetails = ref(false);
  const account = ref<NexusAccount | null>(null);
  const accountChecked = ref(false);
  const downloads = ref<Download[]>([]);
  const keyDialog = ref(false);
  // the download that was waiting for a key; it resumes once the key is accepted
  const pending = ref<{ modId: number; fileId: number } | null>(null);
  /** Links from websites the user hasn't confirmed yet, oldest first. */
  const incoming = ref<NxmLink[]>([]);

  const selected = computed(() => (selectedId.value ? (details.value[selectedId.value] ?? null) : null));
  const active = computed(() => downloads.value.filter((d) => d.state !== "done" && d.state !== "failed"));

  const DAYS = { any: 0, week: 7, month: 30, year: 365 };
  function filters(): NexusFilters {
    const days = DAYS[updated.value];
    return {
      category: category.value,
      updatedSince: days ? Math.floor(Date.now() / 1000) - days * 86400 : null,
      minEndorsements: minEndorsements.value || null,
    };
  }

  function resetFilters() {
    category.value = null;
    updated.value = "any";
    minEndorsements.value = 0;
    hideInstalled.value = false;
  }

  let generation = 0;
  async function search(more = false) {
    const gen = ++generation;
    searching.value = true;
    try {
      const page = await nexus.search(query.value, filters(), sort.value, more ? results.value.length : 0);
      // a newer search started while this one was in flight; its results win
      if (gen !== generation) return;
      results.value = more ? [...results.value, ...page.items] : page.items;
      total.value = page.total;
      categories.value = page.categories;
    } catch (e) {
      if (gen === generation) toast.error(errorText(e));
    } finally {
      if (gen === generation) searching.value = false;
    }
  }

  async function select(modId: number) {
    selectedId.value = modId;
    if (details.value[modId]) return;
    loadingDetails.value = true;
    try {
      const loaded = await nexus.mod(modId);
      // descriptions run to hundreds of KB of BBCode; keep the last few, not the session
      const kept = Object.entries(details.value).slice(-(DETAILS_KEPT - 1));
      details.value = { ...Object.fromEntries(kept), [modId]: loaded };
    } catch (e) {
      if (selectedId.value === modId) toast.error(errorText(e));
    } finally {
      // an older click finishing must not hide the spinner of the one now shown
      if (selectedId.value === modId) loadingDetails.value = false;
    }
  }

  async function loadAccount() {
    try {
      account.value = await nexus.account();
    } catch (e) {
      account.value = null;
      toast.error(errorText(e));
    } finally {
      accountChecked.value = true;
    }
  }

  async function setKey(key: string) {
    try {
      account.value = await nexus.setKey(key);
      toast.success(i18n.global.t("nexus.connected", { name: account.value.name }));
      keyDialog.value = false;
      const next = pending.value;
      pending.value = null;
      if (next) void download(next.modId, next.fileId);
      return true;
    } catch (e) {
      toast.error(errorText(e));
      return false;
    }
  }

  async function clearKey() {
    await nexus.clearKey();
    account.value = null;
  }

  /** Premium downloads directly; everyone else gets the file page with "Mod Manager Download". */
  async function download(modId: number, fileId: number) {
    try {
      await nexus.download(modId, fileId);
    } catch (e) {
      if (isAppError(e) && e.kind === "nexus_no_key") {
        pending.value = { modId, fileId };
        keyDialog.value = true;
      } else if (isAppError(e) && e.kind === "nexus_needs_premium") {
        await nexus.openFile(modId, fileId);
        toast.info(i18n.global.t("nexus.freeHint"), { duration: 10000 });
      } else {
        toast.error(errorText(e));
      }
    }
  }

  async function collectLinks() {
    try {
      for (const link of await nexus.takePendingNxm()) {
        if (!incoming.value.some((l) => l.url === link.url)) incoming.value = [...incoming.value, link];
      }
    } catch (e) {
      toast.error(errorText(e));
    }
  }

  async function init() {
    const library = useLibrary();
    await Promise.all([
      nexusEvents.downloads((list) => {
        for (const d of list) {
          const prev = downloads.value.find((p) => p.id === d.id);
          if (d.state === "failed" && prev?.state !== "failed") toast.error(`${d.name}: ${errorText(d.error)}`);
          if (d.state === "done" && prev?.state !== "done")
            toast.success(i18n.global.t("nexus.installed", { name: d.name }));
        }
        downloads.value = list;
      }),
      nexusEvents.libraryChanged(() => void library.load()),
      nexusEvents.nxmError((e) => toast.error(errorText(e))),
      // the event is only a nudge; the backend keeps the links until collected
      nexusEvents.nxmPending(() => void collectLinks()),
    ]);
    // links that arrived before this listener, such as the one that launched the app
    await collectLinks();
    // listeners first: a failed fetch must not leave downloads and nxm errors unheard
    try {
      downloads.value = await nexus.downloads();
    } catch (e) {
      toast.error(errorText(e));
    }
  }

  return {
    query,
    sort,
    category,
    updated,
    minEndorsements,
    hideInstalled,
    categories,
    filtersActive,
    resetFilters,
    results,
    total,
    searching,
    selectedId,
    selected,
    details,
    loadingDetails,
    account,
    accountChecked,
    downloads,
    keyDialog,
    pending,
    incoming,
    active,
    search,
    select,
    loadAccount,
    setKey,
    clearKey,
    download,
    init,
    clearDownloads: async () => (downloads.value = await nexus.clearDownloads()),
    openNxm: async (url: string) => {
      try {
        await nexus.openNxm(url.trim());
        return true;
      } catch (e) {
        toast.error(errorText(e));
        return false;
      }
    },
  };
});
