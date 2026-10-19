import { onBeforeUnmount, onMounted, ref } from "vue";
import { defineStore } from "pinia";
import { formatCombo, isTyping, matches, parseCombo } from "@/lib/shortcuts";

export type ShortcutId =
  | "nav.library"
  | "nav.browse"
  | "nav.saves"
  | "nav.progress"
  | "nav.game"
  | "nav.console"
  | "nav.settings"
  | "game.play"
  | "game.vanilla"
  | "game.stop"
  | "page.search"
  | "page.install"
  | "page.refresh"
  | "page.save"
  | "list.up"
  | "list.down"
  | "list.toggle"
  | "help";

export interface ShortcutDef {
  id: ShortcutId;
  combos: string[];
  group: "nav" | "game" | "page" | "list" | "help";
}

/** Every shortcut, in the order the cheat sheet lists them. */
export const SHORTCUTS: ShortcutDef[] = [
  { id: "nav.library", combos: ["mod+1"], group: "nav" },
  { id: "nav.browse", combos: ["mod+2"], group: "nav" },
  { id: "nav.saves", combos: ["mod+3"], group: "nav" },
  { id: "nav.progress", combos: ["mod+4"], group: "nav" },
  { id: "nav.game", combos: ["mod+5"], group: "nav" },
  { id: "nav.console", combos: ["mod+6"], group: "nav" },
  { id: "nav.settings", combos: ["mod+,"], group: "nav" },
  { id: "game.play", combos: ["mod+p"], group: "game" },
  { id: "game.vanilla", combos: ["mod+shift+p"], group: "game" },
  { id: "game.stop", combos: ["mod+."], group: "game" },
  { id: "page.search", combos: ["mod+f"], group: "page" },
  { id: "page.install", combos: ["mod+o"], group: "page" },
  { id: "page.refresh", combos: ["mod+r"], group: "page" },
  { id: "page.save", combos: ["mod+s"], group: "page" },
  { id: "list.up", combos: ["arrowup"], group: "list" },
  { id: "list.down", combos: ["arrowdown"], group: "list" },
  { id: "list.toggle", combos: ["space"], group: "list" },
  { id: "help", combos: ["mod+/", "?"], group: "help" },
];

export const isMac = navigator.userAgent.includes("Mac");

/** The first combo of `id`, formatted for this platform, for tooltips. */
export function hint(id: ShortcutId): string {
  const def = SHORTCUTS.find((s) => s.id === id);
  return def ? formatCombo(def.combos[0], isMac) : "";
}

export const useShortcuts = defineStore("shortcuts", () => {
  // newest registration wins, so a page's handler shadows a global one while it's shown
  const handlers = new Map<ShortcutId, (() => void)[]>();
  const help = ref(false);

  function on(id: ShortcutId, fn: () => void) {
    handlers.set(id, [...(handlers.get(id) ?? []), fn]);
    return () =>
      handlers.set(
        id,
        (handlers.get(id) ?? []).filter((f) => f !== fn),
      );
  }

  function onKey(e: KeyboardEvent) {
    if (e.repeat && !e.key.startsWith("Arrow")) return;
    const typing = isTyping(document.activeElement);
    const dialog = document.querySelector('[role="dialog"][data-state="open"], [role="dialog"][aria-modal="true"]');
    for (const def of SHORTCUTS) {
      const combo = def.combos.map(parseCombo).find((c) => matches(c, e, isMac));
      if (!combo) continue;
      // plain keys belong to whatever has focus or is on top
      if (!combo.mod && (typing || dialog)) return;
      const list = handlers.get(def.id) ?? [];
      const fn = list[list.length - 1];
      if (!fn) return;
      e.preventDefault();
      fn();
      return;
    }
  }

  function install() {
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }

  return { help, on, install };
});

/** Binds `fn` to shortcut `id` while the calling component is mounted. */
export function useShortcut(id: ShortcutId, fn: () => void) {
  const store = useShortcuts();
  let off: (() => void) | undefined;
  onMounted(() => (off = store.on(id, fn)));
  onBeforeUnmount(() => off?.());
}
