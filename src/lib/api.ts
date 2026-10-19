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
  /** "official", "fandom" or "off"; null is the official wiki. */
  wiki: string | null;
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
  setWiki: (wiki: string) => invoke<Settings>("settings_set_wiki", { wiki }),
  wikiTitles: (ids: string[]) => invoke<Record<string, string>>("wiki_titles", { ids }),
  detectGames: () => invoke<GameInstall[]>("game_detect"),
  currentGame: () => invoke<GameInstall | null>("game_current"),
  setGamePath: (path: string) => invoke<GameInstall>("game_set_path", { path }),
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
}

export interface PackageView {
  id: string;
  path: string;
  system: boolean;
  enabled: boolean;
  mods: ModView[];
  broken: { relPath: string; error: string }[];
  backupVersion: string | null;
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
}

export interface Profiles {
  active: string;
  profiles: Profile[];
}

export interface LibraryView {
  packages: PackageView[];
  profiles: Profiles;
  importable: string[];
}

export const library = {
  get: () => invoke<LibraryView>("library_get"),
  install: (paths: string[]) => invoke<LibraryView>("library_install", { paths }),
  import: (names: string[]) => invoke<LibraryView>("library_import", { names }),
  remove: (id: string) => invoke<LibraryView>("library_remove", { id }),
  toggle: (id: string, enabled: boolean) => invoke<LibraryView>("library_toggle", { id, enabled }),
  open: (path: string) => invoke<void>("library_open", { path }),
  createProfile: (name: string, copyFrom: string | null) => invoke<LibraryView>("profile_create", { name, copyFrom }),
  renameProfile: (id: string, name: string) => invoke<LibraryView>("profile_rename", { id, name }),
  deleteProfile: (id: string) => invoke<LibraryView>("profile_delete", { id }),
  activateProfile: (id: string) => invoke<LibraryView>("profile_activate", { id }),
  checkUpdates: (force: boolean) => invoke<UpdateReport>("updates_check", { force }),
  update: (id: string) => invoke<UpdateOutcome>("mod_update", { id }),
  rollback: (uniqueId: string) => invoke<LibraryView>("mod_rollback", { uniqueId }),
  setGithub: (uniqueId: string, repo: string | null) => invoke<LibraryView>("mod_set_github", { uniqueId, repo }),
  installDependency: (uniqueId: string) => invoke<UpdateOutcome>("mod_install_dependency", { uniqueId }),
  saveUsage: (id: string) => invoke<SaveUsage[]>("mod_save_usage", { id }),
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
}

export interface CheckupGroup {
  id: string;
  title: string | null;
  /** Heading above the group, such as a Community Center room. */
  category: string | null;
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
  perfection: (id: string) => invoke<Perfection>("saves_perfection", { id }),
  progress: (id: string) => invoke<SaveProgress>("saves_progress", { id }),
  snapshots: (id: string) => invoke<Snapshot[]>("saves_snapshots", { id }),
  backup: (id: string) => invoke<Snapshot>("saves_backup", { id }),
  restore: (file: string) => invoke<string>("saves_restore", { file }),
  export: (id: string, dest: string) => invoke<void>("saves_export", { id, dest }),
  import: (file: string) => invoke<string>("saves_import", { file }),
  delete: (id: string) => invoke<void>("saves_delete", { id }),
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
  apply: (uniqueId: string, values: unknown, notice: string | null) =>
    invoke<Applied>("companion_apply", { uniqueId, values, notice }),
  onStatus: (cb: (s: CompanionStatus) => void): Promise<UnlistenFn> =>
    listen<CompanionStatus>("companion:status", (e) => cb(e.payload)),
  onGmcm: (cb: (ids: string[]) => void): Promise<UnlistenFn> =>
    listen<string[]>("companion:gmcm", (e) => cb(e.payload)),
  onCatalog: (cb: () => void): Promise<UnlistenFn> => listen("companion:catalog", () => cb()),
  forecast: () => invoke<Forecast>("companion_forecast"),
  items: () => invoke<import("./grange").GrangeItem[]>("companion_items"),
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
