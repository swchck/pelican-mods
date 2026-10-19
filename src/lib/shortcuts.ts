/**
 * Keyboard combos written as "mod+shift+p": "mod" is ⌘ on macOS and Ctrl elsewhere,
 * the last part is the key. Matching uses `KeyboardEvent.code` for letters and digits,
 * so combos keep working in the Russian layout, where ⌘P types "з".
 */
export interface Combo {
  mod: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}

export function parseCombo(text: string): Combo {
  const parts = text.toLowerCase().split("+");
  const key = parts.pop() ?? "";
  return { mod: parts.includes("mod"), shift: parts.includes("shift"), alt: parts.includes("alt"), key };
}

// the physical key of letters and digits, independent of the active layout
function keyOf(e: Pick<KeyboardEvent, "key" | "code" | "shiftKey">): string {
  if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3).toLowerCase();
  if (/^Digit\d$/.test(e.code)) return e.code.slice(5);
  if (e.code === "Comma") return ",";
  if (e.code === "Period") return ".";
  // "?" is Shift+/ on US keyboards but sits elsewhere in other layouts; go by the key
  if (e.code === "Slash") return e.shiftKey ? "?" : "/";
  // e.key is " " for Space, which no combo string can spell
  if (e.code === "Space") return "space";
  return e.key.toLowerCase();
}

export function matches(
  combo: Combo,
  e: Pick<KeyboardEvent, "key" | "code" | "metaKey" | "ctrlKey" | "shiftKey" | "altKey">,
  mac: boolean,
): boolean {
  const mod = mac ? e.metaKey : e.ctrlKey;
  // "?" needs Shift to type, so a combo naming it doesn't also have to
  const shiftOk = combo.key === "?" ? true : e.shiftKey === combo.shift;
  const key = combo.key === "?" && e.key === "?" ? "?" : keyOf(e);
  return mod === combo.mod && shiftOk && e.altKey === combo.alt && key === combo.key;
}

const GLYPHS: Record<string, string> = {
  arrowup: "↑",
  arrowdown: "↓",
  enter: "↩",
  escape: "Esc",
  " ": "Space",
  space: "Space",
};

/** "mod+shift+p" → "⇧⌘P" on macOS, "Ctrl+Shift+P" elsewhere. */
export function formatCombo(text: string, mac: boolean): string {
  const c = parseCombo(text);
  const key = GLYPHS[c.key] ?? c.key.toUpperCase();
  if (mac) return `${c.alt ? "⌥" : ""}${c.shift ? "⇧" : ""}${c.mod ? "⌘" : ""}${key}`;
  return [c.mod && "Ctrl", c.shift && "Shift", c.alt && "Alt", key].filter(Boolean).join("+");
}

/** Reports whether keys typed into `el` belong to it rather than to shortcuts. */
export function isTyping(el: Element | null): boolean {
  if (!el) return false;
  if (el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) return true;
  if (el instanceof HTMLInputElement) return !["checkbox", "radio", "button", "submit"].includes(el.type);
  return (el as HTMLElement).isContentEditable;
}
