import { ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";

export type WikiSource = "official" | "fandom" | "off";

/** Where item names link; set from the settings at startup and on change. */
export const wikiSource = ref<WikiSource>("official");

export function setWikiSource(value: string | null) {
  wikiSource.value = value === "fandom" || value === "off" ? value : "official";
}

const BASE = {
  official: "https://stardewvalleywiki.com/",
  fandom: "https://stardewvalley.fandom.com/wiki/",
};

/** The page for English title `title` on the chosen wiki, or null with links off. */
export function wikiUrl(title: string | null | undefined): string | null {
  if (!title || wikiSource.value === "off") return null;
  // both wikis name pages the MediaWiki way: spaces as underscores
  return BASE[wikiSource.value] + encodeURIComponent(title.replace(/ /g, "_"));
}

export function openWiki(title: string | null | undefined) {
  const url = wikiUrl(title);
  if (url) void openUrl(url);
}
