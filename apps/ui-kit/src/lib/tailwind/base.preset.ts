// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Config } from 'tailwindcss';
import { IOTA_COLOR_PALETTE } from './constants/colors.constants';
import { CUSTOM_FONT_SIZES } from './constants';

export const BASE_CONFIG: Partial<Config> = {
    content: ['./src/**/*.{html,js,jsx,ts,tsx,md,mdx}'],
    plugins: [],
    theme: {
        extend: {
            colors: {
                ...IOTA_COLOR_PALETTE,
            },
            fontSize: {
                ...CUSTOM_FONT_SIZES,
            },
            fontFamily: {
                'alliance-no2': ['AllianceNo2', 'sans-serif'],
                inter: ['Inter', 'sans-serif'],
            },
        },
    },
};
