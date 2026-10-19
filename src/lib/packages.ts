import type { Issue, PackageView, UpdateReport } from "./api";
import { compareVersions } from "./version";

export type UpdateSource = "github" | "moddrop" | "nexus" | "page";

export type PackageStatus = "ok" | "off" | "issue" | "broken" | "update";

export interface PackageUpdate {
  version: string;
  url: string;
  /**
   * Where the file comes from: GitHub and ModDrop files install in place, Nexus files download
   * through the queue (or the site's button without Premium), anything else opens the page.
   */
  via: UpdateSource;
}

/** Compatibility verdicts worth a warning; "Ok" and "Optional" are fine. */
const WARN_COMPAT = new Set(["Workaround", "Unofficial", "Broken", "Obsolete", "Abandoned"]);

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

/** The newest suggested update among the package's mods, if smapi.io reported one. */
export function packageUpdate(p: PackageView, report: UpdateReport | null): PackageUpdate | null {
  if (p.system || !report) return null;
  for (const m of p.mods) {
    const info = report.mods[m.manifest.uniqueId.toLowerCase()];
    // the report outlives the install it was made for; an update already applied drops out here
    if (info?.version && info.url && compareVersions(info.version, m.manifest.version) > 0) {
      // a GitHub repo in the update keys is tried first, whichever site smapi.io suggested
      const via = hasGithubKey(m.manifest.updateKeys) ? "github" : updateSource(info.url);
      return { version: info.version, url: info.url, via };
    }
  }
  return null;
}

// mirrors updates::github_key on the backend
function hasGithubKey(keys: string[]): boolean {
  return keys.some((k) => /^\s*github\s*:\s*[^/@\s]+\/[^/@\s]+/i.test(k));
}

function updateSource(url: string): UpdateSource {
  if (url.startsWith("https://github.com/")) return "github";
  if (/^https:\/\/(www\.)?moddrop\.com\/stardew-valley\/mods?\/\d+/i.test(url)) return "moddrop";
  if (/^https:\/\/(www\.)?nexusmods\.com\/stardewvalley\/mods\/\d+/i.test(url)) return "nexus";
  return "page";
}

export function packageCompat(p: PackageView, report: UpdateReport | null): { status: string; summary: string } | null {
  if (!report) return null;
  for (const m of p.mods) {
    const info = report.mods[m.manifest.uniqueId.toLowerCase()];
    if (info?.compatStatus && WARN_COMPAT.has(info.compatStatus)) {
      return { status: info.compatStatus, summary: info.compatSummary ?? "" };
    }
  }
  return null;
}

export function packageStatus(p: PackageView, report: UpdateReport | null = null): PackageStatus {
  if (p.broken.length) return "broken";
  if (p.enabled && packageIssues(p).length) return "issue";
  if (packageUpdate(p, report)) return "update";
  return p.enabled ? "ok" : "off";
}

/** Case-insensitive match on names, authors, folder name and UniqueIDs. */
export function matchesFilter(p: PackageView, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const hay = [p.id, ...p.mods.flatMap((m) => [m.manifest.name, m.manifest.author, m.manifest.uniqueId])];
  return hay.some((s) => s.toLowerCase().includes(q));
}
