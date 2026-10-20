import { describe, expect, it } from "vitest";
import type { PackageView } from "./api";
import { nameFromId, resolveDeps } from "./deps";

function pkg(id: string, uniqueId: string, name: string, version: string, enabled = true): PackageView {
  return {
    id,
    path: "/x",
    system: false,
    enabled,
    broken: [],
    backupVersion: null,
    tags: [],
    mods: [
      {
        relPath: "",
        source: null,
        issues: [],
        requiredBy: [],
        hasConfig: false,
        hasSchema: false,
        github: null,
        githubCustom: false,
        missingTranslation: null,
        manifest: {
          name,
          author: "",
          version,
          description: "",
          uniqueId,
          entryDll: null,
          contentPackFor: null,
          dependencies: [],
          updateKeys: [],
          minimumApiVersion: null,
          minimumGameVersion: null,
        },
      },
    ],
  };
}

describe("nameFromId", () => {
  it("makes ids readable", () => {
    expect(nameFromId("spacechase0.JsonAssets")).toBe("Json Assets");
    expect(nameFromId("FlashShifter.StardewValleyExpandedCP")).toBe("Stardew Valley Expanded CP");
    expect(nameFromId("LemurKat.CleanerSewer.CP")).toBe("CP");
    expect(nameFromId("zander.immersivefarm2")).toBe("immersivefarm2");
  });
});

describe("resolveDeps", () => {
  it("resolves states and orders required, installed first", () => {
    const packages = [
      pkg("JA", "spacechase0.JsonAssets", "Json Assets", "1.11.0"),
      pkg("Guild", "supert.guildexpandedja", "Guild Expanded", "1.0.1"),
      pkg("NPCMap", "Bouhm.NPCMapLocations", "NPC Map Locations", "3.3.3", false),
    ];
    const rows = resolveDeps(
      [
        { uniqueId: "Bouhm.NPCMapLocations", minimumVersion: null, isRequired: false },
        { uniqueId: "Esca.CustomNPCExclusions", minimumVersion: null, isRequired: true },
        { uniqueId: "supert.guildexpandedja", minimumVersion: "1.0.2", isRequired: true },
        { uniqueId: "spacechase0.jsonassets", minimumVersion: null, isRequired: true },
      ],
      packages,
    );
    expect(rows.map((r) => [r.name, r.state, r.required])).toEqual([
      ["Guild Expanded", "old", true],
      ["Json Assets", "ok", true],
      ["Custom NPC Exclusions", "missing", true],
      ["NPC Map Locations", "off", false],
    ]);
    expect(rows[1].packageId).toBe("JA");
  });
});
