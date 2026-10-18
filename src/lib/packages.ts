import type { Issue, PackageView } from "./api";

export type PackageStatus = "ok" | "off" | "issue" | "broken";

/** Display name: the mod's own name, or the folder name for multi-mod packages. */
export function packageTitle(p: PackageView): string {
  return p.mods.length === 1 ? p.mods[0].manifest.name : p.id;
}

export function packageAuthor(p: PackageView): string {
  const authors = [...new Set(p.mods.map((m) => m.manifest.author).filter(Boolean))];
  return authors.join(", ");
}

export function packageVersion(p: PackageView): string {
  return p.mods[0]?.manifest.version ?? "";
}

export function packageSource(p: PackageView): string | null {
  return p.system ? "SMAPI" : (p.mods.find((m) => m.source)?.source ?? null);
}

export function packageIssues(p: PackageView): Issue[] {
  return p.mods.flatMap((m) => m.issues);
}

export function packageStatus(p: PackageView): PackageStatus {
  if (p.broken.length) return "broken";
  if (!p.enabled) return "off";
  return packageIssues(p).length ? "issue" : "ok";
}

/** Case-insensitive match on names, authors, folder name and UniqueIDs. */
export function matchesFilter(p: PackageView, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const hay = [p.id, ...p.mods.flatMap((m) => [m.manifest.name, m.manifest.author, m.manifest.uniqueId])];
  return hay.some((s) => s.toLowerCase().includes(q));
}
