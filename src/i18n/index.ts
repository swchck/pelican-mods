import { createI18n } from "vue-i18n";
import ru from "./ru";
import en from "./en";
import { isAppError } from "@/lib/api";

export const locales = ["ru", "en"] as const;
export type Locale = (typeof locales)[number];

export function systemLocale(): Locale {
  return navigator.language.toLowerCase().startsWith("ru") ? "ru" : "en";
}

export const i18n = createI18n<[typeof ru], Locale, false>({
  legacy: false,
  locale: systemLocale(),
  fallbackLocale: "en",
  messages: { ru, en },
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
