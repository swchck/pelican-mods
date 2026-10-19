import type { GmcmMod, SchemaField } from "./api";

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

interface Common {
  path: string[];
  label: string;
  description?: string;
}

export type Field = Common &
  (
    | { kind: "bool" }
    | { kind: "number"; integer: boolean; min?: number; max?: number; step?: number }
    | { kind: "text" }
    | { kind: "keybind" }
    | { kind: "choice"; options: string[]; multiple: boolean; optionLabels?: Record<string, string> }
    | { kind: "list" }
    | { kind: "json" }
  );

export interface OptionLabel {
  name: string | null;
  description: string | null;
  values: Record<string, string>;
}

/** Same normalization as the backend: "ShowUnknownGiftTastes" ≡ "show-unknown-gift-tastes". */
export function labelKey(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]/g, "");
}

/** Replaces generated labels with the mod's own texts where its translations have them. */
export function applyLabels(sections: Section[], labels: Record<string, OptionLabel>): Section[] {
  const find = (key: string | undefined) => (key ? labels[labelKey(key)] : undefined);
  return sections.map((s) => {
    const sectionKey = s.fields[0]?.path.length === 2 ? s.fields[0].path[0] : undefined;
    return {
      title: find(sectionKey)?.name ?? s.title,
      fields: s.fields.map((f) => {
        const l = find(f.path[f.path.length - 1]);
        if (!l) return f;
        const next = { ...f, label: l.name ?? f.label, description: l.description ?? f.description };
        return next.kind === "choice" && Object.keys(l.values).length ? { ...next, optionLabels: l.values } : next;
      }),
    };
  });
}

export interface Section {
  title: string | null;
  fields: Field[];
}

/** "ShowGiftTastes" → "Show gift tastes", "max_items" → "Max items". */
export function humanize(key: string): string {
  const words = key
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .trim()
    .split(/\s+/);
  return words
    .map((w, i) => (i === 0 ? w.charAt(0).toUpperCase() + w.slice(1) : /^[A-Z0-9]+$/.test(w) ? w : w.toLowerCase()))
    .join(" ");
}

// SMAPI keybinds are SButton names joined by "," (alternatives) and "+" (combos)
const BUTTON =
  /^(F\d{1,2}|[A-Z]|D\d|NumPad\d|Left\w+|Right\w+|Mouse\w+|Controller\w+|Button\w+|Space|Enter|Escape|Tab|Back|Delete|Insert|Home|End|PageUp|PageDown|Up|Down|OemTilde|None)$/;

function looksLikeKeybind(key: string, value: string): boolean {
  if (/key|button|bind|hotkey/i.test(key)) return true;
  const parts = value
    .split(/[,+]/)
    .map((p) => p.trim())
    .filter(Boolean);
  return parts.length > 0 && parts.every((p) => BUTTON.test(p));
}

function primitiveField(path: string[], key: string, value: Json): Field {
  const label = humanize(key);
  if (typeof value === "boolean") return { kind: "bool", path, label };
  if (typeof value === "number") return { kind: "number", path, label, integer: Number.isInteger(value) };
  if (typeof value === "string") {
    return looksLikeKeybind(key, value) ? { kind: "keybind", path, label } : { kind: "text", path, label };
  }
  if (Array.isArray(value) && value.every((v) => typeof v !== "object" || v === null)) {
    return { kind: "list", path, label };
  }
  return { kind: "json", path, label };
}

/** Builds a form from the shape of an existing config.json: top-level values first,
 * each nested object as its own section; anything deeper is edited as raw JSON. */
export function inferSections(values: { [key: string]: Json }): Section[] {
  const top: Field[] = [];
  const nested: Section[] = [];
  for (const [key, value] of Object.entries(values)) {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      nested.push({
        title: humanize(key),
        fields: Object.entries(value).map(([k, v]) => primitiveField([key, k], k, v)),
      });
    } else {
      top.push(primitiveField([key], key, value));
    }
  }
  return [...(top.length ? [{ title: null, fields: top }] : []), ...nested];
}

/** Builds a form from a Content Patcher ConfigSchema, grouped by its Section. */
export function schemaSections(schema: SchemaField[]): Section[] {
  const out = new Map<string | null, Field[]>();
  for (const f of schema) {
    const path = [f.key];
    const label = humanize(f.key);
    const description = f.description ?? undefined;
    const field: Field = f.boolean
      ? { kind: "bool", path, label, description }
      : f.allowValues.length
        ? { kind: "choice", path, label, options: f.allowValues, multiple: f.allowMultiple, description }
        : { kind: "text", path, label, description };
    const key = f.section ?? null;
    out.set(key, [...(out.get(key) ?? []), field]);
  }
  return [...out].map(([title, fields]) => ({ title, fields }));
}

/**
 * Builds a form in the order and grouping of the mod's own GMCM menu, with its names,
 * tooltips, ranges and choices. Keys the menu doesn't cover follow under `otherTitle`.
 */
export function gmcmSections(gmcm: GmcmMod, values: { [key: string]: Json }, otherTitle: string): Section[] {
  const out: Section[] = [];
  const covered = new Set<string>();
  for (const o of gmcm.options) {
    if (!o.path || getAt(values, o.path) === null) continue;
    covered.add(o.path.join("\u0000"));
    const base = { path: o.path, label: o.name, description: o.tooltip ?? undefined };
    const field: Field =
      o.kind === "bool"
        ? { ...base, kind: "bool" }
        : o.kind === "int" || o.kind === "float"
          ? {
              ...base,
              kind: "number",
              integer: o.kind === "int",
              min: o.min ?? undefined,
              max: o.max ?? undefined,
              step: o.interval ?? undefined,
            }
          : o.kind === "keybind"
            ? { ...base, kind: "keybind" }
            : o.kind === "choice" && o.choices
              ? {
                  ...base,
                  kind: "choice",
                  multiple: false,
                  options: o.choices.map((c) => c.value),
                  optionLabels: Object.fromEntries(o.choices.map((c) => [c.value, c.label])),
                }
              : { ...base, kind: "text" };
    const last = out[out.length - 1];
    if (last && last.title === o.section) last.fields.push(field);
    else out.push({ title: o.section, fields: [field] });
  }
  const rest = inferSections(values)
    .flatMap((s) => s.fields)
    .filter((f) => !covered.has(f.path.join("\u0000")));
  return rest.length ? [...out, { title: otherTitle, fields: rest }] : out;
}

export function getAt(obj: Json, path: string[]): Json {
  let cur: Json = obj;
  for (const k of path) {
    if (!cur || typeof cur !== "object" || Array.isArray(cur)) return null;
    cur = cur[k] ?? null;
  }
  return cur;
}

/** Returns a copy of `obj` with `value` at `path`; key order is kept. */
export function setAt(obj: { [key: string]: Json }, path: string[], value: Json): { [key: string]: Json } {
  const [head, ...rest] = path;
  const child = obj[head];
  const next =
    rest.length === 0
      ? value
      : setAt(child && typeof child === "object" && !Array.isArray(child) ? child : {}, rest, value);
  return { ...obj, [head]: next };
}

/** Content Patcher reads booleans and multi-values from strings; keep the type it wrote. */
export function choiceValues(v: Json): string[] {
  if (typeof v === "string")
    return v
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
  if (typeof v === "boolean" || typeof v === "number") return [String(v)];
  return [];
}
