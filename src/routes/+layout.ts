// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import { loadLanguage } from "$lib/i18n";

export const ssr = false;

// Every window (main, help, phone) starts in Rupert's language.
export async function load() {
  await loadLanguage();
  return {};
}
