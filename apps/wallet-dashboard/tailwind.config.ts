// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config } from 'tailwindcss';
import { uiKitResponsivePreset } from '@iota/apps-ui-kit';

export default {
    presets: [uiKitResponsivePreset],
    content: [
        './app/**/*.{js,ts,jsx,tsx,mdx}',
        './pages/**/*.{js,ts,jsx,tsx,mdx}',
        './components/**/*.{js,ts,jsx,tsx,mdx}',
        './node_modules/@iota/apps-ui-kit/dist/**/*.js',
    ],
    darkMode: 'class',
    theme: {
        extend: {},
        plugins: [],
    },
} satisfies Partial<Config>;
