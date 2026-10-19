// Lets `vite dev` run in a plain browser for UI work: Tauri IPC is answered from the
// fixtures below. Imported only in DEV outside Tauri, never shipped.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

type Args = Record<string, unknown>;

const IMG = "https://staticdelivery.nexusmods.com/mods/1303/images";

const manifest = (name: string, id: string, version: string, extra: Record<string, unknown> = {}) => ({
  name,
  author: "Pathoschild",
  version,
  description: `${name} does something cozy for your farm.`,
  uniqueId: id,
  entryDll: `${name}.dll`,
  contentPackFor: null,
  updateKeys: [] as string[],
  minimumApiVersion: "4.0.0",
  minimumGameVersion: null,
  dependencies: [] as { uniqueId: string; minimumVersion: string | null; isRequired: boolean }[],
  ...extra,
});

const pkg = (id: string, m: ReturnType<typeof manifest>, extra: Record<string, unknown> = {}) => ({
  id,
  path: `/mock/mods/${id}`,
  system: false,
  enabled: true,
  mods: [
    {
      relPath: "",
      manifest: m,
      source: m.updateKeys[0]?.split(":")[0] ?? null,
      issues: [],
      requiredBy: [],
      hasConfig: true,
      hasSchema: false,
    },
  ],
  broken: [],
  backupVersion: null,
  ...extra,
});

const library = {
  packages: [
    pkg("ConsoleCommands", manifest("Console Commands", "SMAPI.ConsoleCommands", "4.5.2"), { system: true }),
    pkg(
      "ContentPatcher",
      manifest("Content Patcher", "Pathoschild.ContentPatcher", "2.5.3", { updateKeys: ["Nexus:1915"] }),
    ),
    pkg(
      "LookupAnything",
      manifest("Lookup Anything", "Pathoschild.LookupAnything", "1.49.0", {
        updateKeys: ["Nexus:541"],
        dependencies: [
          { uniqueId: "Pathoschild.ContentPatcher", minimumVersion: "2.0.0", isRequired: true },
          { uniqueId: "spacechase0.JsonAssets", minimumVersion: null, isRequired: true },
          { uniqueId: "CJBok.CheatsMenu", minimumVersion: null, isRequired: true },
          { uniqueId: "Bouhm.NPCMapLocations", minimumVersion: null, isRequired: false },
        ],
      }),
      { backupVersion: "1.48.0" },
    ),
    pkg("CJBCheatsMenu", manifest("CJB Cheats Menu", "CJBok.CheatsMenu", "1.38.0", { updateKeys: ["Nexus:4"] }), {
      enabled: false,
    }),
  ],
  profiles: { active: "default", profiles: [{ id: "default", name: null, enabled: [] }] },
  importable: [],
};

const description = [
  "See live info about whatever's under your cursor when you press [font=Courier New]F1[/font].",
  "<br />",
  `[img]${IMG}/239/239-1522469229-1390441983.png[/img]`,
  "<br />[size=6][b]Install[/b][/size][list=1]",
  "<br />[*][url=https://smapi.io]Install the latest version of SMAPI[/url].",
  "<br />[*]Unzip the mod folder into [font=Courier New]Stardew Valley/Mods[/font].",
  "<br />[/list]",
  `<br />[img]${IMG}/9633/9633-1631550926-1887055055.png[/img]`,
  `<br />[img]${IMG}/1820/1820-1699940399-1369623116.png[/img]`,
  "<br />".repeat(3) + "More text to scroll through. ".repeat(40),
].join("\n");

const nexusMod = {
  modId: 541,
  name: "Lookup Anything",
  summary: "See live info about whatever's under your cursor.",
  description,
  author: "Pathoschild",
  version: "1.55.0",
  pictureUrl: `${IMG}/2400/2400-1739896414-1995424095.png`,
  endorsements: 152000,
  downloads: 9800000,
  updatedAt: "2026-09-30T10:00:00Z",
  category: "User Interface",
  requirements: [
    { modId: 2400, modName: "SMAPI - Stardew Modding API", url: null, notes: null, externalRequirement: false },
  ],
  files: [
    {
      fileId: 1,
      name: "Lookup Anything",
      version: "1.55.0",
      category: "MAIN",
      sizeInBytes: 1_200_000,
      date: 1_790_000_000,
      description: null,
    },
  ],
};

const card = (modId: number, name: string) => ({
  modId,
  name,
  summary: "A short summary of what this mod does for your farm.",
  author: "Someone",
  version: "1.0.0",
  thumbnailUrl: `${IMG}/thumbnails/2400/2400-1739896414-1995424095.png`,
  endorsements: 12000,
  downloads: 300000,
  updatedAt: "2026-09-30T10:00:00Z",
  category: "Gameplay",
});

const save = {
  id: "Anya_123456789",
  path: "/mock/Saves/Anya_123456789",
  farmer: "Аня",
  farmName: "Ромашка",
  money: 1_284_500,
  totalEarned: 3_500_000,
  season: 1,
  day: 14,
  year: 2,
  playTimeMs: 36_000_000,
  gameVersion: "1.6.15",
  skills: { farming: 10, mining: 8, foraging: 7, fishing: 6, combat: 5 },
  modified: Math.floor(Date.now() / 1000) - 3600,
};

mockWindows("main");
mockIPC(
  (cmd, payload) => {
    const a = (payload ?? {}) as Args;
    switch (cmd) {
      case "settings_get":
        return { gamePath: "/Applications/Stardew Valley.app/Contents/MacOS", locale: null };
      case "game_current":
        return {
          path: "/Applications/Stardew Valley.app/Contents/MacOS",
          version: "1.6.15.24356",
          store: "gog",
          smapiVersion: "4.5.2",
          modsPath: "/mock/Mods",
          needsRosetta: false,
        };
      case "game_state":
        return { state: "idle" };
      case "game_log":
      case "downloads_list":
        return [];
      case "smapi_latest":
        return { version: "4.5.2", url: "", size: 0, notesUrl: "https://github.com/Pathoschild/SMAPI/releases" };
      case "library_get":
        return library;
      case "updates_check":
        return {
          checkedAt: Math.floor(Date.now() / 1000) - 600,
          mods: {
            "pathoschild.lookupanything": {
              version: "1.55.0",
              url: "https://www.nexusmods.com/stardewvalley/mods/541",
              compatStatus: "Ok",
              compatSummary: null,
              nexusId: 541,
              errors: [],
            },
          },
        };
      case "nexus_search":
        return {
          total: 40,
          items: [card(541, "Lookup Anything"), card(1915, "Content Patcher"), card(4, "CJB Cheats")],
          categories: [
            ["Gameplay Mechanics", 4240],
            ["User Interface", 1516],
            ["Fishing", 357],
          ],
        };
      case "nexus_mod":
        return { ...nexusMod, modId: Number(a.modId) };
      case "nexus_account":
        return null;
      case "mod_config_get":
        if (a.package === "ContentPatcher") {
          return {
            path: "/mock/mods/ContentPatcher/config.json",
            values: null,
            schema: [
              {
                key: "EnableJohn",
                allowValues: ["true", "false"],
                allowBlank: false,
                allowMultiple: false,
                default: ["true"],
                description: "Adds John to town",
                section: null,
                boolean: true,
              },
              {
                key: "Seasons",
                allowValues: ["spring", "summer", "fall", "winter"],
                allowBlank: false,
                allowMultiple: true,
                default: ["spring"],
                description: null,
                section: "Looks",
                boolean: false,
              },
            ],
          };
        }
        return {
          path: "/mock/mods/LookupAnything/config.json",
          values: {
            Controls: { ToggleLookup: "F1", ToggleSearch: "LeftShift + F1", ScrollUp: "Up" },
            ShowUnknownGiftTastes: false,
            ShowDataMiningFields: false,
            TableRows: 20,
            Scale: 1.25,
            HideOnKeyUp: true,
          },
          schema: null,
        };
      case "saves_list":
        return [save];
      case "saves_progress":
        return {
          ccRooms: ["ccPantry", "ccVault"],
          joja: false,
          museumItems: 41,
          fishSpecies: 38,
          cookingKnown: 52,
          cookingCooked: 30,
          craftingKnown: 60,
          friendships: [
            { name: "Abigail", points: 1250, hearts: 5 },
            { name: "Sebastian", points: 300, hearts: 1 },
          ],
        };
      case "saves_snapshots":
        return [{ saveId: save.id, created: Date.now() - 7_200_000, reason: "launch", size: 1, file: "/mock/b.zip" }];
    }
    console.warn("unmocked IPC", cmd, payload);
    return null;
  },
  { shouldMockEvents: true },
);
