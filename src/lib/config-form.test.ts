import { describe, expect, it } from "vitest";
import { choiceValues, getAt, humanize, inferSections, schemaSections, setAt } from "./config-form";

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
