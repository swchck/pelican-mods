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

test("library filters by status and tag", async ({ page }) => {
  await open(page, "/", "ru");
  const rows = page.getByRole("option");
  await expect(rows).toHaveCount(4);
  await page.getByRole("button", { name: /^Выключены/ }).click();
  await expect(rows).toHaveCount(1);
  await page.getByRole("button", { name: /^Все/ }).click();
  await page.getByRole("button", { name: /^UI/ }).click();
  await expect(rows).toHaveCount(1);
  await rows.first().click();
  const add = page.getByLabel("+ тег");
  await add.fill("Удобство");
  await add.press("Enter");
  await expect(page.getByRole("button", { name: /^Удобство/ })).toBeVisible();
  await page.getByRole("button", { name: "Убрать тег «UI»" }).click();
  // the tag filter goes with its last tag, so every mod is back
  await expect(rows).toHaveCount(4);
});

test("predictions wait for the spoilers setting", async ({ page }) => {
  await open(page, "/helpers", "ru");
  await expect(page.getByText(/Предсказания скрыты/)).toBeVisible();
  await page.goto("/#/settings");
  await page.getByRole("switch", { name: "Спойлеры и предсказания" }).click();
  await page.goto("/#/helpers");
  await expect(page.getByText(/Предсказания скрыты/)).toHaveCount(0);
});

test("crop calculator ranks this season's crops", async ({ page }) => {
  await open(page, "/helpers", "ru");
  await page.getByRole("tab", { name: "Урожай" }).click();
  const rows = page.locator("tbody tr");
  await expect(rows.first()).toBeVisible();
  // the mock's game is on fall 12: pumpkins still make it, parsnips wait for spring
  await expect(rows.filter({ hasText: "Тыква" })).toHaveCount(1);
  await expect(rows.filter({ hasText: "Пастернак" })).toHaveCount(0);
  await page.getByRole("switch", { name: "Теплица или остров" }).click();
  await expect(rows.filter({ hasText: "Пастернак" })).toHaveCount(1);
});

test("map shows the farm and what's ready", async ({ page }) => {
  await open(page, "/helpers", "ru");
  await page.getByRole("tab", { name: "Карта" }).click();
  await expect(page.getByRole("img", { name: "Ферма" })).toBeVisible();
  await expect(page.getByText("Готово: 5 · Не полито: 6 · Машин с готовым: 1")).toBeVisible();
  const map = page.getByRole("img", { name: "Ферма" });
  const box = (await map.boundingBox())!;
  // the mock's pumpkin patch starts at tile (20, 20) of a 40-tile-wide farm
  await page.mouse.move(box.x + (20.5 / 40) * box.width, box.y + (20.5 / 32) * box.height);
  await expect(page.getByText("Тыква · (20, 20) · готово")).toBeVisible();
  await page.getByRole("button", { name: "Остров" }).click();
  await expect(page.getByText("Этого места в сохранении пока нет.")).toBeVisible();
});

test("co-op finds a host on the network and compares lists", async ({ page }) => {
  await open(page, "/", "ru");
  await page.getByRole("button", { name: "Профиль" }).last().click();
  await page.getByRole("menuitem", { name: "Кооп по сети…" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("switch", { name: "Делиться моим списком" }).click();
  await expect(dialog.getByText(/пока открыто это окно/)).toBeVisible();
  await dialog.getByRole("button", { name: "Искать" }).click();
  await dialog.getByRole("button", { name: "Сравнить" }).click();
  await expect(page.getByRole("dialog").getByText("Automate")).toBeVisible();
});

test("walnut tracker shows only with spoilers on", async ({ page }) => {
  await open(page, "/helpers", "ru");
  await expect(page.getByRole("tab", { name: "Орехи" })).toHaveCount(0);
  await page.goto("/#/settings");
  await page.getByRole("switch", { name: "Спойлеры и предсказания" }).click();
  await page.goto("/#/helpers");
  await page.getByRole("tab", { name: "Орехи" }).click();
  await expect(page.getByText("Золотых орехов найдено: 93 из 130")).toBeVisible();
  await expect(page.getByText("Полевой офис: летучая мышь")).toBeVisible();
  await expect(page.getByText("Песня русалки на флейтовых блоках")).toHaveCount(0);
  await page.getByRole("switch", { name: "Только ненайденные" }).click();
  await expect(page.getByText("Песня русалки на флейтовых блоках")).toBeVisible();
});

test("library shows Nexus pictures", async ({ page }) => {
  await open(page, "/", "ru");
  const row = page.getByRole("option").filter({ hasText: "Lookup Anything" });
  await expect(row.locator("img")).toHaveAttribute("src", /^data:image/);
  await expect(page.getByRole("option").filter({ hasText: "Console Commands" }).locator("img")).toHaveCount(0);
});

test("browse reads a Nexus collection", async ({ page }) => {
  await open(page, "/browse", "ru");
  await page.getByRole("button", { name: "Коллекция" }).click();
  const dialog = page.getByRole("dialog");
  await dialog
    .getByLabel("Ссылка или код коллекции")
    .fill("https://next.nexusmods.com/stardewvalley/collections/tckf0m");
  await dialog.getByRole("button", { name: "Открыть" }).click();
  await expect(dialog.getByText("Модов: 4, уже есть: 2, не хватает: 1")).toBeVisible();
  await dialog.getByLabel("Включая необязательные").check();
  await expect(dialog.getByText("Модов: 4, уже есть: 2, не хватает: 2")).toBeVisible();
});

test("library translates a mod without the language", async ({ page }) => {
  await open(page, "/", "ru");
  await page.getByRole("option").filter({ hasText: "Lookup Anything" }).click();
  await page.getByRole("button", { name: "Перевести самому" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("Press F1 to look things up")).toBeVisible();
  await dialog.getByLabel("hud.hint").fill("Нажмите F1, чтобы узнать подробности");
  await expect(dialog.getByText("1 / 2")).toBeVisible();
  await dialog.getByRole("button", { name: "Сохранить перевод" }).click();
  await expect(page.getByText(/Перевод сохранён/)).toBeVisible();
});

test("profiles can keep mod settings of their own", async ({ page }) => {
  await open(page, "/", "ru");
  await page.getByRole("button", { name: "Профиль" }).last().click();
  const item = page.getByRole("menuitemcheckbox", { name: "Свои настройки модов" });
  await expect(item).toHaveAttribute("aria-checked", "false");
  await item.click();
  await page.getByRole("button", { name: "Профиль" }).last().click();
  await expect(page.getByRole("menuitemcheckbox", { name: "Свои настройки модов" })).toHaveAttribute(
    "aria-checked",
    "true",
  );
});

test("library halves its way to a problem's mod", async ({ page }) => {
  await open(page, "/", "ru");
  await page.getByRole("button", { name: "Профиль" }).last().click();
  await page.getByRole("menuitem", { name: "Найти мод, из-за которого проблема" }).click();
  await expect(page.getByText("Поиск виновника · раунд 1")).toBeVisible();
  await page.getByRole("button", { name: "Проблемы нет" }).click();
  await expect(page.getByText(/Похоже, всё дело в/)).toBeVisible();
  // the culprit may only break with a mod the search cleared
  await page.getByRole("button", { name: "Проверить, с кем вместе" }).click();
  await expect(page.getByText(/В этом раунде включён только/)).toBeVisible();
  await page.getByRole("button", { name: "Проблемы нет" }).click();
  await expect(page.getByText(/ломается только вместе с/)).toBeVisible();
});

test("saves compare two states of a farm", async ({ page }) => {
  await open(page, "/saves", "ru");
  await page.getByRole("button", { name: "Что изменилось" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("Рыбная ловля: 6 → 7")).toBeVisible();
  await expect(dialog.getByText("Впервые поймано: 1")).toBeVisible();
  await expect(dialog.getByText("+15")).toBeVisible();
});

test("saves chart a farm's history", async ({ page }) => {
  await open(page, "/saves", "ru");
  await page.getByRole("button", { name: "История" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.locator("circle")).toHaveCount(12);
  await expect(dialog.locator("figcaption").getByText("Весна, 1-й день, год 2")).toBeVisible();
  // perfection is known for every other day only
  await dialog.getByRole("button", { name: "Совершенство" }).click();
  await expect(dialog.locator("circle")).toHaveCount(6);
});

test("helpers find items across chests", async ({ page }) => {
  await open(page, "/helpers", "ru");
  await page.getByRole("tab", { name: "Сундуки" }).click();
  // the mock's game runs with a farm, so the tab reads it live instead of the save
  await expect(page.getByText(/Сейчас в игре/)).toBeVisible();
  await page.getByLabel("Найти предмет…").fill("вино");
  await expect(page.getByText("Подвал")).toBeVisible();
  await expect(page.getByText("Тыква")).toHaveCount(0);
});

test("console measures mods and lists shared patches", async ({ page }) => {
  await page.clock.install();
  await open(page, "/console?tab=load", "ru");
  await page.getByRole("button", { name: /Замерить/ }).click();
  await page.clock.runFor(31_000);
  await expect(page.getByText("Automate")).toBeVisible();
  await page.getByRole("button", { name: "Найти пересечения" }).click();
  await expect(page.getByText("StardewValley.Menus.InventoryMenu.draw")).toBeVisible();
  // owners show as mod names from the library where it knows them
  await expect(page.getByText("Lookup Anything · постфикс")).toBeVisible();
});

test("settings switch between game installs", async ({ page }) => {
  await open(page, "/settings", "ru");
  await expect(page.getByText("Используется")).toHaveCount(1);
  await page.getByRole("button", { name: "Играть в этой" }).click();
  // the Steam copy is now the active one, and the GOG one offers itself back
  await expect(page.locator("li").filter({ hasText: "Steam" })).toContainText("Используется");
  await expect(page.locator("li").filter({ hasText: "GOG" })).toContainText("Играть в этой");
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
