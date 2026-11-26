// What a tag edit would change, in words: shared by the tag manager and
// Add tags… / Remove tags… for several documents (#102).
import type { TagEditPreview } from "#lib/api.ts";
import { m } from "#lib/i18n/index.ts";

export function describeTagEdit(p: TagEditPreview): string {
  const parts = [
    p.inFile && m.tags.inPdfs(p.inFile),
    p.sidecar && m.tags.inSidecars(p.sidecar),
    p.pending && m.tags.pending(p.pending),
    p.download && m.tags.toDownload(p.download),
  ].filter(Boolean);
  return m.tags.documents(p.documents, parts.join(", "));
}
