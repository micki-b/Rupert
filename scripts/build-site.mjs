#!/usr/bin/env node
// Builds rupertlibrarian.com into target/site/ and packs it as
// target/rupertlibrarian-site.zip, ready for Bluehost's File Manager
// (upload into the site's folder, then Extract). macOS only: images are
// resized with `sips`.
//
// - site/*.html: pages; {{header}} and {{footer}} come from site/_header.html
//   and site/_footer.html.
// - privacy/: PRIVACY.md rendered into site/privacy.template.html, so the
//   site always says what the repo says.
// - img/: the app icon, and site/img/ (screenshots of the made-up sample
//   library, light and dark, and the owl in each language's outfit).
// - {{version}} is the app's version from package.json (for download links).
//
// Usage: npm run site
//        npm run site -- --upload   (also uploads, over SSH)
//
// Uploading needs RUPERT_SITE_DEPLOY=user@host:folder (kept out of the repo,
// e.g. in your shell profile) and an SSH key the host accepts
// (RUPERT_SITE_KEY, default ~/.ssh/rupertlibrarian_bluehost). It adds and
// replaces files; nothing on the server is deleted.

import { execFileSync, execSync } from "node:child_process";
import { homedir } from "node:os";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { marked } from "marked";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const site = join(root, "site");
const out = join(root, "target", "site");
const zip = join(root, "target", "rupertlibrarian-site.zip");

const read = (p) => readFileSync(join(site, p), "utf8");
const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const header = read("_header.html");
const footer = read("_footer.html");

/** A page with the shared header (its own nav link marked) and footer. */
function page(html, current) {
  const nav = header.replace(`data-page="${current}"`, `data-page="${current}" aria-current="page"`);
  return html
    .replace("{{header}}", nav.trim())
    .replace("{{footer}}", footer.trim())
    .replaceAll("{{version}}", version);
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

// Pages
write("index.html", page(read("index.html"), "home"));
write("support/index.html", page(read("support/index.html"), "support"));
const policy = marked.parse(readFileSync(join(root, "PRIVACY.md"), "utf8"));
write("privacy/index.html", page(read("privacy.template.html").replace("{{privacy}}", policy), "privacy"));
write("style.css", read("style.css"));
write("home.js", read("home.js"));
write("404.html", page(read("404.html"), "none"));
write(".htaccess", read(".htaccess"));

// Images
image("src-tauri/icons/icon.png", "img/icon.png", 0);
image("src-tauri/icons/icon.png", "apple-touch-icon.png", 180);
image("src-tauri/icons/32x32.png", "favicon.png", 0);
for (const name of readdirSync(join(site, "img"))) {
  if (name.endsWith(".png")) image(join("site", "img", name), join("img", name), 0);
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
