// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Config } from 'tailwindcss';
import { IOTA_PRIMITIVES_COLOR_PALETTE, SHADER_COLOR_PALETTE } from '../constants/colors.constants';
import {
    CUSTOM_FONT_SIZES,
    BORDER_RADIUS,
    CUSTOM_SPACING,
    OPACITY,
    generateVariableSpacing,
} from './constants';

export const BASE_CONFIG: Config = {
    content: ['./src/**/*.{html,js,jsx,ts,tsx}'],
    darkMode: 'class',
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
            },
            fontFamily: {
                'alliance-no2': ['AllianceNo2', 'sans-serif'],
                inter: ['Inter', 'sans-serif'],
            },
        },
    },
};
