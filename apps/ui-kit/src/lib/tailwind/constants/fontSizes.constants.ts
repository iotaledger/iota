// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export type CustomFontSize = Record<
    string,
    [
        string,
        Partial<{
            lineHeight: string;
            letterSpacing: string;
            fontWeight: number;
        }>,
    ]
>;

export const LABEL_SIZES: CustomFontSize = {
    'label-sm': [
        '11px',
        {
            lineHeight: '16px',
            letterSpacing: '0.2px',
            fontWeight: 500,
        },
    ],
    'label-md': [
        '12px',
        {
            lineHeight: '16px',
            letterSpacing: '-0.1px',
            fontWeight: 500,
        },
    ],
    'label-lg': [
        '14px',
        {
            lineHeight: '20px',
            letterSpacing: '-0.1px',
            fontWeight: 500,
        },
    ],
};

export const BODY_SIZES: CustomFontSize = {
    'body-sm': [
        '12px',
        {
            lineHeight: '16px',
            letterSpacing: '0.1px',
            fontWeight: 400,
        },
    ],
    'body-md': [
        '14px',
        {
            lineHeight: '20px',
            letterSpacing: '-0.1px',
            fontWeight: 400,
        },
    ],
    'body-lg': [
        '16px',
        {
            lineHeight: '24px',
            letterSpacing: '0.1px',
            fontWeight: 400,
        },
    ],
};

export const BODY_DISAMBIGUOUS_SIZES: CustomFontSize = {
    'body-ds-sm': [
        '12px',
        {
            lineHeight: '16px',
            letterSpacing: '0.2px',
            fontWeight: 400,
        },
    ],
    'body-ds-md': [
        '14px',
        {
            lineHeight: '20px',
            letterSpacing: '-0.1px',
            fontWeight: 400,
        },
    ],
    'body-ds-lg': [
        '16px',
        {
            lineHeight: '24px',
            letterSpacing: '0.1px',
            fontWeight: 400,
        },
    ],
};

export const TITLE_SIZES: CustomFontSize = {
    'title-sm': [
        '14px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.1px',
            fontWeight: 500,
        },
    ],
    'title-md': [
        '16px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.15px',
            fontWeight: 500,
        },
    ],
    'title-lg': [
        '20px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.4px',
            fontWeight: 500,
        },
    ],
};

export const HEADLINE_SIZES: CustomFontSize = {
    'headline-sm': [
        '24px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.2px',
            fontWeight: 400,
        },
    ],
    'headline-md': [
        '28px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.4px',
            fontWeight: 400,
        },
    ],
    'headline-lg': [
        '32px',
        {
            lineHeight: '120%',
            letterSpacing: '-0.4px',
            fontWeight: 400,
        },
    ],
};

export const DISPLAY_SIZES: CustomFontSize = {
    'display-sm': [
        '36px',
        {
            lineHeight: '120%',
            fontWeight: 400,
        },
    ],
    'display-md': [
        '48px',
        {
            lineHeight: '120%',
            fontWeight: 400,
        },
    ],
    'display-lg': [
        '60px',
        {
            lineHeight: '120%',
            fontWeight: 400,
        },
    ],
};

export const TAILWIND_FONT_SIZES: CustomFontSize = {
    ...LABEL_SIZES,
    ...BODY_SIZES,
    ...BODY_DISAMBIGUOUS_SIZES,
    ...TITLE_SIZES,
    ...HEADLINE_SIZES,
    ...DISPLAY_SIZES,
};
