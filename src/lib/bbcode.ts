/**
 * Nexus mod descriptions are BBCode with literal `<br />` line breaks. This renders the
 * subset Nexus uses into HTML that is safe for `v-html`: the whole text is escaped
 * first, so only tags produced here can appear in the output, and every URL that ends
 * up in an attribute must be http(s).
 */

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

function escape(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ESCAPES[c]);
}

// works on already-escaped text, so `&amp;` in a query string is undone before checking
function safeUrl(escaped: string): string | null {
  const raw = escaped
    .replace(/&amp;/g, "&")
    .replace(/&quot;|&#39;/g, "")
    .trim();
  try {
    const u = new URL(raw);
    if (u.protocol !== "https:" && u.protocol !== "http:") return null;
    // the rules run again over their own output: a "[" left in an attribute lets a later
    // pass build a tag inside it, and that tag's quotes break out of the attribute
    return escape(u.href).replace(/\[/g, "%5B").replace(/\]/g, "%5D");
  } catch {
    return null;
  }
}

function safeColor(c: string): string | null {
  return /^(#[0-9a-f]{3}|#[0-9a-f]{6}|[a-z]{3,20})$/i.test(c) ? c : null;
}

const SIZE_CLASS = ["", "text-xs", "text-sm", "text-base", "text-lg", "text-xl", "text-2xl", "text-3xl"];

type Rule = [RegExp, (...m: string[]) => string];

// applied repeatedly until nothing changes, so nested tags of the same kind resolve
const RULES: Rule[] = [
  [/\[b\]([\s\S]*?)\[\/b\]/gi, (_, t) => `<strong>${t}</strong>`],
  [/\[i\]([\s\S]*?)\[\/i\]/gi, (_, t) => `<em>${t}</em>`],
  [/\[u\]([\s\S]*?)\[\/u\]/gi, (_, t) => `<u>${t}</u>`],
  [/\[s\]([\s\S]*?)\[\/s\]/gi, (_, t) => `<s>${t}</s>`],
  [/\[center\]([\s\S]*?)\[\/center\]/gi, (_, t) => `<div class="text-center">${t}</div>`],
  [/\[right\]([\s\S]*?)\[\/right\]/gi, (_, t) => `<div class="text-right">${t}</div>`],
  [/\[heading\]([\s\S]*?)\[\/heading\]/gi, (_, t) => `<h3>${t}</h3>`],
  [/\[quote(?:=[^\]]*)?\]([\s\S]*?)\[\/quote\]/gi, (_, t) => `<blockquote>${t}</blockquote>`],
  [/\[code\]([\s\S]*?)\[\/code\]/gi, (_, t) => `<code>${t}</code>`],
  [/\[spoiler\]([\s\S]*?)\[\/spoiler\]/gi, (_, t) => `<details><summary>Spoiler</summary>${t}</details>`],
  [/\[font=[^\]]*\]([\s\S]*?)\[\/font\]/gi, (_, t) => t],
  [/\[size=(\d)\]([\s\S]*?)\[\/size\]/gi, (_, n, t) => `<span class="${SIZE_CLASS[Number(n)] ?? ""}">${t}</span>`],
  [
    /\[color=([^\]]+)\]([\s\S]*?)\[\/color\]/gi,
    (_, c, t) => {
      const color = safeColor(c);
      return color ? `<span style="color:${color}">${t}</span>` : t;
    },
  ],
  [
    /\[url=([^\]]+)\]([\s\S]*?)\[\/url\]/gi,
    (_, href, t) => {
      const url = safeUrl(href);
      return url ? `<a href="${url}" data-external>${t}</a>` : t;
    },
  ],
  [
    /\[url\]([\s\S]*?)\[\/url\]/gi,
    (_, href) => {
      const url = safeUrl(href);
      return url ? `<a href="${url}" data-external>${url}</a>` : href;
    },
  ],
  [
    /\[img(?:=[^\]]*)?\]([\s\S]*?)\[\/img\]/gi,
    (_, src) => {
      const url = safeUrl(src);
      return url ? `<img src="${url}" alt="" loading="lazy">` : "";
    },
  ],
  [
    /\[youtube\]([\w-]{6,20})\[\/youtube\]/gi,
    (_, id) => `<a href="https://www.youtube.com/watch?v=${id}" data-external>YouTube</a>`,
  ],
  [
    /\[list(=1)?\]([\s\S]*?)\[\/list\]/gi,
    (_, ordered, body) => {
      const items = body
        .split(/\[\*\]/)
        .slice(1)
        .map((i) => `<li>${i.trim()}</li>`)
        .join("");
      return ordered ? `<ol>${items}</ol>` : `<ul>${items}</ul>`;
    },
  ],
];

/** Converts a Nexus description to sanitized HTML. */
export function bbcodeToHtml(source: string): string {
  // Nexus stores each line break as "\n<br />"; count the pair once
  let text = escape(source.replace(/\r?\n?<br\s*\/?>/gi, "\n"));
  for (let pass = 0; pass < 8; pass++) {
    const before = text;
    for (const [re, fn] of RULES) text = text.replace(re, fn as (...a: string[]) => string);
    if (text === before) break;
  }
  // tags we don't know stay visible as text, which is what the Nexus site does too
  return text.replace(/\n/g, "<br>");
}

/** Image URLs embedded in a description, for the screenshot gallery. */
export function bbcodeImages(source: string): string[] {
  const out: string[] = [];
  for (const m of source.matchAll(/\[img(?:=[^\]]*)?\]([\s\S]*?)\[\/img\]/gi)) {
    try {
      const u = new URL(m[1].trim());
      if (u.protocol === "https:" && !out.includes(u.href)) out.push(u.href);
    } catch {
      // not a URL, skip
    }
  }
  return out;
}
