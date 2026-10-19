import { describe, expect, it } from "vitest";
import { parseChangelog, releasesSince, renderMarkdown } from "./changelog";
import ru from "../../CHANGELOG.ru.md?raw";
import en from "../../CHANGELOG.md?raw";

const SAMPLE = `# What's new

## 0.3.0 — 2026-12-01

### New
- **Profiles** sync

## 0.2.0 - 2026-11-01
- Fix

## [0.1.0]
- First
`;

describe("parseChangelog", () => {
  it("reads versions, dates and bodies", () => {
    const all = parseChangelog(SAMPLE);
    expect(all.map((r) => [r.version, r.date])).toEqual([
      ["0.3.0", "2026-12-01"],
      ["0.2.0", "2026-11-01"],
      ["0.1.0", null],
    ]);
    expect(all[0].body).toContain("### New");
  });

  it("picks what's new since the last seen version", () => {
    const all = parseChangelog(SAMPLE);
    expect(releasesSince(all, "0.1.0", "0.3.0").map((r) => r.version)).toEqual(["0.3.0", "0.2.0"]);
    expect(releasesSince(all, "0.3.0", "0.3.0")).toEqual([]);
    expect(releasesSince(all, null, "0.2.0").map((r) => r.version)).toEqual(["0.2.0", "0.1.0"]);
  });

  it("keeps both shipped changelogs in step", () => {
    const versions = (t: string) => parseChangelog(t).map((r) => r.version);
    expect(versions(ru)).toEqual(versions(en));
    expect(versions(ru).length).toBeGreaterThan(0);
  });
});

describe("renderMarkdown", () => {
  it("renders the changelog subset safely", () => {
    expect(renderMarkdown("### New\n- **Bold** and `code`\n- [link](https://x.dev)\n\nText <b>")).toBe(
      '<h4>New</h4><ul><li><strong>Bold</strong> and <code>code</code></li><li><a href="https://x.dev" data-external>link</a></li></ul><p>Text &lt;b&gt;</p>',
    );
    expect(renderMarkdown("[x](javascript:alert(1))")).toBe("<p>[x](javascript:alert(1))</p>");
  });
});
