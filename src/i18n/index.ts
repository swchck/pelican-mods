import { createI18n } from "vue-i18n";
import ru from "./ru";
import en from "./en";
import de from "./de";
import es from "./es";
import fr from "./fr";
import pt from "./pt";
import it from "./it";
import ja from "./ja";
import ko from "./ko";
import zh from "./zh";
import tr from "./tr";
import hu from "./hu";
import { isAppError } from "@/lib/api";

// the game's own languages, so following the game's language always finds a match
export const locales = ["ru", "en", "de", "es", "fr", "pt", "it", "ja", "ko", "zh", "tr", "hu"] as const;
export type Locale = (typeof locales)[number];

export function systemLocale(): Locale {
  const lang = navigator.language.toLowerCase().split("-")[0];
  return locales.find((l) => l === lang) ?? "en";
}

// "1 мод, 2 мода, 5 модов, 21 мод": vue-i18n only knows the English two-form rule
function russianPlural(choice: number): number {
  const n = Math.abs(choice) % 100;
  const last = n % 10;
  if (n > 10 && n < 20) return 2;
  if (last === 1) return 0;
  if (last > 1 && last < 5) return 1;
  return 2;
}

export const i18n = createI18n<[typeof ru], Locale, false>({
  legacy: false,
  locale: systemLocale(),
  fallbackLocale: "en",
  messages: { ru, en, de, es, fr, pt, it, ja, ko, zh, tr, hu },
  pluralRules: { ru: russianPlural },
});

let gameLanguage: string | null = null;

/** Remembers the game's language; with no language picked, the app follows it. */
export function setGameLanguage(code: string | null) {
  gameLanguage = code;
}

/** Applies `locale`, or with none picked the game's language, or else the system's. */
export function setLocale(locale: string | null) {
  const next = locales.find((l) => l === locale) ?? locales.find((l) => l === gameLanguage) ?? systemLocale();
  i18n.global.locale.value = next;
  document.documentElement.lang = next;
}

/** Turns a rejected command into a sentence in the current UI language. */
export function errorText(e: unknown): string {
  const { t, te } = i18n.global;
  if (isAppError(e)) {
    const key = `errors.${e.kind}`;
    return te(key) ? t(key, { detail: e.detail ?? "" }) : e.message;
  }
  return t("errors.unknown", { detail: String(e) });
}
