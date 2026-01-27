// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function detectsUnclosedQuotes(line: string): boolean {
    // matches non-escaped quotes in the line
    const nonEscapedQuotes = line.match(/(^|[^\\])"/g);
    return (nonEscapedQuotes?.length ?? 0) % 2 !== 0;
}

export function addQuotesToIncompleteLines(code: string): string {
    // regex to match closing syntax like ), ], } maybe followed by commas and spaces
    const syntaxSuffixRegex = /\s*[)\]}]+(?:\s*,\s*)?\s*$/;

    return code
        .split('\n')
        .map((line) => {
            if (!detectsUnclosedQuotes(line)) return line;

            const matchedSyntaxSuffix = line.match(syntaxSuffixRegex);

            // if there's a syntax suffix, insert the quote before it
            if (matchedSyntaxSuffix?.index !== undefined) {
                const matchedSyntaxIndex = matchedSyntaxSuffix.index;
                return `${line.slice(0, matchedSyntaxIndex)}"${line.slice(matchedSyntaxIndex)}`;
            }

            // add quote at the end if no syntax suffix is found
            return `${line}"`;
        })
        .join('\n');
}
