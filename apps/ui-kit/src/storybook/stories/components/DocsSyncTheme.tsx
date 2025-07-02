// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';

const THEMES = ['light', 'dark', 'names'];

export function DocsSyncTheme() {
    useEffect(() => {
        const docsUrl = new URL(document.location.href);
        const globals = docsUrl.searchParams.get('globals');

        const currentTheme = globals?.replace('theme:', '') || THEMES[0];

        for (const theme of THEMES) {
            document.documentElement.classList.toggle(theme, theme === currentTheme);
        }
    }, []);

    return null;
}
