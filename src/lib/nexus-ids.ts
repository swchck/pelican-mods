import type { PackageView } from "./api";

/** Nexus mod ids of installed packages, read from their `Nexus:<id>` update keys. */
export function installedNexusIds(packages: PackageView[]): Set<number> {
  const ids = new Set<number>();
  for (const p of packages) {
    for (const m of p.mods) {
      for (const key of m.manifest.updateKeys) {
        // "Nexus:541" or "Nexus:541@subkey"
        const match = /^\s*nexus\s*:\s*(\d+)/i.exec(key);
        if (match) ids.add(Number(match[1]));
      }
    }
  }
  return ids;
}
