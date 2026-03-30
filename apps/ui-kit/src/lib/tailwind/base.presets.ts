// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config } from 'tailwindcss';
import {
    IOTA_PRIMITIVES_COLOR_PALETTE,
    IOTA_NAMES_COLOR_PALETTE,
    SHADER_COLOR_PALETTE,
} from '../constants/colors.constants';
import {
    BORDER_RADIUS,
    CUSTOM_FONT_SIZES,
    CUSTOM_SPACING,
    FONT_FAMILIES,
    OPACITY,
    generateVariableSpacing,
} from './constants';
import { THEMED_CUSTOM_COLORS } from './constants/customColors.constants';
import { firefoxPlugin, namesVariant } from './plugins';
import { NAMES_GRADIENTS } from './constants/gradients.constants';

export const BASE_CONFIG: Config = {
    content: ['./src/**/*.{html,js,jsx,ts,tsx}'],
    darkMode: 'selector',
    plugins: [firefoxPlugin, namesVariant],
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
                ...IOTA_NAMES_COLOR_PALETTE,
                ...SHADER_COLOR_PALETTE,
                ...THEMED_CUSTOM_COLORS,
            },
            fontFamily: FONT_FAMILIES,
            backgroundImage: {
                ...NAMES_GRADIENTS,
            },
            keyframes: {
                'dialog-overlay-show': {
                    from: { opacity: '0' },
                    to: { opacity: '1' },
                },
                'dialog-content-show': {
                    from: {
                        opacity: '0',
                        transform: 'translate(-50%, -50%) scale(0.92)',
                    },
                    to: {
                        opacity: '1',
                        transform: 'translate(-50%, -50%) scale(1)',
                    },
                },
                'dialog-panel-show': {
                    from: {
                        opacity: '0',
                        transform: 'translateX(12px)',
                    },
                    to: {
                        opacity: '1',
                        transform: 'translateX(0)',
                    },
                },
                'dropdown-show': {
                    from: {
                        opacity: '0',
                        transform: 'scale(0.94)',
                    },
                    to: {
                        opacity: '1',
                        transform: 'scale(1)',
                    },
                },
            },
            animation: {
                'dialog-overlay-show': 'dialog-overlay-show 170ms ease-out',
                'dialog-content-show': 'dialog-content-show 170ms ease-out',
                'dialog-panel-show': 'dialog-panel-show 210ms ease-out',
                'dropdown-show': 'dropdown-show 125ms ease-out',
            },
        },
    },
};
