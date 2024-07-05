// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { dirname, join } from 'path';
import type { StorybookConfig } from '@storybook/react-vite';

function getAbsolutePath(value: string): any {
    return dirname(require.resolve(join(value, 'package.json')));
}

const config: StorybookConfig = {
    stories: [
        '../src/storybook/stories/**/*.mdx',
        '../src/storybook/stories/**/*.stories.@(js|jsx|ts|tsx)',
    ],
    addons: [
        getAbsolutePath('@storybook/addon-a11y'),
        getAbsolutePath('@storybook/addon-links'),
        getAbsolutePath('@storybook/addon-essentials'),
        getAbsolutePath('@storybook/addon-interactions'),
        '@chromatic-com/storybook',
        'storybook-dark-mode',
    ],

    framework: {
        name: getAbsolutePath('@storybook/react-vite'),
        options: {},
    },
    typescript: {
        reactDocgen: 'react-docgen-typescript',
    },
    docs: {},
};

export default config;
