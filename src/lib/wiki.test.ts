import { describe, expect, it } from "vitest";
import { setWikiSource, wikiUrl } from "./wiki";

describe("wikiUrl", () => {
  it("builds page links for the chosen wiki", () => {
    setWikiSource(null);
    expect(wikiUrl("Qi's Walnut Room")).toBe("https://stardewvalleywiki.com/Qi's_Walnut_Room");
    setWikiSource("fandom");
    expect(wikiUrl("Dwarf Scroll I")).toBe("https://stardewvalley.fandom.com/wiki/Dwarf_Scroll_I");
    expect(wikiUrl(null)).toBeNull();
    setWikiSource("off");
    expect(wikiUrl("Parsnip")).toBeNull();
  });
});
