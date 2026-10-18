import { describe, expect, it } from "vitest";
import { relativeTime } from "./time";

describe("relativeTime", () => {
  const now = new Date("2026-10-19T12:00:00Z");
  it("picks the largest unit", () => {
    expect(relativeTime(new Date("2026-10-19T11:55:00Z"), now, "en")).toBe("5 minutes ago");
    expect(relativeTime(new Date("2026-10-19T09:00:00Z"), now, "en")).toBe("3 hours ago");
    expect(relativeTime(new Date("2026-10-19T11:59:50Z"), now, "en")).toBe("this minute");
  });
  it("speaks the UI language", () => {
    expect(relativeTime(new Date("2026-10-19T11:55:00Z"), now, "ru")).toBe("5 минут назад");
  });
});
