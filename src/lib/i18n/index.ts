// The screens' text in Rupert's language. Use `m.section.key` in markup and
// code; it follows the language as it changes. English (`en.ts`) is the
// master catalog: every other language must have the same keys (checked by
// `npm run check`). Messages from Rust are translated in Rust
// (`rupert_core::i18n`).

import { invoke } from "@tauri-apps/api/core";
import en, { type Messages } from "./en";
import es from "./es";
import { currentLang, setLang, type Lang } from "./locale.svelte";

export { currentLang, locale, num, plural, LANGS, type Lang } from "./locale.svelte";
export type { Messages };

const catalogs: Record<Lang, Messages> = { en, es };

/** The current language's catalog. */
export const m: Messages = new Proxy({} as Messages, {
  get: (_, key) => catalogs[currentLang()][key as keyof Messages],
});

/** Each language's name in itself, for the Language setting. */
export const languageNames: Record<Lang, string> = {
  en: "English",
  es: "Español (Rioplatense)",
};

export interface LanguageInfo {
  current: Lang;
  system: Lang;
  /** Phone only: the setting (`null`: automatic). */
  setting?: string | null;
}

/** Ask Rust which language Rupert speaks now, and use it. */
export async function loadLanguage(): Promise<LanguageInfo | null> {
  try {
    const info = await invoke<LanguageInfo>("language");
    setLang(info.current);
    return info;
  } catch {
    return null;
  }
}

export function useLanguage(lang: Lang): void {
  setLang(lang);
}
