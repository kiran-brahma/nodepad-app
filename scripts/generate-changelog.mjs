#!/usr/bin/env node
// Generate CHANGELOG.md from git log using conventional commits.
// Usage: node scripts/generate-changelog.mjs [--tag v0.1.0]
//
// Reads commits since the last tag (or from the beginning) and writes a
// keep-a-changelog formatted entry to CHANGELOG.md.
import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const tag = process.argv.find((a) => a.startsWith("--tag="))?.split("=")[1];
const version = tag ? tag.replace(/^v/, "") : "Unreleased";
const todayIso = new Date().toISOString().split("T")[0];

// Determine the range of commits to include
let range;
if (tag) {
  // Check if the tag already exists
  try {
    execSync(`git rev-parse "${tag}"`, { stdio: "pipe" });
    console.error(`Tag ${tag} already exists — nothing new to log.`);
    process.exit(0);
  } catch {
    // Tag doesn't exist yet; find the previous tag
  }
  try {
    const lastTag = execSync("git describe --tags --abbrev=0 HEAD~1 2>/dev/null", {
      encoding: "utf8",
      stdio: "pipe",
    }).trim();
    range = `${lastTag}..HEAD`;
  } catch {
    range = "HEAD";
  }
} else {
  // Unreleased: everything since the last tag
  try {
    const lastTag = execSync("git describe --tags --abbrev=0", {
      encoding: "utf8",
      stdio: "pipe",
    }).trim();
    range = `${lastTag}..HEAD`;
  } catch {
    range = "HEAD";
  }
}

// Fetch commit log
const log = execSync(
  `git log ${range} --format="%s|||%b|||%H" --no-merges`,
  { encoding: "utf8", stdio: "pipe" }
).trim();

if (!log) {
  console.log("No new commits to log.");
  process.exit(0);
}

// Parse commits into categories
const commits = log.split("\n").map((line) => {
  const [subject, body, hash] = line.split("|||");
  return { subject: subject.trim(), body: (body || "").trim(), hash: hash?.trim() };
});

const categories = {
  feat: { title: "Added", items: [] },
  fix: { title: "Fixed", items: [] },
  docs: { title: "Documentation", items: [] },
  refactor: { title: "Changed", items: [] },
  perf: { title: "Performance", items: [] },
  test: { title: "Testing", items: [] },
  chore: { title: "Chores", items: [] },
  other: { title: "Other", items: [] },
};

for (const commit of commits) {
  const match = commit.subject.match(
    /^(feat|fix|docs|refactor|perf|test|chore)(\([^)]+\))?:\s*(.*)/i
  );
  if (match) {
    const type = match[1].toLowerCase();
    const scope = match[2] || "";
    const message = match[3];
    const prefix = scope ? `**${scope.replace(/[()]/g, "")}:** ` : "";
    categories[type]?.items.push(`- ${prefix}${message}`);
  } else {
    categories.other.items.push(`- ${commit.subject}`);
  }
}

// Build the changelog entry
const header = tag
  ? `## [${version}] - ${todayIso}`
  : `## [Unreleased]`;

let entry = `\n${header}\n`;

let hasContent = false;
for (const [, cat] of Object.entries(categories)) {
  if (cat.items.length > 0) {
    entry += `\n### ${cat.title}\n\n`;
    entry += cat.items.join("\n") + "\n";
    hasContent = true;
  }
}

if (!hasContent) {
  entry += "\nNo notable changes.\n";
}

// Read existing CHANGELOG.md or create one
let changelog;
try {
  changelog = readFileSync("CHANGELOG.md", "utf8");
} catch {
  changelog = "# Changelog\n\nAll notable changes to Nodepad are documented here.\n";
}

// Insert the new entry after the header
const headerMatch = changelog.match(/^# Changelog\n\n[^\n]*\n/);
if (headerMatch) {
  const idx = headerMatch.index + headerMatch[0].length;
  changelog = changelog.slice(0, idx) + entry + changelog.slice(idx);
} else {
  changelog = `# Changelog\n\n${entry}\n${changelog}`;
}

writeFileSync("CHANGELOG.md", changelog);
console.log(`→ CHANGELOG.md: ${tag ? `added [${version}]` : "updated [Unreleased]"} entry`);
