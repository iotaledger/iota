// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    LABEL_SIZES,
    BODY_SIZES,
    BODY_DISAMBIGUOUS_SIZES,
    TITLE_SIZES,
    HEADLINE_SIZES,
    DISPLAY_SIZES,
    type CustomFontSize,
} from '@/lib/tailwind/constants';

export type TypeSetConfig = {
    label: string;
    fontFamily: string;
    typeset: CustomFontSize;
    fontWeight: number;
};

const sizeToLabelMap: Record<string, string> = {
    sm: 'Small',
    md: 'Medium',
    lg: 'Large',
};

export function getTypeSetSize(font: CustomFontSize): string {
    const label = Object.keys(font)[0];
    const labelSplit = label.split('-');
    const size = sizeToLabelMap[label.split('-')[labelSplit.length - 1]];
    return size;
}

export const TYPESETS: TypeSetConfig[] = [
    {
        label: 'Label',
        fontFamily: 'Inter',
        typeset: LABEL_SIZES,
        fontWeight: 500,
    },
    {
        label: 'Body',
        fontFamily: 'Inter',
        typeset: BODY_SIZES,
        fontWeight: 400,
    },
    {
        label: 'Body Disambiguous',
        fontFamily: 'Inter',
        typeset: BODY_DISAMBIGUOUS_SIZES,
        fontWeight: 400,
    },
    {
        label: 'Title',
        fontFamily: 'AllianceNo2',
        typeset: TITLE_SIZES,
        fontWeight: 500,
    },
    {
        label: 'Headline',
        fontFamily: 'AllianceNo2',
        typeset: HEADLINE_SIZES,
        fontWeight: 400,
    },
    {
        label: 'Display',
        fontFamily: 'AllianceNo2',
        typeset: DISPLAY_SIZES,
        fontWeight: 400,
    },
];
