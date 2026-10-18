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
  log: (cb: (l: LogLine) => void): Promise<UnlistenFn> => listen<LogLine>("game:log", (e) => cb(e.payload)),
  state: (cb: (s: RunState) => void): Promise<UnlistenFn> => listen<RunState>("game:state", (e) => cb(e.payload)),
  smapiProgress: (cb: (p: SmapiProgress) => void): Promise<UnlistenFn> =>
    listen<SmapiProgress>("smapi:progress", (e) => cb(e.payload)),
};
