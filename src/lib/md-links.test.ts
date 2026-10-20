import { describe, expect, it } from "vitest";
import { splitLinks } from "./md-links";

describe("splitLinks", () => {
  it("finds links and nexus mod ids", () => {
    const s = splitLinks(
      "⚠ use [unofficial update](https://forums.stardewvalley.net/threads/x.2096/post-1) or [AGE 1.6](https://www.nexusmods.com/stardewvalley/mods/26013?tab=files).",
    );
    expect(s).toEqual([
      { text: "⚠ use " },
      { text: "unofficial update", url: "https://forums.stardewvalley.net/threads/x.2096/post-1" },
      { text: " or " },
      { text: "AGE 1.6", url: "https://www.nexusmods.com/stardewvalley/mods/26013?tab=files", nexusModId: 26013 },
      { text: "." },
    ]);
  });

  it("points smapi.io's own links at its mod list", () => {
    expect(splitLinks("use [Carry Chests](#) instead.")).toEqual([
      { text: "use " },
      { text: "Carry Chests", url: "https://smapi.io/mods#CarryChests" },
      { text: " instead." },
    ]);
  });

  it("leaves other schemes as text", () => {
    expect(splitLinks("[x](javascript:alert(1))")).toEqual([{ text: "[x](javascript:alert(1))" }]);
    expect(splitLinks("plain")).toEqual([{ text: "plain" }]);
  });
});
