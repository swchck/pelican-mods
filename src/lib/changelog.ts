import { compareVersions } from "./version";

export interface Release {
  version: string;
  date: string | null;
  /** Markdown body of the section, without its heading. */
  body: string;
}

/** Splits a changelog into "## <version> — <date>" sections, newest first as written. */
export function parseChangelog(text: string): Release[] {
  return text
    .split(/^## /m)
    .slice(1)
    .map((chunk) => {
      const nl = chunk.indexOf("\n");
      const head = (nl === -1 ? chunk : chunk.slice(0, nl)).trim();
      const [version, date] = head.split(/\s+[—–-]\s+/);
      return {
        version: version.replace(/^\[|\]$/g, "").trim(),
        date: date?.trim() ?? null,
        body: chunk.slice(nl + 1).trim(),
      };
    })
    .filter((r) => r.version);
}

/** Releases newer than `seen` and not newer than `current`. */
export function releasesSince(all: Release[], seen: string | null, current: string): Release[] {
  return all.filter(
    (r) => compareVersions(r.version, current) <= 0 && (seen === null || compareVersions(r.version, seen) > 0),
  );
}

const ESC: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };
const escape = (s: string) => s.replace(/[&<>"']/g, (c) => ESC[c]);

function inline(escaped: string): string {
  return escaped
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^)\s"]+)\)/g, '<a href="$2" data-external>$1</a>');
}

/**
 * Renders the small Markdown subset changelogs use (### headings, "- " lists, bold,
 * code, http links) to HTML safe for v-html: text is escaped before any tag is added.
 */
export function renderMarkdown(md: string): string {
  const out: string[] = [];
  let list = false;
  for (const raw of md.split("\n")) {
    const line = raw.trimEnd();
    const item = /^\s*[-*] (.*)$/.exec(line);
    if (item) {
      if (!list) out.push("<ul>");
      list = true;
      out.push(`<li>${inline(escape(item[1]))}</li>`);
      continue;
    }
    if (list) out.push("</ul>");
    list = false;
    const h = /^(#{1,4}) (.*)$/.exec(line);
    if (h) out.push(`<h4>${inline(escape(h[2]))}</h4>`);
    else if (line.trim()) out.push(`<p>${inline(escape(line))}</p>`);
  }
  if (list) out.push("</ul>");
  return out.join("");
}
