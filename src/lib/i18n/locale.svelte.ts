// Rupert's current language, and number and date formatting in it. Kept
// apart from the catalogs so they can use `num` and `plural` themselves.

export type Lang = "en" | "es";

export const LANGS: Lang[] = ["en", "es"];

const state = $state({ lang: "en" as Lang });

export function currentLang(): Lang {
  return state.lang;
}

export function setLang(lang: Lang): void {
  if (LANGS.includes(lang)) {
    state.lang = lang;
    document.documentElement.lang = lang;
  }
}

/** The locale for numbers and dates ("es-AR" for Rupert's Spanish). */
export function locale(): string {
  return state.lang === "es" ? "es-AR" : "en-US";
}

/** 1234 → "1,234" (English) or "1.234" (Spanish). */
export function num(n: number): string {
  return n.toLocaleString(locale());
}

/** `one` or `other` for `n`, by the language's plural rules. */
export function plural(n: number, one: string, other: string): string {
  return new Intl.PluralRules(locale()).select(n) === "one" ? one : other;
}
