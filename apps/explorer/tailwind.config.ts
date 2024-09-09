// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import preset from '@iota/core/tailwind.config';
import colors from 'tailwindcss/colors';
import { type Config } from 'tailwindcss';
import { uiKitStaticPreset } from '@iota/apps-ui-kit';

export default {
    presets: [preset, uiKitStaticPreset],
    content: [
        './src/**/*.{js,jsx,ts,tsx}',
        './node_modules/@iota/ui/src/**/*.{js,jsx,ts,tsx}',
        './node_modules/@iota/apps-ui-kit/dist/**/*.js',
    ],
    theme: {
        // This COLOR are duplicated from @iota/core tailwind.config.ts!!!
        // They are repeated here cause uiKitStaticPreset overwrites the colors, and they are still used throughout Explorer
        // REMOVE THIS COLORS ONCE @iota/core TAILWIND IS NOT NEEDED ANYMORE
        extend: {
            colors: {
                white: colors.white,
                black: colors.black,
                transparent: colors.transparent,
                inherit: colors.inherit,

                gray: {
                    100: '#182435',
                    95: '#2A3645',
                    90: '#383F47',
                    85: '#5A6573',
                    80: '#636870',
                    75: '#767A81',
                    70: '#898D93',
                    65: '#9C9FA4',
                    60: '#C3C5C8',
                    55: '#D7D8DA',
                    50: '#E9EAEB',
                    45: '#E3E6E8',
                    40: '#F3F6F8',
                    35: '#FEFEFE',
                },

                iota: {
                    DEFAULT: '#6fbcf0',
                    bright: '#2A38EB',
                    light: '#E1F3FF',
                    primaryBlue2023: '#4CA3FF',
                    lightest: '#F1F8FD',
                    dark: '#1F6493',
                },

                steel: {
                    DEFAULT: '#A0B6C3',
                    dark: '#758F9E',
                    darker: '#566873',
                },

                issue: {
                    DEFAULT: '#FF794B',
                    dark: '#EB5A29',
                    light: '#FFECE6',
                },
                hero: {
                    DEFAULT: '#0284AD',
                    dark: '#007195',
                    darkest: '#15527B',
                },
                success: {
                    DEFAULT: '#2DD7A7',
                    dark: '#008C65',
                    light: '#D5F7EE',
                },
                warning: {
                    DEFAULT: '#F2BD24',
                    dark: '#8D6E15',
                    light: '#FFF8E2',
                },
                headerNav: '#2A4362',
                search: {
                    fill: '#162A43',
                },
                offwhite: '#fefefe',
                offblack: '#111111',
                ebony: '#101828',
                avocado: {
                    200: '#CBE5BE',
                },
            },
        },
        screens: {
            sm: '600px',
            md: '905px',
            lg: '1240px',
            xl: '1440px',
        },
    },
} satisfies Partial<Config>;
