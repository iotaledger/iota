// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type NameTree = {
    name: string;
    subnames: NameTree[];
};

export function buildNameTree(name: string, ownedSubnames: string[]): NameTree {
    const normalizedName = name.toLowerCase();
    const parts = normalizedName.split('.');

    const subnames = ownedSubnames
        .filter((sub) => {
            if (sub === normalizedName) return false;
            const subParts = sub.split('.');

            // Check if the subname has the name as parent, and if the length matches
            return (
                subParts.slice(-parts.length).join('.') === parts.join('.') &&
                subParts.length === parts.length + 1
            );
        })
        .sort()
        .map((child) => buildNameTree(child, ownedSubnames));

    return {
        name,
        subnames,
    };
}

export function findInNameTree(tree: NameTree | NameTree[], target: string): NameTree | null {
    const nodes = Array.isArray(tree) ? tree : [tree];

    for (const node of nodes) {
        if (node.name.toLowerCase() === target.toLowerCase()) {
            return node;
        }

        const found = findInNameTree(node.subnames, target);
        if (found) return found;
    }

    return null;
}
export function traverseNameTree(nameTree: NameTree | null, nameparts: string[]): NameTree | null {
    if (!nameTree || nameparts.length === 0) return null;

    let traversedTree: NameTree | null = nameTree;

    for (const part of nameparts) {
        if (!traversedTree) return null;

        if (traversedTree.name === part) continue;

        traversedTree = traversedTree.subnames.find((child) => child.name === part) || null;
    }

    return traversedTree;
}
