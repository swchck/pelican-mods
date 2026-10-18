import { describe, expect, it } from "vitest";
import { looksLikeNexusKey } from "./nexus-key";

describe("looksLikeNexusKey", () => {
  it("accepts the Nexus key shape", () => {
    const key = `${"A".repeat(40)}==--${"b".repeat(24)}--${"C9+/".repeat(6)}==`;
    expect(looksLikeNexusKey(key)).toBe(true);
    expect(looksLikeNexusKey(`  ${key}\n`)).toBe(true);
  });

  it("rejects ordinary clipboard text", () => {
    expect(looksLikeNexusKey("hunter2")).toBe(false);
    expect(looksLikeNexusKey("https://www.nexusmods.com/stardewvalley/mods/541")).toBe(false);
    expect(looksLikeNexusKey(`${"a".repeat(80)}`)).toBe(false);
    expect(looksLikeNexusKey(`${"a".repeat(40)}--${"b".repeat(30)} with spaces`)).toBe(false);
  });
});
