// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import colors from 'tailwindcss/colors';
import { type Config } from 'tailwindcss';
// Note: exception for the tailwind preset import
import uiKitResponsivePreset from '../../apps/ui-kit/src/lib/tailwind/responsive.presets';

export default {
    presets: [uiKitResponsivePreset],
    content: ['./src/**/*.{js,jsx,ts,tsx}', './../ui-kit/src/lib/**/*.{js,jsx,ts,tsx}'],
    darkMode: 'selector',
    theme: {
        extend: {
            colors: {
                white: colors.white,
                black: colors.black,
                transparent: colors.transparent,
                inherit: colors.inherit,
            },
            height: {
                header: '80px',
            },
        },
    },
} satisfies Partial<Config>;
