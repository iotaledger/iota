// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { promises as fs } from 'fs';
import * as path from 'path';

const KIND_FOLDERS: Record<string, string> = {
    class: 'classes',
    abstract: 'classes',
    interface: 'interfaces',
    function: 'functions',
    type: 'types',
    enum: 'enumerations',
    const: 'variables',
    let: 'variables',
    var: 'variables',
    namespace: 'modules',
    module: 'modules',
};

const DECL_RE =
    /^(?:export\s+(?:declare\s+)?|declare\s+)?(abstract\s+)?(class|interface|function|type|enum|const|let|var|namespace|module)\s+([\w$]+)/;

/** Returns the TypeDoc URL for an exported top-level declaration line, or null. */
function urlForLine(line: string, base: string): string | null {
    if (!line.startsWith('export ') && !line.startsWith('declare ')) return null;

    const m = line.match(DECL_RE);
    if (!m) return null;

    const keyword = m[1]?.trim() === 'abstract' ? 'abstract' : m[2];
    const name = m[3];
    if (name.startsWith('_')) return null;

    const folder = KIND_FOLDERS[keyword];
    if (!folder) return null;

    return `${base}/${folder}/${name}.html`;
}

/** Injects `@see` tags into all top-level exported declarations in a .d.ts file. */
function processFileContent(content: string, rawBaseUrl: string): string {
    const docsBaseUrl = rawBaseUrl.replace(/\/$/, '');
    const lines = content.split('\n');
    const result: string[] = [];
    let i = 0;

    while (i < lines.length) {
        const line = lines[i];
        const trimmed = line.trimStart();

        if (trimmed.startsWith('/**')) {
            const indent = line.match(/^(\s*)/)?.[1] ?? '';

            // Normalise single-line /** content */ to multi-line so both forms
            // are handled uniformly when inserting the @see tag.
            let jsdocLines: string[];
            if (trimmed.includes('*/') && trimmed.indexOf('*/') > trimmed.indexOf('/**') + 2) {
                const inner = trimmed.replace(/^\/\*\*\s*/, '').replace(/\s*\*\/$/, '');
                jsdocLines = [
                    `${indent}/**`,
                    ...(inner ? [`${indent} * ${inner}`] : []),
                    `${indent} */`,
                ];
                i++;
            } else {
                jsdocLines = [line];
                i++;
                while (i < lines.length) {
                    jsdocLines.push(lines[i]);
                    if (lines[i].trimStart().startsWith('*/')) {
                        i++;
                        break;
                    }
                    i++;
                }
            }

            // Find the next non-empty line to check if it's a declaration
            let nextIdx = i;
            while (nextIdx < lines.length && lines[nextIdx].trim() === '') nextIdx++;

            const url = urlForLine(lines[nextIdx] ?? '', docsBaseUrl);
            const hasSee = jsdocLines.some((l) => l.includes('@see'));

            if (url && !hasSee) {
                const closeIdx = jsdocLines.length - 1;
                const closeIndent = jsdocLines[closeIdx].match(/^(\s*)/)?.[1] ?? '';
                jsdocLines.splice(closeIdx, 0, `${closeIndent} * @see {@link ${url}}`);
            }

            result.push(...jsdocLines);
            continue;
        }

        // Declaration with no preceding JSDoc — create a minimal one.
        // The backwards scan avoids inserting a duplicate when a JSDoc block for
        // a *different* declaration sits directly above without a blank line.
        const url = urlForLine(line, docsBaseUrl);
        if (url) {
            let prevIdx = result.length - 1;
            while (prevIdx >= 0 && result[prevIdx].trim() === '') prevIdx--;
            if (!result[prevIdx]?.trimEnd().endsWith('*/')) {
                const indent = line.match(/^(\s*)/)?.[1] ?? '';
                result.push(`${indent}/**`, `${indent} * @see {@link ${url}}`, `${indent} */`);
            }
        }

        result.push(line);
        i++;
    }

    return result.join('\n');
}

/**
 * Walks all `.d.ts` files under `dist/` and injects `@see` tags pointing to
 * the hosted TypeDoc API reference, making them clickable in VS Code hover tooltips.
 *
 * @param docsBaseUrl Base URL of the TypeDoc site for this package,
 *                    e.g. `https://docs.iota.org/developer/ts-sdk/dapp-kit/api`
 */
export async function injectDocLinks(docsBaseUrl: string): Promise<void> {
    const distDir = path.join(process.cwd(), 'dist');
    const entries = await fs.readdir(distDir, { recursive: true });
    const dtsFiles = entries
        .filter((f: string) => f.endsWith('.d.ts'))
        .map((f: string) => path.join(distDir, f));

    if (dtsFiles.length === 0) {
        console.warn(`[inject-doc-links] No .d.ts files found under ${distDir}`);
        return;
    }

    let modified = 0;
    for (const file of dtsFiles) {
        const original = await fs.readFile(file, 'utf-8');
        const updated = processFileContent(original, docsBaseUrl);
        if (updated !== original) {
            await fs.writeFile(file, updated, 'utf-8');
            modified++;
        }
    }

    console.log(
        `[inject-doc-links] Processed ${dtsFiles.length} .d.ts files, modified ${modified} (${docsBaseUrl})`,
    );
}
