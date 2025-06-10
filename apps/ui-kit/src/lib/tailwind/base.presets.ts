// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config } from 'tailwindcss';
import { IOTA_PRIMITIVES_COLOR_PALETTE, SHADER_COLOR_PALETTE } from '../constants/colors.constants';
import {
    BORDER_RADIUS,
    CUSTOM_FONT_SIZES,
    CUSTOM_SPACING,
    FONT_FAMILIES,
    OPACITY,
    generateVariableSpacing,
} from './constants';
import { THEMED_CUSTOM_COLORS } from './constants/customColors.constants';
import { firefoxPlugin } from './plugins';

export const BASE_CONFIG: Config = {
    content: ['./src/**/*.{html,js,jsx,ts,tsx}'],
    darkMode: 'class',
    plugins: [firefoxPlugin],
    theme: {
        extend: {
            fontSize: {
                ...CUSTOM_FONT_SIZES,
            },
            borderRadius: {
                ...BORDER_RADIUS,
            },
            spacing: (utils) => {
                const screens = utils.theme('screens');
                const variableSpacing = generateVariableSpacing(screens);
                return {
                    ...CUSTOM_SPACING,
                    ...variableSpacing,
                };
            },
            opacity: {
                ...OPACITY,
                30: '0.3',
            },
            colors: {
                ...IOTA_PRIMITIVES_COLOR_PALETTE,
                ...SHADER_COLOR_PALETTE,
                ...THEMED_CUSTOM_COLORS,
            },
            fontFamily: FONT_FAMILIES,
        },
    },
};
