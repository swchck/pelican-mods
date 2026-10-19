// Fails the release when the tag, the three version fields and the changelogs disagree.
import { readFileSync } from "node:fs";
import process from "node:process";
import { section } from "./changelog.mjs";

const tag = process.env.GITHUB_REF_NAME ?? "";
const version = tag.replace(/^v/, "");
const found = {
  "package.json": JSON.parse(readFileSync("package.json", "utf8")).version,
  "src-tauri/tauri.conf.json": JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version,
  "src-tauri/Cargo.toml": /^version = "([^"]+)"/m.exec(readFileSync("src-tauri/Cargo.toml", "utf8"))?.[1],
};
let ok = true;
for (const [file, v] of Object.entries(found)) {
  if (v !== version) {
    console.log(`::error::${file} has version ${v}, tag is ${tag}`);
    ok = false;
  }
}
for (const file of ["CHANGELOG.md", "CHANGELOG.ru.md"]) {
  if (!section(file, version)) {
    console.log(`::error::${file} has no section for ${version}`);
    ok = false;
  }
}
process.exit(ok ? 0 : 1);
