// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVG_CONFIG, TEXT_TO_SVG_ROBOTO } from '../constants/common.js';

function measureWidth(text: string, fontSize: number): number {
    return TEXT_TO_SVG_ROBOTO.getMetrics(text, { fontSize }).width;
}

export function wrapTextLines(text: string, fontSize: number, maxLines: number): string[] {
    const lines: string[] = [];
    let current = '';

    for (let i = 0; i < text.length; i++) {
        const candidate = current + text[i];
        if (measureWidth(candidate, fontSize) <= SVG_CONFIG.textBoxWidth) {
            current = candidate;
            continue;
        }

        if (lines.length < maxLines - 1) {
            lines.push(current);
            current = text[i];
            continue;
        }
        const remaining = current + text.slice(i);
        lines.push(truncateWithEllipsis(remaining, fontSize));
        return lines;
    }

    if (current && lines.length < maxLines) {
        lines.push(current);
    }
    return lines;
}

function truncateWithEllipsis(text: string, fontSize: number): string {
    let result = text;
    const ellipsisWidth = measureWidth('…', fontSize);
    while (
        result.length > 0 &&
        measureWidth(result, fontSize) + ellipsisWidth > SVG_CONFIG.textBoxWidth
    ) {
        result = result.slice(0, -1);
    }
    return `${result}…`;
}
