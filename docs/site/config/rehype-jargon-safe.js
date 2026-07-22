// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const jargonModule = require("rehype-jargon");

// rehype-jargon@3.1.0 assumes every <em> node's first child is a text node and
// calls `children[0].value.toLowerCase()` on it. Generated Move reference docs
// can produce an <em> that wraps a JSX element instead of text — for example a
// `(*x)` dereference that CommonMark reads as emphasis — whose first child has
// no `value`, which throws and aborts the whole build. Prepending an empty text
// node makes the term lookup miss safely instead of crashing.
function guardEmphasisFirstChild(node) {
  if (!node || typeof node !== "object") return;

  if (node.tagName === "em") {
    const first = node.children && node.children[0];
    if (!first || typeof first.value !== "string") {
      node.children = node.children || [];
      node.children.unshift({ type: "text", value: "" });
    }
  }

  if (Array.isArray(node.children)) {
    for (const child of node.children) guardEmphasisFirstChild(child);
  }
}

module.exports = function rehypeJargonSafe(options) {
  const rehypeJargon = jargonModule.default || jargonModule;
  const transform = rehypeJargon(options);
  return (tree, file) => {
    guardEmphasisFirstChild(tree);
    return transform(tree, file);
  };
};
