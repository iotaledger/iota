// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Post-build utility that injects `@see` JSDoc tags into generated `.d.ts`
 * declaration files, pointing to the hosted TypeDoc API reference for each
 * exported symbol.
 *
 * VS Code's IntelliSense hover tooltip renders `@see` tags as clickable links,
 * so consumers of the SDK packages will see "Documentation" links when hovering
 * any exported type, function, class, interface, or enum.
 *
 * TypeDoc's default URL structure (kind router):
 *   classes/<Name>.html
 *   functions/<Name>.html
 *   interfaces/<Name>.html
 *   types/<Name>.html          (type aliases)
 *   enumerations/<Name>.html
 *   variables/<Name>.html
 *   modules/<Name>.html        (namespaces / modules)
 */

import { promises as fs } from 'fs';
import * as path from 'path';

const KIND_FOLDER: Record<string, string> = {
    class: 'classes',
    interface: 'interfaces',
    function: 'functions',
    'type alias': 'types',
    enum: 'enumerations',
    variable: 'variables',
    namespace: 'modules',
    module: 'modules',
};

function kindFolder(keyword: string): string | null {
    switch (keyword) {
        case 'class':
            return KIND_FOLDER['class'];
        case 'abstract':
        case 'interface':
            return KIND_FOLDER['interface'];
        case 'function':
            return KIND_FOLDER['function'];
        case 'type':
            return KIND_FOLDER['type alias'];
        case 'enum':
            return KIND_FOLDER['enum'];
        case 'const':
        case 'let':
        case 'var':
            return KIND_FOLDER['variable'];
        case 'namespace':
        case 'module':
            return KIND_FOLDER['namespace'];
        default:
            return null;
    }
}

/**
 * Injects a `@see` JSDoc tag into a JSDoc block immediately preceding a
 * top-level exported declaration, if one does not already exist.
 *
 * Strategy:
 * - Walk through the file line-by-line.
 * - Track open JSDoc comment blocks (`/** … *\/`).
 * - When a top-level exported declaration is found on the line immediately
 *   after the closing `*\/` of a JSDoc block, inject `@see <url>` before the
 *   closing `*\/` of that block.
 * - If the declaration has no preceding JSDoc block, create a minimal one.
 */
function processFileContent(content: string, docsBaseUrl: string): string {
    const lines = content.split('\n');
    const result: string[] = [];

    let i = 0;
    while (i < lines.length) {
        const line = lines[i];

        // Detect start of a JSDoc block
        const trimmed = line.trimStart();
        if (trimmed.startsWith('/**')) {
            // Collect the full JSDoc block
            const jsdocLines: string[] = [line];
            const indent = line.match(/^(\s*)/)?.[1] ?? '';

            // Single-line JSDoc: /** ... */
            if (trimmed.includes('*/') && trimmed.indexOf('*/') > trimmed.indexOf('/**') + 2) {
                // Look ahead to see if the next non-empty line is an exported decl
                const nextIdx = i + 1;
                const url = getUrlForLine(lines, nextIdx, docsBaseUrl);
                if (url && !trimmed.includes('@see')) {
                    // Expand the single-line JSDoc to multi-line and add @see
                    const inner = trimmed.replace(/^\/\*\*\s*/, '').replace(/\s*\*\/$/, '');
                    result.push(`${indent}/**`);
                    if (inner) result.push(`${indent} * ${inner}`);
                    result.push(`${indent} * @see {@link ${url}}`);
                    result.push(`${indent} */`);
                    i++;
                    continue;
                }
                result.push(line);
                i++;
                continue;
            }

            // Multi-line JSDoc: collect until */
            i++;
            while (i < lines.length) {
                jsdocLines.push(lines[i]);
                if (lines[i].trimStart().startsWith('*/')) {
                    i++;
                    break;
                }
                i++;
            }

            // Look at next non-empty line for declaration
            let nextIdx = i;
            while (nextIdx < lines.length && lines[nextIdx].trim() === '') {
                nextIdx++;
            }

            const url = getUrlForLine(lines, nextIdx, docsBaseUrl);
            const alreadyHasSee = jsdocLines.some((l) => l.includes('@see'));

            if (url && !alreadyHasSee) {
                // Insert @see before the closing */
                const closeIdx = jsdocLines.length - 1;
                const closeLine = jsdocLines[closeIdx];
                // Preserve indentation
                const lineIndent = closeLine.match(/^(\s*)/)?.[1] ?? '';
                jsdocLines.splice(closeIdx, 0, `${lineIndent} * @see {@link ${url}}`);
            }

            result.push(...jsdocLines);
            continue;
        }

        // Detect a top-level exported declaration that has NO preceding JSDoc
        const url = getUrlForLine(lines, i, docsBaseUrl);
        if (url) {
            // Check that the previous non-empty line did NOT end with */
            // (which would mean we already handled it above)
            let prevIdx = result.length - 1;
            while (prevIdx >= 0 && result[prevIdx].trim() === '') {
                prevIdx--;
            }
            const prevLine = prevIdx >= 0 ? result[prevIdx].trimEnd() : '';
            if (!prevLine.endsWith('*/')) {
                const indent = line.match(/^(\s*)/)?.[1] ?? '';
                result.push(`${indent}/**`);
                result.push(`${indent} * @see {@link ${url}}`);
                result.push(`${indent} */`);
            }
        }

        result.push(line);
        i++;
    }

    return result.join('\n');
}

/**
 * Given an array of lines and an index, check if the line at that index
 * represents a top-level exported declaration. If so, return the TypeDoc URL
 * for that declaration; otherwise return null.
 */
function getUrlForLine(lines: string[], idx: number, docsBaseUrl: string): string | null {
    if (idx >= lines.length) return null;

    const line = lines[idx];
    const trimmed = line.trimStart();

    // Only process top-level declarations (no indentation beyond 0 spaces)
    // Nested members inside classes/interfaces should not get @see tags here.
    if (line.startsWith(' ') || line.startsWith('\t')) return null;

    // Must start with export
    if (!trimmed.startsWith('export ') && !trimmed.startsWith('declare ')) return null;

    const match = trimmed.match(
        /^(?:export\s+(?:declare\s+)?)?(?:declare\s+)?(abstract\s+)?(class|interface|function|type|enum|const|let|var|namespace|module)\s+([\w$]+)/,
    );

    if (!match) return null;

    const keyword = match[1]?.trim() === 'abstract' ? 'class' : match[2];
    const name = match[3];

    // Skip internal/private-by-convention names
    if (name.startsWith('_')) return null;

    const folder = kindFolder(keyword);
    if (!folder) return null;

    const base = docsBaseUrl.replace(/\/$/, '');
    return `${base}/${folder}/${name}.html`;
}

async function walkDts(dir: string, files: string[] = []): Promise<string[]> {
    let entries: import('fs').Dirent[];
    try {
        entries = await fs.readdir(dir, { withFileTypes: true });
    } catch {
        return files;
    }
    for (const entry of entries) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            await walkDts(full, files);
        } else if (entry.name.endsWith('.d.ts')) {
            files.push(full);
        }
    }
    return files;
}

// ─── Public API ──────────────────────────────────────────────────────────────

/**
 * Walk all `.d.ts` files under `dist/` in the current working directory and
 * inject `@see` tags pointing to the TypeDoc-hosted API reference.
 *
 * @param docsBaseUrl  The base URL of the hosted TypeDoc docs for this package,
 *                     e.g. `https://docs.iota.org/developer/ts-sdk/dapp-kit/api`
 */
export async function injectDocLinks(docsBaseUrl: string): Promise<void> {
    const distDir = path.join(process.cwd(), 'dist');
    const dtsFiles = await walkDts(distDir);

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
