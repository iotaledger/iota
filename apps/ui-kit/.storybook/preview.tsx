// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { type Preview, Decorator } from '@storybook/react';
import { useDarkMode } from 'storybook-dark-mode';
import { themes } from '@storybook/theming';

import '../src/lib/styles/index.css';

const withDarkMode: Decorator = (Story, context) => {
    const darkmode = useDarkMode();
    return <Story {...context} darkmode={darkmode} />;
};

const preview: Preview = {
    decorators: [withDarkMode],
    parameters: {
        actions: { argTypesRegex: '^on[A-Z].*' },
        controls: {
            matchers: {
                color: /(background|color)$/i,
                date: /Date$/i,
            },
        },
        darkMode: {
            stylePreview: true,
            dark: { ...themes.dark },
            light: { ...themes.normal },
        },
    },
};

export default preview;
