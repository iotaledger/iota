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
            backgroundImage: {
                placeholderShimmer:
                    'linear-gradient(90deg, #ecf1f4 -24.18%, rgba(237 242 245 / 40%) 73.61%, #f3f7f9 114.81%, #ecf1f4 114.82%)',
                placeholderShimmerDark:
                    'linear-gradient(90deg, #1d1e20 -24.18%, #5e636e 73.61%, #111213 114.81%, #1d1e20 114.82%)',
            },
            keyframes: {
                shimmer: {
                    '0%': { 'background-position': '-1000px 0' },
                    '100%': { 'background-position': '1000px 0' },
                },
            },
            animation: {
                shimmer: 'shimmer 2s infinite linear',
            },
        },
    },
};
