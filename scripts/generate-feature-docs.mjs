import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const catalogPath = resolve(root, "hero.features.json");
const catalog = JSON.parse(await readFile(catalogPath, "utf8"));
const check = process.argv.includes("--check");

const targetLabel = (targets) => targets.map((target) => (target === "native" ? "Native" : `\`${target}\``)).join(" and ");

const pageFor = (feature) => {
  const implied = feature.implies.length === 0 ? "None" : feature.implies.map((name) => `\`${name}\``).join(", ");
  const environment = feature.environment.length === 0
    ? "This feature does not define environment variables."
    : feature.environment.map((name) => `- \`${name}\``).join("\n");
  const apis = feature.primary_apis.length === 0
    ? "No public API is listed."
    : feature.primary_apis.map((api) => `- \`${api}\``).join("\n");
  const reasons = feature.enable_when.map((reason) => `- ${reason}`).join("\n");
  const primaryImport = feature.primary_apis[0]?.endsWith("!")
    ? feature.primary_apis[0].slice(0, -1)
    : feature.primary_apis[0];
  const importExample = primaryImport
    ? `\`\`\`rust,no_run\nuse ${primaryImport};\n\`\`\``
    : "No import example is available.";

  return `---
feature: ${feature.name}
api_root: ${feature.api_root}
targets: [${feature.targets.join(", ")}]
implies: [${feature.implies.join(", ")}]
catalog_schema: 1
---

# ${feature.name}

${feature.summary}

## Enable

\`\`\`toml
[dependencies]
hero = {
    git = "ssh://git@github.com/Midwess/hero.git",
    rev = "<commit>",
    features = ["${feature.name}"]
}
\`\`\`

## Availability

- API root: \`${feature.api_root}\`
- Target: ${targetLabel(feature.targets)}
- Implied Hero features: ${implied}

## Enable When

${reasons}

## Primary APIs

${apis}

## Import Example

${importExample}

## Environment

${environment}

## Selection Notes

- Hero's default feature set is empty.
- Enable \`${feature.name}\` only when the consuming code uses this capability.
- Features listed under "Implied Hero features" are enabled automatically by Cargo.
- Validate the consuming project's actual target after changing its feature list.

## Related Documentation

- [Feature index](README.md)
- [Hero overview](../../README.md)
- [Machine-readable catalog](../../hero.features.json)
- [Migration guide](../../MIGRATION.md)
`;
};

const indexRows = catalog.features
  .map((feature) => `| [\`${feature.name}\`](${feature.name}.md) | \`${feature.api_root}\` | ${targetLabel(feature.targets)} | ${feature.summary} |`)
  .join("\n");

const index = `# Hero Feature Documentation

Hero has an empty default feature set. Select only the capabilities imported by
the consuming project.

| Feature | API root | Target | Purpose |
| --- | --- | --- | --- |
${indexRows}

The machine-readable equivalent is [\`hero.features.json\`](../../hero.features.json).
The executable feature graph is [\`Cargo.toml\`](../../Cargo.toml).
`;

const outputs = new Map([[resolve(root, "docs/features/README.md"), index]]);
for (const feature of catalog.features) {
  outputs.set(resolve(root, feature.docs), pageFor(feature));
}

let failed = false;
for (const [path, expected] of outputs) {
  if (check) {
    let actual;
    try {
      actual = await readFile(path, "utf8");
    } catch {
      actual = null;
    }
    if (actual !== expected) {
      console.error(`generated documentation is stale: ${path.slice(root.length + 1)}`);
      failed = true;
    }
  } else {
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, expected);
  }
}

if (failed) {
  process.exit(1);
}
