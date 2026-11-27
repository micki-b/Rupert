import { locale, m } from "#lib/i18n/index.ts";
import type { FileKind } from "#lib/api.ts";

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const shown = value < 10 ? Math.round(value * 10) / 10 : Math.round(value);
  return `${shown.toLocaleString(locale(), { minimumFractionDigits: value < 10 ? 1 : 0 })} ${units[unit]}`;
}

export function formatDate(ms: number | null): string {
  return ms == null ? "" : new Date(ms).toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" });
}

/** The day only ("Nov 24, 2026"), for narrow columns. */
export function formatDay(ms: number | null): string {
  return ms == null ? "" : new Date(ms).toLocaleDateString(locale(), { dateStyle: "medium" });
}

/** "PDF", "Image", … for a file kind (or "Folder"). */
export function kindLabel(kind: FileKind | "folder" | null): string {
  return m.kinds[kind ?? "other"];
}

/** An amount (#108: "1240.50", with its currency code) in the reader's
 * style: "$1,240.50", "1.240,50 €"; without a currency, just the number. */
export function formatMoney(amount: string | null | undefined, currency?: string | null): string {
  if (!amount) return "";
  const n = Number(amount);
  if (!Number.isFinite(n)) return amount;
  const decimals = amount.includes(".") ? 2 : 0;
  try {
    return currency
      ? n.toLocaleString(locale(), { style: "currency", currency, minimumFractionDigits: decimals })
      : n.toLocaleString(locale(), { minimumFractionDigits: decimals, maximumFractionDigits: 2 });
  } catch {
    // An unknown currency code: the number and the code.
    return `${n.toLocaleString(locale(), { minimumFractionDigits: decimals })} ${currency ?? ""}`.trim();
  }
}

/** Totals per currency (#108), joined: "$1,240.50 · 89,00 €". */
export function formatTotals(totals: { amount: string; currency: string | null }[]): string {
  return totals.map((t) => formatMoney(t.amount, t.currency)).join(" · ");
}

/** A day written YYYY-MM-DD, in the reader's style ("Dec 1, 2026"). */
export function formatIsoDay(iso: string | null | undefined): string {
  if (!iso) return "";
  const [y, mo, d] = iso.split("-").map(Number);
  return new Date(y, mo - 1, d).toLocaleDateString(locale(), { dateStyle: "medium" });
}

/** Amounts added up per currency in the browser (for a selection or a
 * folder; searches ask the index). Exact: in cents. */
export function addUp(docs: { amount?: string | null; currency?: string | null }[]): { amount: string; currency: string | null; count: number }[] {
  const sums = new Map<string, { cents: bigint; count: number }>();
  for (const d of docs) {
    if (!d.amount || !/^-?\d+(\.\d{1,2})?$/.test(d.amount)) continue;
    const neg = d.amount.startsWith("-");
    const [int, dec = ""] = d.amount.replace("-", "").split(".");
    const cents = BigInt(int) * 100n + BigInt(dec.padEnd(2, "0"));
    const key = d.currency ?? "";
    const e = sums.get(key) ?? { cents: 0n, count: 0 };
    e.cents += neg ? -cents : cents;
    e.count++;
    sums.set(key, e);
  }
  return [...sums.entries()]
    .sort((a, b) => b[1].count - a[1].count)
    .map(([c, e]) => {
      const abs = e.cents < 0n ? -e.cents : e.cents;
      return {
        currency: c || null,
        amount: `${e.cents < 0n ? "-" : ""}${abs / 100n}.${String(abs % 100n).padStart(2, "0")}`,
        count: e.count,
      };
    });
}

/** An amount for an input field (#108): grouped in the reader's style
 * ("21,203", "21.203,50"), two decimals where it has any, no currency. */
export function formatAmountInput(amount: string | null | undefined): string {
  if (!amount) return "";
  const n = Number(amount);
  if (!Number.isFinite(n)) return amount;
  const decimals = amount.includes(".") ? 2 : 0;
  return n.toLocaleString(locale(), { minimumFractionDigits: decimals, maximumFractionDigits: 2 });
}

/** What someone typed as an amount, as Rupert keeps it ("21,203" or
 * "21.203" → "21203", "1.240,5" → "1240.5"); null if it isn't one. The
 * same rule as the core's `parse_amount`: a separator followed by one or
 * two digits at the end is the decimal one, the others group thousands. */
export function parseAmount(text: string): string | null {
  const t = text.trim().replace(/[\s  '€$£¥₹]/g, "").replace(/[A-Za-z]{1,3}$/, "").replace(/^[A-Za-z]{1,3}/, "");
  const neg = /^[-−(]/.test(t);
  const body = t.replace(/^[-−(]|\)$/g, "");
  if (!body || !/^[\d.,]+$/.test(body) || !/\d/.test(body)) return null;
  const i = Math.max(body.lastIndexOf("."), body.lastIndexOf(","));
  const tail = i >= 0 ? body.length - i - 1 : 0;
  const [int, dec] = i >= 0 && tail >= 1 && tail <= 2 ? [body.slice(0, i), body.slice(i + 1)] : [body, ""];
  const digits = int.replace(/\D/g, "").replace(/^0+(?=\d)/, "") || "0";
  return `${neg ? "-" : ""}${digits}${dec ? "." + dec : ""}`;
}

/** Regroups a typed amount ("21203" → "21,203"), or leaves it as typed if
 * it isn't one (saving then says so). */
export function tidyAmountInput(text: string): string {
  const a = parseAmount(text);
  return a === null ? text : formatAmountInput(a);
}

/** A count beside a sidebar item, a tag or on the Dock icon: in full up to
 * 999, then 1.2K … 9.9K, 12K … 999K, 1.2M (one decimal below ten, rounded
 * down so it never says more than there is; the language's decimal mark). */
export function shortCount(n: number): string {
  if (n < 1000) return String(n);
  const [value, unit] = n < 1_000_000 ? [n / 1000, "K"] : [n / 1_000_000, "M"];
  const shown = value < 10 ? Math.floor(value * 10) / 10 : Math.floor(value);
  return shown.toLocaleString(locale(), { maximumFractionDigits: 1 }) + unit;
}
