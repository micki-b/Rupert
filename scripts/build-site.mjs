#!/usr/bin/env node
// Builds rupertlibrarian.com into target/site/ and packs it as
// target/rupertlibrarian-site.zip, ready for Bluehost's File Manager
// (upload into the site's folder, then Extract). macOS only: images are
// resized with `sips`.
//
// - Pages come in every language the app speaks: English at /, Spanish at
//   /es/ and German at /de/. site/index.html and site/support.html are
//   templates: {{t.key}} is that language's text from site/i18n/<lang>.json,
//   {{base}} its path prefix ("", "/es", "/de"), {{img}} its screenshots
//   (site/img/<lang>/). {{head}}, {{header}} and {{footer}} come from
//   site/_head.html, _header.html and _footer.html; the header has a
//   language switch, the head tells search engines about the other
//   languages, and the English pages send a Spanish or German browser to
//   its language once (a choice made in the switch wins).
// - privacy/: PRIVACY.md rendered into site/privacy.template.html, so the
//   site always says what the repo says (English only).
// - img/: the app icon, the owls, and each language's screenshots of the
//   made-up sample library, light and dark.
// - {{version}} is the app's version from package.json (download links).
//
// The build stops if a language misses a text the others have, if anything
// is left unfilled, or if a page links to a page or image that isn't there.
//
// Usage: npm run site
//        npm run site -- --upload   (also uploads, over SSH)
//
// Uploading needs RUPERT_SITE_DEPLOY=user@host:folder (kept out of the repo,
// e.g. in your shell profile) and an SSH key the host accepts
// (RUPERT_SITE_KEY, default ~/.ssh/rupertlibrarian_bluehost). It adds and
// replaces files; nothing on the server is deleted.

import { execFileSync, execSync } from "node:child_process";
import { createHash } from "node:crypto";
import { homedir } from "node:os";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { marked } from "marked";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const site = join(root, "site");
const out = join(root, "target", "site");
const zip = join(root, "target", "rupertlibrarian-site.zip");

const read = (p) => readFileSync(join(site, p), "utf8");
const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const ORIGIN = "https://rupertlibrarian.com";
const LANGS = ["en", "es", "de"];
const base = (lang) => (lang === "en" ? "" : `/${lang}`);
const strings = Object.fromEntries(LANGS.map((l) => [l, JSON.parse(read(`i18n/${l}.json`))]));
const problems = [];

// MARK: - Checks: every language has every text

for (const l of LANGS.slice(1)) {
  const want = Object.keys(strings.en);
  const have = Object.keys(strings[l]);
  for (const k of want) if (!have.includes(k)) problems.push(`${l}.json: missing "${k}"`);
  for (const k of have) if (!want.includes(k)) problems.push(`${l}.json: "${k}" isn't in en.json`);
}

// MARK: - Pages

/** The language switch: each language's version of `path` (a path under
 * the language prefix, e.g. "/support/"); `only` for pages in one language. */
function langLinks(lang, path, only) {
  return LANGS.map((l) => {
    const href = only ? (l === "en" ? path : `${base(l)}/`) : `${base(l)}${path}`;
    const current = l === lang ? ' aria-current="true"' : "";
    return `<a href="${href}" hreflang="${l}" lang="${l}" title="${strings[l]["lang.name"]}" data-lang-choice="${l}"${current}>${l.toUpperCase()}</a>`;
  }).join("");
}

function alternates(path) {
  const links = LANGS.map((l) => `<link rel="alternate" hreflang="${l}" href="${ORIGIN}${base(l)}${path}" />`);
  links.push(`<link rel="alternate" hreflang="x-default" href="${ORIGIN}${path}" />`);
  return links.join("\n    ");
}

// English pages send a Spanish or German browser to its language, once:
// a language picked in the switch is remembered and always wins.
const redirect = `<script>
      (function () {
        var chosen;
        try { chosen = localStorage.getItem("rupert-lang"); } catch (e) {}
        var prefs = chosen ? [chosen] : (navigator.languages || [navigator.language || ""]);
        for (var i = 0; i < prefs.length; i++) {
          var l = String(prefs[i]).slice(0, 2).toLowerCase();
          if (l === "en") return;
          if (l === "es" || l === "de") { location.replace("/" + l + location.pathname + location.hash); return; }
        }
      })();
    </script>`;
const remember = `<script>
      document.addEventListener("click", function (e) {
        var a = e.target.closest && e.target.closest("[data-lang-choice]");
        if (a) try { localStorage.setItem("rupert-lang", a.dataset.langChoice); } catch (err) {}
      });
    </script>`;

// Stylesheet and script addresses change with their contents, so browsers
// never pair a new page with an old, cached stylesheet (the server lets them
// keep files for a day).
const stamp = (file) => createHash("sha256").update(read(file)).digest("hex").slice(0, 10);
const assets = { "/style.css": `/style.css?v=${stamp("style.css")}`, "/home.js": `/home.js?v=${stamp("home.js")}` };

/** Fills a template for one language. */
function render(html, { lang, path, current, title, description, only = false }) {
  const t = strings[lang];
  const header = read("_header.html").replace(`data-page="${current}"`, `data-page="${current}" aria-current="page"`);
  const head = read("_head.html")
    .replace("{{alternates}}", only ? "" : alternates(path))
    .replace("{{langredirect}}", (lang === "en" && !only ? redirect + "\n    " : "") + remember);
  let text = html
    .replace("{{head}}", head.trim())
    .replace("{{header}}", header.trim())
    .replace("{{footer}}", read("_footer.html").trim())
    .replace("{{title}}", title ?? "")
    .replace("{{description}}", description ?? "")
    .replace("{{langlinks}}", langLinks(lang, path, only));
  text = text.replace(/\{\{t\.([\w.]+)\}\}/g, (m, key) => {
    if (!(key in t)) problems.push(`${lang}${path}: no text "${key}"`);
    return t[key] ?? m;
  });
  text = text
    .replaceAll("{{base}}", base(lang))
    .replaceAll("{{img}}", `/img/${lang}`)
    .replaceAll("{{lang}}", lang)
    .replaceAll("{{version}}", version);
  for (const [plain, stamped] of Object.entries(assets)) text = text.replaceAll(`"${plain}"`, `"${stamped}"`);
  const left = text.match(/\{\{[^}]*\}\}/g);
  if (left) problems.push(`${lang}${path}: unfilled ${[...new Set(left)].join(", ")}`);
  return text;
}

function write(rel, text) {
  const dest = join(out, rel);
  mkdirSync(dirname(dest), { recursive: true });
  writeFileSync(dest, text);
}

/** Copies an image, scaled so its longest side is at most `max` pixels. */
function image(src, rel, max) {
  const dest = join(out, rel);
  mkdirSync(dirname(dest), { recursive: true });
  if (max) execFileSync("sips", ["-Z", String(max), join(root, src), "--out", dest], { stdio: "ignore" });
  else copyFileSync(join(root, src), dest);
}

rmSync(out, { recursive: true, force: true });
rmSync(zip, { force: true });

for (const lang of LANGS) {
  const t = strings[lang];
  const b = base(lang).slice(1);
  write(join(b, "index.html"), render(read("index.html"), {
    lang, path: "/", current: "home", title: t["home.title"], description: t["home.description"],
  }));
  write(join(b, "support", "index.html"), render(read("support.html"), {
    lang, path: "/support/", current: "support", title: t["support.title"], description: t["support.description"],
  }));
}
const policy = marked.parse(readFileSync(join(root, "PRIVACY.md"), "utf8"));
write("privacy/index.html", render(read("privacy.template.html").replace("{{privacy}}", policy), {
  lang: "en", path: "/privacy/", current: "privacy", title: "Rupert privacy policy",
  description: "Rupert collects nothing about you. What happens to your information when you use Rupert.", only: true,
}));
write("404.html", render(read("404.html"), {
  lang: "en", path: "/", current: "none", title: "Page not found · Rupert", only: true,
}));
write("style.css", read("style.css"));
write("home.js", read("home.js"));
write(".htaccess", read(".htaccess"));

// MARK: - Images

image("src-tauri/icons/icon.png", "img/icon.png", 0);
image("src-tauri/icons/icon.png", "apple-touch-icon.png", 180);
image("src-tauri/icons/32x32.png", "favicon.png", 0);
for (const entry of readdirSync(join(site, "img"), { recursive: true })) {
  if (entry.endsWith(".png")) image(join("site", "img", entry), join("img", entry), 0);
}

// MARK: - Checks: every link and picture on the site exists

for (const file of readdirSync(out, { recursive: true }).filter((f) => f.endsWith(".html"))) {
  const html = readFileSync(join(out, file), "utf8");
  for (const [, ref] of html.matchAll(/(?:href|src|srcset)="(\/[^"#?]*)/g)) {
    const path = ref.endsWith("/") ? `${ref}index.html` : ref;
    if (!existsSync(join(out, path))) problems.push(`${file}: links to ${ref}, which isn't there`);
  }
}
if (problems.length) {
  console.error(`The site has problems:\n  ${problems.join("\n  ")}`);
  process.exit(1);
}

// One zip for uploading.
execFileSync("zip", ["-q", "-r", "-X", zip, ".", "-x", ".DS_Store", "*/.DS_Store"], { cwd: out });
console.log(`Built ${out}\nUpload ${zip}`);

// MARK: - Upload

if (process.argv.includes("--upload")) {
  const target = process.env.RUPERT_SITE_DEPLOY;
  const match = target?.match(/^([^@\s]+@[^:\s]+):(.+)$/);
  if (!match) {
    console.error("Set RUPERT_SITE_DEPLOY=user@host:folder to upload.");
    process.exit(1);
  }
  const [, host, folder] = match;
  const key = process.env.RUPERT_SITE_KEY ?? join(homedir(), ".ssh", "rupertlibrarian_bluehost");
  const ssh = `ssh -i '${key}' -o IdentitiesOnly=yes -o BatchMode=yes ${host}`;
  // Files 644 and folders 755 for the web server; the site's own folder
  // keeps its mode (tar would otherwise change it).
  if (/['"\\$`\s]/.test(folder)) {
    console.error("The folder in RUPERT_SITE_DEPLOY can't contain quotes, spaces or $.");
    process.exit(1);
  }
  const remote = [
    `cd ${folder}`,
    `mode=$(stat -c %a .)`,
    `tar -xf - --no-same-owner 2>/dev/null`,
    `find . -path ./.well-known -prune -o -path ./cgi-bin -prune -o -type d -exec chmod 755 {} +`,
    `find . -path ./.well-known -prune -o -path ./cgi-bin -prune -o -type f -exec chmod 644 {} +`,
    `chmod "$mode" .`,
  ].join(" && ");
  // Single quotes: the command runs on the server, not here.
  execSync(`COPYFILE_DISABLE=1 tar --no-xattrs -C '${out}' -cf - . | ${ssh} '${remote}'`, { stdio: "inherit" });
  console.log(`Uploaded to ${host}`);
}
