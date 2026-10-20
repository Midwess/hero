import { access, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifest = await readFile(resolve(root, "Cargo.toml"), "utf8");
const catalog = JSON.parse(await readFile(resolve(root, "hero.features.json"), "utf8"));

const version = manifest.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
if (!version || catalog.crate.version !== version) {
  throw new Error(`catalog version ${catalog.crate.version} does not match Cargo version ${version}`);
}

const featureSection = manifest.match(/^\[features\]\n([\s\S]*?)(?=^\[)/m)?.[1];
if (!featureSection) {
  throw new Error("Cargo.toml has no [features] section");
}

const cargoFeatures = new Map();
for (const line of featureSection.split("\n")) {
  const match = line.match(/^([a-z0-9-]+)\s*=\s*\[(.*)\]\s*$/);
  if (!match) continue;
  const entries = [...match[2].matchAll(/"([^"]+)"/g)].map((entry) => entry[1]);
  cargoFeatures.set(match[1], entries.filter((entry) => !entry.startsWith("dep:") && !entry.includes("/")));
}

const defaultFeatures = cargoFeatures.get("default");
if (!defaultFeatures || defaultFeatures.length !== 0 || catalog.crate.default_features.length !== 0) {
  throw new Error("Hero default features must be empty in Cargo.toml and hero.features.json");
}

const metadataFeatures = new Map(catalog.features.map((feature) => [feature.name, feature]));
const cargoNames = [...cargoFeatures.keys()].filter((name) => name !== "default").sort();
const metadataNames = [...metadataFeatures.keys()].sort();
if (JSON.stringify(cargoNames) !== JSON.stringify(metadataNames)) {
  throw new Error(`feature mismatch\nCargo: ${cargoNames.join(", ")}\nCatalog: ${metadataNames.join(", ")}`);
}

for (const name of cargoNames) {
  const feature = metadataFeatures.get(name);
  const cargoImplies = cargoFeatures.get(name).sort();
  const metadataImplies = [...feature.implies].sort();
  if (JSON.stringify(cargoImplies) !== JSON.stringify(metadataImplies)) {
    throw new Error(`${name} implies ${metadataImplies.join(", ")} in metadata but ${cargoImplies.join(", ")} in Cargo`);
  }

  for (const key of ["api_root", "summary", "docs"]) {
    if (typeof feature[key] !== "string" || feature[key].length === 0) {
      throw new Error(`${name}.${key} must be a non-empty string`);
    }
  }
  for (const key of ["targets", "implies", "enable_when", "primary_apis", "environment"]) {
    if (!Array.isArray(feature[key])) {
      throw new Error(`${name}.${key} must be an array`);
    }
  }
  await access(resolve(root, feature.docs));
}

