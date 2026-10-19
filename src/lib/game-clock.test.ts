import { describe, expect, it } from "vitest";
import { formatClock, seasonKey } from "./game-clock";

describe("game clock", () => {
  it("formats HHMM and wraps past midnight", () => {
    expect(formatClock(600)).toBe("06:00");
    expect(formatClock(1430)).toBe("14:30");
    expect(formatClock(2530)).toBe("01:30");
  });

  it("knows the seasons", () => {
    expect(seasonKey("Fall")).toBe("fall");
    expect(seasonKey("monsoon")).toBeNull();
  });
});
