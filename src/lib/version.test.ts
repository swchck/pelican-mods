import { describe, expect, it } from "vitest";
import { compareVersions } from "./version";

describe("compareVersions", () => {
  it("compares numerically, not lexically", () => {
    expect(compareVersions("4.10.0", "4.9.1")).toBe(1);
    expect(compareVersions("4.3.2", "4.5.2")).toBe(-1);
  });

  it("treats missing parts as zero", () => {
    expect(compareVersions("4.5", "4.5.0")).toBe(0);
  });

  it("ignores v prefix and build metadata", () => {
    expect(compareVersions("v4.5.2+821167e", "4.5.2")).toBe(0);
  });

  it("sorts prereleases before the release", () => {
    expect(compareVersions("4.5.0-beta.2", "4.5.0")).toBe(-1);
    expect(compareVersions("4.5.0", "4.5.0-beta.2")).toBe(1);
  });
});
