const fs = require("fs");
const path = require("path");

const REV_FILE = path.resolve(__dirname, "../iota-sdk-references.rev");
const PLACEHOLDER = /\{SDK_REV\}/g;

function readRev() {
    const rev = fs.readFileSync(REV_FILE, "utf8").trim();
    if (!/^[0-9a-f]{40}$/.test(rev)) {
        throw new Error(
            `${REV_FILE} must hold a full 40-character commit hash, found "${rev}"`,
        );
    }
    return rev;
}

/**
 * Replaces `{SDK_REV}` in code blocks with the iota-rust-sdk revision from
 * `iota-sdk-references.rev`, so the example embeds and the generated API
 * references come from the same commit and a bump touches one file.
 *
 * Line ranges in the embed URLs only stay correct against a fixed revision,
 * which is why the examples are not embedded from a branch.
 */
module.exports = function remarkSdkRev() {
    const rev = readRev();
    return (tree) => {
        const substitute = (node) => {
            if (node.type === "code" && typeof node.value === "string") {
                node.value = node.value.replace(PLACEHOLDER, rev);
            }
            for (const child of node.children ?? []) {
                substitute(child);
            }
        };
        substitute(tree);
    };
};
