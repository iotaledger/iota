// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Preview } from '@storybook/react';
import { withThemeByClassName } from '@storybook/addon-themes';
import { UIKitTheme } from '../src/lib/enums/theme.enums';

import '../src/lib/styles/index.css';

const preview: Preview = {
    parameters: {
        actions: { argTypesRegex: '^on[A-Z].*' },
        controls: {
            matchers: {
                color: /(background|color)$/i,
                date: /Date$/i,
            },
        },
        backgrounds: {
            disable: true,
        },
    },
    decorators: [
        withThemeByClassName({
            themes: UIKitTheme,
            defaultTheme: 'light',
        }),
    ],
    globalTypes: {
        theme: {
            name: 'Theme',
            description: 'Global theme for components',
            defaultValue: 'light',
            toolbar: {
                icon: 'paintbrush',
                items: Object.values(UIKitTheme),
                showName: true,
            },
        },
    },
};

export default preview;
