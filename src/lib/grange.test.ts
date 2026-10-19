import { describe, expect, it } from "vitest";
import { bestGrange, grangeScore, itemPoints, place, type GrangeItem } from "./grange";

const item = (name: string, category: number, quality: number, price: number, stack = 1): GrangeItem => ({
  id: name,
  name,
  category,
  quality,
  price,
  stack,
  where: "inventory",
});

describe("grange", () => {
  it("scores items like the judge", () => {
    expect(itemPoints(item("Parsnip", -75, 0, 35))).toBe(2);
    expect(itemPoints(item("Gold melon", -79, 2, 375))).toBe(6);
    // cheap-quality bonuses: 300+ below gold, 400+ at normal quality
    expect(itemPoints(item("Pumpkin", -75, 0, 420))).toBe(6);
    expect(itemPoints(item("Iridium wine", -26, 4, 1200))).toBe(8);
  });

  it("totals a display with variety and empty slots", () => {
    expect(grangeScore([])).toBe(14 - 9);
    const two = [item("Parsnip", -75, 0, 35), item("Egg", -5, 0, 50)];
    // 14 + 2 + 2 + 10 + (9 - 14)
    expect(grangeScore(two)).toBe(23);
    expect(place(90)).toBe(1);
    expect(place(74)).toBe(3);
  });

  it("keeps a strong single item when variety doesn't pay", () => {
    const pool = [item("Iridium starfruit", -79, 4, 1100, 20), item("Wine", -26, 0, 400), item("Egg", -5, 0, 50)];
    // 8 starfruit and the wine (103) beat any display that also fits the egg
    expect(grangeScore(bestGrange(pool))).toBe(103);
  });

  it("finds a display that covers groups when that wins", () => {
    const pool = [
      item("Starfruit", -79, 0, 750, 20),
      item("Parsnip", -75, 0, 35),
      item("Egg", -5, 0, 50),
      item("Quartz", -2, 0, 25),
      item("Sardine", -4, 0, 40),
      item("Leek", -81, 0, 60),
      item("Wine", -26, 0, 400),
    ];
    const best = bestGrange(pool);
    expect(best).toHaveLength(9);
    // 9 starfruit score 82; covering the groups with the rest and topping up with starfruit wins
    expect(new Set(best.map((i) => i.name)).size).toBeGreaterThanOrEqual(6);
    expect(grangeScore(best)).toBeGreaterThan(grangeScore(Array(9).fill(pool[0])));
    expect(place(grangeScore(best))).toBe(1);
  });
});
