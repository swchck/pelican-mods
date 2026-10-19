import { expect, test, type Page } from "@playwright/test";

const ROUTES = ["/", "/browse", "/saves", "/progress", "/helpers", "/game", "/console", "/settings", "/config"];

// the browser mock reads the UI language from here instead of the settings file
async function open(page: Page, path: string, locale: string) {
  await page.addInitScript((l) => localStorage.setItem("mockLocale", l), locale);
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`/#${path}`);
  await page.waitForLoadState("networkidle");
  return errors;
}

for (const locale of ["ru", "en"] as const) {
  for (const path of ROUTES) {
    test(`${path} renders in ${locale}`, async ({ page }) => {
      const errors = await open(page, path, locale);
      await expect(page.locator("main")).not.toBeEmpty();
      expect(errors).toEqual([]);
    });
  }

  test(`progress menu labels aren't cut in ${locale}`, async ({ page }) => {
    await open(page, "/progress", locale);
    const labels = page.locator("nav[aria-label] button span:first-child");
    await expect(labels.first()).toBeVisible();
    for (const l of await labels.all()) {
      const cut = await l.evaluate((el) => el.scrollWidth > el.clientWidth + 1);
      expect(cut, await l.textContent()).toBe(false);
    }
  });
}

test("helpers tabs switch without the game", async ({ page }) => {
  await open(page, "/helpers", "ru");
  for (const tab of ["Календарь", "Рыба", "Музей"]) {
    await page.getByRole("tab", { name: tab }).click();
    await expect(page.getByRole("tab", { name: tab })).toHaveAttribute("data-state", "active");
  }
  await page.getByRole("tab", { name: "Календарь" }).click();
  await expect(page.getByRole("button", { name: "Абигейл" })).toBeVisible();
});

for (const locale of ["de", "es", "fr", "pt", "it", "ja", "ko", "zh", "tr", "hu"]) {
  test(`library and progress render in ${locale}`, async ({ page }) => {
    const errors = await open(page, "/", locale);
    await expect(page.locator("html")).toHaveAttribute("lang", locale);
    await page.goto("/#/progress");
    await page.waitForLoadState("networkidle");
    await expect(page.locator("main")).not.toBeEmpty();
    expect(errors).toEqual([]);
  });
}

for (const locale of ["ru", "en", "de", "es", "fr", "pt", "it", "ja", "ko", "zh", "tr", "hu"]) {
  test(`play card fits in ${locale}`, async ({ page }) => {
    await open(page, "/", locale);
    const card = page.locator("[data-test=play-card]");
    const fits = async () => {
      const box = await card.boundingBox();
      for (const b of await card.getByRole("button").all()) {
        const r = await b.boundingBox();
        expect(r!.x + r!.width).toBeLessThanOrEqual(box!.x + box!.width + 0.5);
      }
    };
    await fits();
    // the running state swaps in the stop button, the longest label
    await page.evaluate(() => {
      const pinia = (
        document.querySelector("#app") as unknown as {
          __vue_app__: { config: { globalProperties: { $pinia: { state: { value: { game: { run: unknown } } } } } } };
        }
      ).__vue_app__.config.globalProperties.$pinia;
      pinia.state.value.game.run = { state: "running", modded: true, pid: 1 };
    });
    await expect(card.getByRole("button")).toHaveCount(1);
    await fits();
  });
}
