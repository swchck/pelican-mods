import type { Crop } from "@/lib/crops";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Store = "steam" | "gog" | "other";

export interface GameInstall {
  path: string;
  version: string | null;
  store: Store;
  smapiVersion: string | null;
  modsPath: string;
  needsRosetta: boolean;
}

export interface Settings {
  gamePath: string | null;
  locale: string | null;
  /** The companion mod is linked into the game unless this is set. */
  companionDisabled: boolean;
  /** Whether helpers show what the game keeps hidden until it happens. */
  spoilers?: boolean;
  briefingDisabled?: boolean;
  /** "official", "fandom" or "off"; null is the official wiki. */
  wiki: string | null;
  /** Folder changed saves are copied to after each session. */
  backupFolder: string | null;
}

export interface SmapiRelease {
  version: string;
  url: string;
  size: number;
  notesUrl: string;
}

export type SmapiProgress = { stage: "download"; received: number; total: number } | { stage: "extract" | "install" };

export type RunState =
  | { state: "idle" }
  | { state: "running"; modded: boolean; pid: number | null }
  | { state: "exited"; code: number | null };

export type LogLevel = "trace" | "debug" | "info" | "warn" | "error" | "alert";

export interface LogLine {
  seq: number;
  level: LogLevel;
  text: string;
}

export type Place = "game" | "mods" | "smapiLog" | "saves";

/** Error shape every command rejects with; `kind` keys the translated message. */
export interface AppError {
  kind: string;
  detail: string | null;
  message: string;
}

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

export const api = {
  settings: () => invoke<Settings>("settings_get"),
  setLocale: (locale: string | null) => invoke<Settings>("settings_set_locale", { locale }),
  gameLanguage: () => invoke<string | null>("game_language"),
  logReport: () => invoke<LogReport>("smapi_log_report"),
  preflight: () => invoke<Preflight | null>("game_preflight"),
  crash: () => invoke<Crash | null>("game_crash"),
  gameCatalog: () => invoke<GameCatalog>("game_catalog"),
  setWiki: (wiki: string) => invoke<Settings>("settings_set_wiki", { wiki }),
  setSpoilers: (enabled: boolean) => invoke<Settings>("settings_set_spoilers", { enabled }),
  setBackupFolder: (folder: string | null) => invoke<Settings>("settings_set_backup_folder", { folder }),
  storage: () => invoke<StorageUsage[]>("storage_usage"),
  compatForNexus: (ids: number[]) => invoke<Record<number, CompatVerdict>>("compat_for_nexus", { ids }),
  cleanStorage: (kind: StorageUsage["kind"]) => invoke<number>("storage_clean", { kind }),
  wikiTitles: (ids: string[]) => invoke<Record<string, string>>("wiki_titles", { ids }),
  detectGames: () => invoke<GameInstall[]>("game_detect"),
  currentGame: () => invoke<GameInstall | null>("game_current"),
  setGamePath: (path: string) => invoke<GameInstall>("game_set_path", { path }),
  forgetGame: (path: string) => invoke<void>("game_forget", { path }),
  smapiLatest: () => invoke<SmapiRelease>("smapi_latest"),
  smapiRun: (action: "install" | "uninstall") => invoke<GameInstall>("smapi_run", { action }),
  launch: (modded: boolean) => invoke<RunState>("game_launch", { modded }),
  stop: () => invoke<void>("game_stop"),
  runState: () => invoke<RunState>("game_state"),
  log: () => invoke<LogLine[]>("game_log"),
  console: (line: string) => invoke<void>("game_console", { line }),
  open: (place: Place) => invoke<void>("open_place", { place }),
  installRosetta: () => invoke<void>("rosetta_install"),
};

export const events = {
  /** Console lines arrive in batches of whatever was printed in the last ~80 ms. */
  log: (cb: (lines: LogLine[]) => void): Promise<UnlistenFn> => listen<LogLine[]>("game:log", (e) => cb(e.payload)),
  state: (cb: (s: RunState) => void): Promise<UnlistenFn> => listen<RunState>("game:state", (e) => cb(e.payload)),
  smapiProgress: (cb: (p: SmapiProgress) => void): Promise<UnlistenFn> =>
    listen<SmapiProgress>("smapi:progress", (e) => cb(e.payload)),
};

export interface Dependency {
  uniqueId: string;
  minimumVersion: string | null;
  isRequired: boolean;
}

export interface Manifest {
  name: string;
  author: string;
  version: string;
  description: string;
  uniqueId: string;
  entryDll: string | null;
  contentPackFor: Dependency | null;
  dependencies: Dependency[];
  updateKeys: string[];
  minimumApiVersion: string | null;
  minimumGameVersion: string | null;
}

export type Issue =
  | { kind: "missingDependency"; id: string; min: string | null }
  | { kind: "disabledDependency"; id: string; package: string }
  | { kind: "outdatedDependency"; id: string; need: string; have: string }
  | { kind: "duplicateId"; package: string }
  | { kind: "needsSmapi"; need: string }
  | { kind: "needsGame"; need: string };

export interface ModView {
  relPath: string;
  manifest: Manifest;
  source: string | null;
  issues: Issue[];
  requiredBy: string[];
  hasConfig: boolean;
  /** A content pack whose content.json declares a ConfigSchema. */
  hasSchema: boolean;
  /** "owner/repo" updates come from first: the user's, else the manifest's. */
  github: string | null;
  /** Whether `github` is one the user attached. */
  githubCustom: boolean;
  /** The game's language code when the mod has translations but none in it. */
  missingTranslation: string | null;
}

export interface PackageView {
  id: string;
  path: string;
  system: boolean;
  enabled: boolean;
  mods: ModView[];
  broken: { relPath: string; error: string }[];
  backupVersion: string | null;
  /** The player's labels for the package. */
  tags: string[];
}

export interface UpdateInfo {
  version: string | null;
  url: string | null;
  compatStatus: string | null;
  compatSummary: string | null;
  nexusId: number | null;
  errors: string[];
}

export type UpdateOutcome =
  { kind: "installed"; view: LibraryView } | { kind: "queued" } | { kind: "openPage"; url: string };

export interface UpdateReport {
  /** Unix seconds; 0 means never checked. */
  checkedAt: number;
  /** Keyed by lowercased UniqueID. */
  mods: Record<string, UpdateInfo>;
}

export interface Profile {
  id: string;
  name: string | null;
  enabled: string[];
  /** Keeps its own copy of every mod's settings. */
  ownConfigs: boolean;
}

export interface Profiles {
  active: string;
  profiles: Profile[];
  /** The profile each save was last played with, by save id. */
  saveProfiles: Record<string, string>;
}

export interface LibraryView {
  packages: PackageView[];
  profiles: Profiles;
  importable: string[];
  /** The search for a problem's mod, while one runs. */
  bisect: Bisect | null;
}

/** A search for the mod behind a problem: each round turns on half the suspects. */
export interface Bisect {
  original: string;
  profile: string;
  suspects: string[];
  cleared: string[];
  testing: string[];
  round: number;
  /** Always on: the mod a partner search looks for a partner of. */
  pinned: string[];
  /** This round runs the pinned mod alone. */
  alone: boolean;
}

export type BisectStep =
  | { kind: "next"; enabled: string[] }
  | { kind: "found"; package: string; with: string | null; cleared: string[] }
  | { kind: "clean" };

/** A partner on the local network sharing their mod list. */
export interface LanHost {
  name: string;
  addr: string;
  port: number;
}

export const lan = {
  /** Shares the active profile's list until stopped. */
  shareStart: () => invoke<void>("lan_share_start"),
  shareStop: () => invoke<void>("lan_share_stop"),
  discover: () => invoke<LanHost[]>("lan_discover"),
  /** Fetches a host's list; `file` is where it's kept for reading again after installs. */
  fetch: (addr: string, port: number) => invoke<{ file: string; view: ModListView }>("lan_fetch", { addr, port }),
};

export const library = {
  get: () => invoke<LibraryView>("library_get"),
  install: (paths: string[]) => invoke<LibraryView>("library_install", { paths }),
  /** Builds a Content Patcher pack from pictures, each replacing one asset, and installs it. */
  makePack: (name: string, author: string, items: { file: string; target: string }[]) =>
    invoke<LibraryView>("library_make_pack", { name, author, items }),
  import: (names: string[]) => invoke<LibraryView>("library_import", { names }),
  remove: (id: string) => invoke<LibraryView>("library_remove", { id }),
  toggle: (id: string, enabled: boolean) => invoke<LibraryView>("library_toggle", { id, enabled }),
  open: (path: string) => invoke<void>("library_open", { path }),
  createProfile: (name: string, copyFrom: string | null) => invoke<LibraryView>("profile_create", { name, copyFrom }),
  renameProfile: (id: string, name: string) => invoke<LibraryView>("profile_rename", { id, name }),
  deleteProfile: (id: string) => invoke<LibraryView>("profile_delete", { id }),
  activateProfile: (id: string) => invoke<LibraryView>("profile_activate", { id }),
  setOwnConfigs: (id: string, on: boolean) => invoke<LibraryView>("profile_set_own_configs", { id, on }),
  checkUpdates: (force: boolean) => invoke<UpdateReport>("updates_check", { force }),
  update: (id: string) => invoke<UpdateOutcome>("mod_update", { id }),
  rollback: (uniqueId: string) => invoke<LibraryView>("mod_rollback", { uniqueId }),
  setGithub: (uniqueId: string, repo: string | null) => invoke<LibraryView>("mod_set_github", { uniqueId, repo }),
  rememberProfile: (since: number) => invoke<LibraryView>("saves_remember_profile", { since }),
  history: () => invoke<HistoryEntry[]>("history_list"),
  undo: (at: number) => invoke<LibraryView>("history_undo", { at }),
  setTags: (id: string, tags: string[]) => invoke<LibraryView>("library_set_tags", { id, tags }),
  bisectStart: (name: string) => invoke<LibraryView>("bisect_start", { name }),
  bisectAnswer: (problem: boolean) => invoke<{ step: BisectStep; view: LibraryView }>("bisect_answer", { problem }),
  bisectCancel: () => invoke<LibraryView>("bisect_cancel"),
  bisectPartner: (name: string, pkg: string, candidates: string[]) =>
    invoke<LibraryView>("bisect_partner", { name, package: pkg, candidates }),
  installDependency: (uniqueId: string, updateKeys?: string[], github?: string | null) =>
    invoke<UpdateOutcome>("mod_install_dependency", { uniqueId, updateKeys, github }),
  exportList: (dest: string) => invoke<void>("modlist_export", { dest }),
  readList: (file: string) => invoke<ModListView>("modlist_read", { file }),
  profileFromList: (name: string, uniqueIds: string[]) => invoke<LibraryView>("modlist_profile", { name, uniqueIds }),
  saveUsage: (id: string) => invoke<SaveUsage[]>("mod_save_usage", { id }),
  conflicts: () => invoke<Conflict[]>("library_conflicts"),
  changelog: (id: string) => invoke<Changelog | null>("mod_changelog", { id }),
};

export type NexusSort = "endorsements" | "downloads" | "updated" | "created";

export interface NexusCard {
  modId: number;
  name: string;
  summary: string | null;
  author: string | null;
  version: string | null;
  thumbnailUrl: string | null;
  endorsements: number;
  downloads: number;
  updatedAt: string | null;
  category: string | null;
}

export interface NexusPage {
  total: number;
  items: NexusCard[];
  /** Categories matching the query, with mod counts. */
  categories: [string, number][];
}

export interface NexusFilters {
  category: string | null;
  /** Unix seconds. */
  updatedSince: number | null;
  minEndorsements: number | null;
}

export interface NexusFile {
  fileId: number;
  name: string;
  version: string | null;
  category: string | null;
  sizeInBytes: number | null;
  /** Unix seconds. */
  date: number;
  description: string | null;
}

export interface NexusRequirement {
  modId: number | null;
  modName: string | null;
  url: string | null;
  notes: string | null;
  externalRequirement: boolean;
}

export interface NexusMod {
  modId: number;
  name: string;
  summary: string | null;
  description: string | null;
  author: string | null;
  version: string | null;
  pictureUrl: string | null;
  endorsements: number;
  downloads: number;
  updatedAt: string | null;
  category: string | null;
  requirements: NexusRequirement[];
  files: NexusFile[];
}

export interface NexusAccount {
  name: string;
  isPremium: boolean;
  profileUrl: string | null;
}

export type DownloadState =
  | { state: "resolving" }
  | { state: "downloading"; received: number; total: number }
  | { state: "installing" }
  | { state: "done" }
  | { state: "failed"; error: AppError };

export type Download = { id: number; name: string; nexusModId: number | null } & DownloadState;

export const nexus = {
  search: (query: string, filters: NexusFilters, sort: NexusSort, offset: number) =>
    invoke<NexusPage>("nexus_search", { query, filters, sort, offset }),
  mod: (modId: number) => invoke<NexusMod>("nexus_mod", { modId }),
  account: () => invoke<NexusAccount | null>("nexus_account"),
  setKey: (key: string) => invoke<NexusAccount>("nexus_set_key", { key }),
  clearKey: () => invoke<void>("nexus_clear_key"),
  download: (modId: number, fileId: number) => invoke<void>("nexus_download", { modId, fileId }),
  filePage: (modId: number, fileId: number) => invoke<string>("nexus_file_page", { modId, fileId }),
  collection: (input: string) => invoke<CollectionView>("nexus_collection", { input }),
  thumbnails: (ids: number[]) => invoke<Record<number, string>>("nexus_thumbnails", { ids }),
  openNxm: (url: string) => invoke<void>("nxm_open", { url }),
  takePendingNxm: () => invoke<NxmLink[]>("nxm_take_pending"),
  downloads: () => invoke<Download[]>("downloads_list"),
  clearDownloads: () => invoke<Download[]>("downloads_clear"),
};

export const nexusEvents = {
  downloads: (cb: (d: Download[]) => void): Promise<UnlistenFn> =>
    listen<Download[]>("downloads:changed", (e) => cb(e.payload)),
  libraryChanged: (cb: () => void): Promise<UnlistenFn> => listen("library:changed", () => cb()),
  nxmError: (cb: (e: AppError) => void): Promise<UnlistenFn> => listen<AppError>("nxm:error", (e) => cb(e.payload)),
  /** An nxm link from outside the app is waiting; collect it with `takePendingNxm`. */
  nxmPending: (cb: () => void): Promise<UnlistenFn> => listen("nxm:pending", () => cb()),
};

export interface Skills {
  farming: number;
  mining: number;
  foraging: number;
  fishing: number;
  combat: number;
}

export interface SaveSummary {
  id: string;
  path: string;
  farmer: string;
  farmName: string;
  money: number;
  totalEarned: number;
  /** 0 spring … 3 winter. */
  season: number;
  day: number;
  year: number;
  playTimeMs: number;
  gameVersion: string | null;
  skills: Skills;
  /** Unix seconds. */
  modified: number;
}

export interface SaveProgress {
  ccRooms: string[];
  joja: boolean;
  museumItems: number;
  fishSpecies: number;
  cookingKnown: number;
  cookingCooked: number;
  craftingKnown: number;
  friendships: { name: string; points: number; hearts: number }[];
}

export interface Snapshot {
  saveId: string;
  /** Unix milliseconds. */
  created: number;
  reason: "manual" | "launch" | "replaced";
  size: number;
  file: string;
}

export type PerfectionPartId =
  | "shipping"
  | "obelisks"
  | "clock"
  | "slayer"
  | "friends"
  | "level"
  | "stardrops"
  | "cooking"
  | "crafting"
  | "fish"
  | "walnuts";

export interface PerfectionMissing {
  key: string;
  name: string;
  /** Hearts or kills so far, for villagers and Monster Slayer goals. */
  have: number | null;
  need: number | null;
  /** For recipes: learned yet. */
  known: boolean | null;
  /** English page title on the Stardew wikis, for base game things. */
  wiki: string | null;
}

export interface PerfectionPart {
  id: PerfectionPartId;
  /** Share of the 100% this part is worth. */
  weight: number;
  done: number;
  /** null until the companion has sent the game's lists. */
  total: number | null;
  score: number | null;
  missing: PerfectionMissing[];
}

export interface Perfection {
  /** 0–100, once every part has a total. */
  percent: number | null;
  catalog: CatalogInfo | null;
  parts: PerfectionPart[];
}

export interface CheckupItem {
  id: string;
  /** A name from the game; otherwise translate `label` with have/need. */
  name: string | null;
  label: string | null;
  have: number | null;
  need: number | null;
  done: boolean;
  note: string | null;
  /** English page title on the Stardew wikis, for base game things. */
  wiki: string | null;
  /** Gifts a villager loves, for birthdays. */
  loves?: string[];
}

export interface CheckupGroup {
  id: string;
  title: string | null;
  /** Heading above the group, such as a Community Center room. */
  category: string | null;
  /** That heading as the game names it, when the launcher has the game's lists. */
  categoryName: string | null;
  done: number;
  total: number;
  items: CheckupItem[];
}

export interface CheckupSection {
  id: string;
  done: number;
  total: number;
  groups: CheckupGroup[];
}

export interface Checkup {
  sections: CheckupSection[];
  catalog: CatalogInfo | null;
}

/** Where a report's names and totals came from. */
export interface CatalogInfo {
  game: string;
  language: string;
  /** The launcher's own base-game lists: no mod content. */
  builtin: boolean;
}

export const saves = {
  list: () => invoke<SaveSummary[]>("saves_list"),
  checkup: (id: string) => invoke<Checkup>("saves_checkup", { id }),
  items: (id: string) => invoke<StoredItem[]>("saves_items", { id }),
  walnuts: (id: string) => invoke<Walnuts>("saves_walnuts", { id }),
  /** Money and progress by in-game day, oldest first. */
  /** A top-down map of "Farm", "Greenhouse" or "IslandWest" as of the last save. */
  map: (id: string, location: string) => invoke<FarmMap>("saves_map", { id, location }),
  history: (id: string) => invoke<FarmPoint[]>("saves_history", { id }),
  /** Each side is "current", "previous" or a snapshot's file. */
  compare: (id: string, from: string, to: string) => invoke<SaveDiff>("saves_compare", { id, from, to }),
  perfection: (id: string) => invoke<Perfection>("saves_perfection", { id }),
  progress: (id: string) => invoke<SaveProgress>("saves_progress", { id }),
  snapshots: (id: string) => invoke<Snapshot[]>("saves_snapshots", { id }),
  backup: (id: string) => invoke<Snapshot>("saves_backup", { id }),
  restore: (file: string) => invoke<string>("saves_restore", { file }),
  export: (id: string, dest: string) => invoke<void>("saves_export", { id, dest }),
  import: (file: string) => invoke<string>("saves_import", { file }),
  delete: (id: string) => invoke<void>("saves_delete", { id }),
  /** Copies changed saves to the backup folder from the settings; returns how many. */
  mirror: () => invoke<number>("saves_mirror"),
};

export interface SchemaField {
  key: string;
  allowValues: string[];
  allowBlank: boolean;
  allowMultiple: boolean;
  default: string[];
  description: string | null;
  section: string | null;
  boolean: boolean;
}

export interface ModConfig {
  path: string;
  /** null before the mod has written config.json. */
  values: Record<string, import("./config-form").Json> | null;
  schema: SchemaField[] | null;
  labels: Record<string, import("./config-form").OptionLabel>;
  /** The mod's Generic Mod Config Menu as the companion last saw it in a running game. */
  gmcm: GmcmMod | null;
  /** The untouched config: schema defaults, or what a fresh C# mod instance writes. */
  defaults: Record<string, import("./config-form").Json> | null;
}

export interface GmcmMod {
  id: string;
  titleScreenOnly: boolean;
  options: GmcmOption[];
}

export interface GmcmOption {
  /** config.json key path; null when the mod's getter didn't reveal it. */
  path: string[] | null;
  name: string;
  tooltip: string | null;
  kind: "bool" | "int" | "float" | "string" | "keybind" | "choice";
  section: string | null;
  min: number | null;
  max: number | null;
  interval: number | null;
  choices: { value: string; label: string }[] | null;
}

/** What the player is doing right now. */
export interface GameSnapshot {
  farmer: string;
  farm: string;
  season: string;
  day: number;
  year: number;
  /** Game clock as HHMM, 600–2600. */
  time: number;
  money: number;
  location: string | null;
  /** "sun", "rain", "storm" or "snow"; empty from an older companion. */
  weather: string;
}

export interface CompanionStatus {
  connected: boolean;
  hello: { protocol: number; smapi: string; game: string; mods: string[] } | null;
  state: GameSnapshot | null;
}

export interface Applied {
  ok: boolean;
  /** "gmcm" when the mod's own save flow ran, "reload" when only its config object changed. */
  method: "gmcm" | "reload" | null;
  /** "not_loaded", "unsupported", or the game's exception message. */
  error: string | null;
}

export interface Forecast {
  /** Today's daily luck, about -0.1 to 0.1. */
  luck: number;
  /** "Sun", "Rain", "Storm", "Snow", "Wind", "GreenRain", "Festival", "Wedding"… */
  weatherTomorrow: string | null;
  islandWeatherTomorrow: string | null;
  geodesCracked: number;
  geodes: TreasureSeries[];
  mysteryBoxesOpened: number;
  mysteryBoxes: TreasureSeries[];
  /** Nights with a random event in the next four weeks; quiet nights are left out. */
  nights: { day: number; season: number; year: number; event: NightEvent }[];
  /** The bookseller's days this season. */
  bookseller: number[] | null;
  /** The traveling cart's stock when it's in the forest today, else null. */
  cart: { id: string; name: string; price: number; stock: number }[] | null;
}

export type NightEvent = "fairy" | "witch" | "meteor" | "owl" | "capsule" | "raccoonTree" | "qiPlane" | "earthquake";

/** What a geode or box will give, for the next openings in order. */
export interface TreasureSeries {
  id: string;
  name: string;
  next: { id: string; name: string; stack: number }[];
}

export interface NxmLink {
  url: string;
  modId: number;
  fileId: number;
}

export const companion = {
  status: () => invoke<CompanionStatus>("companion_status"),
  available: () => invoke<boolean>("companion_available"),
  setEnabled: (enabled: boolean) => invoke<Settings>("settings_set_companion", { enabled }),
  setBriefing: (enabled: boolean) => invoke<Settings>("settings_set_briefing", { enabled }),
  apply: (uniqueId: string, values: unknown, notice: string | null) =>
    invoke<Applied>("companion_apply", { uniqueId, values, notice }),
  onStatus: (cb: (s: CompanionStatus) => void): Promise<UnlistenFn> =>
    listen<CompanionStatus>("companion:status", (e) => cb(e.payload)),
  onGmcm: (cb: (ids: string[]) => void): Promise<UnlistenFn> =>
    listen<string[]>("companion:gmcm", (e) => cb(e.payload)),
  onCatalog: (cb: () => void): Promise<UnlistenFn> => listen("companion:catalog", () => cb()),
  forecast: () => invoke<Forecast>("companion_forecast"),
  items: () => invoke<import("./grange").GrangeItem[]>("companion_items"),
  profileStart: () => invoke<number>("companion_profile_start"),
  profileStop: () => invoke<HandlerTimes>("companion_profile_stop"),
  patches: () => invoke<PatchReport>("companion_patches"),
};

/** Mods' SMAPI event handler time over a measurement, slowest first; times in ms. */
export interface HandlerTimes {
  seconds: number;
  mods: {
    mod: string;
    totalMs: number;
    maxMs: number;
    calls: number;
    events: { event: string; totalMs: number; calls: number }[];
  }[];
}

/** How many game methods carry Harmony patches, and those more than one mod patches. */
export interface PatchReport {
  patched: number;
  shared: PatchedMethod[];
}

/** A game method more than one mod patches with Harmony; owners are usually UniqueIDs. */
export interface PatchedMethod {
  method: string;
  patches: { owner: string; kind: "prefix" | "postfix" | "transpiler" | "finalizer"; canSkip: boolean }[];
}

/** One line of a mod's own text to translate. */
export interface TranslationLine {
  key: string;
  source: string;
  text: string;
}

export const translations = {
  get: (pkg: string, relPath: string, language: string) =>
    invoke<TranslationLine[]>("mod_translation_get", { package: pkg, relPath, language }),
  save: (pkg: string, relPath: string, language: string, texts: Record<string, string>) =>
    invoke<LibraryView>("mod_translation_save", { package: pkg, relPath, language, texts }),
};

export const modConfig = {
  get: (pkg: string, relPath: string, locale: string) =>
    invoke<ModConfig>("mod_config_get", { package: pkg, relPath, locale }),
  save: (pkg: string, relPath: string, values: unknown, locale: string) =>
    invoke<ModConfig>("mod_config_save", { package: pkg, relPath, values, locale }),
  defaults: (pkg: string, relPath: string, locale: string) =>
    invoke<ModConfig>("mod_config_defaults", { package: pkg, relPath, locale }),
  generate: () => invoke<void>("mods_generate_configs"),
};

/** What SMAPI's log from the last session says went wrong. */
export interface LogReport {
  /** Unix seconds; 0 without a log. */
  written: number;
  skipped: SkippedMod[];
  /** Save locations the game ignored because no loaded mod adds them; saving drops them. */
  lostLocations: string[];
  lostNpcs: string[];
  mods: ModProblems[];
  /** Seconds from the game starting to SMAPI being ready. */
  startupSecs: number | null;
  /** Mods the load paused on for 2 seconds or more, slowest first. */
  slow: { name: string; secs: number }[];
  /** Mods loading or editing the most game assets. */
  heavy: { name: string; assets: number }[];
}

export interface SkippedMod {
  name: string;
  version: string;
  kind: "incompatible" | "dependency" | "smapi" | "other";
  needs: string | null;
  reason: string;
}

export interface ModProblems {
  source: string;
  errors: number;
  warnings: number;
  entries: { level: "error" | "warn"; text: string; count: number }[];
}

/** How one save depends on a mod. */
export interface SaveUsage {
  saveId: string;
  farmer: string;
  farmName: string;
  /** Mentions of the mod's UniqueIDs: its items, buildings and data. */
  references: number;
  /** The first few of the mod's locations the save has. */
  locations: string[];
  locationCount: number;
}

/** A shared mod list next to what the library has of it. */
export interface ModListView {
  name: string;
  game: string | null;
  smapi: string | null;
  mods: ListedMod[];
  /** Mods enabled here that the list lacks; in co-op the other side won't have them. */
  extra: ListedMod[];
}

export interface ListedMod {
  uniqueId: string;
  name: string;
  version: string;
  updateKeys: string[];
  github: string | null;
  /** The installed version, if any. */
  installed: string | null;
}

/** The parts of the game's lists the calendar and fish helpers use. */
export interface GameCatalog {
  language: string;
  items: { id: string; name: string }[];
  villagers: {
    key: string;
    name: string;
    romance: boolean;
    birthSeason: string | null;
    birthDay: number;
    /** Unqualified item ids. */
    loves: string[];
  }[];
  festivals: { season: string; day: number; endDay: number; name: string }[];
  crops: Crop[];
  fishInfo: {
    id: string;
    name: string;
    seasons: string[];
    weather: "sunny" | "rainy" | "both" | "any";
    times: [number, number][];
    locations: string[];
    trap: boolean;
  }[];
}

/** What the last played save would lose if the game started now. */
export interface Preflight {
  saveId: string;
  farmer: string;
  farmName: string;
  missing: {
    package: string;
    name: string;
    references: number;
    locationCount: number;
    /** Off in the active profile; otherwise SMAPI skipped it last session. */
    disabled: boolean;
  }[];
  /** The profile the save was last played with, when it isn't the active one. */
  switchTo: string | null;
}

/** Content packs loading the same asset exclusively; the game drops all of them. */
export interface Conflict {
  target: string;
  packages: string[];
  /** Some patches apply only under conditions, so the packs may never meet. */
  conditional: boolean;
}

/** What changed between the installed version and the update, newest first. */
export interface Changelog {
  source: "github" | "nexus";
  versions: { version: string; lines: string[] }[];
}

/** The error a session ended on, and the mod its stack points at. */
export interface Crash {
  source: string;
  message: string;
  details: string[];
  suspect: string | null;
}

/** One install, update or removal in the library's history. */
export interface HistoryEntry {
  /** Unix milliseconds; also the entry's id. */
  at: number;
  kind: "installed" | "updated" | "removed";
  package: string;
  name: string;
  uniqueId: string;
  version: string;
  previous: string | null;
  undone: boolean;
  /** False once what undoing needs is gone: a cleaned-up backup or removed mod. */
  canUndo: boolean;
}

/** How much one kind of the launcher's kept data takes. */
export interface StorageUsage {
  kind: "modBackups" | "removed" | "saveBackups" | "downloads";
  bytes: number;
  files: number;
}

/** SMAPI's compatibility list on a mod that isn't simply fine. */
export interface CompatVerdict {
  status: "Broken" | "Abandoned" | "Obsolete" | "Workaround" | "Unofficial";
  summary: string | null;
}

/** One stack a save keeps somewhere, as of its last save. */
export interface StoredItem {
  /** Qualified id, such as "(O)337". */
  id: string;
  /** Internal English name. */
  name: string;
  /** The name in the game's language where its lists have one. */
  display: string;
  stack: number;
  quality: number;
  place: {
    location: string;
    kind: "chest" | "fridge" | "grabber" | "junimo" | "inventory";
    tile: [number, number] | null;
  };
  /** The location as the game names it. */
  locationName: string | null;
}

/** One place on Ginger Island that golden walnuts come from. */
export interface WalnutSource {
  /** The game's key, such as "Bush_IslandNorth_13_33" or "Darts". */
  key: string;
  kind: "bush" | "buried" | "flag" | "limited";
  /** Internal location name; "Island" for the whole island. */
  area: string;
  tile: [number, number] | null;
  have: number;
  /** Walnuts it gives at most; a flag counts as 1 even where the game gives a few. */
  max: number;
}

export interface Walnuts {
  found: number;
  total: number;
  sources: WalnutSource[];
}

/** Something that takes up tiles on a save's map. */
export interface MapFeature {
  x: number;
  y: number;
  w: number;
  h: number;
  kind:
    | "building"
    | "crop"
    | "deadCrop"
    | "dirt"
    | "tree"
    | "fruitTree"
    | "bush"
    | "grass"
    | "flooring"
    | "clump"
    | "debris"
    | "chest"
    | "sprinkler"
    | "scarecrow"
    | "machine"
    | "fence"
    | "object";
  id: string;
  name: string;
  /** Days until a crop can be picked; 0 when ready. */
  days: number | null;
  watered: boolean;
  /** A machine with something to collect. */
  ready: boolean;
}

export interface FarmMap {
  location: string;
  width: number;
  height: number;
  features: MapFeature[];
}

/** A farm on one in-game day. */
export interface FarmPoint {
  year: number;
  season: number;
  day: number;
  money: number;
  totalEarned: number;
  playTimeMs: number;
  /** The five skill levels summed. */
  skills: number;
  /** Known for days a perfection check ran on. */
  perfection: number | null;
}

/** One state of a save in a comparison. */
export interface SaveSide {
  year: number;
  season: number;
  day: number;
  money: number;
  totalEarned: number;
  playTimeMs: number;
  perfection: number | null;
}

export interface SaveChange {
  key: string;
  name: string;
  from: number;
  to: number;
}

/** What changed between two states of a save. */
export interface SaveDiff {
  from: SaveSide;
  to: SaveSide;
  skills: SaveChange[];
  /** Whole hearts. */
  friends: SaveChange[];
  firsts: { kind: "shipped" | "fish" | "cooked" | "crafted" | "donated" | "achievement"; names: string[] }[];
  /** Stored item totals, biggest change first. */
  items: SaveChange[];
}

/** A Nexus collection's latest revision and which of its mods the library has. */
export interface CollectionView {
  collection: {
    slug: string;
    name: string;
    author: string | null;
    revision: number;
    files: { modId: number; fileId: number; modName: string; version: string | null; optional: boolean }[];
  };
  /** Nexus mod ids found in the library's update keys. */
  installed: number[];
}
