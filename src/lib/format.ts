export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export function formatDate(ms: number | null): string {
  return ms == null ? "" : dateFormat.format(new Date(ms));
}

export const kindLabel: Record<string, string> = {
  pdf: "PDF",
  image: "Image",
  office: "Office",
  text: "Text",
  email: "Email",
  other: "File",
};
