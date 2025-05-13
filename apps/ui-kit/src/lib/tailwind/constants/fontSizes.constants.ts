// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ThemeConfig } from 'tailwindcss/types/config';

type TailwindFontSize = ThemeConfig['fontSize'];

export type TailwindFontSizeConfig = Partial<{
    lineHeight: string;
    letterSpacing: string;
    fontWeight: string | number;
}>;

export type TailwindCustomFonts = Record<
    string,
    [
        string,
        Partial<{
            lineHeight: string;
            letterSpacing: string;
            fontWeight: string | number;
        }>,
    ]
>;

export const TEXT_LABEL_CLASSES: TailwindCustomFonts = {
    'text-label-sm': [
        '11px',
        {
            lineHeight: '16px',
            letterSpacing: '0.2px',
            fontWeight: 500,
        },
    ],
    'text-label-md': [
        '12px',
        {
            lineHeight: '16px',
            letterSpacing: '-0.1px',
            fontWeight: 500,
        },
    ],
    'text-label-lg': [
        '14px',
        {
            lineHeight: '20px',
            letterSpacing: '-0.1px',
            fontWeight: 500,
        },
    ],
} satisfies TailwindFontSize;

export const TEXT_BODY_CLASSES: TailwindCustomFonts = {
    'text-body-sm': [
        '12px',
        {
            lineHeight: '16px',
            letterSpacing: '0.1px',
            fontWeight: 400,
        },
    ],
    'text-body-md': [
        '14px',
        {
            lineHeight: '20px',
            letterSpacing: '-0.1px',
            fontWeight: 400,
        },
    ],
    'text-body-lg': [
        '16px',
        {
            lineHeight: '24px',
            letterSpacing: '0.1px',
            fontWeight: 400,
        },
    ],
};

export const TEXT_BODY_DISAMBIGUOUS_CLASSES: TailwindCustomFonts = {
    'text-body-ds-sm': [
        '12px',
        {
            lineHeight: '16px',
            letterSpacing: '0.1px',
            fontWeight: 400,
        },
    ],
    'text-body-ds-md': [
        '14px',
        {
            lineHeight: '20px',
            letterSpacing: '-0.1px',
            fontWeight: 400,
        },
    ],
    'text-body-ds-lg': [
        '16px',
        {
            lineHeight: '24px',
            letterSpacing: '0.2px',
            fontWeight: 400,
        },
    ],
};

export const TEXT_TITLE_CLASSES: TailwindCustomFonts = {
    'text-title-sm': [
        '14px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.1px',
            fontWeight: 500,
        },
    ],
    'text-title-md': [
        '16px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.15px',
            fontWeight: 500,
        },
    ],
    'text-title-lg': [
        '20px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.4px',
            fontWeight: 500,
        },
    ],
};

export const TEXT_HEADLINE_CLASSES: TailwindCustomFonts = {
    'text-headline-sm': [
        '24px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.2px',
            fontWeight: 500,
        },
    ],
    'text-headline-md': [
        '28px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.4px',
            fontWeight: 500,
        },
    ],
    'text-headline-lg': [
        '32px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.4px',
            fontWeight: 500,
        },
    ],
};

export const TEXT_DISPLAY_CLASSES: TailwindCustomFonts = {
    'text-display-sm': [
        '36px',
        {
            lineHeight: '120%',
            fontWeight: 500,
        },
    ],
    'text-display-md': [
        '48px',
        {
            lineHeight: '120%',
            fontWeight: 500,
        },
    ],
    'text-display-lg': [
        '60px',
        {
            lineHeight: '120%',
            fontWeight: 500,
        },
    ],
};

const CUSTOM_TEXT_CLASSES: TailwindCustomFonts = {
    ...TEXT_LABEL_CLASSES,
    ...TEXT_BODY_CLASSES,
    ...TEXT_BODY_DISAMBIGUOUS_CLASSES,
    ...TEXT_TITLE_CLASSES,
    ...TEXT_HEADLINE_CLASSES,
    ...TEXT_DISPLAY_CLASSES,
};

// Remove the 'text-' prefix from the keys to match the TailwindCSS font size keys
export const CUSTOM_FONT_SIZES: TailwindCustomFonts = Object.entries(CUSTOM_TEXT_CLASSES).reduce(
    (acc, [className, properties]) => {
        const key = className.startsWith('text-') ? className.replace('text-', '') : className;
        return { ...acc, [key]: properties };
    },
    {},
);
