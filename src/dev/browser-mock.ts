// Lets `vite dev` run in a plain browser for UI work: Tauri IPC is answered from the
// fixtures below. Imported only in DEV outside Tauri, never shipped.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import ruCatalog from "../../src-tauri/resources/catalogs/ru.json";
import enCatalog from "../../src-tauri/resources/catalogs/en.json";

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
      missingTranslation: m.uniqueId === "Pathoschild.LookupAnything" ? "ru" : null,
    },
  ],
  broken: [],
  backupVersion: null,
  tags: [] as string[],
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
      { backupVersion: "1.48.0", tags: ["UI"] },
    ),
    pkg("CJBCheatsMenu", manifest("CJB Cheats Menu", "CJBok.CheatsMenu", "1.38.0", { updateKeys: ["Nexus:4"] }), {
      enabled: false,
    }),
  ],
  profiles: { active: "default", profiles: [{ id: "default", name: null, enabled: [] }] },
  bisect: null as null | {
    original: string;
    profile: string;
    suspects: string[];
    cleared: string[];
    testing: string[];
    round: number;
  },
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

const gmcmOption = (path: string[], name: string, kind: string, section: string) => ({
  path,
  name,
  tooltip: null,
  kind,
  section,
  min: null,
  max: null,
  interval: null,
  choices: null,
});

const games = [
  {
    path: "/Applications/Stardew Valley.app/Contents/MacOS",
    version: "1.6.15.24356",
    store: "gog",
    smapiVersion: "4.5.2",
    modsPath: "/mock/Mods",
    needsRosetta: false,
  },
  {
    path: "/mock/Steam/steamapps/common/Stardew Valley/Contents/MacOS",
    version: "1.6.16.24380",
    store: "steam",
    smapiVersion: null,
    modsPath: "/mock/Steam/Mods",
    needsRosetta: false,
  },
];
let currentGame = games[0].path;

mockWindows("main");
mockIPC(
  (cmd, payload) => {
    const a = (payload ?? {}) as Args;
    switch (cmd) {
      case "plugin:app|version":
        return "0.1.0";
      case "settings_get":
        return {
          gamePath: "/Applications/Stardew Valley.app/Contents/MacOS",
          // the WebKit screen tests pick the language here
          locale: localStorage.getItem("mockLocale"),
          companionDisabled: false,
        };
      case "settings_set_companion":
        return {
          gamePath: "/Applications/Stardew Valley.app/Contents/MacOS",
          locale: null,
          companionDisabled: !a.enabled,
        };
      case "companion_available":
        return true;
      case "companion_profile_start":
        return 42;
      case "companion_profile_stop":
        return {
          seconds: 30,
          mods: [
            {
              mod: "Automate",
              totalMs: 830,
              maxMs: 119,
              calls: 1800,
              events: [{ event: "UpdateTicked", totalMs: 800, calls: 1800 }],
            },
            {
              mod: "Lookup Anything",
              totalMs: 12,
              maxMs: 0.8,
              calls: 1800,
              events: [{ event: "UpdateTicked", totalMs: 12, calls: 1800 }],
            },
          ],
        };
      case "companion_patches":
        return {
          patched: 741,
          shared: [
            {
              method: "StardewValley.Menus.InventoryMenu.draw",
              patches: [
                { owner: "Pathoschild.LookupAnything", kind: "postfix", canSkip: false },
                { owner: "spacechase0.SpaceCore", kind: "prefix", canSkip: true },
              ],
            },
          ],
        };
      case "companion_status":
        return {
          connected: true,
          hello: { protocol: 1, smapi: "4.5.2", game: "1.6.15", mods: ["Pathoschild.LookupAnything"] },
          state: {
            farmer: "Аня",
            farm: "Ромашка",
            season: "fall",
            day: 12,
            year: 2,
            time: 1430,
            money: 12345,
            location: "Town",
          },
        };
      case "companion_forecast": {
        const g = (id: string, name: string, next: string[]) => ({
          id,
          name,
          next: next.map((n, i) => ({ id: `(O)${i}`, name: n, stack: n === "Медная руда" ? 10 : 1 })),
        });
        return {
          luck: 0.054,
          weatherTomorrow: "Rain",
          islandWeatherTomorrow: "Sun",
          geodesCracked: 41,
          mysteryBoxesOpened: 3,
          mysteryBoxes: [
            g("(O)MysteryBox", "Таинственная коробка", ["Тотем-телепорт: Горы", "Кофейные зёрна", "Бомба"]),
            g("(O)GoldenMysteryBox", "Золотая таинственная коробка", ["Призматический осколок", "Книга тайн", "Алмаз"]),
          ],
          nights: [
            { day: 9, season: 2, year: 1, event: "meteor" },
            { day: 21, season: 2, year: 1, event: "fairy" },
          ],
          bookseller: [4, 19],
          cart: [
            { id: "(O)266", name: "Красная капуста", price: 780, stock: 1 },
            { id: "(F)1226", name: "Картина «Мост»", price: 1800, stock: 1 },
          ],
          geodes: [
            g("(O)535", "Жеода", ["Медная руда", "Кварц", "Аметист", "Глина", "Тигровый глаз"]),
            g("(O)536", "Замёрзшая жеода", ["Кварц", "Аквамарин", "Медная руда", "Лёд", "Кварц"]),
            g("(O)537", "Магматическая жеода", ["Огненный кварц", "Золотая руда", "Обсидиан", "Кварц", "Рубин"]),
            g("(O)749", "Всежеода", ["Призматический осколок", "Кварц", "Бирюза", "Огненный опал", "Нефрит"]),
            g("(O)275", "Сокровищница", ["Древний меч", "Кукла", "Наконечник", "Чаша", "Монета"]),
            g("(O)791", "Золотой кокос", ["Банан", "Манго", "Кокос", "Банан", "Манго"]),
          ],
        };
      }
      case "companion_items": {
        const it = (
          name: string,
          category: number,
          quality: number,
          price: number,
          stack: number,
          where = "inventory",
        ) => ({
          id: name,
          name,
          category,
          quality,
          price,
          stack,
          where,
        });
        return [
          it("Тыква", -75, 2, 480, 6),
          it("Карамбола", -79, 1, 937, 3, "Ферма"),
          it("Большое яйцо", -5, 2, 142, 4),
          it("Сардина", -4, 0, 40, 2),
          it("Аметист", -2, 0, 100, 1, "Ферма"),
          it("Порей", -81, 1, 75, 3),
          it("Вино", -26, 2, 1050, 2, "Подвал"),
          it("Пастернак", -75, 0, 35, 20),
        ];
      }
      case "companion_apply":
        return { ok: true, method: "gmcm", error: null };
      case "game_current":
        return games.find((g) => g.path === currentGame);
      case "game_detect":
        return games;
      case "game_set_path":
        currentGame = (a as { path: string }).path;
        return games.find((g) => g.path === currentGame);
      case "game_forget":
        return null;
      case "game_state":
        return { state: "idle" };
      case "game_log":
      case "downloads_list":
        return [];
      case "smapi_latest":
        return { version: "4.5.2", url: "", size: 0, notesUrl: "https://github.com/Pathoschild/SMAPI/releases" };
      case "library_get":
        return library;
      case "bisect_start": {
        const all = library.packages.filter((p) => !p.system && p.enabled).map((p) => p.id);
        library.bisect = {
          original: "default",
          profile: "p9",
          suspects: all,
          cleared: [],
          testing: all.slice(0, Math.ceil(all.length / 2)),
          round: 1,
        };
        return structuredClone(library);
      }
      case "bisect_answer": {
        const b = library.bisect!;
        b.suspects = (a as { problem: boolean }).problem ? b.testing : b.suspects.filter((s) => !b.testing.includes(s));
        if (b.suspects.length > 1) {
          b.testing = b.suspects.slice(0, Math.ceil(b.suspects.length / 2));
          b.round++;
          return { step: { kind: "next", enabled: b.testing }, view: structuredClone(library) };
        }
        const step = b.suspects.length ? { kind: "found", package: b.suspects[0] } : { kind: "clean" };
        library.bisect = null;
        return { step, view: structuredClone(library) };
      }
      case "bisect_cancel":
        library.bisect = null;
        return structuredClone(library);
      case "nexus_collection":
        return {
          collection: {
            slug: "tckf0m",
            name: "Cozy Valley",
            author: "Someone",
            revision: 7,
            files: [
              { modId: 541, fileId: 1, modName: "Lookup Anything", version: "1.55.0", optional: false },
              { modId: 1915, fileId: 2, modName: "Content Patcher", version: "2.5.3", optional: false },
              { modId: 3753, fileId: 3, modName: "Stardew Valley Expanded", version: "1.15.11", optional: false },
              { modId: 2400, fileId: 4, modName: "SMAPI Extras", version: "1.0.0", optional: true },
            ],
          },
          installed: [541, 1915],
        };
      case "nexus_file_page":
        return "https://www.nexusmods.com/stardewvalley/mods/3753?tab=files&file_id=3";
      case "mod_translation_get":
        return [
          { key: "config.title", source: "Lookup Anything settings", text: "" },
          { key: "hud.hint", source: "Press F1 to look things up", text: "" },
        ];
      case "mod_translation_save":
        return structuredClone(library);
      case "library_set_tags": {
        const { id, tags } = a as { id: string; tags: string[] };
        const p = library.packages.find((x) => x.id === id);
        if (p) p.tags = tags;
        // a fresh object, as from the backend: the store's view must see it change
        return structuredClone(library);
      }
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
            labels: {},
            gmcm: null,
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
          labels: {
            showunknowngifttastes: {
              name: "Show unknown gift tastes",
              description: "Whether to show gift tastes you haven't learned about in-game yet.",
              values: {},
            },
            togglelookup: {
              name: "Toggle lookup",
              description: "Shows the lookup for whatever is under the cursor.",
              values: {},
            },
            controls: { name: "Keybinds", description: null, values: {} },
          },
          gmcm: {
            id: "Pathoschild.LookupAnything",
            titleScreenOnly: false,
            options: [
              gmcmOption(["ShowUnknownGiftTastes"], "Show unknown gift tastes", "bool", "Progression mode"),
              gmcmOption(["ShowDataMiningFields"], "Show data mining fields", "bool", "Advanced"),
              { ...gmcmOption(["TableRows"], "Table rows", "int", "Advanced"), min: 5, max: 50, interval: 5 },
              { ...gmcmOption(["Scale"], "Scale", "float", "Advanced"), min: 0.5, max: 2, interval: 0.25 },
              gmcmOption(["Controls", "ToggleLookup"], "Toggle lookup", "keybind", "Controls"),
            ],
          },
        };
      case "saves_list":
        return [save];
      case "saves_perfection": {
        const m = (key: string, name = key, extra = {}) => ({
          key,
          name,
          have: null,
          need: null,
          known: null,
          ...extra,
        });
        const part = (id: string, weight: number, done: number, total: number, missing: unknown[] = []) => ({
          id,
          weight,
          done,
          total,
          score: Math.round((done / total) * weight * 10) / 10,
          missing,
        });
        return {
          percent: 41.3,
          catalog: { game: "1.6.15", language: "ru", builtin: false },
          parts: [
            part("shipping", 15, 98, 154, [m("188", "Зелёная фасоль"), m("190", "Цветная капуста")]),
            part("obelisks", 4, 1, 4, [m("Water Obelisk"), m("Desert Obelisk"), m("Island Obelisk")]),
            { ...part("clock", 10, 0, 1, [m("Gold Clock")]), score: 0 },
            { ...part("slayer", 10, 7, 12, [m("Slimes", "Слаймы", { have: 640, need: 1000 })]), score: 0 },
            part("friends", 11, 6, 34, [
              m("Pierre", "Пьер", { have: 6, need: 10 }),
              m("Abigail", "Эбигейл", { have: 5, need: 8 }),
            ]),
            part("level", 5, 19, 25),
            {
              ...part("stardrops", 10, 3, 7, [m("CF_Sewer"), m("CF_Statue"), m("CF_Fish"), m("museumComplete")]),
              score: 0,
            },
            part("cooking", 10, 30, 81, [m("Omelet", "Омлет", { known: true }), m("Salad", "Салат", { known: false })]),
            part("crafting", 10, 60, 149, [m("Keg", "Бочонок", { known: false })]),
            part("fish", 10, 38, 72, [m("(O)128", "Иглобрюх")]),
            part("walnuts", 5, 65, 130),
          ],
        };
      }
      case "saves_mirror":
        return 2;
      case "settings_set_backup_folder":
        return { gamePath: null, locale: null, companionDisabled: false, wiki: null, backupFolder: a.folder };
      case "saves_delete":
        return null;
      case "mod_save_usage":
        return [
          {
            saveId: "Anya_1",
            farmer: "Аня",
            farmName: "Ромашка",
            references: 3,
            locations: ["Custom_AndyHouse", "Custom_SophiaHouse"],
            locationCount: 120,
          },
        ];
      case "mod_changelog":
        return { source: "github", versions: [{ version: "1.55.0", lines: ["## What's Changed", "* Fixed a crash"] }] };
      case "library_conflicts":
        return [];
      case "compat_for_nexus":
        return {};
      case "storage_usage":
        return [
          { kind: "modBackups", bytes: 48_000_000, files: 120 },
          { kind: "removed", bytes: 0, files: 0 },
          { kind: "saveBackups", bytes: 31_500_000, files: 40 },
          { kind: "downloads", bytes: 2_100_000, files: 3 },
        ];
      case "history_list":
        return [
          {
            at: Date.now() - 3600_000,
            kind: "updated",
            package: "LookupAnything",
            name: "Lookup Anything",
            uniqueId: "Pathoschild.LookupAnything",
            version: "1.49.0",
            previous: "1.48.0",
            undone: false,
            canUndo: true,
          },
        ];
      case "game_crash":
        return null;
      case "game_preflight":
        return null;
      case "game_catalog":
        return localStorage.getItem("mockLocale") === "ru" ? ruCatalog : enCatalog;
      case "smapi_log_report":
        return {
          written: Math.floor(Date.now() / 1000) - 600,
          skipped: [
            {
              name: "SpaceCore",
              version: "1.25.2",
              kind: "incompatible",
              needs: null,
              reason: "it's no longer compatible.",
            },
            {
              name: "Json Assets",
              version: "1.11.8",
              kind: "dependency",
              needs: "SpaceCore",
              reason: "it needs the 'SpaceCore' mod",
            },
          ],
          lostLocations: ["Custom_AndyHouse", "Custom_SophiaHouse"],
          lostNpcs: ["Andy", "Sophia"],
          startupSecs: 48,
          slow: [{ name: "Stardew Valley Expanded", secs: 9 }],
          heavy: [{ name: "Content Patcher", assets: 412 }],
          mods: [
            {
              source: "Content Patcher",
              errors: 2,
              warnings: 0,
              entries: [
                { level: "error", text: "Two content packs want to load the 'Animals/horse' asset.", count: 2 },
              ],
            },
          ],
        };
      case "wiki_titles":
        return Object.fromEntries((a.ids as string[]).map((id) => [id, "Parsnip"]));
      case "settings_set_wiki":
        return { gamePath: null, locale: null, companionDisabled: false, wiki: a.wiki };
      case "game_language":
        return "ru";
      case "nxm_take_pending":
        return [];
      case "saves_compare":
        return {
          from: {
            year: 2,
            season: 0,
            day: 13,
            money: 589232,
            totalEarned: 2365488,
            playTimeMs: 264071168,
            perfection: 25.7,
          },
          to: {
            year: 2,
            season: 0,
            day: 14,
            money: 588232,
            totalEarned: 2368488,
            playTimeMs: 264733536,
            perfection: 26.1,
          },
          skills: [{ key: "fishing", name: "Рыбная ловля", from: 6, to: 7 }],
          friends: [{ key: "Abigail", name: "Абигейл", from: 4, to: 5 }],
          firsts: [{ kind: "fish", names: ["Тунец"] }],
          items: [
            { key: "(O)344", name: "Джем", from: 1, to: 16 },
            { key: "(O)404", name: "Шампиньон", from: 70, to: 53 },
          ],
        };
      case "saves_items":
        return [
          {
            id: "(O)337",
            name: "Iridium Bar",
            display: "Iridium Bar",
            stack: 130,
            quality: 0,
            place: { location: "Big Shed", kind: "chest", tile: [10, 9] },
            locationName: "Big Shed",
          },
          {
            id: "(O)337",
            name: "Iridium Bar",
            display: "Iridium Bar",
            stack: 4,
            quality: 0,
            place: { location: "inventory", kind: "inventory", tile: null },
            locationName: null,
          },
          {
            id: "(O)184",
            name: "Milk",
            display: "Milk",
            stack: 12,
            quality: 2,
            place: { location: "Deluxe Barn", kind: "grabber", tile: null },
            locationName: "Deluxe Barn",
          },
          {
            id: "(O)196",
            name: "Salad",
            display: "Salad",
            stack: 3,
            quality: 0,
            place: { location: "FarmHouse", kind: "fridge", tile: null },
            locationName: null,
          },
        ];
      case "saves_checkup": {
        const it = (id: string, name: string | null, done: boolean, extra = {}) => ({
          id,
          name,
          label: null,
          wiki: name,
          have: null,
          need: null,
          done,
          note: null,
          ...extra,
        });
        const goal = (label: string, have: number, need: number) => ({
          id: `${label}-${need}`,
          name: null,
          label,
          have,
          need,
          done: have >= need,
          note: null,
        });
        const group = (id: string, items: { done: boolean }[], extra = {}) => ({
          id,
          title: null,
          category: null,
          done: items.filter((i) => i.done).length,
          total: items.length,
          items,
          ...extra,
        });
        const section = (id: string, groups: { done: number; total: number }[]) => ({
          id,
          done: groups.reduce((a, g) => a + g.done, 0),
          total: groups.reduce((a, g) => a + g.total, 0),
          groups,
        });
        return {
          catalog: { game: "1.6.15", language: "ru", builtin: false },
          sections: [
            section("achievements", [
              group("achievementList", [
                it("0", "Greenhorn", true),
                it("4", "Legend", false),
                it("5", "A Complete Collection", false),
              ]),
            ]),
            section("cc", [
              group(
                "bundle-0",
                [
                  it("0-0", "Пастернак", true),
                  it("0-1", "Зелёная фасоль", false),
                  it("0-2", "Цветная капуста", true, { need: 5, note: "gold" }),
                ],
                {
                  title: "Весенние культуры",
                  category: "Pantry",
                },
              ),
              group("bundle-23", [{ ...goal("gold", 2500, 2500), done: false }], {
                title: "2 500з",
                category: "Vault",
              }),
            ]),
            section("museum", [
              group("museumGoals", [
                goal("treasureTrove", 41, 40),
                goal("rustyKey", 41, 60),
                goal("completeCollection", 41, 95),
              ]),
              group("minerals", [
                it("74", "Призматический осколок", false, { note: "foundNotDonated" }),
                it("60", "Изумруд", true),
              ]),
            ]),
            section("friends", [
              group("villagers", [
                it("Abigail", "Эбигейл", false, { have: 6, need: 8, note: "dating" }),
                it("Krobus", "Кробус", false, { have: 0, need: 10, note: "notMet" }),
              ]),
            ]),
            section("money", [group("earned", [goal("earn", 60000, 15000), goal("earn", 60000, 1000000)])]),
          ],
        };
      }
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
