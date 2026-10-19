import { describe, expect, it } from "vitest";
import type { GmcmMod, GmcmOption } from "./api";
import {
  applyLabels,
  choiceValues,
  getAt,
  gmcmSections,
  humanize,
  inferSections,
  schemaSections,
  setAt,
} from "./config-form";

describe("humanize", () => {
  it("splits camel case and keeps acronyms", () => {
    expect(humanize("ShowGiftTastes")).toBe("Show gift tastes");
    expect(humanize("EnableNPCMap")).toBe("Enable NPC map");
    expect(humanize("max_items")).toBe("Max items");
  });
});

describe("inferSections", () => {
  it("types fields from values", () => {
    const sections = inferSections({
      ToggleLookup: "F1, LeftStick",
      ShowDataMiningFields: false,
      TableRows: 20,
      Scale: 1.25,
      Title: "Hello",
      Ignore: ["a", "b"],
      Controls: { ScrollUp: "Up", Nested: { Deep: 1 } },
    });
    expect(sections[0].title).toBeNull();
    expect(sections[0].fields.map((f) => f.kind)).toEqual(["keybind", "bool", "number", "number", "text", "list"]);
    expect(sections[0].fields[3]).toMatchObject({ integer: false });
    expect(sections[1].title).toBe("Controls");
    expect(sections[1].fields.map((f) => [f.kind, f.path.join(".")])).toEqual([
      ["keybind", "Controls.ScrollUp"],
      ["json", "Controls.Nested"],
    ]);
  });
});

describe("schemaSections", () => {
  it("groups content patcher options", () => {
    const sections = schemaSections([
      {
        key: "EnableJohn",
        allowValues: ["true", "false"],
        allowBlank: false,
        allowMultiple: false,
        default: ["true"],
        description: null,
        section: null,
        boolean: true,
      },
      {
        key: "Material",
        allowValues: ["Wood", "Metal"],
        allowBlank: false,
        allowMultiple: false,
        default: ["Wood"],
        description: "Look",
        section: "Looks",
        boolean: false,
      },
    ]);
    expect(sections.map((s) => s.title)).toEqual([null, "Looks"]);
    expect(sections[1].fields[0]).toMatchObject({ kind: "choice", options: ["Wood", "Metal"], description: "Look" });
  });
});

describe("paths", () => {
  it("reads and writes nested values immutably, keeping order", () => {
    const obj = { A: 1, B: { C: "x" }, D: true };
    const next = setAt(obj, ["B", "C"], "y");
    expect(getAt(next, ["B", "C"])).toBe("y");
    expect(getAt(obj, ["B", "C"])).toBe("x");
    expect(Object.keys(next)).toEqual(["A", "B", "D"]);
    expect(getAt(next, ["Missing", "X"])).toBeNull();
  });

  it("splits choice values", () => {
    expect(choiceValues("spring, fall")).toEqual(["spring", "fall"]);
    expect(choiceValues(true)).toEqual(["true"]);
    expect(choiceValues(null)).toEqual([]);
  });
});

describe("applyLabels", () => {
  it("uses the mod's own names and descriptions", () => {
    const sections = inferSections({ ShowUnknownGiftTastes: false, Controls: { ToggleLookup: "F1" } });
    const out = applyLabels(sections, {
      showunknowngifttastes: { name: "Show unknown gift tastes", description: "Even unlearned ones.", values: {} },
      controls: { name: "Keybinds", description: null, values: {} },
    });
    expect(out[0].fields[0]).toMatchObject({ label: "Show unknown gift tastes", description: "Even unlearned ones." });
    expect(out[1].title).toBe("Keybinds");
    expect(out[1].fields[0].label).toBe("Toggle lookup");
  });
});

describe("gmcmSections", () => {
  const option = (over: Partial<GmcmOption>): GmcmOption => ({
    path: null,
    name: "",
    tooltip: null,
    kind: "bool",
    section: null,
    min: null,
    max: null,
    interval: null,
    choices: null,
    ...over,
  });
  const gmcm: GmcmMod = {
    id: "A.B",
    titleScreenOnly: false,
    options: [
      option({ path: ["Enabled"], name: "Turn on", tooltip: "Does it" }),
      option({ path: ["Tastes", "Loved"], name: "Loved gifts", section: "Gifts" }),
      option({ path: ["Count"], name: "Count", kind: "int", section: "Gifts", min: 1, max: 9, interval: 1 }),
      option({
        path: ["Mode"],
        name: "Mode",
        kind: "choice",
        section: "Look",
        choices: [
          { value: "A", label: "Alpha" },
          { value: "B", label: "Beta" },
        ],
      }),
      option({ path: null, name: "Unmapped" }),
      option({ path: ["Gone"], name: "Not in the file" }),
    ],
  };
  const values = { Enabled: true, Tastes: { Loved: true, Hated: false }, Count: 3, Mode: "A", Extra: "x" };

  it("follows the menu and keeps leftovers", () => {
    const s = gmcmSections(gmcm, values, "Other");
    expect(s.map((x) => [x.title, x.fields.map((f) => f.label)])).toEqual([
      [null, ["Turn on"]],
      ["Gifts", ["Loved gifts", "Count"]],
      ["Look", ["Mode"]],
      ["Other", ["Extra", "Hated"]],
    ]);
    expect(s[0].fields[0].description).toBe("Does it");
    expect(s[1].fields[1]).toMatchObject({ kind: "number", integer: true, min: 1, max: 9, step: 1 });
    expect(s[2].fields[0]).toMatchObject({
      kind: "choice",
      options: ["A", "B"],
      optionLabels: { A: "Alpha", B: "Beta" },
    });
  });
});
