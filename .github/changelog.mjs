// Reads one version's section out of a changelog in the "## <version> — <date>" format
// the app itself parses (src/lib/changelog.ts).
import { readFileSync } from "node:fs";

export function section(file, version) {
  const text = readFileSync(file, "utf8");
  for (const chunk of text.split(/^## /m).slice(1)) {
    const nl = chunk.indexOf("\n");
    const head = chunk.slice(0, nl).trim();
    if (head.split(/\s+[—–-]\s+/)[0].replace(/^\[|\]$/g, "") === version) return chunk.slice(nl + 1).trim();
  }
  return null;
}
