// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeIotaName } from '@iota/iota-names-sdk';
import z from 'zod';

import {
    SUBTITLE_CONFIG,
    SVG_CONFIG,
    TEXT_CONFIG,
    TEXT_TO_SVG_INTER,
    TEXT_TO_SVG_ROBOTO,
} from '../constants/common.js';
import { generateLayoutForNameAndSubname } from './generateLayout.js';

interface LineRenderingInfo {
    text: string;
    fontSize: number;
    lineHeight: number;
    measuredWidth: number;
}

const DEBUG_DISPLAY = process.env.DEBUG_DISPLAY === 'true';

export const Params = z.object({
    name: z.string(),
    timestamp: z.coerce.number(),
});

type RenderSvgParameters = z.infer<typeof Params> & {
    invalidated: boolean;
    addTestDataAttributes?: boolean;
    template: string;
};

export function renderSvg({
    name,
    timestamp,
    invalidated,
    addTestDataAttributes,
    template,
}: RenderSvgParameters) {
    const formattedDate = TEXT_CONFIG.dateFormatter.format(new Date(timestamp));

    const [rawSubname, rawName] = normalizeIotaName(name, 'at').split('@');

    const { subnameLines, nameLines } = generateLayoutForNameAndSubname(
        rawSubname?.toUpperCase() ?? null,
        `@${rawName.toUpperCase()}`,
    );

    const hasSubname = subnameLines.length > 0;

    const subnameFontSize = TEXT_CONFIG.subnameFontSize;
    const subnameLineHeight = TEXT_CONFIG.subnameLineHeight;
    const nameFontSize = hasSubname
        ? TEXT_CONFIG.nameFontSizeWithSubname
        : TEXT_CONFIG.nameFontSize;
    const nameLineHeight = hasSubname
        ? TEXT_CONFIG.nameLineHeightWithSubname
        : TEXT_CONFIG.nameLineHeight;

    const gapBetweenBlocks = hasSubname ? SVG_CONFIG.gapBetweenParagraphs : 0;

    const allTextLines: LineRenderingInfo[] = [];

    for (const textLine of subnameLines) {
        const measuredWidth = TEXT_TO_SVG_ROBOTO.getMetrics(textLine, {
            fontSize: subnameFontSize,
        }).width;
        allTextLines.push({
            text: textLine,
            fontSize: subnameFontSize,
            lineHeight: subnameLineHeight,
            measuredWidth,
        });
    }

    for (const textLine of nameLines) {
        const measuredWidth = TEXT_TO_SVG_ROBOTO.getMetrics(textLine, {
            fontSize: nameFontSize,
        }).width;
        allTextLines.push({
            text: textLine,
            fontSize: nameFontSize,
            lineHeight: nameLineHeight,
            measuredWidth,
        });
    }

    const maximumBlockWidth = allTextLines.reduce(
        (max, line) => Math.max(max, line.measuredWidth),
        0,
    );

    const totalTextBlockHeight =
        subnameLines.length * subnameLineHeight +
        nameLines.length * nameLineHeight +
        gapBetweenBlocks;

    const horizontalOffset =
        SVG_CONFIG.paddingX + (SVG_CONFIG.textBoxWidth - maximumBlockWidth) / 2;
    const verticalOffset =
        SVG_CONFIG.textBoxTop + (SVG_CONFIG.textBoxHeight - totalTextBlockHeight) / 2;

    let currentYPosition = verticalOffset;
    let svgContent = DEBUG_DISPLAY ? getDebugRectangle() : '';

    for (let index = 0; index < allTextLines.length; index++) {
        const lineInfo = allTextLines[index];

        if (index === subnameLines.length && hasSubname) {
            currentYPosition += gapBetweenBlocks;
        }

        const xPosition = horizontalOffset + (maximumBlockWidth - lineInfo.measuredWidth) / 2;
        const yMiddle = currentYPosition + lineInfo.lineHeight / 2;

        const pathData = TEXT_TO_SVG_ROBOTO.getD(lineInfo.text, {
            x: xPosition,
            y: yMiddle,
            fontSize: lineInfo.fontSize,
            anchor: 'middle',
        });

        const dataTestText = addTestDataAttributes ? ` data-test-line-text="${lineInfo.text}"` : '';
        svgContent += `<path d="${pathData}"${dataTestText} fill="${TEXT_CONFIG.color}"/>\n`;

        currentYPosition += lineInfo.lineHeight;
    }

    const subtitle = invalidated ? 'Invalidated' : formattedDate;
    const subtitleTestId = addTestDataAttributes ? ` data-test-subtitle="${subtitle}"` : '';
    const subtitleX = SVG_CONFIG.svgWidth - SUBTITLE_CONFIG.paddingRight;
    const subtitleY = SVG_CONFIG.svgHeight - SUBTITLE_CONFIG.paddingBottom;
    const subtitlePath = TEXT_TO_SVG_INTER.getD(subtitle, {
        x: subtitleX,
        y: subtitleY,
        fontSize: SUBTITLE_CONFIG.fontSize,
        anchor: 'right bottom',
    });

    svgContent += `<path d="${subtitlePath}"${subtitleTestId} fill="${TEXT_CONFIG.color}"/>\n`;

    return template.replace('{{{CONTENT}}}', svgContent);
}

function getDebugRectangle(): string {
    const { svgWidth, svgHeight, paddingX, textBoxTop, textBoxBottom } = SVG_CONFIG;
    const x1 = paddingX;
    const y1 = textBoxTop;
    const x2 = svgWidth - paddingX;
    const y2 = svgHeight - textBoxBottom;
    const d = [`M${x1} ${y1}`, `L${x2} ${y1}`, `L${x2} ${y2}`, `L${x1} ${y2}`, 'Z'].join(' ');

    const rectAttrs = `fill="none" stroke="red" stroke-width="1" stroke-dasharray="4 2"`;
    return `<path d="${d}" ${rectAttrs} />`;
}
