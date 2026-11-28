// What a saved search (#110) finds, in words; shared by the window and the
// phone (#162).

import type { SavedSearch } from "#lib/api.ts";
import { m } from "#lib/i18n/index.ts";

/** "“boat”, tagged Taxes and 2025, dated this year". `day` formats a
 * YYYY-MM-DD date; `sourceName` names a source by id. */
export function findsText(s: SavedSearch, day: (iso: string) => string, sourceName: (id: string) => string): string {
  const t = m.savedSearches;
  const parts: string[] = [];
  if (s.text.trim()) parts.push(`“${s.text.trim()}”`);
  if (s.tags.length) parts.push(s.tags.length > 1 && s.tagMode === "all" ? t.taggedAll(s.tags) : t.taggedAny(s.tags));
  if (s.favorites) parts.push(t.favorites);
  if (s.date?.kind === "thisYear") parts.push(t.thisYear);
  else if (s.date?.kind === "lastYear") parts.push(t.lastYear);
  else if (s.date?.kind === "range") parts.push(t.between(day(s.date.from) || "…", day(s.date.to) || "…"));
  if (s.kind && s.kind in m.kinds) parts.push(m.kinds[s.kind as keyof typeof m.kinds]);
  if (s.from) parts.push(t.from(s.from));
  if (s.due === "upcoming") parts.push(m.filters.dueUpcoming.toLowerCase());
  else if (s.due === "past") parts.push(m.filters.duePast.toLowerCase());
  if (s.scope) {
    parts.push(
      s.scope === "folder" && s.folder
        ? m.filters.inFolder(s.folder.slice(s.folder.lastIndexOf("/") + 1))
        : m.filters.inSource(sourceName(s.sourceId)),
    );
  }
  return parts.length ? parts.join(", ") : t.everything;
}
