// Builds release notes from the CHANGELOG section for the version plus the provenance block
// produced by the release workflow.
import { readFileSync } from "node:fs";

const version = process.argv[2];
const provenance = process.argv[3] ? readFileSync(process.argv[3], "utf8") : "";
const changelog = readFileSync("CHANGELOG.md", "utf8");
const start = changelog.indexOf(`## [${version}]`);
if (start < 0) {
  console.error(`No changelog section for ${version}`);
  process.exit(1);
}
const rest = changelog.slice(start);
const end = rest.indexOf("\n## [", 1);
const section = (end > 0 ? rest.slice(0, end) : rest).trim();
const body = section.split("\n").slice(1).join("\n").trim();

console.log(`${body}

${provenance}
## Which file do I download?

| Your computer | File |
|---|---|
| Windows 10/11 PC with an Intel or AMD processor | \`VM-Setup-Assistant-${version}-windows-x64-setup.exe\` |
| Mac with Apple Silicon (M1, M2, M3, M4…) | \`VM-Setup-Assistant-${version}-macos-apple-silicon.dmg\` |
| Mac with an Intel processor | \`VM-Setup-Assistant-${version}-macos-intel.dmg\` |

This app does not contain Windows or VirtualBox. It guides you through downloading both from their official sources. See the README for what is tested and what is not.
`);
