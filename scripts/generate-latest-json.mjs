#!/usr/bin/env node
// Generates latest.json for Tauri updater after a macOS build.
// Usage: node scripts/generate-latest-json.mjs v0.1.0
import { readFileSync, readdirSync, writeFileSync } from "fs";
import { join } from "path";

const tag = process.argv[2];
if (!tag) {
  console.error("Usage: generate-latest-json.mjs <tag>  e.g. v0.1.0");
  process.exit(1);
}

const version = tag.replace(/^v/, "");
const bundleDir = "src-tauri/target/release/bundle/macos";

const sigFile = readdirSync(bundleDir).find((f) => f.endsWith(".app.tar.gz.sig"));
if (!sigFile) {
  console.error("No .app.tar.gz.sig file found in", bundleDir);
  process.exit(1);
}

const tarFile = sigFile.replace(".sig", "");
const signature = readFileSync(join(bundleDir, sigFile), "utf-8").trim();

const base = `https://github.com/kiran-brahma/nodepad-app/releases/download/${tag}`;

// GitHub converts spaces to dots in asset filenames when uploaded via gh CLI
const url = `${base}/${tarFile.replace(/ /g, ".")}`;

const manifest = {
  version,
  notes: `Nodepad ${tag}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "darwin-aarch64": {
      signature,
      url,
    },
    "darwin-x86_64": {
      signature,
      url,
    },
  },
};

writeFileSync("latest.json", JSON.stringify(manifest, null, 2));
console.log("→ latest.json written for version", version);
console.log("  download url:", url);
