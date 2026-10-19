import type { Dependency, PackageView } from "./api";
import { compareVersions } from "./version";

export type DepState = "ok" | "off" | "old" | "missing";

export interface DepRow {
  uniqueId: string;
  /** The installed mod's name, or a readable guess from the id. */
  name: string;
  minimumVersion: string | null;
  required: boolean;
  state: DepState;
  installedVersion: string | null;
  /** Package holding the dependency, when installed. */
  packageId: string | null;
}

/** "spacechase0.JsonAssets" → "Json Assets"; ids are the only name a missing mod has. */
export function nameFromId(id: string): string {
  const last = id.split(".").filter(Boolean).pop() ?? id;
  return last
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .trim();
}

/** Resolves dependencies against the library, required first, then installed first. */
export function resolveDeps(deps: Dependency[], packages: PackageView[]): DepRow[] {
  const byId = new Map<string, { pkg: PackageView; name: string; version: string }>();
  for (const pkg of packages) {
    for (const m of pkg.mods) {
      byId.set(m.manifest.uniqueId.toLowerCase(), { pkg, name: m.manifest.name, version: m.manifest.version });
    }
  }
  const rows = deps.map((d): DepRow => {
    const hit = byId.get(d.uniqueId.toLowerCase());
    let state: DepState = "missing";
    if (hit) {
      if (!hit.pkg.enabled) state = "off";
      else if (d.minimumVersion && compareVersions(hit.version, d.minimumVersion) < 0) state = "old";
      else state = "ok";
    }
    return {
      uniqueId: d.uniqueId,
      name: hit?.name ?? nameFromId(d.uniqueId),
      minimumVersion: d.minimumVersion,
      required: d.isRequired,
      state,
      installedVersion: hit?.version ?? null,
      packageId: hit?.pkg.id ?? null,
    };
  });
  const rank = (r: DepRow) => (r.required ? 0 : 2) + (r.state === "missing" ? 1 : 0);
  return rows.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
}
