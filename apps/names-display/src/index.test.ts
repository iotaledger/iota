// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from 'node:fs';
import { JSDOM } from 'jsdom';
import { describe, expect, it, vi } from 'vitest';

import { renderSvg } from './utils/renderSvg.js';

const { validateParams } = await import('../src/test-utils');
const baseSvgTemplate = readFileSync(new URL('./svg/base.svg', import.meta.url), 'utf-8');
const invalidatedSvgTemplate = readFileSync(
    new URL('./svg/invalidated.svg', import.meta.url),
    'utf-8',
);

const JAN_1_2022_TIMESTAMP = 1640995200000;
const MOCK_SUBNAME = 'subname';
const MOCK_NAME = 'test';

function generateTestSvg({
    subname,
    name = MOCK_NAME,
    timestamp = JAN_1_2022_TIMESTAMP,
}: { subname?: string; name?: string; timestamp?: number } = {}): string {
    const subnamePart = subname ? subname + '.' : '';

    return renderSvg({
        name: `${subnamePart}${name}.iota`,
        timestamp: timestamp,
        addTestDataAttributes: true,
        template: baseSvgTemplate,
        invalidated: false,
    }).trim();
}

describe('names-display service', () => {
    describe('SVG generation', () => {
        it('produces a well-formed SVG with at least one text <path>', () => {
            const svg = generateTestSvg();

            expect(svg.startsWith('<svg')).toBe(true);
            expect(svg.endsWith('</svg>')).toBe(true);

            const dom = new JSDOM(svg, { contentType: 'image/svg+xml' });
            const document = dom.window.document;

            const textPaths = Array.from(document.querySelectorAll('path[fill="#FFF"]'));
            expect(textPaths.length).toBeGreaterThan(0);
        });
    });

    describe('Parameter validation', () => {
        it('accepts valid params', () => {
            const result = validateParams({
                name: MOCK_NAME + '.iota',
                timestamp: '1640995200000',
            });
            expect(result.success).toBe(true);
            expect(result.data?.name).toBe(MOCK_NAME + '.iota');
            expect(result.data?.timestamp).toBe(1640995200000);
        });

        it('rejects empty name or non-numeric timestamp', () => {
            const result = validateParams({ name: '', timestamp: 'not-a-number' });
            expect(result.success).toBe(false);
        });
    });

    describe('Base SVG template', () => {
        it('loads base.svg with the new placeholder', () => {
            const baseTemplate = readFileSync(new URL('./svg/base.svg', import.meta.url), 'utf-8');
            expect(baseTemplate).toContain('<svg');
            expect(baseTemplate).toContain('{{{CONTENT}}}');
        });
    });

    describe('SVG Texts', () => {
        it('contains a name formatted in at notation', () => {
            const svg = generateTestSvg();

            const dom = new JSDOM(svg, { contentType: 'image/svg+xml' });
            const document = dom.window.document;

            const testId = 'data-test-line-text';
            const textLines = [...document.querySelectorAll(`[${testId}]`)]
                .map((e) => e.getAttribute(testId))
                .filter((e) => e !== null);

            expect(textLines.length).toBeGreaterThan(0);
            expect(textLines).toContain(`@${MOCK_NAME.toUpperCase()}`);
        });

        it('contains name and subname in two lines', () => {
            const svg = generateTestSvg({ subname: MOCK_SUBNAME });

            const dom = new JSDOM(svg, { contentType: 'image/svg+xml' });
            const document = dom.window.document;

            const testId = 'data-test-line-text';
            const textLines = [...document.querySelectorAll(`[${testId}]`)]
                .map((e) => e.getAttribute(testId))
                .filter((e) => e !== null);

            expect(textLines.length).toBeGreaterThan(1);

            const [subnameLine, nameLine] = textLines;

            expect(subnameLine).toBe(MOCK_SUBNAME.toUpperCase());
            expect(nameLine).toBe(`@${MOCK_NAME}`.toUpperCase());
        });
    });

    describe('Invalidated SVG template', () => {
        it('loads invalidated.svg with the placeholder', () => {
            expect(invalidatedSvgTemplate).toContain('<svg');
            expect(invalidatedSvgTemplate).toContain('{{{CONTENT}}}');
        });

        it('has a grayscale filter and gray stroke instead of a gradient', () => {
            expect(invalidatedSvgTemplate).toContain('filter="url(#grayscale)"');
            expect(invalidatedSvgTemplate).toContain('<feColorMatrix type="saturate" values="0"');
            expect(invalidatedSvgTemplate).toContain('stroke="#808080"');
            expect(invalidatedSvgTemplate).not.toContain('linearGradient');
        });

        it('produces a well-formed SVG when rendered with the invalidated template', () => {
            const svg = renderSvg({
                name: `${MOCK_NAME}.iota`,
                timestamp: JAN_1_2022_TIMESTAMP,
                addTestDataAttributes: true,
                template: invalidatedSvgTemplate,
                invalidated: true,
            }).trim();

            expect(svg.startsWith('<svg')).toBe(true);
            expect(svg.endsWith('</svg>')).toBe(true);

            const dom = new JSDOM(svg, { contentType: 'image/svg+xml' });
            const document = dom.window.document;

            const textPaths = Array.from(document.querySelectorAll('path[fill="#FFF"]'));
            expect(textPaths.length).toBeGreaterThan(0);
        });
    });

    describe('isInvalidated', () => {
        async function loadInvalidation(envValue?: string) {
            vi.resetModules();
            if (envValue !== undefined) {
                process.env.INVALIDATED_NAMES = envValue;
            } else {
                delete process.env.INVALIDATED_NAMES;
            }
            const mod = await import('./utils/invalidation.js');
            return mod.isInvalidated;
        }

        it('returns false when INVALIDATED_NAMES is not set', async () => {
            const isInvalidated = await loadInvalidation();
            expect(isInvalidated('0xabc123')).toBe(false);
        });

        it('returns true for an object ID in the invalidated list', async () => {
            const isInvalidated = await loadInvalidation(JSON.stringify(['0xabc123', '0xdef456']));
            expect(isInvalidated('0xabc123')).toBe(true);
            expect(isInvalidated('0xdef456')).toBe(true);
        });

        it('returns false for an object ID not in the invalidated list', async () => {
            const isInvalidated = await loadInvalidation(JSON.stringify(['0xabc123']));
            expect(isInvalidated('0x999999')).toBe(false);
        });

        it('returns false when objectId is undefined', async () => {
            const isInvalidated = await loadInvalidation(JSON.stringify(['0xabc123']));
            expect(isInvalidated(undefined)).toBe(false);
        });
    });
});
