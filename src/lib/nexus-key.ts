/**
 * Reports whether `text` has the shape of a Nexus personal API key: one line of
 * base64 segments joined by "--", around a hundred characters. Used only to prefill
 * the key field from the clipboard; the key is never sent until the user confirms.
 */
export function looksLikeNexusKey(text: string): boolean {
  const s = text.trim();
  return s.length >= 60 && s.length <= 512 && s.includes("--") && /^[A-Za-z0-9+/=_-]+$/.test(s);
}
