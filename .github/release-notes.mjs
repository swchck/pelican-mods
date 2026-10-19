// Prints the GitHub release body: this version's changelog in both languages, then
// install help.
import process from "node:process";
import { section } from "./changelog.mjs";

const version = (process.env.GITHUB_REF_NAME ?? "").replace(/^v/, "");
console.log(`${section("CHANGELOG.ru.md", version)}

<details><summary>English</summary>

${section("CHANGELOG.md", version)}

</details>

---

**macOS:** \`Pelican.Mods_*_aarch64.dmg\` — Apple silicon (M1 и новее), \`*_x64.dmg\` — Intel.
Приложение не подписано сертификатом Apple: при первом запуске откройте его правой кнопкой → «Открыть»,
а на macOS 15 и новее — System Settings → Privacy & Security → «Open Anyway».`);
