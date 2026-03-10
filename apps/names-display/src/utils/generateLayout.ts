// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVG_CONFIG, TEXT_CONFIG } from '../constants/common.js';
import { wrapTextLines } from './textLayout.js';

export function generateLayoutForNameAndSubname(
    subname: string | null,
    name: string,
): { subnameLines: string[]; nameLines: string[] } {
    const hasSubname = Boolean(subname);

    const subnameFontSize = TEXT_CONFIG.subnameFontSize;
    const subnameLineHeight = TEXT_CONFIG.subnameLineHeight;

    const nameFontSize = hasSubname
        ? TEXT_CONFIG.nameFontSizeWithSubname
        : TEXT_CONFIG.nameFontSize;
    const nameLineHeight = hasSubname
        ? TEXT_CONFIG.nameLineHeightWithSubname
        : TEXT_CONFIG.nameLineHeight;

    const totalHeight = SVG_CONFIG.textBoxHeight;

    const gap = hasSubname ? SVG_CONFIG.gapBetweenParagraphs : 0;

    const maxSubnameLines = hasSubname
        ? Math.floor((totalHeight - gap - nameLineHeight) / subnameLineHeight)
        : 0;

    const subnameLines = hasSubname
        ? wrapTextLines(subname!, subnameFontSize, maxSubnameLines)
        : [];

    const usedHeight =
        subnameLines.length * subnameLineHeight + (subnameLines.length > 0 ? gap : 0);
    const remainingHeight = totalHeight - usedHeight;

    const maxNameLines = Math.floor(remainingHeight / nameLineHeight);

    const nameLines = wrapTextLines(name, nameFontSize, maxNameLines);

    return { subnameLines, nameLines };
}
