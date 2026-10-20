import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const catalog = JSON.parse(await readFile(resolve(root, "hero.features.json"), "utf8"));
const check = process.argv.includes("--check");

const sources = [
  "README.md",
  "Cargo.toml",
  "hero.features.json",
  "AGENTS.md",
  "docs/architecture.md",
  "docs/features/README.md",
  ...catalog.features.map((feature) => feature.docs),
  "docs/indexing.md",
  "docs/mcp-resource-contract.md",
  "docs/publishing.md"
];

const sections = [];
for (const source of sources) {
  const content = (await readFile(resolve(root, source), "utf8")).trimEnd();
  sections.push(`# Source: ${source}\n\n${content}`);
}

const output = `${sections.join("\n\n---\n\n")}\n`;
const outputPath = resolve(root, "llms-full.txt");

if (check) {
  let existing;
  try {
    existing = await readFile(outputPath, "utf8");
  } catch {
    existing = null;
  }
  if (existing !== output) {
    console.error("llms-full.txt is stale; run node scripts/generate-llms-full.mjs");
    process.exit(1);
  }
} else {
  await writeFile(outputPath, output);
}
