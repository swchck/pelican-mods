/** A run of plain text, or a link from `[text](url)`. */
export type Segment = { text: string; url?: string; nexusModId?: number };

const LINK = /\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g;
const NEXUS_MOD = /^https?:\/\/(?:www\.)?nexusmods\.com\/stardewvalley\/mods\/(\d+)/i;

/**
 * Splits the Markdown-ish text smapi.io uses in compatibility notes into text and
 * links. Only http(s) links are recognized; everything else stays literal text.
 */
export function splitLinks(text: string): Segment[] {
  const out: Segment[] = [];
  let last = 0;
  for (const m of text.matchAll(LINK)) {
    if (m.index > last) out.push({ text: text.slice(last, m.index) });
    const nexus = NEXUS_MOD.exec(m[2]);
    out.push({ text: m[1], url: m[2], ...(nexus ? { nexusModId: Number(nexus[1]) } : {}) });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ text: text.slice(last) });
  return out;
}
