import { describe, expect, it } from "vitest";
import { formatCombo, matches, parseCombo } from "./shortcuts";

const ev = (over: Partial<KeyboardEvent>) =>
  ({ key: "", code: "", metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...over }) as KeyboardEvent;

describe("shortcuts", () => {
  it("matches by physical key so other layouts work", () => {
    const play = parseCombo("mod+p");
    expect(matches(play, ev({ key: "з", code: "KeyP", metaKey: true }), true)).toBe(true);
    expect(matches(play, ev({ key: "p", code: "KeyP", ctrlKey: true }), true)).toBe(false);
    expect(matches(play, ev({ key: "p", code: "KeyP", ctrlKey: true }), false)).toBe(true);
    expect(matches(play, ev({ key: "P", code: "KeyP", metaKey: true, shiftKey: true }), true)).toBe(false);
  });

  it("handles digits, punctuation and plain keys", () => {
    expect(matches(parseCombo("mod+1"), ev({ key: "1", code: "Digit1", metaKey: true }), true)).toBe(true);
    expect(matches(parseCombo("mod+,"), ev({ key: "б", code: "Comma", metaKey: true }), true)).toBe(true);
    expect(matches(parseCombo("arrowdown"), ev({ key: "ArrowDown", code: "ArrowDown" }), true)).toBe(true);
    expect(matches(parseCombo("?"), ev({ key: "?", code: "Slash", shiftKey: true }), true)).toBe(true);
    expect(matches(parseCombo("?"), ev({ key: "/", code: "Slash", shiftKey: true }), true)).toBe(true);
    expect(matches(parseCombo("mod+/"), ev({ key: "/", code: "Slash", metaKey: true }), true)).toBe(true);
    expect(matches(parseCombo("space"), ev({ key: " ", code: "Space" }), true)).toBe(true);
  });

  it("formats for each platform", () => {
    expect(formatCombo("mod+shift+p", true)).toBe("⇧⌘P");
    expect(formatCombo("mod+shift+p", false)).toBe("Ctrl+Shift+P");
    expect(formatCombo("arrowup", true)).toBe("↑");
    expect(formatCombo("space", true)).toBe("Space");
  });
});
