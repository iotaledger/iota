// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config } from 'tailwindcss';
// Note: exception for the tailwind preset import
import uiKitResponsivePreset from '../../apps/ui-kit/src/lib/tailwind/responsive.presets';

export default {
    presets: [uiKitResponsivePreset],
    content: [
        './app/**/*.{js,ts,jsx,tsx,mdx}',
        './pages/**/*.{js,ts,jsx,tsx,mdx}',
        './components/**/*.{js,ts,jsx,tsx,mdx}',
        './../ui-kit/src/lib/**/*.{js,jsx,ts,tsx}',
        './../core/src/components/**/*.{js,jsx,ts,tsx}',
    ],
    darkMode: 'class',
    theme: {
        extend: {},
        plugins: [],
    },
} satisfies Partial<Config>;
