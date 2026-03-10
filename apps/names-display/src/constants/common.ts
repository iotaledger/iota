// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import TextToSVG from 'text-to-svg';

export const TEXT_TO_SVG_ROBOTO = TextToSVG.loadSync(
    new URL('../fonts/RobotoFlex-Custom.ttf', import.meta.url).pathname,
);

export const TEXT_TO_SVG_INTER = TextToSVG.loadSync(
    new URL('../fonts/Inter-Medium.ttf', import.meta.url).pathname,
);

export const SVG_CONFIG = {
    svgWidth: 220,
    svgHeight: 220,
    paddingX: 16,
    textBoxTop: 58,
    textBoxBottom: 62,
    get textBoxHeight() {
        return this.svgHeight - this.textBoxTop - this.textBoxBottom;
    },
    get textBoxWidth() {
        return this.svgWidth - this.paddingX * 2;
    },
    gapBetweenParagraphs: 4,
};

const FORMATTER = new Intl.DateTimeFormat('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
});

export const TEXT_CONFIG = {
    subnameFontSize: 20,
    nameFontSize: 20,
    nameFontSizeWithSubname: 16,
    subnameLineHeight: 20,
    nameLineHeight: 20,
    nameLineHeightWithSubname: 14,
    color: '#FFF',
    dateFormatter: FORMATTER,
};

export const SUBTITLE_CONFIG = {
    paddingBottom: 18,
    paddingRight: 20,
    fontSize: 8,
    fontWeight: 500,
    fontFamily: 'Inter',
};
