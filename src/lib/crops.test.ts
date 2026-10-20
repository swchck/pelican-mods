import { describe, expect, it } from "vitest";
import { daysLeft, growthDays, plan, speedBonus, type Crop } from "./crops";

const crop = (over: Partial<Crop>): Crop => ({
  id: "24",
  seed: "472",
  seasons: ["spring"],
  phases: [1, 1, 1, 1],
  regrow: -1,
  minStack: 1,
  maxStack: 1,
  extraChance: 0,
  paddy: false,
  trellis: false,
  price: 35,
  seedPrice: 20,
  ...over,
});

describe("crops", () => {
  it("speeds growth like the game", () => {
    // worked through the game's passes by hand
    const cauliflower = [1, 2, 4, 4, 1];
    expect(growthDays(cauliflower, 0)).toBe(12);
    expect(growthDays(cauliflower, 0.1)).toBe(10);
    expect(growthDays(cauliflower, 0.25)).toBe(9);
    expect(growthDays(cauliflower, speedBonus(crop({}), { fertilizer: "deluxe", agriculturist: true }))).toBe(7);
    const parsnip = [1, 1, 1, 1];
    expect(growthDays(parsnip, 0.1)).toBe(3);
    expect(growthDays(parsnip, 0.33)).toBe(2);
  });

  it("counts the days a crop can still grow", () => {
    const corn = crop({ seasons: ["summer", "fall"] });
    expect(daysLeft(corn, { season: "summer", day: 1, anySeason: false })).toBe(55);
    expect(daysLeft(corn, { season: "spring", day: 1, anySeason: false })).toBe(0);
    expect(daysLeft(corn, { season: "winter", day: 20, anySeason: true })).toBe(28);
  });

  it("plans replanting and regrowth", () => {
    const opts = {
      season: "spring",
      day: 1,
      fertilizer: "none",
      agriculturist: false,
      tiller: false,
      anySeason: false,
    } as const;
    const parsnip = plan(crop({}), opts)!;
    expect([parsnip.harvests, parsnip.seeds, parsnip.profit]).toEqual([6, 6, 6 * 35 - 6 * 20]);
    const blueberry = plan(
      crop({
        seasons: ["summer"],
        phases: [1, 3, 3, 4, 2],
        regrow: 4,
        minStack: 3,
        maxStack: 3,
        extraChance: 0.02,
        price: 50,
        seedPrice: 80,
      }),
      { ...opts, season: "summer" },
    )!;
    expect([blueberry.harvests, blueberry.seeds]).toEqual([4, 1]);
    expect(blueberry.perHarvest).toBeCloseTo(3.0204, 3);
    expect(plan(crop({ phases: [] }), opts)).toBeNull();
  });
});
