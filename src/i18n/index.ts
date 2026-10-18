import { createI18n } from "vue-i18n";
import ru from "./ru";
import en from "./en";
import { isAppError } from "@/lib/api";

export const locales = ["ru", "en"] as const;
export type Locale = (typeof locales)[number];

export function systemLocale(): Locale {
  return navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
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
  messages: { ru, en },
  pluralRules: { ru: russianPlural },
});

export function setLocale(locale: string | null) {
  const next = locales.find((l) => l === locale) ?? systemLocale();
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
