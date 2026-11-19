// Markdown documents in Rupert's reader, gallery and tiles: rendered with
// `marked`, then cleaned with DOMPurify, so a file can't run scripts or
// carry event handlers into the window. Pictures with relative paths load
// through the `thumb:` scheme (only files inside the source); links are
// handled by the view (see `MarkdownView.svelte`).
import DOMPurify from "dompurify";
import { marked } from "marked";
import type { Entry } from "$lib/api";
import { thumbUrl } from "$lib/Thumb.svelte";

/** Whether an entry is a Markdown file. */
export function isMarkdown(e: Pick<Entry, "name" | "isDir">): boolean {
  return !e.isDir && /\.(md|markdown)$/i.test(e.name);
}

/** A link or picture path relative to the document, as a path in the same
 * source; null for web addresses, anchors, absolute paths, or a path that
 * leaves the source. */
export function resolveRel(docRel: string, href: string): string | null {
  if (!href || /^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("#") || href.startsWith("/")) return null;
  let path: string;
  try {
    path = decodeURIComponent(href.split("#")[0].split("?")[0]);
  } catch {
    return null;
  }
  const parts = docRel.split("/").slice(0, -1);
  for (const seg of path.split("/")) {
    if (seg === "" || seg === ".") continue;
    if (seg === "..") {
      if (!parts.length) return null;
      parts.pop();
    } else parts.push(seg);
  }
  return parts.join("/") || null;
}

/** The document as safe HTML for the view. */
export function renderMarkdown(text: string, sourceId: string, docRel: string): string {
  const raw = marked.parse(text, { async: false, gfm: true }) as string;
  // No scripts or handlers (DOMPurify's default), and nothing that could
  // restyle or cover Rupert's own window, or send a form.
  const clean = DOMPurify.sanitize(raw, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: ["style", "form", "button", "textarea", "select", "iframe", "object", "embed"],
    FORBID_ATTR: ["style", "class"],
  });
  const tpl = document.createElement("template");
  tpl.innerHTML = clean;
  for (const img of tpl.content.querySelectorAll("img")) {
    const rel = resolveRel(docRel, img.getAttribute("src") ?? "");
    if (rel) img.setAttribute("src", thumbUrl(sourceId, { relPath: rel, modifiedMs: 0 } as Entry, 0, 1600));
    img.setAttribute("loading", "lazy");
  }
  for (const a of tpl.content.querySelectorAll("a")) a.removeAttribute("target");
  return tpl.innerHTML;
}
