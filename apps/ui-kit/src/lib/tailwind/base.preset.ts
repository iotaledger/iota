// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Config } from 'tailwindcss';
import { IOTA_PRIMITIVES_COLOR_PALETTE, SHADER_COLOR_PALETTE } from './constants/colors.constants';
import { CUSTOM_FONT_SIZES, BORDER_RADIUS, PADDINGS, OPACITY } from './constants';

export const BASE_CONFIG: Partial<Config> = {
    content: ['./src/**/*.{html,js,jsx,ts,tsx,md,mdx}'],
    darkMode: 'class',
    plugins: [],
    theme: {
        fontSize: {
            ...CUSTOM_FONT_SIZES,
        },
        borderRadius: {
            ...BORDER_RADIUS,
        },
        padding: {
            ...PADDINGS,
        },
        opacity: {
            ...OPACITY,
        },
        extend: {
            colors: {
                ...IOTA_PRIMITIVES_COLOR_PALETTE,
                ...SHADER_COLOR_PALETTE,
            },
            fontFamily: {
                'alliance-no2': ['AllianceNo2', 'sans-serif'],
                inter: ['Inter', 'sans-serif'],
            },
            opacity: {
                30: '0.3',
            },
        },
    },
};
