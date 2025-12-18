import type { Config } from 'tailwindcss';
import { uiKitResponsivePreset } from '@iota/apps-ui-kit';

export default {
    content: [
        './src/**/*.{js,ts,jsx,tsx,mdx}',
        '@iota/apps-ui-kit/dist/**/*.js',
        './../core/src/**/*.{js,ts,jsx,tsx}',
    ],
    presets: [uiKitResponsivePreset as Config],
    darkMode: 'class',
    theme: {
        extend: {},
    },
    plugins: [],
} satisfies Config;
