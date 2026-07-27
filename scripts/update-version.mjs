#!/usr/bin/env node
// Bump version in package.json, tauri.conf.json, and Cargo.toml, then refresh
// Cargo.lock. No LLM; deterministic file edits only.
import { readFileSync, writeFileSync } from "node:fs";
import { execSync } from "node:child_process";

const version = process.argv[2]?.replace(/^v/, "");
if (!version || !/^\d+\.\d+\.\d+/.test(version)) {
  console.error("Usage: update-version.mjs <version>  e.g. 0.1.0");
  process.exit(1);
}

function bumpJson(path, key) {
  const data = JSON.parse(readFileSync(path, "utf8"));
  data[key] = version;
  writeFileSync(path, JSON.stringify(data, null, 2) + "\n");
  console.log(`→ ${path}: ${version}`);
}

function bumpToml(path) {
  let text = readFileSync(path, "utf8");
  const updated = text.replace(/^version = "[^"]+"/m, `version = "${version}"`);
  if (updated === text) {
    console.error(`Could not find version line in ${path}`);
    process.exit(1);
  }
  writeFileSync(path, updated);
  console.log(`→ ${path}: ${version}`);
}

bumpJson("package.json", "version");
bumpJson("src-tauri/tauri.conf.json", "version");
bumpToml("src-tauri/Cargo.toml");

console.log("→ Refreshing Cargo.lock…");
execSync("cargo update --package nodepad", {
  stdio: "inherit",
  cwd: "src-tauri",
});
