// Verifies that package.json, the Cargo workspace, the changelog and (optionally) a git tag agree
// on the version. Used by CI and the release workflow.
import { readFileSync } from "node:fs";

const pkg = JSON.parse(readFileSync("package.json", "utf8")).version;
const cargo = readFileSync("Cargo.toml", "utf8").match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m)?.[1];
const changelog = readFileSync("CHANGELOG.md", "utf8");
const tag = process.argv[2]?.replace(/^refs\/tags\//, "");

const problems = [];
if (cargo !== pkg) problems.push(`Cargo.toml workspace version ${cargo} != package.json ${pkg}`);
if (!changelog.includes(`## [${pkg}]`)) problems.push(`CHANGELOG.md has no "## [${pkg}]" section`);
if (tag && tag !== `v${pkg}`) problems.push(`tag ${tag} does not match package.json version v${pkg}`);
if (problems.length) {
  console.error("Version check failed:\n - " + problems.join("\n - "));
  process.exit(1);
}
console.log(`Version ${pkg} is consistent${tag ? ` with tag ${tag}` : ""}.`);
