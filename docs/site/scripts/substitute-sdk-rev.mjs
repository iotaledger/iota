// Substitutes {SDK_REV} in the generated llms text files.
//
// The pages themselves are handled by config/remark-sdk-rev.js, but
// docusaurus-plugin-llms concatenates the markdown sources straight from
// disk without running the MDX pipeline, so the placeholder would reach
// agents as an unusable URL.

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const siteDir = dirname(dirname(fileURLToPath(import.meta.url)));
const buildDir = join(siteDir, "build");
const rev = readFileSync(join(siteDir, "iota-sdk-references.rev"), "utf8").trim();

if (!/^[0-9a-f]{40}$/.test(rev)) {
    throw new Error(`iota-sdk-references.rev must hold a full commit hash, found "${rev}"`);
}

let files = 0;
let replaced = 0;
for (const name of readdirSync(buildDir)) {
    if (!name.startsWith("llms") || !name.endsWith(".txt")) {
        continue;
    }
    const path = join(buildDir, name);
    const text = readFileSync(path, "utf8");
    const occurrences = text.split("{SDK_REV}").length - 1;
    if (occurrences === 0) {
        continue;
    }
    writeFileSync(path, text.replaceAll("{SDK_REV}", rev));
    files += 1;
    replaced += occurrences;
}

console.log(`Substituted ${replaced} SDK revision references in ${files} llms file(s).`);
