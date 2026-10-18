import { describe, expect, it } from "vitest";
import type { ModView, PackageView } from "./api";
import {
  matchesFilter,
  packageAuthor,
  packageCompat,
  packageSource,
  packageStatus,
  packageTitle,
  packageUpdate,
} from "./packages";

function mod(name: string, id: string, extra: Partial<ModView> = {}): ModView {
  return {
    relPath: "",
    source: null,
    issues: [],
    requiredBy: [],
    manifest: {
      name,
      author: "Pathoschild",
      version: "1.0.0",
      description: "",
      uniqueId: id,
      entryDll: null,
      contentPackFor: null,
      dependencies: [],
      updateKeys: [],
      minimumApiVersion: null,
      minimumGameVersion: null,
    },
    ...extra,
  };
}

function pkg(extra: Partial<PackageView>): PackageView {
  return {
    id: "Folder",
    path: "/x",
    system: false,
    enabled: true,
    mods: [],
    broken: [],
    backupVersion: null,
    ...extra,
  };
}

describe("packages", () => {
  it("names single and multi-mod packages", () => {
    expect(packageTitle(pkg({ mods: [mod("Lookup Anything", "P.L")] }))).toBe("Lookup Anything");
    expect(packageTitle(pkg({ id: "SVE", mods: [mod("A", "a"), mod("B", "b")] }))).toBe("SVE");
    expect(packageAuthor(pkg({ mods: [mod("A", "a"), mod("B", "b")] }))).toBe("Pathoschild");
  });

  it("derives status in priority order", () => {
    const issue = mod("A", "a", { issues: [{ kind: "needsSmapi", need: "5.0" }] });
    expect(packageStatus(pkg({ mods: [issue], broken: [{ relPath: "", error: "x" }] }))).toBe("broken");
    expect(packageStatus(pkg({ mods: [issue], enabled: false }))).toBe("off");
    expect(packageStatus(pkg({ mods: [issue] }))).toBe("issue");
    expect(packageStatus(pkg({ mods: [mod("A", "a")] }))).toBe("ok");
  });

  it("reports source and filters", () => {
    const p = pkg({ mods: [mod("Lookup Anything", "Pathoschild.LookupAnything", { source: "Nexus" })] });
    expect(packageSource(p)).toBe("Nexus");
    expect(packageSource({ ...p, system: true })).toBe("SMAPI");
    expect(matchesFilter(p, "lookup")).toBe(true);
    expect(matchesFilter(p, "pathoschild.look")).toBe(true);
    expect(matchesFilter(p, "fishing")).toBe(false);
  });

  it("merges update reports", () => {
    const p = pkg({ mods: [mod("Lookup", "Pathoschild.Lookup")], enabled: false });
    const info = (url: string, compatStatus = "Ok") => ({
      checkedAt: 1,
      mods: {
        "pathoschild.lookup": { version: "1.55.0", url, compatStatus, compatSummary: "s", nexusId: 541, errors: [] },
      },
    });
    const nexus = info("https://www.nexusmods.com/stardewvalley/mods/541");
    expect(packageUpdate(p, nexus)).toEqual({
      version: "1.55.0",
      url: nexus.mods["pathoschild.lookup"].url,
      direct: false,
    });
    expect(packageUpdate(p, info("https://github.com/a/b/releases"))?.direct).toBe(true);
    expect(packageStatus(p, nexus)).toBe("update");
    expect(packageStatus(p, null)).toBe("off");
    expect(packageCompat(p, nexus)).toBeNull();
    expect(packageCompat(p, info("x", "Broken"))).toEqual({ status: "Broken", summary: "s" });
  });
});
