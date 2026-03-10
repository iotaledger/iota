// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface ForbiddenWordMatch {
    word: string;
    start: number;
    end: number;
}

function escapeRegex(str: string): string {
    return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

let cachedWords: string[] | null = null;
let cachedRegex: RegExp | null = null;

function getCompiledRegex(forbiddenWords: string[]): RegExp | null {
    if (
        cachedWords === forbiddenWords &&
        cachedRegex &&
        cachedWords.length === forbiddenWords.length
    ) {
        return cachedRegex;
    }

    const filtered = forbiddenWords.filter((word) => word.length > 0);

    if (filtered.length === 0) {
        cachedWords = forbiddenWords;
        cachedRegex = null;
        return null;
    }

    const pattern = filtered.map((word) => escapeRegex(word)).join('|');

    cachedRegex = new RegExp(pattern, 'gi');
    cachedWords = forbiddenWords;

    return cachedRegex;
}

export function findForbiddenWords(name: string, forbiddenWords: string[]): ForbiddenWordMatch[] {
    if (!name || forbiddenWords.length === 0) {
        return [];
    }

    const regex = getCompiledRegex(forbiddenWords);

    if (!regex) {
        return [];
    }

    const matches: ForbiddenWordMatch[] = [];
    let match: RegExpExecArray | null;
    while ((match = regex.exec(name)) !== null) {
        matches.push({
            word: match[0],
            start: match.index,
            end: match.index + match[0].length,
        });
    }

    return matches;
}
