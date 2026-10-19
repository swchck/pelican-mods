import { describe, expect, it } from "vitest";
import { bbcodeImages, bbcodeToHtml } from "./bbcode";

describe("bbcodeToHtml", () => {
  it("renders common tags and nested formatting", () => {
    expect(bbcodeToHtml("[b][color=#6aa84f][b]hi[/b][/color][/b]")).toBe(
      '<strong><span style="color:#6aa84f"><strong>hi</strong></span></strong>',
    );
    expect(bbcodeToHtml("[size=6][b]Install[/b][/size]")).toBe(
      '<span class="text-2xl"><strong>Install</strong></span>',
    );
    expect(bbcodeToHtml("[font=Courier New]F1[/font]")).toBe("F1");
  });

  it("turns breaks into <br> and lists into items", () => {
    expect(bbcodeToHtml("a\n<br />b<br>c")).toBe("a<br>b<br>c");
    expect(bbcodeToHtml("[list=1][*]one[*]two[/list]")).toBe("<ol><li>one</li><li>two</li></ol>");
  });

  it("keeps safe links and drops script urls", () => {
    expect(bbcodeToHtml("[url=https://smapi.io]SMAPI[/url]")).toBe(
      '<a href="https://smapi.io/" data-external>SMAPI</a>',
    );
    expect(bbcodeToHtml("[url=javascript:alert(1)]x[/url]")).toBe("x");
    expect(bbcodeToHtml("[img]javascript:alert(1)[/img]")).toBe("");
  });

  it("escapes markup in the source", () => {
    const html = bbcodeToHtml('<script>alert(1)</script>[url="><img src=x onerror=alert(1)>]x[/url]');
    expect(html).not.toContain("<script");
    expect(html).not.toContain("<img src=x");
    expect(html).not.toContain("onerror=alert(1)>");
  });

  it("refuses css injection through color", () => {
    expect(bbcodeToHtml("[color=red;background:url(x)]t[/color]")).toBe("t");
  });
});

describe("bbcodeImages", () => {
  it("collects https images once", () => {
    const src = "[img]https://a/1.png[/img] [img]http://b/2.png[/img] [img]https://a/1.png[/img]";
    expect(bbcodeImages(src)).toEqual(["https://a/1.png"]);
  });

  it("keeps tags from forming inside attributes on later passes", () => {
    const evil = "[url]https://evil/[u[img]x[/img]rl]https://h/onmouseover=alert(1)//[/u[img]x[/img]rl][/url]";
    const html = bbcodeToHtml(evil);
    // every tag with attributes must be exactly the shape the renderer writes
    for (const tag of html.match(/<(a|img)\b[^>]*>/g) ?? []) {
      expect(tag).toMatch(
        /^<a href="https?:\/\/[^"<>[\]]*" data-external>$|^<img src="https?:\/\/[^"<>[\]]*" alt="" loading="lazy">$/,
      );
    }
    expect(html).not.toContain('onmouseover=alert(1)//"');
  });
});
